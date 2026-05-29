pub mod db;
pub mod fs;
pub mod metadata;
pub mod sidecar;

use std::collections::HashSet;
use rusqlite::Connection;
use tauri::{AppHandle, Emitter};

#[derive(Clone, serde::Serialize)]
pub struct ScanProgressPayload {
    pub is_scanning: bool,
    pub progress: f64,
    pub current_file: String,
    pub processed_count: usize,
    pub total_count: usize,
}

pub trait ScannerReporter: Send + Sync + 'static {
    fn report_progress(&self, payload: ScanProgressPayload);
}

impl ScannerReporter for AppHandle {
    fn report_progress(&self, payload: ScanProgressPayload) {
        let _ = self.emit("scan:progress", payload);
    }
}

pub struct LibraryScanner;

impl LibraryScanner {
    pub fn scan<R: ScannerReporter>(reporter: &R, conn: &mut Connection) -> Result<String, String> {
        // 1. Fetch all watched directories
        let dirs = {
            let mut stmt = match conn.prepare("SELECT id, name, path FROM watched_directories") {
                Ok(s) => s,
                Err(e) => return Err(e.to_string()),
            };

            let dir_rows = match stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            }) {
                Ok(r) => r,
                Err(e) => return Err(e.to_string()),
            };

            let mut list = Vec::new();
            for dir_res in dir_rows {
                if let Ok(d) = dir_res {
                    list.push(d);
                }
            }
            list
        };

        // Emit scan start event
        reporter.report_progress(ScanProgressPayload {
            is_scanning: true,
            progress: 0.0,
            current_file: "Initializing scanner...".to_string(),
            processed_count: 0,
            total_count: 0,
        });

        let mut all_discovered_files = Vec::new();
        let mut directory_file_maps = Vec::new();

        // 2. Walk directories on disk (I/O Bound)
        for (dir_id, dir_name, dir_path) in &dirs {
            reporter.report_progress(ScanProgressPayload {
                is_scanning: true,
                progress: 0.0,
                current_file: format!("Traversing folder: {}...", dir_name),
                processed_count: 0,
                total_count: 0,
            });

            match fs::walk_directory(dir_path) {
                Ok(Some(files)) => {
                    all_discovered_files.extend(files.clone());
                    directory_file_maps.push((*dir_id, files));
                }
                Ok(None) => {
                    eprintln!("Drive Safety Check: Directory '{}' ({}) is dismounted. Skipping folder reconciliations.", dir_name, dir_path);
                }
                Err(e) => {
                    eprintln!("Failed to scan directory '{}': {:?}", dir_path, e);
                }
            }
        }

        let total_files = all_discovered_files.len();
        if total_files == 0 {
            // Reconcile deleted files for the successfully scanned directories (which are empty)
            for (dir_id, files) in &directory_file_maps {
                let active_paths: HashSet<String> = files.iter().map(|f| f.path.clone()).collect();
                let _ = db::reconcile_deleted_tracks(conn, *dir_id, &active_paths);
            }

            reporter.report_progress(ScanProgressPayload {
                is_scanning: false,
                progress: 100.0,
                current_file: "Scan complete. No audio files found.".to_string(),
                processed_count: 0,
                total_count: 0,
            });
            return Ok("No files discovered.".to_string());
        }

        // 3. Cache Validation (Incremental Scanning)
        let mut cache_misses = Vec::new();
        let mut cache_hits_count = 0;

        for file in all_discovered_files {
            if let Some((cached_size, cached_modified)) = db::get_cached_track_details(conn, &file.path) {
                if cached_size == file.size_bytes && cached_modified == file.last_modified {
                    cache_hits_count += 1;
                    continue;
                }
            }
            cache_misses.push(file);
        }

        let total_misses = cache_misses.len();

        reporter.report_progress(ScanProgressPayload {
            is_scanning: true,
            progress: (cache_hits_count as f64 / total_files as f64) * 100.0,
            current_file: format!("Cache verified. Found {} new/modified files.", total_misses),
            processed_count: cache_hits_count,
            total_count: total_files,
        });

        // 4. Parallel Lofty Parsing (CPU Bound Rayon step in chunks)
        // Run chunks of 50 to emit progress logs and maintain responsiveness
        let chunk_size = 50;
        let mut processed_count = cache_hits_count;
        let mut parsed_results = Vec::new();

        for chunk in cache_misses.chunks(chunk_size) {
            let chunk_results = metadata::parse_multiple_files_parallel(chunk);
            parsed_results.extend(chunk_results);
            processed_count += chunk.len();

            let last_file = chunk.last().map(|f| f.filename.clone()).unwrap_or_default();
            reporter.report_progress(ScanProgressPayload {
                is_scanning: true,
                progress: (processed_count as f64 / total_files as f64) * 100.0,
                current_file: format!("Parsing: {}", last_file),
                processed_count,
                total_count: total_files,
            });
        }

        // Build path-to-directory mapping
        let mut path_to_dir_map = std::collections::HashMap::new();
        for (dir_id, files) in &directory_file_maps {
            for file in files {
                path_to_dir_map.insert(file.path.clone(), *dir_id);
            }
        }

        let mut tracks_to_upsert = parsed_results;
        for track in &mut tracks_to_upsert {
            if let Some(dir_id) = path_to_dir_map.get(&track.path) {
                track.watched_directory_id = *dir_id;
            }
        }

        // 5. Transactional Batch Database Imports (High-speed SQLite)
        if !tracks_to_upsert.is_empty() {
            reporter.report_progress(ScanProgressPayload {
                is_scanning: true,
                progress: 95.0,
                current_file: "Saving records to database...".to_string(),
                processed_count,
                total_count: total_files,
            });

            if let Err(e) = db::upsert_tracks_transactional(conn, &tracks_to_upsert) {
                eprintln!("Database upsert error: {:?}", e);
            }
        }

        // 5b. Sidecar restore — reload ML fields for newly-upserted tracks
        if !tracks_to_upsert.is_empty() {
            let paths: Vec<&str> = tracks_to_upsert.iter().map(|t| t.path.as_str()).collect();
            let id_map = db::get_track_ids_by_paths(conn, &paths);
            for track in &tracks_to_upsert {
                if let Some(&track_id) = id_map.get(&track.path) {
                    if let Err(e) = sidecar::restore(conn, track_id, &track.path) {
                        eprintln!("Sidecar restore failed for '{}': {}", track.filename, e);
                    }
                }
            }
        }

        // 6. Deletion Reconciliation (Prune tracks no longer present on disk)
        for (dir_id, files) in &directory_file_maps {
            let active_paths: HashSet<String> = files.iter().map(|f| f.path.clone()).collect();
            if let Err(e) = db::reconcile_deleted_tracks(conn, *dir_id, &active_paths) {
                eprintln!("Deletion reconciliation error: {:?}", e);
            }
        }

        // Finalize scan progress
        reporter.report_progress(ScanProgressPayload {
            is_scanning: false,
            progress: 100.0,
            current_file: "Scan complete!".to_string(),
            processed_count: total_files,
            total_count: total_files,
        });

        Ok(format!("Scan complete. Indexed {} files.", total_files))
    }
}

#[tauri::command]
pub async fn scan_all_libraries(
    app_handle: AppHandle,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db_manager = crate::database::DbManager::new(&app_handle);
        let mut conn = match db_manager.connect_and_migrate() {
            Ok(c) => c,
            Err(e) => {
                let err_msg = format!("Failed to open scanner DB connection: {}", e);
                eprintln!("{}", err_msg);
                return Err(err_msg);
            }
        };

        LibraryScanner::scan(&app_handle, &mut conn)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::setup_test_db;
    use std::sync::Mutex;

    struct DummyReporter {
        payloads: Mutex<Vec<ScanProgressPayload>>,
    }

    impl ScannerReporter for DummyReporter {
        fn report_progress(&self, payload: ScanProgressPayload) {
            self.payloads.lock().unwrap().push(payload);
        }
    }

    #[test]
    fn test_library_scanner_empty_dir() {
        let mut conn = setup_test_db();
        
        // Setup a watched directory
        conn.execute(
            "INSERT INTO watched_directories (id, name, path) VALUES (1, 'Empty Collection', '/non_existent_path_to_force_empty')",
            [],
        ).unwrap();

        let reporter = DummyReporter {
            payloads: Mutex::new(Vec::new()),
        };

        let result = LibraryScanner::scan(&reporter, &mut conn);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "No files discovered.");

        let payloads = reporter.payloads.lock().unwrap();
        assert!(!payloads.is_empty());
        // Verify start and end progress reports
        assert!(payloads[0].is_scanning);
        assert_eq!(payloads[0].current_file, "Initializing scanner...");
        
        let last_payload = payloads.last().unwrap();
        assert!(!last_payload.is_scanning);
        assert_eq!(last_payload.progress, 100.0);
    }
}

