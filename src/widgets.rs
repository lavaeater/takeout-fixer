use std::env;
use crate::drive::{download_file, list_google_drive};
use google_drive::types::File;
use ratatui::prelude::{Buffer, Constraint, Rect, StatefulWidget, Style, Stylize, Widget};
use ratatui::widgets::{Block, HighlightSpacing, Row, Table, TableState};
use std::sync::{Arc, RwLock};
use tokio::io::BufReader;
use zip::read::read_zipfile_from_stream;
use zip::ZipArchive;

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
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
enum LoadingState {
    #[default]
    Idle,
    Loading,
    Loaded,
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

    async fn fetch_files_in_folder(self, folder: Option<DriveItem>) {
        // this runs once, but you could also run this in a loop, using a channel that accepts
        // messages to refresh on demand, or with an interval timer to refresh every N seconds
        self.set_loading_state(LoadingState::Loading);
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
                        tokio::spawn(download_file_and_unzip_that_bitch(file.clone()));
                    }
                    DriveItem::Folder(_, _) => {
                        self.list_files(Some(file.clone()));
                    }
                }
            }
        }
    }
}

pub async fn download_file_and_unzip_that_bitch(drive_item: DriveItem) -> anyhow::Result<()> {
    let (name, bytes) = download_file(drive_item).await?;
    let file_path = dirs::home_dir().expect("Could not find home dir");
    let target_folder = file_path.join(env::var("TARGET_FOLDER").expect("Missing the TARGET_FOLDER environment variable."));
    let file_path = target_folder.clone().join(name);
    tokio::fs::write(file_path.clone(), bytes).await?;
    let f = std::fs::File::open(file_path).expect("GOEFOEF");
    
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

        // // Get and Set permissions
        // #[cfg(unix)]
        // {
        //     use std::os::unix::fs::PermissionsExt;
        // 
        //     if let Some(mode) = file.unix_mode() {
        //         fs::set_permissions(&outpath, fs::Permissions::from_mode(mode)).unwrap();
        //     }
        // }
    }
    
    Ok(())
}

impl Widget for &FileListWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = self.state.write().unwrap();

        // // a block with a right aligned title with the loading state on the right
        // let loading_state = Line::from(format!("{:?}", state.loading_state)).right_aligned();
        let block = Block::bordered()
            .title("File Id")
            .title("File Name")
            .title("Folder?")
            .title_bottom("j/k to scroll, q to quit");

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
