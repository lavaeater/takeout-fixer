use crate::file_list_widget::DriveItem;
use anyhow::Error;
use anyhow::Result;
use google_drive::files::Files;
use entity::takeout_zip::{ActiveModel as TakeoutZipActiveModel, Column, Model as TakeoutZip};
use entity::{file_in_zip, media_file, takeout_zip};
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, NotSet, PaginatorTrait, QueryFilter};

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

pub async fn check_number_of_takeouts_with_status(status: &str, n: i32) -> Result<bool> {
    let db = get_db_connection().await?;
    let count = takeout_zip::Entity::find().filter(Column::Status.eq(status)).count(&db).await?;
    Ok(count as i32 <= n)
}

pub async fn fetch_next_takeout(
    status: &str,
    new_status: Option<&str>,
    max_with_new_status: Option<i32>
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
                if let Some(n) = max_with_new_status {
                    if check_number_of_takeouts_with_status(new_status, n).await? {
                        let mut model = model.into_active_model();
                        model.status = Set(new_status.to_string());
                        Ok(Some(model.update(&db).await?.into_active_model()))
                    } else {
                        Ok(None)
                    }
                } else {
                    let mut model = model.into_active_model();
                    model.status = Set(new_status.to_string());
                    Ok(Some(model.update(&db).await?.into_active_model()))
                }
                
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
    
    let extension = path.clone();
    let extension = extension.split('.').last().unwrap();
    let name_no_ext = name.clone().replace(&format!(".{}", extension), "");
    let am = file_in_zip::ActiveModel {
        takeout_zip_id: Set(takeout_zip_id),
        name: Set(name),
        name_no_ext: Set(name_no_ext),
        path: Set(path),
        status: Set("new".to_owned()),
        log: Set(serde_json::Value::String("".to_owned())),
        file_type: Set(file_type.to_owned()),
        related_id: NotSet,
        extension: Set(extension.to_owned()),
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

pub async fn fetch_json_if_exists(media_file: &file_in_zip::Model) -> Result<Option<file_in_zip::Model>> {
    let conn = get_db_connection().await?;
    let model = file_in_zip::Entity::find()
        .filter(file_in_zip::Column::NameNoExt.eq(&media_file.name_no_ext))
        .filter(file_in_zip::Column::FileType.eq("json"))
        .one(&conn)
        .await?;
    Ok(model)
}

pub async fn fetch_related_for_file_in_zip(file_in_zip: &file_in_zip::Model) -> Result<file_in_zip::Model> {
    let conn = get_db_connection().await?;
    Ok(file_in_zip::Entity::find_by_id(file_in_zip.related_id.unwrap())
        .one(&conn).await?.unwrap())
}

pub async fn fetch_new_media_and_set_status_to_processing() -> Result<Option<file_in_zip::Model>> {
    fetch_media_file_to_process("new", "media", Some("processing")).await
}

pub async fn fetch_new_json_and_set_status_to_processing() -> Result<Option<file_in_zip::Model>> {
    fetch_media_file_to_process("new", "json", Some("processing")).await
}

pub async fn fetch_media_file_to_process(
    status: &str,
    file_type: &str,
    new_status: Option<&str>,
) -> Result<Option<file_in_zip::Model>> {
    let conn = get_db_connection().await?;
    let model = file_in_zip::Entity::find()
        .filter(file_in_zip::Column::Status.eq(status))
        .filter(file_in_zip::Column::FileType.eq(file_type))
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