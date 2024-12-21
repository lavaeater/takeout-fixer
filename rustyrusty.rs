use sea_orm::{entity::*, query::*, DatabaseConnection, DbErr};
use tokio::{sync::mpsc, task, time};
use std::time::Duration;

// Item model
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "items")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub status: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

// Simulated processing tasks
async fn download_item(item: ActiveModel) -> Result<(), DbErr> {
    println!("Downloading item: {:?}", item.id.as_ref().unwrap());
    time::sleep(Duration::from_secs(2)).await; // Simulate work
    Ok(())
}

async fn list_files(item: ActiveModel) -> Result<(), DbErr> {
    println!("Listing files for item: {:?}", item.id.as_ref().unwrap());
    time::sleep(Duration::from_secs(1)).await; // Simulate work
    Ok(())
}

// Poll database for the next item with the specified status
async fn fetch_next_item(db: &DatabaseConnection, status: &str) -> Result<Option<ActiveModel>, DbErr> {
    // Fetch the first item with the given status
    if let Some(model) = Entity::find()
        .filter(Column::Status.eq(status))
        .one(db)
        .await? 
    {
        return Ok(Some(model.into_active_model()));
    }
    Ok(None)
}

// Main process loop
async fn process_items(db: DatabaseConnection) -> Result<(), DbErr> {
    let mut interval = time::interval(Duration::from_secs(3)); // Poll every 3 seconds

    loop {
        interval.tick().await; // Wait before each poll

        // Check for "new" items
        if let Some(item) = fetch_next_item(&db, "new").await? {
            let db = db.clone();
            task::spawn(async move {
                // Simulate processing for "new" items
                if let Err(e) = download_item(item.clone()).await {
                    eprintln!("Error processing download: {}", e);
                }

                // Update status to "downloaded" after processing
                let mut updated_item = item;
                updated_item.status = Set("downloaded".to_string());
                updated_item.update(&db).await.unwrap();
            });
        }

        // Check for "downloaded" items
        if let Some(item) = fetch_next_item(&db, "downloaded").await? {
            let db = db.clone();
            task::spawn(async move {
                // Simulate processing for "downloaded" items
                if let Err(e) = list_files(item.clone()).await {
                    eprintln!("Error processing file listing: {}", e);
                }

                // Update status to "processed" after processing
                let mut updated_item = item;
                updated_item.status = Set("processed".to_string());
                updated_item.update(&db).await.unwrap();
            });
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), DbErr> {
    let db = Database::connect("sqlite::memory:").await?;

    // Start processing loop
    process_items(db).await?;

    Ok(())
}
