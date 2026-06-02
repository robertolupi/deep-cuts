#![recursion_limit = "512"]

mod acoustid;
mod analysis;
mod bpm;
mod classifier;
pub mod commands;
mod database;
mod dsp;
mod embeddings;
pub mod error;
pub mod hardware;
mod llama;
mod models;
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
                    // Crash recovery: reset any in-flight pending AcoustID lookups
                    let _ = conn.execute(
                        "UPDATE tracks SET acoustid_status = NULL WHERE acoustid_status = 'pending'",
                        [],
                    );

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

            // Manage the thread-safe model download state
            app.manage(commands::download::DownloadState::default());

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
            commands::config::get_acoustid_setting,
            commands::config::save_acoustid_setting,
            commands::config::get_sidecar_setting,
            commands::config::save_sidecar_setting,
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
            commands::manifest::fetch_app_manifest,
            commands::manifest::get_update_settings,
            commands::manifest::set_update_settings,
            commands::download::check_pending_resume,
            commands::download::cancel_model_download,
            commands::download::download_models,
            commands::chat::ask_qwen,
            commands::chat::create_chat_session,
            commands::chat::list_chat_sessions,
            commands::chat::rename_chat_session,
            commands::chat::delete_chat_session,
            commands::chat::get_chat_messages,
            commands::chat::save_chat_message,
            commands::chat::search_chats,
            enrich_track_metadata,
            commands::playlists::get_playlists,
            commands::playlists::create_playlist,
            commands::playlists::delete_playlist,
            commands::playlists::rename_playlist,
            commands::playlists::get_playlist_tracks,
            commands::playlists::add_tracks_to_playlist,
            commands::playlists::remove_track_from_playlist,
            commands::playlists::reorder_playlist_track,
            commands::playlists::get_saved_searches,
            commands::playlists::create_saved_search,
            commands::playlists::delete_saved_search,
            commands::playlists::update_saved_search,
            commands::playlists::get_playlists_for_track,
            commands::playlists::remove_track_from_playlist_by_id,
            commands::playlists::export_m3u_playlist,
            commands::statistics::get_track_stats,
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

#[tauri::command]
async fn enrich_track_metadata(
    track_id: i64,
    force: Option<bool>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    log::info!("[ipc] enrich_track_metadata called for track_id: {}", track_id);
    let force_val = force.unwrap_or(false);
    
    // Spawn the async enrichment pipeline in a background task so it doesn't block IPC
    tauri::async_runtime::spawn(async move {
        if let Err(e) = acoustid::enrich_track(track_id, force_val, &app_handle).await {
            log::error!("[acoustid] Metadata enrichment failed: {}", e);
        }
    });

    Ok(())
}
