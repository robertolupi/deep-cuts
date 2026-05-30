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
