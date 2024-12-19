use sea_orm::{DatabaseConnection, DbConn};
use sea_orm::sqlx::Connection;

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

