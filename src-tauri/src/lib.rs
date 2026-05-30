#![recursion_limit = "512"]

mod database;
mod dsp;
mod embeddings;
mod spectrogram;
mod classifier;
mod bpm;
mod scanner;
mod analysis;
mod commands;
pub mod error;
pub mod hardware;

use database::DbManager;
use std::sync::Mutex;
use tauri::Manager;

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
            commands::config::get_theme,
            commands::config::save_theme,
            commands::library::select_directory,
            commands::library::get_watched_directories,
            commands::library::add_watched_directory,
            commands::library::remove_watched_directory,
            commands::library::get_track_count,
            commands::library::get_tracks,
            commands::library::reveal_in_finder,
            commands::library::save_sidecar,
            commands::library::export_sidecars,
            scanner::scan_all_libraries,
            commands::analysis::run_analysis_pipeline,
            commands::analysis::is_analysis_running,
            commands::analysis::get_pass_stats,
            commands::analysis::reset_pass,
            commands::analysis::reset_all_passes,
            commands::map::get_projection_coordinates,
            commands::map::search_similar_tracks_audio,
            commands::map::recompute_projection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
