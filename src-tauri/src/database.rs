use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct WatchedDirectory {
    pub id: i64,
    pub name: String,
    pub path: String,
}

pub struct DbManager {
    db_path: PathBuf,
}

impl DbManager {
    /// Resolves standard sandbox OS storage folders and defines database file path.
    pub fn new(app_handle: &AppHandle) -> Self {
        // Resolve: ~/Library/Application Support/com.rlupi.deep-cuts/
        let app_data_dir = app_handle
            .path()
            .app_data_dir()
            .expect("Failed to resolve standard OS App Data Directory");
        
        // Ensure path exists
        if !app_data_dir.exists() {
            fs::create_dir_all(&app_data_dir).expect("Failed to create App Data Directory");
        }

        let db_path = app_data_dir.join("deep_cuts.db");
        DbManager { db_path }
    }

    /// Connects to SQLite and performs DB migrations up to the latest schema.
    pub fn connect_and_migrate(&self) -> Result<Connection, Box<dyn std::error::Error>> {
        let mut conn = Connection::open(&self.db_path)?;

        // Enable foreign key constraints
        conn.execute("PRAGMA foreign_keys = ON;", [])?;

        // Run migrations up to latest version
        let migrations = get_migrations();
        migrations.to_latest(&mut conn)?;

        Ok(conn)
    }
}

/// Returns the schema migrations vector chronologically.
pub fn get_migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(
            "CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT OR IGNORE INTO app_settings (key, value) VALUES ('theme', 'system');"
        ),
        M::up(
            "CREATE TABLE IF NOT EXISTS watched_directories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                path TEXT NOT NULL UNIQUE
            );"
        ),
    ])
}

#[cfg(test)]
pub fn setup_test_db() -> Connection {
    // Dynamically load the C-based sqlite-vec extension globally before booting the in-memory test database
    unsafe {
        let _ = rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    }
    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute("PRAGMA foreign_keys = ON;", []).unwrap();
    let migrations = get_migrations();
    migrations.to_latest(&mut conn).unwrap();
    conn
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_migrations_boot_successfully() {
        let conn = setup_test_db();

        let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table'").unwrap();
        let table_names: Vec<String> = stmt.query_map([], |row| row.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        assert!(table_names.contains(&"app_settings".to_string()));
        assert!(table_names.contains(&"watched_directories".to_string()));

        // Migrations must seed default theme setting
        let theme: String = conn.query_row(
            "SELECT value FROM app_settings WHERE key = 'theme'",
            [],
            |row| row.get(0),
        ).expect("theme setting missing after migrations");
        assert_eq!(theme, "system");

        // Verify CRUD on watched_directories
        conn.execute(
            "INSERT INTO watched_directories (name, path) VALUES ('My Music', '/Users/rlupi/Music')",
            [],
        ).unwrap();

        let dir: WatchedDirectory = conn.query_row(
            "SELECT id, name, path FROM watched_directories WHERE name = 'My Music'",
            [],
            |row| {
                Ok(WatchedDirectory {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    path: row.get(2)?,
                })
            },
        ).unwrap();
        assert_eq!(dir.name, "My Music");
        assert_eq!(dir.path, "/Users/rlupi/Music");
    }
}
