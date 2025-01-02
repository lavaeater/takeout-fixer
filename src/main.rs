mod app;
mod event;
mod handler;
mod tui;
pub(crate) mod drive;
mod widgets;
mod ui;
mod db;

use std::io;
use std::str::FromStr;
use chrono::{DateTime, Datelike, FixedOffset, NaiveDateTime, Utc};
use dotenv::dotenv;
use gufo_exif::Exif;
use gufo_jpeg::Jpeg;
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;
use crate::app::{App, AppResult};
use crate::event::{Event, EventHandler};
use crate::handler::handle_key_events;
use crate::tui::Tui;

fn main() {
    let data = std::fs::read("/home/tommie/Pictures/Takeout/2001/January/24/DSCF0092.JPG").unwrap();
    let jpeg = Jpeg::new(&data);
    let exif = Exif::new(jpeg.exif_data().next().unwrap().to_vec()).unwrap();
    let dt = exif.date_time_original().unwrap();
    println!("{}", dt);

    // Parse as NaiveDateTime first since the string doesn't contain a time zone
    let naive_dt = NaiveDateTime::parse_from_str(&dt, "%Y-%m-%dT%H:%M:%S")
        .expect("Failed to parse date");

    // Convert to DateTime<Utc>
    let datetime_utc: DateTime<Utc> = DateTime::from_naive_utc_and_offset(naive_dt, Utc);
    let year = datetime_utc.year();
    let month_name = datetime_utc.format("%B").to_string();
    let day = datetime_utc.day();
    println!("{}/{}/{}/", year, month_name, day);
}

// 
// #[tokio::main]
// async fn main() -> AppResult<()> {
//     // Create an application.
//     dotenv().ok();
//     
//     let mut app = App::new();
// 
//     // Initialize the terminal user interface.
//     let backend = CrosstermBackend::new(io::stdout());
//     let terminal = Terminal::new(backend)?;
//     let events = EventHandler::new(250);
//     let mut tui = Tui::new(terminal, events);
//     tui.init()?;
//     app.file_list_widget.list_files(None);
// 
//     // Start the main loop.
//     while app.file_list_widget.is_running {
//         // Render the user interface.
//         tui.draw(&mut app)?;
//         // Handle events.
//         match tui.events.next().await? {
//             Event::Key(key_event) => handle_key_events(key_event, &mut app)?,
//             Event::Mouse(_mouse_event) => {}
//             Event::Resize(_x, _y) => {}
//             Event::Tick => {}
//         }
//     }
// 
//     // Exit the user interface.
//     tui.exit()?;
//     Ok(())
// }

