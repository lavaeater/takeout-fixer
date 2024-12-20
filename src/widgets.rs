use crate::db::{list_takeouts, store_file};
use crate::drive::{download, get_file_path, list_google_drive};
use takeout_zip::Model as TakeoutZip;
use entity::takeout_zip;
use google_drive::types::File;
use ratatui::prelude::{
    Alignment, Buffer, Color, Constraint, Layout, Line, Modifier, Rect, StatefulWidget, Style,
    Stylize, Widget,
};
use ratatui::style::palette::material::BLUE;
use ratatui::style::palette::tailwind::SLATE;
use ratatui::symbols;
use ratatui::widgets::{
    Block, Borders, HighlightSpacing, Padding, Paragraph, Row, Table, TableState, Wrap,
};
use std::io::Cursor;
use std::sync::{Arc, RwLock};
use zip::ZipArchive;

#[derive(Debug, Clone, Default)]
pub struct FileListWidget {
    state: Arc<RwLock<FileListState>>,
}

#[derive(Debug, Default)]
pub struct FileListState {
    files: Vec<DriveItem>,
    zip_files: Vec<takeout_zip::Model>,
    loading_state: LoadingState,
    view_state: FileListWidgetViewState,
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
enum FileListWidgetViewState {
    #[default]
    Files,
    Processing,
}
const TODO_HEADER_STYLE: Style = Style::new().fg(SLATE.c100).bg(BLUE.c800);
const NORMAL_ROW_BG: Color = SLATE.c950;
const SELECTED_STYLE: Style = Style::new().bg(SLATE.c800).add_modifier(Modifier::BOLD);
const TEXT_FG_COLOR: Color = SLATE.c200;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum UiActions {
    StartProcessing,
    ScrollDown,
    ScrollUp,
    #[default]
    SelectItem,
    SwitchView,
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
        self.set_current_folder(folder.clone());
        match list_google_drive(folder).await {
            Ok(files) => self.on_load(&files),
            Err(err) => self.on_err(&err),
        }
    }

    pub fn list_takeouts(&self) {
        let this = self.clone();
        tokio::spawn(this.fetch_takeouts());
    }

    async fn fetch_takeouts(self) {
        self.set_loading_state(LoadingState::Loading);
        match list_takeouts().await {
            Ok(takeouts) => self.on_fetch_takeouts(&takeouts),
            Err(err) => self.on_err(&err),
        }
    }

    pub fn handle_action(&self, ui_action: UiActions) {
        let state = self.state.read().unwrap();
        match state.view_state {
            FileListWidgetViewState::Files => {
                match ui_action {
                    UiActions::StartProcessing => {
                        self.store_files();
                    }
                    UiActions::ScrollDown => {}
                    UiActions::ScrollUp => {}
                    UiActions::SelectItem => {}
                    UiActions::SwitchView => {
                        self.show_processing();
                    }
                }
            }
            FileListWidgetViewState::Processing => {
                match ui_action {
                    UiActions::StartProcessing => {
                        self.start_processing();
                    }
                    UiActions::ScrollDown => {}
                    UiActions::ScrollUp => {}
                    UiActions::SelectItem => {}
                    UiActions::SwitchView => {
                        self.show_files();
                    }
                }
            }
        }
    }
    
    pub fn store_files(&self) {
        let this = self.clone();
        if let Ok(state) = self.state.read() {
            tokio::spawn(this.store_files_in_db(state.files.clone()));
        }
    }

    pub fn show_processing(&self) {
        self.set_view_state(FileListWidgetViewState::Processing);
        self.list_takeouts();
    }
    pub fn show_files(&self) {
        self.set_view_state(FileListWidgetViewState::Files);
        self.list_files(self.state.read().unwrap().current_folder.clone());
    }

    async fn store_files_in_db(self, files: Vec<DriveItem>) {
        self.set_loading_state(LoadingState::Processing);
        let len = files.len();
        for (i, file) in files.iter().enumerate() {
            if let DriveItem::File(_, name) = file {
                self.update_file_progress(&format!("Storing: {}", name), i as f64 / len as f64);
                store_file(file.clone())
                    .await
                    .expect("Failed to store file");
            }
        }
        self.set_loading_state(LoadingState::Idle);
    }

    fn on_load(&self, files: &[File]) {
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

    fn on_fetch_takeouts(&self, takeouts: &[TakeoutZip]) {
        let mut state = self.state.write().unwrap();

        state.zip_files.clear();
        state.zip_files.extend(takeouts.to_vec());
        if !state.zip_files.is_empty() {
            state.table_state.select(Some(0));
        }
        state.loading_state = LoadingState::Loaded;
    }

    fn on_err(&self, err: &anyhow::Error) {
        self.set_loading_state(LoadingState::Error(err.to_string()));
    }

    fn set_loading_state(&self, state: LoadingState) {
        self.state.write().unwrap().loading_state = state;
    }

    fn set_view_state(&self, state: FileListWidgetViewState) {
        self.state.write().unwrap().view_state = state;
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
    
    pub fn start_processing(&self) {
        /*
        What does processing mean in this context?
        Ideally we want this to
        
        1. Select a file from the DB that is status new
        2. Download the file from the drive - status Downloading
        3. When done, status changes to downloaded.
        4. Examine the File, status ZipDiscover - all files will be entered into db
        5. When done, status changes to ZipDiscovered
        6. That file is now ready for step two of file processing.
        7. Unzip all files that have a corresponding JSON file attached to them
        - or for which there exists in the database a JSON file. 
        I am just assuming here but I think that the zip files not necessarily
        contain 100% matching pairs of jsons and images... not sure though.
         */
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

    fn render_status(&mut self, area: Rect, buf: &mut Buffer) {
        let state = self.state.read().unwrap();
        let info = if state.loading_state == LoadingState::Downloading {
            format!(
                "Downloading: {}, {:.2}%",
                state.file_name,
                state.progress * 100.0
            )
        } else if state.loading_state == LoadingState::Processing {
            format!(
                "Processing: {}, {:.2}%",
                state.file_name,
                state.progress * 100.0
            )
        } else {
            format!("Status: {:?}", state.loading_state)
        };
        // We show the list item's info under the list in this paragraph
        let block = Block::new()
            .title(Line::raw("Status").centered())
            .borders(Borders::TOP)
            .border_set(symbols::border::EMPTY)
            .border_style(TODO_HEADER_STYLE)
            .bg(NORMAL_ROW_BG)
            .padding(Padding::horizontal(1));

        // We can now render the status
        Paragraph::new(info)
            .block(block)
            .fg(TEXT_FG_COLOR)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }

    fn render_processing_area(&mut self, area: Rect, buf: &mut Buffer) {
        let mut state = self.state.write().unwrap();

        // // a block with a right aligned title with the loading state on the right
        let mut block = Block::bordered()
            .title("Jobs, brah")
            .title_alignment(Alignment::Center);

        if let Some(folder) = &state.current_folder {
            let folder_name = match folder {
                DriveItem::Folder(_, name) => name,
                _ => "",
            };
            block = block.title_top(format!("Files in: {}", folder_name));
        }

        // a table with the list of db zip files
        let rows = state.zip_files.iter();
        let widths = [
            Constraint::Percentage(25),
            Constraint::Percentage(70),
            Constraint::Percentage(5),
        ];
        let table = Table::new(rows, widths)
            .block(block)
            .highlight_spacing(HighlightSpacing::Always)
            .highlight_symbol(">>")
            .row_highlight_style(SELECTED_STYLE);

        StatefulWidget::render(table, area, buf, &mut state.table_state);
    }

    fn render_file_list_area(&mut self, area: Rect, buf: &mut Buffer) {
        let mut state = self.state.write().unwrap();

        // // a block with a right aligned title with the loading state on the right
        let mut block = Block::bordered()
            .title("File Id")
            .title("File Name")
            .title("Folder?")
            .title_alignment(Alignment::Center);

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
            .row_highlight_style(SELECTED_STYLE);

        StatefulWidget::render(table, area, buf, &mut state.table_state);
    }

    pub fn render_processing_view(&mut self, area: Rect, buf: &mut Buffer) {
        let [header_area, main_area, footer_area] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(area);

        let [list_area, status_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(3)]).areas(main_area);

        render_header(header_area, buf);
        render_processing_footer(footer_area, buf);
        self.render_processing_area(list_area, buf);
        self.render_status(status_area, buf);
    }

    pub fn render_file_view(&mut self, area: Rect, buf: &mut Buffer) {
        let [header_area, main_area, footer_area] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(area);

        let [list_area, status_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(3)]).areas(main_area);

        render_header(header_area, buf);
        render_file_footer(footer_area, buf);
        self.render_file_list_area(list_area, buf);
        self.render_status(status_area, buf);
    }
}

impl Widget for &mut FileListWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let state = self.state.read().unwrap().view_state.clone();
        match state {
            FileListWidgetViewState::Files => {
                self.render_file_view(area, buf);
            }
            FileListWidgetViewState::Processing => {
                self.render_processing_view(area, buf);
            }
        }
    }
}

fn render_file_footer(area: Rect, buf: &mut Buffer) {
    Paragraph::new(
        "Use ↓↑ to move, Enter to select, s to store to db\n, p for processing, q to quit",
    )
    .centered()
    .render(area, buf);
}

fn render_processing_footer(area: Rect, buf: &mut Buffer) {
    Paragraph::new("Use ↓↑ to move, Enter to select, s to store to db\n, f for files, q to quit")
        .centered()
        .render(area, buf);
}

fn render_header(area: Rect, buf: &mut Buffer) {
    Paragraph::new("Takeout Fixer")
        .bold()
        .centered()
        .render(area, buf);
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
