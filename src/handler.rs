use crate::app::{App, AppResult};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Handles the key events and updates the state of [`App`].
pub fn handle_key_events(key_event: KeyEvent, app: &mut App) -> AppResult<()> {
    match key_event.code {
        // Exit application on `ESC` or `q`
        KeyCode::Esc | KeyCode::Char('q') => {
            app.quit();
        }
        // Exit application on `Ctrl-C`
        KeyCode::Char('c') | KeyCode::Char('C') => {
            if key_event.modifiers == KeyModifiers::CONTROL {
                app.quit();
            }
        }
        KeyCode::Char('l') | KeyCode::Char('L') => {
            app.file_list_widget.list_files(None);
        }
        KeyCode::Up => {
            app.file_list_widget.scroll_up();
        }
        KeyCode::Down => {
            app.file_list_widget.scroll_down();
        }
        KeyCode::Enter  => {
            app.file_list_widget.process_file();
        }
        KeyCode::Char('s') | KeyCode::Char('S') => {
            app.file_list_widget.store_files();
        }
        // Other handlers you could add here.
        _ => {}
    }
    Ok(())
}
