use anyhow::Result;
use chrono::{DateTime, Utc};
use nom_exif::{ExifIter, ExifTag, MediaParser, MediaSource, TrackInfo, TrackInfoTag};
use std::path::Path;

pub async fn rexif_get_taken_date<P: AsRef<Path>>(path: P) -> Result<Option<DateTime<Utc>>> {
    if path.as_ref().is_file() {
        let mut parser = MediaParser::new();
        let ms = MediaSource::file_path(path)?;
        let r = if ms.has_exif() {
            let iter: ExifIter = parser.parse(ms)?;
            iter.into_iter()
            .find(|x| x.tag_code() == ExifTag::CreateDate.code())
                .and_then(|mut x| x.take_value()
                    .and_then(|v| v.as_time().and_then(|t| Some(t.to_utc()))))
        } else {
            let info: TrackInfo = parser.parse(ms)?;
            info.get(TrackInfoTag::CreateDate)
                .and_then(|v| v.as_time().and_then(|t| Some(t.to_utc())))
        };
        Ok(r)
    } else {
        Ok(None)
    }
}