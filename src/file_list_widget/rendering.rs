use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::buffer::Buffer;
use ratatui::widgets::{Block, Borders, HighlightSpacing, Padding, Paragraph, Table, Wrap};
use ratatui::prelude::{Color, Line, Modifier, StatefulWidget, Style, Stylize, Widget};
use ratatui::style::palette::tailwind::SLATE;
use ratatui::style::palette::material::BLUE;
use google_drive::types::File as GoogleDriveFile;
use entity::takeout_zip::Model as TakeoutZipModel;
use ratatui::symbols;
use crate::file_list_widget::{DriveItem, FileListWidget, FileListWidgetViewState, LoadingState};

pub const TODO_HEADER_STYLE: Style = Style::new().fg(SLATE.c100).bg(BLUE.c800);
pub const NORMAL_ROW_BG: Color = SLATE.c950;
pub const SELECTED_STYLE: Style = Style::new().bg(SLATE.c800).add_modifier(Modifier::BOLD);
pub const TEXT_FG_COLOR: Color = SLATE.c200;

impl Widget for &mut FileListWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let state = self.get_read_state().view_state.clone();
        match state {
            FileListWidgetViewState::Files => {
                self.render_file_view(area, buf);
            }
            FileListWidgetViewState::Processing => {
                self.render_processing_view(area, buf);
            }
        }
        let mut state = self.get_write_state();
        state.progress_hash.clear();
    }
}

pub fn render_file_footer(area: Rect, buf: &mut Buffer) {
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

pub fn render_header(area: Rect, buf: &mut Buffer) {
    Paragraph::new("Takeout Fixer")
        .bold()
        .centered()
        .render(area, buf);
}

// RENDERING
impl FileListWidget {
    pub(crate) fn on_load(&self, files: &[GoogleDriveFile]) {
        let mut all_files: Vec<DriveItem> = files.iter().map(Into::into).collect();
        all_files.sort_by(|a, b| match (a, b) {
            (DriveItem::Folder { .. }, DriveItem::File { .. }) => std::cmp::Ordering::Less,
            (DriveItem::File { .. }, DriveItem::Folder { .. }) => std::cmp::Ordering::Greater,
            (DriveItem::Folder(.., name_a), DriveItem::Folder(.., name_b))
            | (DriveItem::File(.., name_a), DriveItem::File(.., name_b)) => {
                name_a.to_lowercase().cmp(&name_b.to_lowercase())
            }
        });

        let mut state = self.get_write_state();
        state.loading_state = LoadingState::Loaded;
        state.files.clear();
        state.files.extend(all_files);
        if !state.files.is_empty() {
            state.table_state.select(Some(0));
        }
    }

    pub(crate) fn on_fetch_takeouts(&self, takeouts: &[TakeoutZipModel]) {
        let mut state = self.get_write_state();

        state.zip_files.clear();
        state.zip_files.extend(takeouts.to_vec());
        if !state.zip_files.is_empty() {
            state.table_state.select(Some(0));
        }
        state.loading_state = LoadingState::Loaded;
    }

    pub(crate) fn on_err(&self, err: &anyhow::Error) {
        self.set_loading_state(LoadingState::Error(err.to_string()));
    }
    
    fn render_status(&mut self, area: Rect, buf: &mut Buffer) {
        let state = self.state.read().unwrap();
        let info =
            state
                .progress_hash
                .iter()
                .fold(String::new(), |mut acc, (key, (task, progress))| {
                    acc = format!("{}\n{}: {}, {:.2}%", acc, task, key, progress * 100.0);
                    acc
                });
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
        let mut state = self.get_write_state();

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
            Constraint::Percentage(5),
            Constraint::Percentage(60),
            Constraint::Percentage(35),
        ];
        let table = Table::new(rows, widths)
            .block(block)
            .highlight_spacing(HighlightSpacing::Always)
            .highlight_symbol(">>")
            .row_highlight_style(SELECTED_STYLE);

        StatefulWidget::render(table, area, buf, &mut state.table_state);
    }

    fn render_file_list_area(&mut self, area: Rect, buf: &mut Buffer) {
        let mut state = self.get_write_state();

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
            Constraint::Percentage(5),
            Constraint::Percentage(70),
            Constraint::Percentage(25),
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
            Layout::vertical([Constraint::Fill(1), Constraint::Length(20)]).areas(main_area);

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
            Layout::vertical([Constraint::Fill(1), Constraint::Length(20)]).areas(main_area);

        render_header(header_area, buf);
        render_file_footer(footer_area, buf);
        self.render_file_list_area(list_area, buf);
        self.render_status(status_area, buf);
    }
}