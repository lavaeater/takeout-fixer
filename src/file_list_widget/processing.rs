use std::time::Duration;
use sea_orm::ActiveValue::Set;
use chrono::{DateTime, Datelike};
use entity::file_in_zip::Model as FileInZipModel;
use serde_json::Value;
use entity::takeout_zip::Model as TakeoutZipModel;
use tokio::fs::File as TokioFile;
use tokio::io::{AsyncWriteExt, BufReader};
use async_compression::tokio::bufread::GzipDecoder;
use tokio_tar::{Archive, EntryType};
use sea_orm::{IntoActiveModel, TryIntoModel};
use futures::StreamExt;
use crate::db::{create_file_in_zip, create_media_file, fetch_new_media_and_set_status_to_processing, fetch_next_takeout, fetch_related_for_file_in_zip, store_file, update_file_in_zip, update_takeout_zip};
use crate::drive::{download, get_file_path, get_target_folder};
use crate::file_list_widget::{DriveItem, FileListWidget, LoadingState, PhotoMetadata, Task};

impl FileListWidget {
    pub(crate) async fn store_files_in_db(self, files: Vec<DriveItem>) {
        self.set_loading_state(LoadingState::Processing);
        let len = files.len();
        for (i, file) in files.iter().enumerate() {
            if let DriveItem::File(_, name) = file {
                self.update_item_progress(name, "Storing", i as f64 / len as f64);
                store_file(file.clone())
                    .await
                    .expect("Failed to store file");
            }
        }
        self.set_loading_state(LoadingState::Idle);
    }

    pub fn start_processing(&self) {
        let mut state = self.get_write_state();
        state.processing = true;
        state.max_task_counts.insert(Task::Download, 3);
        state.max_task_counts.insert(Task::Examination, 3);
        state.max_task_counts.insert(Task::FileProcessing, 3);
        let this = self.clone();
        tokio::spawn(this.start_processing_pipeline());
    }

    async fn start_processing_pipeline(self) {
        self.set_loading_state(LoadingState::Processing);
        let mut interval = tokio::time::interval(Duration::from_millis(100)); // Poll every 3 seconds

        while self.is_processing() {
            interval.tick().await; // Wait before each poll

            // Check for "new" items to download
            if self.start_task(Task::Download) {
                if let Ok(Some(mut item)) = fetch_next_takeout(
                    "new",
                    Some("downloading"),
                    Some(self.get_max_number_of_downloaded()),
                )
                .await
                {
                    let this = self.clone();

                    tokio::spawn(async move {
                        let later = this.clone();
                        // Simulate processing for "new" items
                        match this
                            .download_to_disk_with_progress(DriveItem::File(
                                item.drive_id.clone().unwrap(),
                                item.name.clone().unwrap(),
                            ))
                            .await
                        {
                            Ok(path) => {
                                item.status = Set("downloaded".to_string());
                                item.local_path = Set(path.clone());
                                later.stop_task(Task::Download);
                            }
                            Err(_) => {
                                item.status = Set("download_failed".to_string());
                                later.stop_task(Task::Download);
                            }
                        }
                        update_takeout_zip(item).await.unwrap();
                    });
                } else {
                    self.stop_task(Task::Download);
                }
            }

            // Check for "downloaded" items
            if self.start_task(Task::Examination) {
                if let Ok(Some(mut item)) =
                    fetch_next_takeout("downloaded", Some("processing_zip"), None).await
                {
                    let this = self.clone();

                    tokio::spawn(async move {
                        let later = this.clone();
                        match this
                            .examine_zip_with_progress(item.clone().try_into_model().unwrap())
                            .await
                        {
                            Ok(_) => {
                                item.status = Set("processed_zip".to_string());
                                later.stop_task(Task::Examination);
                            }
                            Err(err) => {
                                item.status = Set(format!("{} - processing_failed", err));
                                later.stop_task(Task::Examination);
                            }
                        }
                        update_takeout_zip(item).await.unwrap();
                    });
                }
            }

            if self.start_task(Task::FileProcessing) {
                if let Ok(Some(item)) = fetch_new_media_and_set_status_to_processing().await {
                    let this = self.clone();

                    tokio::spawn(async move {
                        this.process_media_file(item.clone())
                            .await
                            .expect("Failed to process media file");
                    });
                } else {
                    self.stop_task(Task::FileProcessing);
                }
            }
        }
    }

    pub fn process_file(&self) {
        if let Ok(state) = self.state.read() {
            if let Some(selected) = state.table_state.selected() {
                let file = &state.files[selected];
                match file {
                    DriveItem::File(_, _) => {}
                    DriveItem::Folder(_, _) => {
                        self.list_files(Some(file.clone()));
                    }
                }
            }
        }
    }

    async fn process_media_file(self, media_file: FileInZipModel) -> anyhow::Result<()> {
        self.update_item_progress(&media_file.name, "processing", 0.1);
        /*
        Above all else, the media file is an image or a video file, etc,
        that we can apply some metadata from a json on using the exif thingie.
        After having used that we can then move it to its proper place on
        the hard drive...
         */

        //Remove
        let json_data = fetch_related_for_file_in_zip(&media_file).await?;
        /*
        Now we have the paths... what to do next? Read that goshdarn
        json and do stuff to it.
         */
        let file_content = tokio::fs::read_to_string(&json_data.path).await?;
        let metadata: PhotoMetadata = serde_json::from_str(&file_content)?;
        self.update_item_progress(&media_file.name, "processing", 0.3);

        // Extract the timestamp
        let timestamp: i64 = metadata.photo_taken_time.timestamp.parse()?; // Parse string to i64

        let datetime_utc = DateTime::from_timestamp(timestamp, 0)
            .expect("Failed to convert timestamp to datetime");
        let year = datetime_utc.year();
        let month_name = datetime_utc.format("%B").to_string();
        let day = datetime_utc.day();

        let target_folder = get_target_folder().join(format!("{}/{}/{}/", year, month_name, day));
        tokio::fs::create_dir_all(&target_folder).await?;
        let json_path = target_folder.clone().join(&json_data.name);
        let media_path = target_folder.join(&media_file.name);
        self.update_item_progress(&media_file.name, "processing", 0.4);

        tokio::fs::rename(&json_data.path, &json_path).await?;
        self.update_item_progress(&media_file.name, "processing", 0.5);

        tokio::fs::rename(&media_file.path, &media_path).await?;
        self.update_item_progress(&media_file.name, "processing", 0.6);

        let mut json_file = json_data.into_active_model();
        let mut media_file = media_file.into_active_model();
        json_file.status = Set("processed".to_string());
        media_file.status = Set("processed".to_string());
        json_file.path = Set(json_path.to_str().unwrap().to_owned());
        media_file.path = Set(media_path.to_str().unwrap().to_owned());

        let json_file = update_file_in_zip(json_file).await?;
        let media_file = update_file_in_zip(media_file).await?;
        self.update_item_progress(&media_file.name, "processing", 0.7);

        let file_content = tokio::fs::read_to_string(json_file.path).await?;
        self.update_item_progress(&media_file.name, "processing", 0.8);

        let raw_json: Value = serde_json::from_str(&file_content)?;
        self.update_item_progress(&media_file.name, "processing", 0.9);

        let _ = create_media_file(&media_file.name, &media_file.path, &raw_json).await?;
        self.update_item_progress(&media_file.name, "processing", 1.0);

        self.stop_task(Task::FileProcessing);

        Ok(())
    }

    async fn examine_zip_with_progress(self, takeout_zip: TakeoutZipModel) -> anyhow::Result<()> {
        let file = TokioFile::open(&takeout_zip.local_path).await?;
        let buf_reader = BufReader::new(file);
        // Create an asynchronous Gzip decoder
        let decoder = GzipDecoder::new(buf_reader);
        let mut archive = Archive::new(decoder);
        let mut entries = archive.entries()?;
        let target_folder = get_target_folder();
        let mut total = 0;
        // count all...
        while let Some(file) = entries.next().await {
            let entry = file?;
            if entry.header().entry_type() == EntryType::Regular {
                total += 1;
            }
        }

        let mut count = 0;
        let file = TokioFile::open(&takeout_zip.local_path).await?;
        let buf_reader = BufReader::new(file);
        // Create an asynchronous Gzip decoder
        let decoder = GzipDecoder::new(buf_reader);
        let mut archive = Archive::new(decoder);
        let mut entries = archive.entries()?;
        while let Some(file) = entries.next().await {
            let mut entry = file?;
            let full_path = target_folder.clone().join(&entry.path()?).into_boxed_path();
            // Check the type of entry
            // Check the type of entry
            if entry.header().entry_type() == EntryType::Regular {
                count += 1;
                // Ensure parent directories exist
                if let Some(parent) = full_path.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                /*
                Modify so we first extract this file to where it is supposed to be,
                then we add the data for the file to the database
                */
                let mut output_file = tokio::fs::File::create(&full_path).await?;
                tokio::io::copy(&mut entry, &mut output_file).await?;

                let _file_in_zip = create_file_in_zip(
                    takeout_zip.id,
                    entry
                        .path()?
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_owned(),
                    full_path.to_str().unwrap().to_owned(),
                )
                .await?;
                let progress = if total > 0 {
                    (count as f64 / total as f64).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                self.update_item_progress(&takeout_zip.name, "unzipping", progress);
            }
        }
        tokio::fs::remove_file(&takeout_zip.local_path).await?;
        let mut takeout_zip = takeout_zip.into_active_model();
        takeout_zip.local_path = Set("".to_string());
        update_takeout_zip(takeout_zip).await?;
        Ok(())
    }

    async fn download_to_disk_with_progress(self, file_item: DriveItem) -> anyhow::Result<String> {
        if let DriveItem::File(id, name) = file_item {
            let local_path = get_file_path(&name);
            let mut response = download(id).await?;
            let size = response.content_length().unwrap_or_default();
            let mut written = usize::default();
            let mut async_file = tokio::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(local_path.clone())
                .await?;
            while let Some(chunk) = response.chunk().await? {
                written += async_file.write(chunk.as_ref()).await?;
                self.update_item_progress(&name, "downloading", written as f64 / size as f64);
            }
            Ok(local_path.to_str().unwrap().to_string())
        } else {
            Err(anyhow::Error::msg("Not a file"))
        }
    }

}