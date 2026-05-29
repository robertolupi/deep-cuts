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

/// Retrieve all indexed tracks from the database.
#[tauri::command]
fn get_tracks(
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<Vec<database::Track>, String> {
    let conn = conn_state.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, watched_directory_id, path, filename, size_bytes, last_modified,
                    duration_seconds, sample_rate, bitrate, channels, bit_depth,
                    title, artist, album, genre, year, track_number, track_total,
                    disc_number, disc_total, album_artist, composer, comment, bpm, lyrics
             FROM tracks ORDER BY artist ASC, album ASC, track_number ASC",
        )
        .map_err(|e| e.to_string())?;
    
    let rows = stmt
        .query_map([], |row| {
            Ok(database::Track {
                id: row.get(0)?,
                watched_directory_id: row.get(1)?,
                path: row.get(2)?,
                filename: row.get(3)?,
                size_bytes: row.get(4)?,
                last_modified: row.get(5)?,
                duration_seconds: row.get(6)?,
                sample_rate: row.get(7)?,
                bitrate: row.get(8)?,
                channels: row.get(9)?,
                bit_depth: row.get(10)?,
                title: row.get(11)?,
                artist: row.get(12)?,
                album: row.get(13)?,
                genre: row.get(14)?,
                year: row.get(15)?,
                track_number: row.get(16)?,
                track_total: row.get(17)?,
                disc_number: row.get(18)?,
                disc_total: row.get(19)?,
                album_artist: row.get(20)?,
                composer: row.get(21)?,
                comment: row.get(22)?,
                bpm: row.get(23)?,
                lyrics: row.get(24)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut list = Vec::new();
    for row in rows {
        list.push(row.map_err(|e| e.to_string())?);
    }
    Ok(list)
}

/// Opens the system file manager and selects the given file.
/// macOS: open -R <path>  |  Windows: explorer /select,<path>  |  Linux: xdg-open <parent dir>
#[tauri::command]
fn reveal_in_finder(path: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-R", &path])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(format!("/select,{}", path))
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        let parent = std::path::Path::new(&path)
            .parent()
            .ok_or_else(|| "Could not determine parent directory".to_string())?;
        std::process::Command::new("xdg-open")
            .arg(parent)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
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
            get_tracks,
            reveal_in_finder,
            scanner::scan_all_libraries
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
