use std::error;
use crate::file_list_widget::FileListWidget;

/// Application result type.
pub type AppResult<T> = std::result::Result<T, Box<dyn error::Error>>;

#[derive(Debug, Default)]
pub struct App {
    pub file_list_widget: FileListWidget
}

impl App {
    pub fn new() -> Self {
        Self {
            file_list_widget: FileListWidget::default()
        }
    }
}
