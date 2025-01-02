use ratatui::widgets::Row;
use takeout_zip::Model as TakeoutZip;
pub mod takeout_zip;
pub mod prelude;
pub mod file_in_zip;
pub mod media_file;

impl From<&TakeoutZip> for Row<'_> {
    fn from(df: &TakeoutZip) -> Self {
        Row::new(vec![df.id.to_string(), df.name.to_string(), df.status.to_string(), df.local_path.to_string()])
    }
}