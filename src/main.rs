mod app;
mod event;
mod handler;
mod tui;
pub(crate) mod drive;
mod widgets;
mod ui;
mod db;

use std::io;
use dotenv::dotenv;
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;
use crate::app::{App, AppResult};
use crate::event::{Event, EventHandler};
use crate::handler::handle_key_events;
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

    // Start the main loop.
    while app.running {
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

/*
{"state"=>"f19ea489-ea80-460a-905e-f4259227cc13", "code"=>"4/0AanRRrsbNdQnzqNqBYiluRBGkY6OiIbutw1goIG7VgS23ypELRuvS_ztJCNvaGLx1R3gBA", "scope"=>"https://www.googleapis.com/auth/drive", "controller"=>"supervisor/crm_integration", "action"=>"drive"}
 */

