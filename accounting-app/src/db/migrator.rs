use crate::error::Result;
use rusqlite::Connection;
use std::path::Path;

pub struct DbMigrator;

impl DbMigrator {
    pub const INITIAL_SCHEMA: &'static str = include_str!("../../migrations/V001__initial_schema.sql");

    /// Initializes a company SQLite database and executes migrations within a transaction.
    pub fn initialize_database(conn: &mut Connection) -> Result<()> {
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;

        let tx = conn.transaction()?;

        // Schema history tracking table
        tx.execute(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                description TEXT NOT NULL,
                applied_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            )",
            [],
        )?;

        let current_version: Option<i64> = tx
            .query_row(
                "SELECT MAX(version) FROM schema_migrations",
                [],
                |r| r.get(0),
            )
            .unwrap_or(None);

        if current_version.is_none() || current_version.unwrap() < 1 {
            // Apply V001
            tx.execute_batch(Self::INITIAL_SCHEMA)?;
            tx.execute(
                "INSERT INTO schema_migrations (version, description) VALUES (1, 'initial_schema')",
                [],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    /// Opens or creates a database at path and runs migrations.
    pub fn open_and_migrate<P: AsRef<Path>>(path: P) -> Result<Connection> {
        let mut conn = Connection::open(path)?;
        Self::initialize_database(&mut conn)?;
        Ok(conn)
    }

    /// Creates an in-memory database with migrations applied (ideal for unit/integration tests).
    pub fn open_in_memory() -> Result<Connection> {
        let mut conn = Connection::open_in_memory()?;
        Self::initialize_database(&mut conn)?;
        Ok(conn)
    }
}
