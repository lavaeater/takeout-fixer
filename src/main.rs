mod app;
mod event;
mod handler;
mod tui;
pub(crate) mod drive;
mod widgets;
mod ui;

use oauth2::TokenResponse;
use serde::{Deserialize, Serialize};
use std::io;
use google_drive::traits::FileOps;
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use crate::app::{App, AppResult};
use crate::event::{Event, EventHandler};
use crate::handler::handle_key_events;
use crate::tui::Tui;

const TAKEOUT_FOLDER_ID: &str = "1M2IDkPkChp8nBisf18-p_2-ZhG-nFSIhk68Acy8GQIlEIlrCb6XAGDc0Ty30MEoQDr-JHu1m";

#[tokio::main]
async fn main() -> AppResult<()> {
    // Create an application.
    
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
            Event::Mouse(_) => {}
            Event::Resize(_, _) => {}
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

