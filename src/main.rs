mod app;
mod event;
mod tui;
pub(crate) mod drive;
mod ui;
mod db;
mod media_utils;
mod file_list_widget;

use std::io;
use crate::app::{App, AppResult};
use dotenv::dotenv;
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;
use crate::event::{Event, EventHandler};
use file_list_widget::ui_actions::handle_key_events;
use crate::tui::Tui;

#[tokio::main]
async fn main() -> AppResult<()> {
    // Create an application.
    dotenv().ok();
    
    let mut app = App::new();

    // Initialize the terminal user interface.
    let backend = CrosstermBackend::new(io::stdout());
    let terminal = Terminal::new(backend)?;
    let events = EventHandler::new(250);
    let mut tui = Tui::new(terminal, events);
    tui.init()?;
    app.file_list_widget.list_files(None);

    // Start the main loop.
    while app.file_list_widget.is_running {
        // Render the user interface.
        tui.draw(&mut app)?;
        // Handle events.
        match tui.events.next().await? {
            Event::Key(key_event) => handle_key_events(key_event, &mut app)?,
            Event::Mouse(_mouse_event) => {}
            Event::Resize(_x, _y) => {}
            Event::Tick => {}
        }
    }

    // Exit the user interface.
    tui.exit()?;
    Ok(())
}

