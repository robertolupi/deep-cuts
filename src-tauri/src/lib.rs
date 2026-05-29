#![recursion_limit = "512"]

mod database;

use database::DbManager;
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
        .invoke_handler(tauri::generate_handler![get_theme, save_theme])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
