#![recursion_limit = "512"]

mod database;
mod scanner;

use database::{DbManager, WatchedDirectory};
use rusqlite::Connection;
use std::sync::Mutex;
use tauri::Manager;

#[tauri::command]
fn get_theme(
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<String, String> {
    let conn = conn_state.lock().map_err(|e| e.to_string())?;
    let theme: String = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'theme'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(theme)
}

#[tauri::command]
fn save_theme(
    conn_state: tauri::State<'_, Mutex<Connection>>,
    theme: String,
) -> Result<(), String> {
    let conn = conn_state.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('theme', ?)",
        [theme],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Spawns a native directory picker dialog using rfd and returns selected path.
#[tauri::command]
fn select_directory() -> Result<Option<String>, String> {
    let folder = rfd::FileDialog::new()
        .set_title("Select Music Folder")
        .pick_folder();
    
    if let Some(path_buf) = folder {
        Ok(Some(path_buf.to_string_lossy().into_owned()))
    } else {
        Ok(None)
    }
}

/// Retrieve all registered directories from database.
#[tauri::command]
fn get_watched_directories(
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<Vec<WatchedDirectory>, String> {
    let conn = conn_state.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, path FROM watched_directories ORDER BY id DESC")
        .map_err(|e| e.to_string())?;
    
    let rows = stmt
        .query_map([], |row| {
            Ok(WatchedDirectory {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut list = Vec::new();
    for row in rows {
        list.push(row.map_err(|e| e.to_string())?);
    }
    Ok(list)
}

/// Registers a new directory with path validation and unique checks.
#[tauri::command]
fn add_watched_directory(
    name: String,
    path: String,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<(), String> {
    let trimmed_name = name.trim();
    let trimmed_path = path.trim();
    
    if trimmed_name.is_empty() || trimmed_path.is_empty() {
        return Err("Collection name and directory path cannot be empty.".to_string());
    }
    
    let conn = conn_state.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO watched_directories (name, path) VALUES (?1, ?2)",
        [trimmed_name, trimmed_path],
    )
    .map_err(|e| {
        let err_str = e.to_string();
        if err_str.contains("UNIQUE constraint failed") {
            "This folder path is already registered under another collection.".to_string()
        } else {
            err_str
        }
    })?;
    Ok(())
}

/// Unregisters a watched directory by ID.
#[tauri::command]
fn remove_watched_directory(
    id: i64,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<(), String> {
    let conn = conn_state.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM watched_directories WHERE id = ?1", [id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Queries the total number of track records in the database.
#[tauri::command]
fn get_track_count(
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<i64, String> {
    let conn = conn_state.lock().map_err(|e| e.to_string())?;
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM tracks", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    Ok(count)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Dynamically load the C-based sqlite-vec extension globally before booting any database
    unsafe {
        let _ = rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_drag::init())
        .plugin(tauri_plugin_log::Builder::default().build())
        .setup(|app| {
            // Initialize database manager and bootstrap SQLite
            let db_manager = DbManager::new(app.handle());
            match db_manager.connect_and_migrate() {
                Ok(conn) => {
                    // Manage the thread-safe connection state inside Tauri
                    app.manage(Mutex::new(conn));
                }
                Err(err) => {
                    log::error!("Database initialization failed: {}", err);
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_theme,
            save_theme,
            select_directory,
            get_watched_directories,
            add_watched_directory,
            remove_watched_directory,
            get_track_count,
            scanner::scan_all_libraries
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
