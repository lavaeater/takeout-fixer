use crate::drive::{download, get_file_path, list_google_drive};
use google_drive::types::File;
use ratatui::prelude::{Buffer, Constraint, Rect, StatefulWidget, Style, Stylize, Widget};
use ratatui::widgets::{Block, HighlightSpacing, Row, Table, TableState};
use std::io::Cursor;
use std::sync::{Arc, RwLock};
use zip::ZipArchive;
use crate::db::{store_file};

/// A widget that displays a list of pull requests.
///
/// This is an async widget that fetches the list of pull requests from the GitHub API. It contains
/// an inner `Arc<RwLock<PullRequestListState>>` that holds the state of the widget. Cloning the
/// widget will clone the Arc, so you can pass it around to other threads, and this is used to spawn
/// a background task to fetch the pull requests.
#[derive(Debug, Clone, Default)]
pub struct FileListWidget {
    state: Arc<RwLock<FileListState>>,
}

#[derive(Debug, Default)]
pub struct FileListState {
    files: Vec<DriveItem>,
    loading_state: LoadingState,
    table_state: TableState,
    progress: f64,
    file_name: String,
    current_folder: Option<DriveItem>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
enum LoadingState {
    #[default]
    Idle,
    Loading,
    Loaded,
    Downloading,
    Processing,
    Error(String),
}

impl FileListWidget {
    /// Start fetching the pull requests in the background.
    ///
    /// This method spawns a background task that fetches the pull requests from the GitHub API.
    /// The result of the fetch is then passed to the `on_load` or `on_err` methods.
    pub fn list_files(&self, folder: Option<DriveItem>) {
        let this = self.clone(); // clone the widget to pass to the background task
        tokio::spawn(this.fetch_files_in_folder(folder));
    }

    pub fn store_files(&self) {
        let this = self.clone();
        if let Ok(state) = self.state.read() {
            tokio::spawn(this.store_files_in_db(state.files.clone()));
        }
    }

    async fn store_files_in_db(self, files: Vec<DriveItem>) {
        self.set_loading_state(LoadingState::Processing);
        let len = files.len();
        for (i, file) in files.iter().enumerate() {
            if let DriveItem::File(_, name) = file {
                self.update_file_progress(&format!("Storing: {}", name), i as f64 / len as f64);
                store_file(file.clone()).await.expect("Failed to store file");
            }
        }
        self.set_loading_state(LoadingState::Idle);
    }

    async fn fetch_files_in_folder(self, folder: Option<DriveItem>) {
        // this runs once, but you could also run this in a loop, using a channel that accepts
        // messages to refresh on demand, or with an interval timer to refresh every N seconds
        self.set_loading_state(LoadingState::Loading);
        self.set_current_folder(folder.clone());
        match list_google_drive(folder).await {
            Ok(files) => self.on_load(&files),
            Err(err) => self.on_err(&err),
        }
    }

    fn on_load(&self, files: &Vec<File>) {
        let mut all_files: Vec<DriveItem> = files.iter().map(Into::into).collect();
        all_files.sort_by(|a, b| match (a, b) {
            (DriveItem::Folder { .. }, DriveItem::File { .. }) => std::cmp::Ordering::Less,
            (DriveItem::File { .. }, DriveItem::Folder { .. }) => std::cmp::Ordering::Greater,
            (DriveItem::Folder(.., name_a), DriveItem::Folder(.., name_b))
            | (DriveItem::File(.., name_a), DriveItem::File(.., name_b)) => {
                name_a.to_lowercase().cmp(&name_b.to_lowercase())
            }
        });

        let mut state = self.state.write().unwrap();
        state.loading_state = LoadingState::Loaded;
        state.files.clear();
        state.files.extend(all_files);
        if !state.files.is_empty() {
            state.table_state.select(Some(0));
        }
    }

    fn on_err(&self, err: &anyhow::Error) {
        self.set_loading_state(LoadingState::Error(err.to_string()));
    }

    fn set_loading_state(&self, state: LoadingState) {
        self.state.write().unwrap().loading_state = state;
    }

    fn set_current_folder(&self, folder: Option<DriveItem>) {
        if let Some(folder) = folder {
            self.state.write().unwrap().current_folder = Some(folder);
        }
    }

    pub fn update_file_progress(&self, file_name: &str, progress: f64) {
        if let Ok(mut state) = self.state.write() {
            state.progress = progress;
            state.file_name = file_name.to_string();
        }
    }

    pub fn scroll_down(&self) {
        self.state.write().unwrap().table_state.scroll_down_by(1);
    }

    pub fn scroll_up(&self) {
        self.state.write().unwrap().table_state.scroll_up_by(1);
    }

    pub fn process_file(&self) {
        if let Ok(state) = self.state.read() {
            if let Some(selected) = state.table_state.selected() {
                let file = &state.files[selected];
                match file {
                    DriveItem::File(_, _) => {
                        if state.loading_state != LoadingState::Downloading {
                            self.download_to_disk(file);
                        }
                        // tokio::spawn(download_file_and_unzip_that_bitch(file.clone()));
                    }
                    DriveItem::Folder(_, _) => {
                        self.list_files(Some(file.clone()));
                    }
                }
            }
        }
    }

    pub fn download_to_disk(&self, file_item: &DriveItem) {
        let this = self.clone();
        tokio::spawn(this.download_and_unzip_with_progress(file_item.clone()));
    }

    async fn download_and_unzip_with_progress(self, file_item: DriveItem) -> anyhow::Result<()> {
        self.set_loading_state(LoadingState::Downloading);
        if let DriveItem::File(id, name) = file_item {
            let mut response = download(id).await?;
            let size = response.content_length().unwrap_or_default();
            let mut written = usize::default();
            let mut acc = Vec::new();
            while let Some(chunk) = response.chunk().await? {
                acc.extend_from_slice(chunk.as_ref());
                written += chunk.len();
                self.update_file_progress(&name, written as f64 / size as f64);
            }
            // Create a Cursor for in-memory usage
            let cursor = Cursor::new(acc);

            self.set_loading_state(LoadingState::Processing);
            // Use the zip crate to read from the stream
            let mut archive = ZipArchive::new(cursor)?;
            let archive_len = archive.len();
            for i in 0..archive_len {
                let mut file = archive.by_index(i)?;
                let out_path = match file.enclosed_name() {
                    Some(path) => path,
                    None => continue,
                };
                self.update_file_progress(
                    out_path.to_str().unwrap(),
                    i as f64 / archive_len as f64,
                );

                let out_path = get_file_path(out_path.to_str().unwrap());

                if file.is_dir() {
                    std::fs::create_dir_all(&out_path)?;
                } else {
                    if let Some(p) = out_path.parent() {
                        if !p.exists() {
                            std::fs::create_dir_all(p)?;
                        }
                    }
                    let mut outfile = std::fs::File::create(&out_path)?;
                    std::io::copy(&mut file, &mut outfile)?;
                }
            }
        }
        self.set_loading_state(LoadingState::Idle);
        Ok(())
    }
}

/*
   let mut archive = ZipArchive::new(f).expect("Could not open");
   for i in 0..archive.len() {
       let mut file = archive.by_index(i)?;
       let outpath = match file.enclosed_name() {
           Some(path) => path,
           None => continue,
       };
       let outpath = target_folder.clone().join(outpath);

       if file.is_dir() {
           std::fs::create_dir_all(&outpath)?;
       } else {
           if let Some(p) = outpath.parent() {
               if !p.exists() {
                   std::fs::create_dir_all(p)?;
               }
           }
           let mut outfile = std::fs::File::create(&outpath)?;
           std::io::copy(&mut file, &mut outfile)?;
       }
*/
impl Widget for &FileListWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = self.state.write().unwrap();

        // // a block with a right aligned title with the loading state on the right
        let mut block = Block::bordered()
            .title("File Id")
            .title("File Name")
            .title("Folder?")
            .title_bottom("j/k to scroll, enter to select / process, s to save to db, q to quit");

        if state.loading_state == LoadingState::Downloading {
            let progress = format!(
                "Downloading: {}, {:.2}%",
                state.file_name,
                state.progress * 100.0
            );
            block = block.title_bottom(progress);
        }

        if state.loading_state == LoadingState::Processing {
            let progress = format!(
                "Processing: {}, {:.2}%",
                state.file_name,
                state.progress * 100.0
            );
            block = block.title_bottom(progress);
        }

        if let Some(folder) = &state.current_folder {
            let folder_name = match folder {
                DriveItem::Folder(_, name) => name,
                _ => "",
            };
            block = block.title_top(format!("Files in: {}", folder_name));
        }

        // a table with the list of pull requests
        let rows = state.files.iter();
        let widths = [
            Constraint::Percentage(25),
            Constraint::Percentage(70),
            Constraint::Percentage(5),
        ];
        let table = Table::new(rows, widths)
            .block(block)
            .highlight_spacing(HighlightSpacing::Always)
            .highlight_symbol(">>")
            .row_highlight_style(Style::new().on_blue());

        StatefulWidget::render(table, area, buf, &mut state.table_state);
    }
}

#[derive(Debug, Clone)]
pub enum DriveItem {
    File(String, String),
    Folder(String, String),
}

impl From<&DriveItem> for Row<'_> {
    fn from(df: &DriveItem) -> Self {
        let df = df.clone();
        match df {
            DriveItem::File(id, name) => Row::new(vec![id, name, "File".to_string()]),
            DriveItem::Folder(id, name) => Row::new(vec![id, name, "Folder".to_string()]),
        }
    }
}

impl From<&File> for DriveItem {
    fn from(file: &File) -> Self {
        if file.mime_type == "application/vnd.google-apps.folder" {
            DriveItem::Folder(file.id.to_string(), file.name.to_string())
        } else {
            DriveItem::File(file.id.to_string(), file.name.to_string())
        }
    }
}
