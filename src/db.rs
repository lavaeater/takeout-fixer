use std::time::Duration;
use crate::widgets::DriveItem;
use anyhow::Error;
use anyhow::Result;
use entity::takeout_zip::{ActiveModel as TakeoutZipActiveModel, Column, Model as TakeoutZip};
use entity::{file_in_zip, media_file, takeout_zip};
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, DatabaseConnection, EntityTrait, IntoActiveModel, NotSet, PaginatorTrait, QueryFilter, Statement};

pub fn get_db_url() -> String {
    dotenv::var("DATABASE_URL").unwrap_or("sqlite::memory:".to_string())
}

async fn get_db_connection() -> Result<DatabaseConnection> {
    let db_url = get_db_url();

    // let mut opt = ConnectOptions::new(db_url);
    // opt.max_connections(20)
    //     .min_connections(5)
    //     .connect_timeout(Duration::from_secs(3))
    //     .acquire_timeout(Duration::from_secs(3))
    //     .idle_timeout(Duration::from_secs(3))
    //     .max_lifetime(Duration::from_secs(3)); 
    match sea_orm::Database::connect(&db_url).await {
        Ok(db_conn) => Ok(db_conn),
        Err(e) => Err(Error::new(e)),
    }
}

pub async fn check_number_of_downloaded_takeouts(n: i32) -> Result<bool> {
    let db = get_db_connection().await?;
    let count = takeout_zip::Entity::find().filter(Column::Status.eq("downloaded")).count(&db).await?;
    Ok(count as i32 <= n)
}

pub async fn fetch_next_takeout(
    status: &str,
    new_status: Option<&str>,
) -> Result<Option<TakeoutZipActiveModel>> {
    let db = get_db_connection().await?;
    let model = takeout_zip::Entity::find()
        .filter(Column::Status.eq(status))
        .one(&db)
        .await?;
    match model {
        Some(model) => match new_status {
            None => Ok(Some(model.into_active_model())),
            Some(new_status) => {
                let mut model = model.into_active_model();
                model.status = Set(new_status.to_string());
                Ok(Some(model.update(&db).await?.into_active_model()))
            }
        },
        None => Ok(None),
    }
}

pub async fn create_file_in_zip(
    takeout_zip_id: i32,
    name: String,
    path: String,
) -> Result<file_in_zip::Model> {
    let file_type = if path.contains(".json") {
        "json"
    } else {
        "media"
    };
    let am = file_in_zip::ActiveModel {
        takeout_zip_id: Set(takeout_zip_id),
        name: Set(name),
        path: Set(path),
        status: Set("new".to_owned()),
        log: Set(serde_json::Value::String("".to_owned())),
        file_type: Set(file_type.to_owned()),
        json_id: NotSet,
        ..Default::default()
    };
    match am.insert(&get_db_connection().await?).await {
        Ok(model) => Ok(model),
        Err(e) => Err(Error::new(e)),
    }
}

pub fn get_model(file: DriveItem) -> Result<takeout_zip::ActiveModel> {
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

pub async fn set_file_types() -> Result<()> {
    let db = get_db_connection().await?;
    // Raw SQL query
    let raw_sql = r#"
    UPDATE file_in_zip AS media
    SET json_id = json.id, status = 'ready_to_process'
    FROM file_in_zip AS json
    WHERE media.file_type = 'media'
      AND media.status = 'new'
      AND media.json_id IS NULL
      AND json.file_type = 'json'
      AND json.takeout_zip_id = media.takeout_zip_id
      AND json.name = CONCAT(media.name, '.json');
"#;

    // Execute the raw SQL statement
    db.execute(Statement::from_string(
        db.get_database_backend(),
        raw_sql.to_string(),
    ))
    .await?;

    Ok(())
}

pub async fn fetch_json_for_media_file(media_file: &file_in_zip::Model) -> Result<file_in_zip::Model> {
    let conn = get_db_connection().await?;
    Ok(file_in_zip::Entity::find_by_id(media_file.json_id.unwrap())
        .one(&conn).await?.unwrap())
}

pub async fn fetch_media_file_to_process(
    status: &str,
    new_status: Option<&str>,
) -> Result<Option<file_in_zip::Model>> {
    let conn = get_db_connection().await?;
    let model = file_in_zip::Entity::find()
        .filter(file_in_zip::Column::Status.eq(status))
        .one(&conn)
        .await?;
    match model {
        Some(model) => match new_status {
            None => Ok(Some(model)),
            Some(new_status) => {
                let mut model = model.into_active_model();
                model.status = Set(new_status.to_string());
                Ok(Some(model.update(&conn).await?))
            }
        },
        None => { 
            Ok(None)
        },
    }
}

pub async fn update_file_in_zip(model: file_in_zip::ActiveModel) -> Result<file_in_zip::Model> {
    let conn = get_db_connection().await?;
    Ok(model.update(&conn).await?)
}

pub async fn create_media_file(
    file_name: &str,
    path: &str,
    json_meta: &serde_json::Value,
) -> Result<media_file::Model> {
    let m = media_file::ActiveModel {
        file_name: Set(file_name.to_owned()),
        path: Set(path.to_owned()),
        json_meta: Set(json_meta.clone()),
        ..Default::default()
    };
    match m.insert(&get_db_connection().await?).await {
        Ok(model) => Ok(model),
        Err(e) => Err(Error::new(e)),
    }
}