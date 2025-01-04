use crate::db::{create_file_in_zip, create_media_file, fetch_file_in_zip_by_id, fetch_json_if_exists, fetch_media_file_if_exists, fetch_new_json_and_set_status_to_processing, fetch_new_media_and_set_status_to_processing, fetch_next_takeout, store_file, update_file_in_zip, update_takeout_zip, MEDIA_STATUS_FAILED, MEDIA_STATUS_NEW, MEDIA_STATUS_NO_DATE, MEDIA_STATUS_NO_MEDIA, MEDIA_STATUS_PROCESSED, MEDIA_STATUS_PROCESSING};
use crate::drive::{download, get_file_path, get_target_folder};
use crate::file_list_widget::{DriveItem, FileListWidget, LoadingState, PhotoMetadata, Task};
use crate::media_utils::rexif_get_taken_date;
use anyhow::Result;
use async_compression::tokio::bufread::GzipDecoder;
use chrono::{DateTime, Datelike, Utc};
use entity::file_in_zip::{Model as FileInZipModel, Model};
use entity::takeout_zip::Model as TakeoutZipModel;
use futures::StreamExt;
use sea_orm::ActiveValue::Set;
use sea_orm::{IntoActiveModel, TryIntoModel};
use serde_json::Value;
use std::path::Path;
use std::time::Duration;
use tokio::fs;
use tokio::fs::File as TokioFile;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio_tar::{Archive, EntryType};

pub const REMOVE_ZIPS_AFTER_PROCESSING: bool = true;

impl FileListWidget {
    pub(crate) async fn store_files_in_db(self, files: Vec<DriveItem>) {
        self.set_loading_state(LoadingState::Processing);
        let len = files.len();
        for (i, file) in files.iter().enumerate() {
            if let DriveItem::File(_, name) = file {
                self.update_item_progress("Storing", name, i as f64 / len as f64);
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
        state.max_task_counts.insert(Task::Download, 5);
        state.max_task_counts.insert(Task::Examination, 5);
        state.max_task_counts.insert(Task::MediaProcessing, 10);
        state.max_task_counts.insert(Task::JsonProcessing, 10);

        state.task_counts.insert(Task::Download, 0);
        state.task_counts.insert(Task::Examination, 0);
        state.task_counts.insert(Task::MediaProcessing, 0);
        state.task_counts.insert(Task::JsonProcessing, 0);
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
                } else {
                    self.stop_task(Task::Examination);
                }
            }

            if self.start_task(Task::MediaProcessing) {
                if let Ok(Some(item)) = fetch_new_media_and_set_status_to_processing().await {
                    let this = self.clone();

                    tokio::spawn(async move {
                        let later = this.clone();
                        match this.process_media_file(item.clone()).await {
                            Ok(_) => {
                                later.stop_task(Task::MediaProcessing);
                            }
                            Err(err) => {
                                let mut item = item.into_active_model();
                                item.status = Set(format!("{}: {}", MEDIA_STATUS_FAILED, err));
                                update_file_in_zip(item).await.unwrap();
                                later.stop_task(Task::MediaProcessing);
                            }
                        }
                    });
                } else {
                    self.stop_task(Task::MediaProcessing);
                }
            }

            if self.start_task(Task::JsonProcessing) {
                if let Ok(Some(item)) = fetch_new_json_and_set_status_to_processing().await {
                    let this = self.clone();

                    tokio::spawn(async move {
                        let later = this.clone();
                        match this.process_json_file(item.clone()).await {
                            Ok(_) => {
                                let item = fetch_file_in_zip_by_id(item.id).await.unwrap().unwrap();
                                match item.status.as_str() {
                                    MEDIA_STATUS_PROCESSING => {
                                        let mut item = item.into_active_model();
                                        item.status = Set(MEDIA_STATUS_PROCESSED.to_owned());
                                        update_file_in_zip(item).await.unwrap();
                                    }
                                    _ => {}
                                }
                                later.stop_task(Task::JsonProcessing);
                            }
                            Err(err) => {
                                let mut item = item.into_active_model();
                                item.status = Set(format!("{}: {}", MEDIA_STATUS_FAILED, err));
                                update_file_in_zip(item).await.unwrap();
                                later.stop_task(Task::JsonProcessing);
                            }
                        }
                    });
                } else {
                    self.stop_task(Task::JsonProcessing);
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

    async fn process_json_file(self, json_file: FileInZipModel) -> Result<()> {
        self.update_item_progress(&json_file.name, "start processing", 0.1);
        let media_file = fetch_media_file_if_exists(&json_file).await?;
        self.update_item_progress(&json_file.name, "check for media file", 0.2);
        if let Some(media_file) = media_file {
            //there is a media file. It might or might not be processed...
            match media_file.status.as_str() {
                MEDIA_STATUS_NO_DATE | MEDIA_STATUS_NEW => {
                    self.update_item_progress(&json_file.name, "associate media with json", 0.3);
                    if media_file.related_id.is_none() {
                        let (_media_file, _json_file) = self.associate_media_with_json(&media_file, &json_file).await?;
                    }
                    let mut media_file = media_file.clone().into_active_model();
                    media_file.status = Set(MEDIA_STATUS_NEW.to_owned());
                    self.update_item_progress(&json_file.name, "set media to new", 0.35);
                    update_file_in_zip(media_file).await?;
                }
                MEDIA_STATUS_PROCESSING => {
                    self.update_item_progress(&json_file.name, "associate media with json", 0.3);
                    if media_file.related_id.is_none() {
                        let (_media_file, _json_file) = self.associate_media_with_json(&media_file, &json_file).await?;
                    }
                    let mut json_file = json_file.into_active_model();
                    json_file.status = Set(MEDIA_STATUS_NEW.to_owned());
                    let _json_file = update_file_in_zip(json_file).await?;
                }
                MEDIA_STATUS_PROCESSED => {
                    self.update_item_progress(&json_file.name, "media processed", 0.4);
                    self.update_item_progress(&json_file.name, "associte with json", 0.5);
                    let media_file = match media_file.related_id {
                        None => { self.associate_media_with_json(&media_file, &json_file).await?.0 }
                        Some(_) => { media_file }
                    };
                    let path = Path::new(&media_file.path);
                    let target_folder = path.parent().unwrap();
                    let json_path = target_folder.join(&json_file.name);
                    fs::rename(&json_file.path, &json_path).await?;
                    self.update_item_progress(&media_file.name, "json moved", 0.6);
                    let mut json_file = json_file.into_active_model();
                    json_file.status = Set(MEDIA_STATUS_PROCESSED.to_string());
                    json_file.path = Set(json_path.to_str().unwrap().to_owned());
                    let json_file = update_file_in_zip(json_file).await?;
                    self.update_item_progress(&media_file.name, "read json contents", 0.7);
                    let file_content = fs::read_to_string(&json_file.path).await?;
                    self.update_item_progress(&media_file.name, "convert to raw json", 0.8);
                    let raw_json: Value = serde_json::from_str(&file_content)?;
                    self.update_item_progress(&media_file.name, "create media file in db", 0.9);
                    let _ = create_media_file(&media_file.name, &media_file.path, &raw_json).await?;
                    self.update_item_progress(&media_file.name, "created media file in db", 1.0);
                }
                MEDIA_STATUS_FAILED => {
                    let mut json_file = json_file.into_active_model();
                    json_file.status = Set(format!("{}: media file already failed", MEDIA_STATUS_FAILED));
                    update_file_in_zip(json_file).await?;
                }
                _ => {}
            }
        } else {
            self.update_item_progress(&json_file.name, "no media file", 0.5);
            let mut json_file = json_file.into_active_model();
            json_file.status = Set(MEDIA_STATUS_NO_MEDIA.to_owned());
            //This one will simply wait for its turn.
            let json_file = update_file_in_zip(json_file).await?;
            self.update_item_progress(&json_file.name, "no media file", 1.0);
        }
        Ok(())
    }

    async fn get_date_taken_from_json(
        &self,
        json_file: FileInZipModel,
    ) -> Result<Option<DateTime<Utc>>> {
        let file_content = fs::read_to_string(&json_file.path).await?;
        let metadata: PhotoMetadata = serde_json::from_str(&file_content)?;
        let timestamp: i64 = metadata.photo_taken_time.timestamp.parse()?;

        Ok(DateTime::from_timestamp(timestamp, 0))
    }

    async fn process_media_file(&self, media_file: FileInZipModel) -> Result<()> {
        self.update_item_progress(&media_file.name, "start processing", 0.1);

        let json_file = fetch_json_if_exists(&media_file).await?;

        if let Some(json_data) = json_file.clone() {
            self.update_item_progress(&media_file.name, "associating json", 0.2);

            self.associate_media_with_json(&media_file, &json_data).await?;
            self.update_item_progress(&media_file.name, "done with json", 0.3);
        }

        // Extract the timestamp
        self.update_item_progress(&media_file.name, "read taken date", 0.35);
        let datetime_utc = match rexif_get_taken_date(&media_file.path).await {
            Ok(Some(dt)) => Some(dt),
            _ => match json_file.clone() {
                Some(json_file) => {
                    self.update_item_progress(&media_file.name, "read date from json", 0.4);
                    self.get_date_taken_from_json(json_file).await?
                }
                _ => None,
            },
        };

        let datetime_utc = match datetime_utc {
            Some(dt) => dt,
            None => {
                let mut media_file = media_file.into_active_model();
                media_file.status = Set(MEDIA_STATUS_NO_DATE.to_owned()); // This can then be handled using
                                                                //the json I guess.
                let media_file = update_file_in_zip(media_file).await?;
                self.update_item_progress(&media_file.name, "no date", 1.0);
                return Ok(());
            }
        };

        let year = datetime_utc.year();
        let month_name = datetime_utc.format("%B").to_string();
        let day = datetime_utc.day();

        let target_folder = get_target_folder().join(format!("{}/{}/{}/", year, month_name, day));
        fs::create_dir_all(&target_folder).await?;

        let media_path = target_folder.join(&media_file.name);
        self.update_item_progress(&media_file.name, "move media file", 0.4);
        fs::rename(&media_file.path, &media_path).await?;
        self.update_item_progress(&media_file.name, "update path in db", 0.5);
        let mut media_file = media_file.into_active_model();
        media_file.status = Set(MEDIA_STATUS_PROCESSED.to_string());
        media_file.path = Set(media_path.to_str().unwrap().to_owned());
        let media_file = update_file_in_zip(media_file).await?;
        self.update_item_progress(&media_file.name, "done with media file", 0.6);

        if let Some(json_file) = json_file {
            self.update_item_progress(&media_file.name, "move json file if exists", 0.65);
            let json_path = target_folder.clone().join(&json_file.name);
            fs::rename(&json_file.path, &json_path).await?;
            self.update_item_progress(&media_file.name, "json moved", 0.7);
            let mut json_file = json_file.into_active_model();
            json_file.status = Set(MEDIA_STATUS_PROCESSED.to_string());
            json_file.path = Set(json_path.to_str().unwrap().to_owned());
            let json_file = update_file_in_zip(json_file).await?;
            self.update_item_progress(&media_file.name, "read json contents", 0.75);
            let file_content = fs::read_to_string(json_file.path).await?;
            self.update_item_progress(&media_file.name, "convert to raw json", 0.8);
            let raw_json: Value = serde_json::from_str(&file_content)?;
            self.update_item_progress(&media_file.name, "create media file in db", 0.85);

            let _ = create_media_file(&media_file.name, &media_file.path, &raw_json).await?;
            self.update_item_progress(&media_file.name, "create media file in db", 0.9);
        }

        self.update_item_progress(&media_file.name, "done", 1.0);

        Ok(())
    }

    async fn associate_media_with_json(&self, media_file: &Model, json_data: &Model) -> Result<(Model, Model)> {
        let mut to_save_media_file = media_file.clone().into_active_model();
        to_save_media_file.related_id = Set(Some(json_data.id));
        let to_save_media_file = update_file_in_zip(to_save_media_file).await?;
        let mut json_data = json_data.clone().into_active_model();
        json_data.related_id = Set(Some(to_save_media_file.id));
        let json_data = update_file_in_zip(json_data).await?;
        Ok((to_save_media_file, json_data))
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
                    fs::create_dir_all(parent).await?;
                }
                /*
                Modify so we first extract this file to where it is supposed to be,
                then we add the data for the file to the database
                */
                let mut output_file = fs::File::create(&full_path).await?;
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
        if REMOVE_ZIPS_AFTER_PROCESSING {
            fs::remove_file(&takeout_zip.local_path).await?;
            let mut takeout_zip = takeout_zip.into_active_model();
            takeout_zip.local_path = Set("".to_string());
            update_takeout_zip(takeout_zip).await?;
        }
        Ok(())
    }

    async fn download_to_disk_with_progress(self, file_item: DriveItem) -> anyhow::Result<String> {
        if let DriveItem::File(id, name) = file_item {
            let local_path = get_file_path(&name);
            let mut response = download(id).await?;
            let size = response.content_length().unwrap_or_default();
            let mut written = usize::default();
            let mut async_file = fs::OpenOptions::new()
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
