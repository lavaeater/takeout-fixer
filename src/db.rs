use sea_orm::{ActiveModelBehavior, ActiveModelTrait, DatabaseConnection, DbConn};
use sea_orm::ActiveValue::Set;
use crate::widgets::DriveItem;
use entity::takeout_zip;

pub fn get_db_url() -> String {
    dotenv::var("DATABASE_URL").unwrap_or("sqlite::memory:".to_string())
}

async fn get_db_connection() -> anyhow::Result<DatabaseConnection> {
    let db_url = get_db_url();
    match sea_orm::Database::connect(&db_url).await {
        Ok(db_conn) => Ok(db_conn),
        Err(e) => Err(anyhow::Error::new(e))
    }
}

pub async fn store_files(files: Vec<DriveItem>) -> anyhow::Result<()> {
    let conn = get_db_connection().await?;
    for file in files {
        if let DriveItem::File(id, name) = &file {
            let takeout_zip =
                takeout_zip::ActiveModel {
                    drive_id: Set(id.to_owned()),
                    name: Set(name.to_owned()),
                    status: Set("new".to_owned()),
                    ..Default::default()
                };
            let takeout_zip = takeout_zip.insert(&conn).await?;
        }
    }
    Ok(())
} 