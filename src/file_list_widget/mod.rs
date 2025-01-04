pub(crate) mod ui_actions;
mod processing;
mod rendering;

use google_drive::types::File as GoogleDriveFile;
use std::sync::{Arc, RwLock, RwLockReadGuard};
use entity::takeout_zip::Model as TakeoutZipModel;
use ratatui::widgets::{Row, TableState};
use std::collections::HashMap;
use serde::Deserialize;
use ui_actions::UiActions;
use crate::db::list_takeouts;
use crate::drive::list_google_drive;

#[derive(Debug, Clone)]
pub struct FileListWidget {
    pub is_running: bool,
    state: Arc<RwLock<FileListState>>,
}

impl Default for FileListWidget {
    fn default() -> Self {
        Self {
            is_running: true,
            state: Arc::new(RwLock::new(FileListState::default())),
        }
    }
}

#[derive(Debug, Eq, Hash, PartialEq, Copy, Clone)]
pub enum Task {
    Download,
    Examination,
    MediaProcessing,
    JsonProcessing,
}

#[derive(Debug)]
pub struct FileListState {
    files: Vec<DriveItem>,
    zip_files: Vec<TakeoutZipModel>,
    loading_state: LoadingState,
    view_state: FileListWidgetViewState,
    table_state: TableState,
    current_folder: Option<DriveItem>,
    processing: bool,
    max_task_counts: HashMap<Task, u8>,
    task_counts: HashMap<Task, u8>,
    progress_hash: HashMap<String, (String, f64)>,
    pub max_downloaded_zip_files: i32,
}

impl Default for FileListState {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            zip_files: Vec::new(),
            loading_state: LoadingState::Idle,
            view_state: FileListWidgetViewState::Files,
            table_state: TableState::default(),
            current_folder: None,
            processing: false,
            task_counts: HashMap::new(),
            max_task_counts: HashMap::new(),
            progress_hash: HashMap::new(),
            max_downloaded_zip_files: 10,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
enum LoadingState {
    #[default]
    Idle,
    Loading,
    Loaded,
    Processing,
    Error(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum FileListWidgetViewState {
    #[default]
    Files,
    Processing,
}

// UI ACTIONS
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
        self.set_current_folder(folder.clone());
        match list_google_drive(folder).await {
            Ok(files) => self.on_load(&files),
            Err(err) => self.on_err(&err),
        }
    }

    pub fn list_takeout_zips(&self) {
        let this = self.clone();
        tokio::spawn(this.fetch_takeout_zips());
    }

    async fn fetch_takeout_zips(self) {
        self.set_loading_state(LoadingState::Loading);
        match list_takeouts().await {
            Ok(takeouts) => self.on_fetch_takeouts(&takeouts),
            Err(err) => self.on_err(&err),
        }
    }

    pub fn handle_action(&mut self, ui_action: UiActions) {
        let view_state = self.get_read_state().view_state.clone();
        match ui_action {
            UiActions::StartProcessing => match view_state {
                FileListWidgetViewState::Files => {
                    self.store_files();
                }
                FileListWidgetViewState::Processing => {
                    if self.is_processing() {
                        self.stop_processing();
                    } else {
                        self.start_processing();
                    }
                }
            },
            UiActions::ScrollDown => {
                self.scroll_down();
            }
            UiActions::ScrollUp => {
                self.scroll_up();
            }
            UiActions::SelectItem => {
                self.process_file();
            }
            UiActions::SwitchView => match view_state {
                FileListWidgetViewState::Files => {
                    self.show_processing();
                }
                FileListWidgetViewState::Processing => {
                    self.show_files();
                }
            },
            UiActions::Quit => {
                self.quit();
            }
        }
    }

    pub fn store_files(&self) {
        let this = self.clone();
        if let Ok(state) = self.state.read() {
            tokio::spawn(this.store_files_in_db(state.files.clone()));
        }
    }

    pub fn quit(&mut self) {
        self.is_running = false;
    }

    pub fn show_processing(&self) {
        self.set_view_state(FileListWidgetViewState::Processing);
        self.list_takeout_zips();
    }

    pub fn show_files(&self) {
        self.set_view_state(FileListWidgetViewState::Files);
        self.list_files(self.get_read_state().current_folder.clone());
    }

    fn set_loading_state(&self, state: LoadingState) {
        self.get_write_state().loading_state = state;
    }

    fn get_max_number_of_downloaded(&self) -> i32 {
        self.get_read_state().max_downloaded_zip_files
    }

    fn set_view_state(&self, state: FileListWidgetViewState) {
        self.get_write_state().view_state = state;
    }

    fn set_current_folder(&self, folder: Option<DriveItem>) {
        if let Some(folder) = folder {
            self.get_write_state().current_folder = Some(folder);
        }
    }

    pub fn update_item_progress(&self, item: &str, task: &str, progress: f64) {
        if let Ok(mut state) = self.state.write() {
            state
                .progress_hash
                .insert(item.to_string(), (task.to_string(), progress));
        }
    }

    pub fn get_read_state(&self) -> RwLockReadGuard<'_, FileListState> {
        self.state.read().unwrap()
    }

    pub fn get_write_state(&self) -> std::sync::RwLockWriteGuard<'_, FileListState> {
        self.state.write().unwrap()
    }

    pub fn scroll_down(&self) {
        self.get_write_state().table_state.scroll_down_by(1);
    }

    pub fn scroll_up(&self) {
        self.get_write_state().table_state.scroll_up_by(1);
    }

    pub fn stop_processing(&self) {
        self.get_write_state().processing = false;
    }

    pub fn is_processing(&self) -> bool {
        self.get_read_state().processing
    }

    pub fn start_task(&self, task: Task) -> bool {
        let mut state = self.get_write_state();
        let mut did_start = false;

        let max = *state.max_task_counts.get(&task).unwrap_or(&1);

        state.task_counts.entry(task).and_modify(|current| {
            if *current < max {
                did_start = true;
                *current += 1;
            }
        });
        did_start
    }

    pub fn stop_task(&self, task: Task) -> bool {
        let mut state = self.get_write_state();
        let mut did_stop = false;
        state.task_counts.entry(task).and_modify(|current| {
            if *current > 0 {
                did_stop = true;
                *current -= 1;
            }
        });
        did_stop
    }
}

#[derive(Debug, Clone)]
pub enum DriveItem {
    File(String, String),
    Folder(String, String),
}

impl From<&GoogleDriveFile> for DriveItem {
    fn from(file: &GoogleDriveFile) -> Self {
        if file.mime_type == "application/vnd.google-apps.folder" {
            DriveItem::Folder(file.id.to_string(), file.name.to_string())
        } else {
            DriveItem::File(file.id.to_string(), file.name.to_string())
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PhotoMetadata {
    photo_taken_time: PhotoTakenTime,
}

#[derive(Debug, Deserialize)]
struct PhotoTakenTime {
    timestamp: String,
    #[allow(dead_code)]
    formatted: String,
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