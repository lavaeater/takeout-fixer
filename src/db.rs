use crate::widgets::DriveItem;
use entity::takeout_zip;
use entity::takeout_zip::{Column, Model as TakeoutZip, ActiveModel as TakeoutZipActiveModel};
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QueryFilter};
use anyhow::Result;
use anyhow::Error;

pub fn get_db_url() -> String {
    dotenv::var("DATABASE_URL").unwrap_or("sqlite::memory:".to_string())
}

async fn get_db_connection() -> Result<DatabaseConnection> {
    let db_url = get_db_url();
    match sea_orm::Database::connect(&db_url).await {
        Ok(db_conn) => Ok(db_conn),
        Err(e) => Err(Error::new(e)),
    }
}

pub async fn fetch_next_takeout(status: &str) -> Result<Option<TakeoutZip>> {
    let db = get_db_connection().await?;
    let model = takeout_zip::Entity::find()
        .filter(Column::Status.eq(status))
        .one(&db)
        .await?;
    match model {
        Some(model) => {
            Ok(Some(model))
        }
        None => Ok(None)
    }
}


pub fn get_model(file: DriveItem) -> anyhow::Result<takeout_zip::ActiveModel> {
    if let DriveItem::File(id, name) = file {
        Ok(takeout_zip::ActiveModel {
            id: Default::default(),
            drive_id: Set(id),
            name: Set(name),
            status: Set("new".to_string()),
            local_path: Set("".to_string()),
        })
    } else {
        Err(Error::msg("Not a File Item"))
    }
}

pub async fn store_file(file: DriveItem) -> anyhow::Result<()> {
    let conn = get_db_connection().await?;
    let _takeout_zip = get_model(file)?.insert(&conn).await;
    Ok(())
}

pub async fn list_takeouts() -> anyhow::Result<Vec<TakeoutZip>> {
    let conn = get_db_connection().await?;
    Ok(takeout_zip::Entity::find().all(&conn).await?)
}

pub async fn update_takeout_zip(model: takeout_zip::ActiveModel) -> anyhow::Result<TakeoutZip> {
    let conn = get_db_connection().await?;
    Ok(model.update(&conn).await?)
}

#[allow(dead_code)]
pub async fn store_files(files: Vec<DriveItem>) -> anyhow::Result<()> {
    let conn = get_db_connection().await?;
    for file in files {
        if let DriveItem::File(_, _) = file {
            let _takeout_zip = get_model(file)?.insert(&conn).await?;
        }
    }
    Ok(())
}
