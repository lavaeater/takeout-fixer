use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::app::{App, AppResult};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum UiActions {
    StartProcessing,
    ScrollDown,
    ScrollUp,
    #[default]
    SelectItem,
    SwitchView,
    Quit,
}

/// Handles the key events and updates the state of [`App`].
pub fn handle_key_events(key_event: KeyEvent, app: &mut App) -> AppResult<()> {
    match key_event.code {
        // Exit application on `ESC` or `q`
        KeyCode::Esc | KeyCode::Char('q') => {
            app.file_list_widget.handle_action(UiActions::Quit);
        }
        // Exit application on `Ctrl-C`
        KeyCode::Char('c') | KeyCode::Char('C') => {
            if key_event.modifiers == KeyModifiers::CONTROL {
                app.file_list_widget.handle_action(UiActions::Quit);
            }
        }
        KeyCode::Enter => {
            app.file_list_widget.handle_action(UiActions::SelectItem);
        }
        KeyCode::Up => {
            app.file_list_widget.handle_action(UiActions::ScrollUp);
        }
        KeyCode::Down => {
            app.file_list_widget.handle_action(UiActions::ScrollDown);
        }
        KeyCode::Char('s') | KeyCode::Char('S') => {
            app.file_list_widget.handle_action(UiActions::StartProcessing);
        }
        KeyCode::Char('v') | KeyCode::Char('V') => {
            app.file_list_widget.handle_action(UiActions::SwitchView);
        }
        // Other handlers you could add here.
        _ => {}
    }
    Ok(())
}