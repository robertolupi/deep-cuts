#![recursion_limit = "512"]

mod analysis;
mod bpm;
mod classifier;
mod commands;
mod database;
mod dsp;
mod embeddings;
pub mod error;
pub mod hardware;
mod llama;
mod scanner;
mod spectrogram;

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

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_drag::init())
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Debug)
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("deep-cuts".into()),
                    }),
                ])
                .build(),
        )
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

            // Manage the thread-safe llama-server background child process
            app.manage(llama::LlamaServerState {
                child: Mutex::new(None),
                port: Mutex::new(None),
            });

            // Automatically scan all libraries on startup to detect changed files
            let app_handle_scan = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let _ = scanner::scan_all_libraries(app_handle_scan).await;
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::config::get_theme,
            commands::config::save_theme,
            commands::config::get_model_path_setting,
            commands::config::save_model_path_setting,
            commands::library::select_directory,
            commands::library::get_watched_directories,
            commands::library::add_watched_directory,
            commands::library::remove_watched_directory,
            commands::library::get_track_count,
            commands::library::get_tracks,
            commands::library::reveal_in_finder,
            commands::library::get_cover_art,
            commands::library::save_sidecar,
            commands::library::export_sidecars,
            commands::library::search_semantic_tracks,
            commands::library::search_clap_tracks,
            scanner::scan_all_libraries,
            commands::analysis::run_analysis_pipeline,
            commands::analysis::is_analysis_running,
            commands::analysis::get_pass_stats,
            commands::analysis::recover_stuck_passes,
            commands::analysis::reset_pass,
            commands::analysis::reset_all_passes,
            commands::analysis::check_models_exist,
            commands::map::get_projection_coordinates,
            commands::map::search_similar_tracks_audio,
            commands::map::recompute_projection,
            commands::map::find_duplicate_pairs,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| match event {
        tauri::RunEvent::Exit => {
            log::info!("[tauri] Deep Cuts application exiting. Cleaning up processes...");
            llama::terminate_llama_server(app_handle);
        }
        _ => {}
    });
}
