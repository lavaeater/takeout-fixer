use ratatui::Frame;

use crate::app::App;

/// Renders the user interface widgets.
pub fn render(app: &mut App, frame: &mut Frame) {
    frame.render_widget(&mut app.file_list_widget, frame.area());
}
