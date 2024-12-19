pub fn get_db_url() -> String {
    dotenv::var("DATABASE_URL").unwrap_or("sqlite::memory:".to_string())
}

