#![recursion_limit = "512"]

mod database;
mod dsp;
mod embeddings;
mod scanner;

use database::{pass_status, DbManager, WatchedDirectory};
use rusqlite::Connection;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{Emitter, Manager};

static ANALYSIS_ACTIVE: AtomicBool = AtomicBool::new(false);

// RAII guard that clears ANALYSIS_ACTIVE when the pipeline scope exits
struct ActiveGuard;
impl Drop for ActiveGuard {
    fn drop(&mut self) {
        ANALYSIS_ACTIVE.store(false, Ordering::SeqCst);
    }
}

struct SleepPreventer {
    _handle: Option<keepawake::AwakeHandle>,
}

impl SleepPreventer {
    fn new() -> Self {
        let handle = keepawake::Builder::new()
            .display(false)
            .idle(true)
            .sleep(true)
            .reason("Deep Cuts Backend Analysis")
            .create();
        
        match handle {
            Ok(h) => {
                log::info!("[sleep-preventer] Sleep prevention active across all platforms!");
                Self { _handle: Some(h) }
            }
            Err(e) => {
                log::warn!("[sleep-preventer] Failed to enable sleep prevention: {}", e);
                Self { _handle: None }
            }
        }
    }
}


#[derive(serde::Serialize)]
struct PassError {
    path: String,
    log: Option<String>,
    duration_ms: Option<i64>,
    last_run_at: Option<String>,
}

#[derive(serde::Serialize)]
struct PassStats {
    pass_name: String,
    pending: i64,
    in_progress: i64,
    done: i64,
    failed: i64,
    total: i64,
    avg_duration_ms: Option<f64>,
    errors: Vec<PassError>,
}

struct SpoolJob {
    pass_id: i64,
    track_id: i64,
    path: String,
}

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
                    disc_number, disc_total, album_artist, composer, comment, bpm, lyrics,
                    waveform_data, key, scale, key_strength, loudness_lufs, loudness_range
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
                waveform_data: row.get(25)?,
                key: row.get(26)?,
                scale: row.get(27)?,
                key_strength: row.get(28)?,
                loudness_lufs: row.get(29)?,
                loudness_range: row.get(30)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut list = Vec::new();
    for row in rows {
        list.push(row.map_err(|e| e.to_string())?);
    }
    Ok(list)
}

/// Writes a .dc.json sidecar file next to the given track's audio file.
#[tauri::command]
fn save_sidecar(
    track_id: i64,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<(), String> {
    let conn = conn_state.lock().map_err(|e| e.to_string())?;
    scanner::sidecar::save(&conn, track_id).map_err(|e| e.to_string())
}

/// Writes .dc.json sidecar files for every track in the database.
/// Returns the number of files written successfully.
#[tauri::command]
fn export_sidecars(
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<usize, String> {
    let conn = conn_state.lock().map_err(|e| e.to_string())?;
    scanner::sidecar::export_all(&conn).map_err(|e| e.to_string())
}

/// Runs the audio analysis pass on all pending tracks concurrently.
/// Backfills track_passes rows, then processes them in parallel using num_cpus/2 threads.
#[tauri::command]
fn run_analysis_pipeline(
    app: tauri::AppHandle,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<(), String> {
    use std::collections::VecDeque;
    use std::sync::Arc;

    if ANALYSIS_ACTIVE.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return Err("Analysis is already running".to_string());
    }
    let _guard = ActiveGuard;
    let sleep_preventer = SleepPreventer::new();

    let pending: Vec<SpoolJob> = {
        let conn = conn_state.lock().map_err(|e| e.to_string())?;

        // Reset interrupted/failed rows for retry
        conn.execute(
            "UPDATE track_passes SET status = ?1, log = NULL, last_run_at = NULL
             WHERE status IN (?2, ?3)",
            rusqlite::params![pass_status::PENDING, pass_status::IN_PROGRESS, pass_status::FAILED],
        ).map_err(|e| e.to_string())?;

        // Backfill: insert a row for every track that doesn't have one yet
        conn.execute(
            "INSERT OR IGNORE INTO track_passes (track_id, pass_name, priority, status)
             SELECT id, 'audio_analysis', 10, ?1 FROM tracks",
            [pass_status::PENDING],
        ).map_err(|e| e.to_string())?;

        // Backfill clap pass (priority 20 — runs after audio_analysis)
        conn.execute(
            "INSERT OR IGNORE INTO track_passes (track_id, pass_name, priority, status)
             SELECT id, 'clap', 20, ?1 FROM tracks",
            [pass_status::PENDING],
        ).map_err(|e| e.to_string())?;

        let mut stmt = conn
            .prepare(
                "SELECT tp.id, tp.track_id, t.path
                 FROM track_passes tp
                 JOIN tracks t ON t.id = tp.track_id
                 WHERE tp.status = ?1 AND tp.pass_name = 'audio_analysis'
                 ORDER BY tp.id ASC",
            )
            .map_err(|e| e.to_string())?;

        let rows: Vec<SpoolJob> = stmt
            .query_map([pass_status::PENDING], |row| {
                Ok(SpoolJob {
                    pass_id: row.get(0)?,
                    track_id: row.get(1)?,
                    path: row.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        for job in &rows {
            let _ = conn.execute(
                "UPDATE track_passes SET status = ?1, last_run_at = CURRENT_TIMESTAMP
                 WHERE id = ?2",
                rusqlite::params![pass_status::IN_PROGRESS, job.pass_id],
            );
        }
        rows
    };

    let total = pending.len();

    // Check if there are any pending CLAP passes
    let has_clap = {
        let conn = conn_state.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM track_passes WHERE status = ?1 AND pass_name = 'clap')",
            [pass_status::PENDING],
            |row| row.get(0),
        ).unwrap_or(false)
    };

    if total == 0 && !has_clap {
        return Ok(());
    }

    let concurrency = (std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2)
        / 2)
        .max(1);

    let queue = Arc::new(Mutex::new(VecDeque::from(pending)));
    let conn_arc = Arc::new(Mutex::new({
        let db_manager = database::DbManager::new(&app);
        db_manager.connect_and_migrate().map_err(|e| e.to_string())?
    }));

    let mut handles = Vec::new();
    if total > 0 {
        for _ in 0..concurrency {
            let queue_clone = Arc::clone(&queue);
            let conn_clone = Arc::clone(&conn_arc);
            let app_clone = app.clone();

            handles.push(std::thread::spawn(move || {
                loop {
                    let job = {
                        let mut q = queue_clone.lock().unwrap();
                        q.pop_front()
                    };
                    let job = match job {
                        Some(j) => j,
                        None => break,
                    };

                    let start = std::time::Instant::now();
                    let result = dsp::run_audio_analysis(&job.path);
                    let elapsed_ms = start.elapsed().as_millis() as i64;

                    let conn = conn_clone.lock().unwrap();
                    match result {
                        Ok(analysis) => {
                            let _ = conn.execute(
                                "UPDATE tracks SET
                                    duration_seconds = ?1,
                                    waveform_data = ?2,
                                    bpm = ?3,
                                    key = ?4,
                                    scale = ?5,
                                    key_strength = ?6,
                                    loudness_lufs = ?7,
                                    loudness_range = ?8
                                 WHERE id = ?9",
                                rusqlite::params![
                                    analysis.duration_seconds as i64,
                                    analysis.waveform_data,
                                    analysis.bpm,
                                    analysis.key,
                                    analysis.scale,
                                    analysis.key_strength,
                                    analysis.loudness_lufs,
                                    analysis.loudness_range,
                                    job.track_id,
                                ],
                            );
                            let _ = conn.execute(
                                "UPDATE track_passes SET status = ?1, duration_ms = ?2,
                                 last_run_at = CURRENT_TIMESTAMP WHERE id = ?3",
                                rusqlite::params![pass_status::DONE, elapsed_ms, job.pass_id],
                            );
                            let _ = app_clone.emit("analysis-progress", serde_json::json!({
                                "track_id": job.track_id,
                                "status": pass_status::DONE,
                            }));
                        }
                        Err(e) => {
                            let _ = conn.execute(
                                "UPDATE track_passes SET status = ?1, log = ?2, duration_ms = ?3,
                                 last_run_at = CURRENT_TIMESTAMP WHERE id = ?4",
                                rusqlite::params![pass_status::FAILED, e, elapsed_ms, job.pass_id],
                            );
                            let _ = app_clone.emit("analysis-progress", serde_json::json!({
                                "track_id": job.track_id,
                                "status": pass_status::FAILED,
                            }));
                        }
                    }
                }
            }));
        }
    }

    // Wait for audio_analysis workers on a background thread so the IPC call returns immediately.
    // Then immediately run the clap pass sequentially (1 thread — model is memory-heavy).
    // Moving _guard into the thread keeps ANALYSIS_ACTIVE=true until all passes finish.
    std::thread::spawn(move || {
        let _guard = _guard;
        let _preventer_guard = sleep_preventer;

        // ── Phase 1: audio_analysis (parallel) ────────────────────────────
        for h in handles {
            let _ = h.join();
        }
        let _ = app.emit("analysis-phase-complete", serde_json::json!({ "pass": "audio_analysis" }));

        // ── Phase 2: clap (single thread, model loaded once via OnceLock) ─
        let clap_pending: Vec<SpoolJob> = {
            let conn = conn_arc.lock().unwrap();
            let mut stmt = match conn.prepare(
                "SELECT tp.id, tp.track_id, t.path
                 FROM track_passes tp
                 JOIN tracks t ON t.id = tp.track_id
                 WHERE tp.status = ?1 AND tp.pass_name = 'clap'
                 ORDER BY tp.id ASC",
            ) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[clap] Failed to prepare clap query: {}", e);
                    return;
                }
            };
            let rows: Vec<SpoolJob> = match stmt.query_map([pass_status::PENDING], |row| {
                Ok(SpoolJob {
                    pass_id: row.get(0)?,
                    track_id: row.get(1)?,
                    path: row.get(2)?,
                })
            }) {
                Ok(mapped) => mapped.filter_map(|r| r.ok()).collect(),
                Err(_) => Vec::new(),
            };
            for job in &rows {
                let _ = conn.execute(
                    "UPDATE track_passes SET status = ?1, last_run_at = CURRENT_TIMESTAMP WHERE id = ?2",
                    rusqlite::params![pass_status::IN_PROGRESS, job.pass_id],
                );
            }
            rows
        };

        for job in clap_pending {
            let start = std::time::Instant::now();
            let result = embeddings::run_clap_audio_embed(&job.path, Some(&app));
            let elapsed_ms = start.elapsed().as_millis() as i64;

            let conn = conn_arc.lock().unwrap();
            match result {
                Ok(embedding) => {
                    // Serialise 512 floats as little-endian bytes for sqlite-vec
                    let blob: Vec<u8> = embedding.iter().flat_map(|&f| f.to_le_bytes()).collect();
                    let _ = conn.execute(
                        "INSERT OR REPLACE INTO audio_embeddings (track_id, embedding) VALUES (?1, ?2)",
                        rusqlite::params![job.track_id, blob],
                    );
                    let _ = conn.execute(
                        "UPDATE track_passes SET status = ?1, duration_ms = ?2,
                         last_run_at = CURRENT_TIMESTAMP WHERE id = ?3",
                        rusqlite::params![pass_status::DONE, elapsed_ms, job.pass_id],
                    );
                    let _ = app.emit("analysis-progress", serde_json::json!({
                        "track_id": job.track_id,
                        "pass_name": "clap",
                        "status": pass_status::DONE,
                    }));
                }
                Err(e) => {
                    let _ = conn.execute(
                        "UPDATE track_passes SET status = ?1, log = ?2, duration_ms = ?3,
                         last_run_at = CURRENT_TIMESTAMP WHERE id = ?4",
                        rusqlite::params![pass_status::FAILED, e, elapsed_ms, job.pass_id],
                    );
                    let _ = app.emit("analysis-progress", serde_json::json!({
                        "track_id": job.track_id,
                        "pass_name": "clap",
                        "status": pass_status::FAILED,
                    }));
                }
            }
        }

        let _ = app.emit("analysis-complete", ());
    });

    Ok(())
}

#[tauri::command]
fn is_analysis_running() -> bool {
    ANALYSIS_ACTIVE.load(Ordering::SeqCst)
}

#[tauri::command]
fn get_pass_stats(
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<Vec<PassStats>, String> {
    let conn = conn_state.lock().map_err(|e| e.to_string())?;

    if !ANALYSIS_ACTIVE.load(Ordering::SeqCst) {
        let _ = conn.execute(
            "UPDATE track_passes SET status = ?1, log = NULL, last_run_at = NULL
             WHERE status = ?2",
            rusqlite::params![pass_status::PENDING, pass_status::IN_PROGRESS],
        );
    }

    let mut counts_stmt = conn
        .prepare(
            "SELECT pass_name,
                    SUM(CASE WHEN status = 0 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN status = 1 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN status = 2 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN status = 3 THEN 1 ELSE 0 END),
                    COUNT(*),
                    AVG(CASE WHEN status = 2 THEN duration_ms ELSE NULL END)
             FROM track_passes
             GROUP BY pass_name
             ORDER BY pass_name",
        )
        .map_err(|e| e.to_string())?;

    let count_rows: Vec<(String, i64, i64, i64, i64, i64, Option<f64>)> = counts_stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let mut errors_stmt = conn
        .prepare(
            "SELECT tp.pass_name, t.path, tp.log, tp.last_run_at, tp.duration_ms
             FROM track_passes tp
             JOIN tracks t ON t.id = tp.track_id
             WHERE tp.status = 3
             ORDER BY tp.pass_name, tp.last_run_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let error_rows: Vec<(String, String, Option<String>, Option<String>, Option<i64>)> = errors_stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let stats = count_rows
        .into_iter()
        .map(|(pass_name, pending, in_progress, done, failed, total, avg_duration_ms)| {
            let errors = error_rows
                .iter()
                .filter(|(p, _, _, _, _)| p == &pass_name)
                .map(|(_, path, log, last_run_at, duration_ms)| PassError {
                    path: path.clone(),
                    log: log.clone(),
                    duration_ms: *duration_ms,
                    last_run_at: last_run_at.clone(),
                })
                .collect();
            PassStats { pass_name, pending, in_progress, done, failed, total, avg_duration_ms, errors }
        })
        .collect();

    Ok(stats)
}

#[tauri::command]
fn reset_pass(
    pass_name: String,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<(), String> {
    let conn = conn_state.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE track_passes SET status = ?1, log = NULL, result = NULL,
         last_run_at = NULL, duration_ms = NULL WHERE pass_name = ?2",
        rusqlite::params![pass_status::PENDING, &pass_name],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn reset_all_passes(
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<(), String> {
    let conn = conn_state.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE track_passes SET status = ?1, log = NULL, result = NULL,
         last_run_at = NULL, duration_ms = NULL",
        [pass_status::PENDING],
    ).map_err(|e| e.to_string())?;
    Ok(())
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
            save_sidecar,
            export_sidecars,
            scanner::scan_all_libraries,
            run_analysis_pipeline,
            is_analysis_running,
            get_pass_stats,
            reset_pass,
            reset_all_passes,
            get_projection_coordinates,
            search_similar_tracks_audio,
            recompute_projection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ── Music Map ─────────────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
struct MappedTrackPoint {
    id: i64,
    x: f64,
    y: f64,
    watched_directory_id: i64,
    title: Option<String>,
    filename: String,
    artist: Option<String>,
    genre: Option<String>,
    bpm: Option<f64>,
    key: Option<String>,
    scale: Option<String>,
}

#[derive(serde::Serialize)]
struct AudioSimilarityResult {
    id: i64,
    distance: f64,
    title: Option<String>,
    artist: Option<String>,
    bpm: Option<f64>,
    key: Option<String>,
    scale: Option<String>,
}

fn bytes_to_floats(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

fn l2_normalize(vec: &[f32]) -> Vec<f32> {
    let norm = vec.iter().map(|&x| x * x).sum::<f32>().sqrt();
    if norm == 0.0 { vec.to_vec() } else { vec.iter().map(|&x| x / norm).collect() }
}

fn standardize_to_100(coords: &[(f64, f64)]) -> Vec<(f64, f64)> {
    if coords.is_empty() { return Vec::new(); }
    let x_min = coords.iter().map(|p| p.0).fold(f64::MAX, f64::min);
    let x_max = coords.iter().map(|p| p.0).fold(f64::MIN, f64::max);
    let y_min = coords.iter().map(|p| p.1).fold(f64::MAX, f64::min);
    let y_max = coords.iter().map(|p| p.1).fold(f64::MIN, f64::max);
    let xs = if x_max == x_min { 1.0 } else { x_max - x_min };
    let ys = if y_max == y_min { 1.0 } else { y_max - y_min };
    coords.iter().map(|&(x, y)| (
        (x - x_min) / xs * 100.0,
        (y - y_min) / ys * 100.0,
    )).collect()
}

/// Returns the stored 2D UMAP coordinates joined with basic track metadata.
#[tauri::command]
fn get_projection_coordinates(
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<Vec<MappedTrackPoint>, String> {
    let conn = conn_state.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT tc.track_id, tc.x, tc.y,
                    t.watched_directory_id, t.title, t.filename, t.artist,
                    t.genre, t.bpm, t.key, t.scale
             FROM track_coords tc
             JOIN tracks t ON t.id = tc.track_id",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| Ok(MappedTrackPoint {
            id:                   row.get(0)?,
            x:                    row.get(1)?,
            y:                    row.get(2)?,
            watched_directory_id: row.get(3)?,
            title:                row.get(4)?,
            filename:             row.get(5)?,
            artist:               row.get(6)?,
            genre:                row.get(7)?,
            bpm:                  row.get(8)?,
            key:                  row.get(9)?,
            scale:                row.get(10)?,
        }))
        .map_err(|e| e.to_string())?;
    rows.map(|r| r.map_err(|e| e.to_string())).collect()
}

/// KNN similarity search: given a seed track_id, returns the N nearest tracks
/// by L2 distance in the CLAP audio embedding space.
#[tauri::command]
fn search_similar_tracks_audio(
    track_id: i64,
    directory_id: Option<i64>,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<Vec<AudioSimilarityResult>, String> {
    let conn = conn_state.lock().map_err(|e| e.to_string())?;

    let blob: Vec<u8> = conn
        .query_row(
            "SELECT embedding FROM audio_embeddings WHERE track_id = ?1",
            [track_id],
            |row| row.get(0),
        )
        .map_err(|_| "Track has no CLAP embedding yet — run analysis first.".to_string())?;

    // Build valid track ID set, optionally scoped to a directory
    let valid_ids: std::collections::HashSet<i64> = if let Some(dir_id) = directory_id {
        let mut s = conn.prepare("SELECT id FROM tracks WHERE watched_directory_id = ?1")
            .map_err(|e| e.to_string())?;
        let rows = s.query_map([dir_id], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        rows.filter_map(|r| r.ok()).collect()
    } else {
        let mut s = conn.prepare("SELECT id FROM tracks").map_err(|e| e.to_string())?;
        let rows = s.query_map([], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        rows.filter_map(|r| r.ok()).collect()
    };

    let k = if directory_id.is_some() { 500i64 } else { (valid_ids.len() + 1) as i64 };
    let knn_sql = format!(
        "SELECT ae.track_id, ae.distance, t.title, t.artist, t.bpm, t.key, t.scale
         FROM audio_embeddings ae
         JOIN tracks t ON t.id = ae.track_id
         WHERE ae.embedding MATCH ?1 AND k = {}
         ORDER BY ae.distance ASC",
        k
    );
    let mut stmt = conn.prepare(&knn_sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![blob], |row| Ok(AudioSimilarityResult {
            id:       row.get(0)?,
            distance: row.get(1)?,
            title:    row.get(2)?,
            artist:   row.get(3)?,
            bpm:      row.get(4)?,
            key:      row.get(5)?,
            scale:    row.get(6)?,
        }))
        .map_err(|e| e.to_string())?;

    let mut list: Vec<AudioSimilarityResult> = rows
        .filter_map(|r| r.ok())
        .filter(|r| r.id != track_id && valid_ids.contains(&r.id))
        .collect();
    list.truncate(20);
    Ok(list)
}

/// Runs UMAP on all CLAP audio embeddings and persists the 2D coordinates in
/// `track_coords`. Emits `projection-updated` when done.
#[tauri::command]
async fn recompute_projection(
    _algorithm: String,
    _n_neighbors: i32,
    _min_dist: f64,
    _perplexity: f64,
    app: tauri::AppHandle,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<usize, String> {
    // Collect all CLAP embeddings
    let (track_ids, blended_vectors) = {
        let conn = conn_state.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT ae.track_id, ae.embedding FROM audio_embeddings ae")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let blob: Vec<u8> = row.get(1)?;
                Ok((id, blob))
            })
            .map_err(|e| e.to_string())?;
        let mut ids = Vec::new();
        let mut vecs = Vec::new();
        for row in rows.filter_map(|r| r.ok()) {
            ids.push(row.0);
            vecs.push(l2_normalize(&bytes_to_floats(&row.1)));
        }
        (ids, vecs)
    };

    if blended_vectors.is_empty() {
        return Err("No tracks with CLAP embeddings found. Run the analysis pipeline first.".to_string());
    }

    let n = blended_vectors.len();
    let coords: Vec<(f64, f64)> = if n < 4 {
        // Too few points for UMAP — spread evenly on a horizontal line
        (0..n).map(|i| {
            let x = if n > 1 { i as f64 / (n - 1) as f64 * 100.0 } else { 50.0 };
            (x, 50.0)
        }).collect()
    } else {
        let raw = rag_umap::convert_to_2d(blended_vectors)
            .map_err(|e| format!("UMAP projection failed: {:?}", e))?;
        standardize_to_100(
            &raw.iter().map(|v| (v[0] as f64, v[1] as f64)).collect::<Vec<_>>(),
        )
    };

    // Persist inside a transaction
    {
        let mut conn = conn_state.lock().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM track_coords", []).map_err(|e| e.to_string())?;
        {
            let mut ins = tx.prepare(
                "INSERT INTO track_coords (track_id, x, y) VALUES (?1, ?2, ?3)",
            ).map_err(|e| e.to_string())?;
            for (i, &(x, y)) in coords.iter().enumerate() {
                ins.execute(rusqlite::params![track_ids[i], x, y])
                    .map_err(|e| e.to_string())?;
            }
        }
        tx.commit().map_err(|e| e.to_string())?;
    }

    let _ = app.emit("projection-updated", ());
    Ok(coords.len())
}
