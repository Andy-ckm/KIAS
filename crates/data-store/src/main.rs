//! KIAS Data Store binary.
//!
//! Standalone binary that initializes the database and runs migrations.

use kias_data_store::MigrationRunner;
use sqlx::sqlite::SqlitePoolOptions;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let db_path = std::env::var("KIAS_DB_PATH").unwrap_or_else(|_| "kias.db".to_string());
    let url = format!("sqlite:{db_path}?mode=rwc");

    info!("Connecting to database: {db_path}");
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await?;

    let runner = MigrationRunner::new(pool);
    let applied = runner.run_all().await?;

    if applied.is_empty() {
        info!("Database is up to date");
    } else {
        info!("Applied {} migration(s): {:?}", applied.len(), applied);
    }

    Ok(())
}
