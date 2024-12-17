use std::sync::{Arc, RwLock};
use google_drive::types::File;
use ratatui::prelude::{Buffer, Constraint, Line, Rect, StatefulWidget, Style, Stylize, Widget};
use ratatui::widgets::{Block, HighlightSpacing, Row, Table, TableState};
use crate::drive::list_google_drive;

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
    files: Vec<DriveFile>,
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
    pub fn list_files(&self) {
        let this = self.clone(); // clone the widget to pass to the background task
        tokio::spawn(this.fetch_files());
    }

    async fn fetch_files(self) {
        // this runs once, but you could also run this in a loop, using a channel that accepts
        // messages to refresh on demand, or with an interval timer to refresh every N seconds
        self.set_loading_state(LoadingState::Loading);
        match list_google_drive(None).await {
            Ok(files) => self.on_load(&files),
            Err(err) => self.on_err(&err),
        }
    }
    fn on_load(&self, files: &Vec<File>) {
        let d_files = files.iter().map(Into::into);
        let mut state = self.state.write().unwrap();
        state.loading_state = LoadingState::Loaded;
        state.files.extend(d_files);
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

    fn scroll_down(&self) {
        self.state.write().unwrap().table_state.scroll_down_by(1);
    }

    fn scroll_up(&self) {
        self.state.write().unwrap().table_state.scroll_up_by(1);
    }
}

impl Widget for &FileListWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = self.state.write().unwrap();

        // a block with a right aligned title with the loading state on the right
        let loading_state = Line::from(format!("{:?}", state.loading_state)).right_aligned();
        let block = Block::bordered()
            .title("Pull Requests")
            .title(loading_state)
            .title_bottom("j/k to scroll, q to quit");

        // a table with the list of pull requests
        let rows = state.files.iter();
        let widths = [
            Constraint::Length(5),
            Constraint::Fill(1),
            Constraint::Max(49),
            Constraint::Length(3),
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
struct DriveFile {
    id: String,
    name: String,
    url: String,
    is_folder: bool,
}

impl From<&DriveFile> for Row<'_> {
    fn from(df: &DriveFile) -> Self {
        let df = df.clone();
        Row::new(vec![df.id, df.name, df.url, df.is_folder.to_string()])
    }
}

impl From<&File> for DriveFile {
    fn from(file: &File) -> Self {
        Self {
            id: file.id.to_string(),
            name: file.name.to_string(),
            url: file.web_view_link.to_string(),
            is_folder: file.mime_type == "application/vnd.google-apps.folder",
        }
    }
}