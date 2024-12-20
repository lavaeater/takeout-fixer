use ratatui::widgets::Row;
use takeout_zip::Model as TakeoutZip;
pub mod takeout_zip;


pub mod prelude {
    pub use super::takeout_zip::Model as TakeoutZip;
}

impl From<&TakeoutZip> for Row<'_> {
    fn from(df: &TakeoutZip) -> Self {
        Row::new(vec![df.id.to_string(), df.name.to_string(), df.status.to_string(), df.local_path.to_string()])
    }
}