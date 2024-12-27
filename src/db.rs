use crate::widgets::DriveItem;
use anyhow::Error;
use anyhow::Result;
use entity::takeout_zip::{ActiveModel as TakeoutZipActiveModel, Column, Model as TakeoutZip};
use entity::{file_in_zip, takeout_zip};
use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait,
    IntoActiveModel, NotSet, QueryFilter, Statement,
};

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

pub async fn fetch_media_file_to_process() -> Result<Option<file_in_zip::Model>> {
    let conn = get_db_connection().await?;
    let model = file_in_zip::Entity::find()
        .filter(Column::Status.eq("ready_to_process"))
        .one(&conn)
        .await?;
    Ok(model)
}
