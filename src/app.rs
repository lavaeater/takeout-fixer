use std::error;
use crate::widgets::FileListWidget;

/// Application result type.
pub type AppResult<T> = std::result::Result<T, Box<dyn error::Error>>;

#[derive(Debug, Default)]
pub struct App {
    /// Is the application running?
    pub running: bool,
    pub file_list_widget: FileListWidget
}

impl App {
    pub fn new() -> Self {
        Self {
            running: true,
            file_list_widget: FileListWidget::default()
        }
    }
    
    pub fn quit(&mut self) {
        self.running = false;
    }
}
