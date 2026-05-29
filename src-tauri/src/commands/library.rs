use std::sync::Mutex;
use rusqlite::Connection;
use crate::database::{WatchedDirectory, Track};
use crate::scanner;
use crate::error::AppError;

/// Spawns a native directory picker dialog using rfd and returns selected path.
#[tauri::command]
pub fn select_directory() -> Result<Option<String>, AppError> {
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
pub fn get_watched_directories(
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<Vec<WatchedDirectory>, AppError> {
    let conn = conn_state.lock().map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
    let dirs = WatchedDirectory::find_all(&conn)?;
    Ok(dirs)
}

/// Registers a new directory with path validation and unique checks.
#[tauri::command]
pub fn add_watched_directory(
    name: String,
    path: String,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<(), AppError> {
    let trimmed_name = name.trim();
    let trimmed_path = path.trim();
    
    if trimmed_name.is_empty() || trimmed_path.is_empty() {
        return Err(AppError::Generic("Collection name and directory path cannot be empty.".to_string()));
    }
    
    let conn = conn_state.lock().map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
    let dir = WatchedDirectory {
        id: 0,
        name: trimmed_name.to_string(),
        path: trimmed_path.to_string(),
    };
    dir.insert(&conn).map_err(|e| {
        let err_str = e.to_string();
        if err_str.contains("UNIQUE constraint failed") {
            AppError::Generic("This folder path is already registered under another collection.".to_string())
        } else {
            AppError::Database(e)
        }
    })?;
    Ok(())
}

/// Unregisters a watched directory by ID.
#[tauri::command]
pub fn remove_watched_directory(
    id: i64,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<(), AppError> {
    let conn = conn_state.lock().map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
    WatchedDirectory::delete(&conn, id)?;
    Ok(())
}

/// Queries the total number of track records in the database.
#[tauri::command]
pub fn get_track_count(
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<i64, AppError> {
    let conn = conn_state.lock().map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
    let count = Track::count(&conn)?;
    Ok(count)
}

/// Retrieve all indexed tracks from the database.
#[tauri::command]
pub fn get_tracks(
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<Vec<Track>, AppError> {
    let conn = conn_state.lock().map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
    let tracks = Track::find_all(&conn)?;
    Ok(tracks)
}

/// Writes a .dc.json sidecar file next to the given track's audio file.
#[tauri::command]
pub fn save_sidecar(
    track_id: i64,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<(), AppError> {
    let conn = conn_state.lock().map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
    scanner::sidecar::save(&conn, track_id)?;
    Ok(())
}

/// Writes .dc.json sidecar files for every track in the database.
/// Returns the number of files written successfully.
#[tauri::command]
pub fn export_sidecars(
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<usize, AppError> {
    let conn = conn_state.lock().map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
    let count = scanner::sidecar::export_all(&conn)?;
    Ok(count)
}

/// Opens the system file manager and selects the given file.
/// macOS: open -R <path>  |  Windows: explorer /select,<path>  |  Linux: xdg-open <parent dir>
#[tauri::command]
pub fn reveal_in_finder(path: String) -> Result<(), AppError> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-R", &path])
            .spawn()?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(format!("/select,{}", path))
            .spawn()?;
    }
    #[cfg(target_os = "linux")]
    {
        let parent = std::path::Path::new(&path)
            .parent()
            .ok_or_else(|| AppError::Generic("Could not determine parent directory".to_string()))?;
        std::process::Command::new("xdg-open")
            .arg(parent)
            .spawn()?;
    }
    Ok(())
}
