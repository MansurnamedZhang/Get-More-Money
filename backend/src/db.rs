use sqlx::{
    SqlitePool,
    migrate::MigrateError,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};
use std::{path::Path, str::FromStr, time::Duration};
use thiserror::Error;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[derive(Debug, Error)]
pub enum DbInitError {
    #[error("failed to prepare database directory: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to connect to database: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("failed to migrate database: {0}")]
    Migrate(#[from] MigrateError),
}

pub async fn connect(database_url: &str) -> Result<SqlitePool, DbInitError> {
    ensure_database_directory(database_url)?;

    let is_memory = database_url.contains(":memory:");
    let mut options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5));

    if !is_memory {
        options = options
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal);
    }

    let pool = SqlitePoolOptions::new()
        .max_connections(if is_memory { 1 } else { 5 })
        .connect_with(options)
        .await?;

    MIGRATOR.run(&pool).await?;
    Ok(pool)
}

fn ensure_database_directory(database_url: &str) -> Result<(), std::io::Error> {
    if database_url.contains(":memory:") {
        return Ok(());
    }

    if let Some(raw_path) = database_url.strip_prefix("sqlite://") {
        let raw_path = raw_path.split('?').next().unwrap_or(raw_path);
        if let Some(parent) = Path::new(raw_path).parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
    }

    Ok(())
}
