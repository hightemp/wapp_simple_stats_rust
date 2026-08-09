use std::time::Duration;

use rocket::http::Status;
use rusqlite::Connection;

use crate::config::AppConfig;
use crate::HandlerResult;

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) fn open(config: &AppConfig) -> HandlerResult<Connection> {
    let connection = Connection::open(config.database_path()).map_err(map_error)?;
    connection.busy_timeout(BUSY_TIMEOUT).map_err(map_error)?;
    Ok(connection)
}

pub(crate) fn map_error(error: rusqlite::Error) -> Status {
    eprintln!("database operation failed: {error}");
    Status::InternalServerError
}

pub(crate) fn initialize(config: &AppConfig) -> rusqlite::Result<()> {
    let connection = Connection::open(config.database_path())?;
    connection.busy_timeout(BUSY_TIMEOUT)?;
    connection.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;
         CREATE TABLE IF NOT EXISTS visitors (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             path TEXT NOT NULL CHECK(length(path) BETWEEN 1 AND 255),
             timestamp TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
             ip TEXT NOT NULL,
             json TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_visitors_path_timestamp
             ON visitors(path, timestamp DESC);
         CREATE INDEX IF NOT EXISTS idx_visitors_timestamp
             ON visitors(timestamp DESC);",
    )?;
    Ok(())
}
