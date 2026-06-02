use crate::error::AppError;
use rusqlite::Connection;
use std::sync::Mutex;

#[tauri::command]
pub fn get_theme(conn_state: tauri::State<'_, Mutex<Connection>>) -> Result<String, AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
    let theme: String = conn.query_row(
        "SELECT value FROM app_settings WHERE key = 'theme'",
        [],
        |row| row.get(0),
    )?;
    Ok(theme)
}

#[tauri::command]
pub fn save_theme(
    conn_state: tauri::State<'_, Mutex<Connection>>,
    theme: String,
) -> Result<(), AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('theme', ?)",
        [theme],
    )?;
    Ok(())
}

#[tauri::command]
pub fn get_model_path_setting(
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<Option<String>, AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
    let path = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'model_path'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    Ok(path)
}

#[tauri::command]
pub fn save_model_path_setting(
    conn_state: tauri::State<'_, Mutex<Connection>>,
    path: Option<String>,
) -> Result<(), AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
    let path = path
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    if let Some(path) = path {
        conn.execute(
            "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('model_path', ?)",
            [path],
        )?;
    } else {
        conn.execute("DELETE FROM app_settings WHERE key = 'model_path'", [])?;
    }
    Ok(())
}

#[tauri::command]
pub fn get_acoustid_setting(
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<String, AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
    let val: String = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'acoustid_enrichment_enabled'",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "silent".to_string());
    Ok(val)
}

#[tauri::command]
pub fn save_acoustid_setting(
    conn_state: tauri::State<'_, Mutex<Connection>>,
    value: String,
) -> Result<(), AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('acoustid_enrichment_enabled', ?)",
        [value],
    )?;
    Ok(())
}
