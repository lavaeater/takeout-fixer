use anyhow::Result;
use chrono::{DateTime, NaiveDateTime, Utc};
use gufo_exif::Exif;
use gufo_jpeg::Jpeg;
use std::path::Path;

#[allow(dead_code)]
async fn get_taken_date<P: AsRef<Path>>(path: P) -> Result<Option<DateTime<Utc>>> {
    //    "/home/tommie/Pictures/Takeout/2001/January/24/DSCF0092.JPG")
    let extension = path
        .as_ref()
        .extension()
        .expect("No extension")
        .to_str()
        .unwrap();
    if extension != "JPG" {
        return Ok(None);
    }

    let data = tokio::fs::read(path).await?;
    let jpeg = Jpeg::new(&data);
    let exif = Exif::new(jpeg.exif_data().next().unwrap().to_vec())?;
    if let Some(dt) = exif.date_time_original() {
        // Parse as NaiveDateTime first since the string doesn't contain a time zone
        let naive_dt =
            NaiveDateTime::parse_from_str(&dt, "%Y-%m-%dT%H:%M:%S").expect("Failed to parse date");

        // Convert to DateTime<Utc>
        let datetime_utc: DateTime<Utc> = DateTime::from_naive_utc_and_offset(naive_dt, Utc);
        // let year = datetime_utc.year();
        // let month_name = datetime_utc.format("%B").to_string();
        // let day = datetime_utc.day();
        Ok(Some(datetime_utc))
    } else {
        Ok(None)
    }
}
