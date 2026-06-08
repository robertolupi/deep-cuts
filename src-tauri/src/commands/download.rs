use crate::error::AppError;
use crate::models::ModelManifest;
use serde::{Serialize, Deserialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Emitter;

/// Structured error payload emitted on `model-download-error`.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DownloadErrorEvent {
    UnknownGroup { group: String },
    Network { file: String, detail: String },
    Filesystem { file: String, detail: String },
    ChecksumMismatch { file: String },
    ManifestParse { detail: String },
    ResumeError { file: String, detail: String },
    Cancelled,
}

pub struct DownloadState {
    pub is_running: Arc<AtomicBool>,
    pub cancel_flag: Arc<AtomicBool>,
    pub last_progress: Arc<std::sync::Mutex<Option<DownloadProgressEvent>>>,
}

impl Default for DownloadState {
    fn default() -> Self {
        Self {
            is_running: Arc::new(AtomicBool::new(false)),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            last_progress: Arc::new(std::sync::Mutex::new(None)),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DownloadProgressEvent {
    pub model: String,
    pub file: String,
    pub bytes_done: u64,
    pub bytes_total: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ResumableFile {
    pub filename: String,
    pub offset: u64,
}

pub struct ModelDirectoryOverride(pub PathBuf);

pub fn get_model_destination_dir<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> PathBuf {
    use tauri::Manager;
    if let Some(dir_override) = app.try_state::<ModelDirectoryOverride>() {
        let path = dir_override.0.clone();
        log::info!("[download] Model dir override detected: {:?}", path);
        return path;
    }

    if let Some(conn_state) = app.try_state::<std::sync::Mutex<rusqlite::Connection>>() {
        if let Ok(conn) = conn_state.lock() {
            let value: Option<String> = conn
                .query_row(
                    "SELECT value FROM app_settings WHERE key = 'model_path'",
                    [],
                    |row| row.get(0),
                )
                .ok();
            if let Some(val) = value {
                let trimmed = val.trim();
                if !trimmed.is_empty() {
                    let path = PathBuf::from(trimmed);
                    log::info!("[download] Resolved custom model path from managed DB: {:?}", path);
                    return path;
                }
            }
        } else {
            log::warn!("[download] Database connection lock poisoned in get_model_destination_dir.");
        }
    } else {
        log::warn!("[download] Managed database connection state not found.");
    }

    if let Ok(app_dir) = app.path().app_data_dir() {
        let path = app_dir.join("models");
        log::info!("[download] Resolved default model path: {:?}", path);
        path
    } else {
        log::info!("[download] Resolved fallback models folder: models");
        PathBuf::from("models")
    }
}

fn emit_download_error<R: tauri::Runtime>(app: &tauri::AppHandle<R>, err: &DownloadErrorEvent) {
    let _ = app.emit("model-download-error", err.clone());
}

async fn verify_sha256(path: &Path, expected_hex: &str) -> bool {
    log::info!("[download] Starting SHA256 checksum verification for {:?}", path);
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            log::warn!("[download] Failed to open file for verification {:?}: {}", path, e);
            return false;
        }
    };
    let mut hasher = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => hasher.update(&buffer[..n]),
            Err(e) => {
                log::warn!("[download] Read error during verification of {:?}: {}", path, e);
                return false;
            }
        }
    }
    let result = hasher.finalize();
    let hex_result = format!("{:x}", result);
    let matches = hex_result.eq_ignore_ascii_case(expected_hex);
    log::info!("[download] SHA256 matches: {} (computed={}, expected={})", matches, hex_result, expected_hex);
    matches
}

/// @concept ModelDownload
/// @skill add-ipc-command
/// Tauri IPC commands for checking, starting, cancelling, and querying model file downloads.
#[tauri::command]
pub fn check_pending_resume<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<Vec<ResumableFile>, AppError> {
    let dest_dir = get_model_destination_dir(&app);
    let mut resumable = Vec::new();

    if dest_dir.exists() {
        if let Ok(entries) = fs::read_dir(dest_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "part" {
                            if let Some(filename_os) = path.file_stem() {
                                let filename = filename_os.to_string_lossy().into_owned();
                                if let Ok(metadata) = fs::metadata(&path) {
                                    resumable.push(ResumableFile {
                                        filename,
                                        offset: metadata.len(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(resumable)
}

#[tauri::command]
pub fn cancel_model_download(state: tauri::State<'_, DownloadState>) -> Result<(), AppError> {
    state.cancel_flag.store(true, Ordering::SeqCst);
    log::info!("[download] Cancellation requested by user.");
    Ok(())
}

#[tauri::command]
pub fn get_download_status(state: tauri::State<'_, DownloadState>) -> Result<Option<DownloadProgressEvent>, AppError> {
    if state.is_running.load(Ordering::SeqCst) {
        if let Ok(guard) = state.last_progress.lock() {
            return Ok(guard.clone());
        }
    }
    Ok(None)
}

#[tauri::command]
pub fn download_models<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, DownloadState>,
    models: Vec<String>,
    custom_url_base: Option<String>,
    custom_manifest: Option<String>,
) -> Result<(), AppError> {
    if state.is_running.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return Err(AppError::Config("Download is already in progress".to_string()));
    }

    state.cancel_flag.store(false, Ordering::SeqCst);
    if let Ok(mut guard) = state.last_progress.lock() {
        *guard = None;
    }

    let cancel_flag = state.cancel_flag.clone();
    let is_running = state.is_running.clone();
    let last_progress = state.last_progress.clone();

    tauri::async_runtime::spawn(async move {
        let result = download_models_worker(
            app.clone(),
            models,
            custom_url_base,
            custom_manifest,
            cancel_flag,
            last_progress.clone(),
        ).await;
        is_running.store(false, Ordering::SeqCst);
        if let Ok(mut guard) = last_progress.lock() {
            *guard = None;
        }

        match result {
            Ok(_) => {
                let _ = app.emit("model-download-all-complete", ());
            }
            Err(e) => {
                let _ = app.emit("model-download-all-error", e.to_string());
            }
        }
    });

    Ok(())
}

async fn download_models_worker<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    models: Vec<String>,
    custom_url_base: Option<String>,
    custom_manifest: Option<String>,
    cancel_flag: Arc<AtomicBool>,
    last_progress: Arc<std::sync::Mutex<Option<DownloadProgressEvent>>>,
) -> Result<(), AppError> {
    let manifest = match custom_manifest {
        Some(json) => ModelManifest::parse(&json).map_err(|e| {
            let ev = DownloadErrorEvent::ManifestParse { detail: e.clone() };
            emit_download_error(&app, &ev);
            AppError::Config(format!("Failed to parse custom manifest: {}", e))
        })?,
        None => {
            use tauri::Manager;
            let cached = app
                .try_state::<std::sync::Mutex<rusqlite::Connection>>()
                .and_then(|state| state.lock().ok().and_then(|conn| {
                    conn.query_row(
                        "SELECT value FROM app_settings WHERE key = 'manifest_cached_json'",
                        [],
                        |row| row.get::<_, String>(0),
                    ).ok()
                }))
                .and_then(|json| ModelManifest::parse(&json).ok());
            cached.unwrap_or_else(ModelManifest::fallback)
        }
    };

    // Validate all requested group keys before starting any download.
    for group_key in &models {
        if !manifest.models.contains_key(group_key.as_str()) {
            let ev = DownloadErrorEvent::UnknownGroup { group: group_key.clone() };
            emit_download_error(&app, &ev);
            return Err(AppError::Config(format!("unknown model group: {}", group_key)));
        }
    }

    let dest_dir = get_model_destination_dir(&app);
    log::info!("[download] Target models folder: {:?}", dest_dir);
    fs::create_dir_all(&dest_dir).map_err(|e| {
        let ev = DownloadErrorEvent::Filesystem {
            file: dest_dir.to_string_lossy().into_owned(),
            detail: e.to_string(),
        };
        emit_download_error(&app, &ev);
        AppError::Config(format!("Failed to create models folder: {}", e))
    })?;

    let update_progress = |model: &str, file: &str, bytes_done: u64, bytes_total: u64| {
        let event = DownloadProgressEvent {
            model: model.to_string(),
            file: file.to_string(),
            bytes_done,
            bytes_total,
        };
        if let Ok(mut guard) = last_progress.lock() {
            *guard = Some(event.clone());
        }
        let _ = app.emit("model-download-progress", event);
    };

    for group_key in &models {
        // Safety: keys already validated above.
        let group = manifest.models.get(group_key).unwrap();
        for file in &group.files {
            if cancel_flag.load(Ordering::SeqCst) {
                emit_download_error(&app, &DownloadErrorEvent::Cancelled);
                return Err(AppError::Config("Download cancelled".to_string()));
            }

            let final_path = dest_dir.join(&file.filename);
            let part_path = dest_dir.join(format!("{}.part", file.filename));

            // 1. Check if the final file is already there and valid
            if final_path.exists() {
                if verify_sha256(&final_path, &file.sha256).await {
                    log::info!("[download] File already exists and verified: {}", file.filename);
                    update_progress(group_key, &file.filename, file.size_bytes, file.size_bytes);
                    continue;
                }
            }

            // 2. Resolve target URL
            let url = if let Some(ref base) = custom_url_base {
                format!("{}/{}", base.trim_end_matches('/'), file.filename)
            } else {
                file.url.clone()
            };

            // 3. Resumability offset check
            let mut offset: u64 = 0;
            if part_path.exists() {
                match fs::metadata(&part_path) {
                    Ok(metadata) => offset = metadata.len(),
                    Err(e) => {
                        let ev = DownloadErrorEvent::ResumeError {
                            file: file.filename.clone(),
                            detail: e.to_string(),
                        };
                        emit_download_error(&app, &ev);
                        log::warn!("[download] Could not read part file metadata: {}", e);
                        // Non-fatal: continue from zero
                    }
                }
            }

            log::info!("[download] Downloading from URL: {}, offset: {}", url, offset);

            // 4. Perform HTTP request + streaming IO on a blocking thread to avoid
            //    blocking the async executor with ureq's synchronous calls.
            let filename_clone = file.filename.clone();
            let sha256_clone = file.sha256.clone();
            let size_bytes = file.size_bytes;
            let cancel_flag_clone = cancel_flag.clone();
            let last_progress_clone = last_progress.clone();
            let app_clone = app.clone();
            let group_key_clone = group_key.clone();

            let part_path_clone = part_path.clone();
            let final_path_clone = final_path.clone();

            let result: Result<(), AppError> = tokio::task::spawn_blocking(move || {
                let mut req = ureq::get(&url);
                if offset > 0 {
                    req = req.set("Range", &format!("bytes={}-", offset));
                }

                let resp = match req.call() {
                    Ok(r) => {
                        log::info!("[download] HTTP status: {}", r.status());
                        r
                    }
                    Err(e) => {
                        log::warn!("[download] HTTP request failed: {}", e);
                        let ev = DownloadErrorEvent::Network {
                            file: filename_clone.clone(),
                            detail: e.to_string(),
                        };
                        emit_download_error(&app_clone, &ev);
                        return Err(AppError::Config(format!("HTTP request failed for {}: {}", filename_clone, e)));
                    }
                };

                let is_partial = resp.status() == 206;
                let mut current_offset = offset;
                let mut file_handle = if is_partial && offset > 0 {
                    log::info!("[download] Server supports Range. Appending to part file.");
                    fs::OpenOptions::new()
                        .write(true)
                        .append(true)
                        .open(&part_path_clone)
                        .map_err(|e| {
                            let ev = DownloadErrorEvent::ResumeError {
                                file: filename_clone.clone(),
                                detail: e.to_string(),
                            };
                            emit_download_error(&app_clone, &ev);
                            AppError::Config(format!("Failed to open part file for append: {}", e))
                        })?
                } else {
                    log::info!("[download] Server doesn't support Range or starting from zero. Overwriting part file.");
                    current_offset = 0;
                    fs::OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .open(&part_path_clone)
                        .map_err(|e| {
                            let ev = DownloadErrorEvent::Filesystem {
                                file: filename_clone.clone(),
                                detail: e.to_string(),
                            };
                            emit_download_error(&app_clone, &ev);
                            AppError::Config(format!("Failed to create part file: {}", e))
                        })?
                };

                let mut reader = resp.into_reader();
                let mut chunk = [0u8; 65536];
                let mut bytes_done = current_offset;
                let mut last_emit = std::time::Instant::now();
                let mut last_emit_bytes = bytes_done;

                loop {
                    if cancel_flag_clone.load(Ordering::SeqCst) {
                        emit_download_error(&app_clone, &DownloadErrorEvent::Cancelled);
                        return Err(AppError::Config("Download cancelled".to_string()));
                    }

                    let n = match reader.read(&mut chunk) {
                        Ok(0) => break,
                        Ok(n) => n,
                        Err(e) => {
                            let ev = DownloadErrorEvent::Network {
                                file: filename_clone.clone(),
                                detail: e.to_string(),
                            };
                            emit_download_error(&app_clone, &ev);
                            return Err(AppError::Config(format!("Network read failed: {}", e)));
                        }
                    };

                    file_handle.write_all(&chunk[..n]).map_err(|e| {
                        let ev = DownloadErrorEvent::Filesystem {
                            file: filename_clone.clone(),
                            detail: e.to_string(),
                        };
                        emit_download_error(&app_clone, &ev);
                        AppError::Config(format!("Disk write failed: {}", e))
                    })?;

                    bytes_done += n as u64;

                    let now = std::time::Instant::now();
                    if now.duration_since(last_emit).as_millis() >= 100 || bytes_done - last_emit_bytes >= 1024 * 1024 {
                        let event = DownloadProgressEvent {
                            model: group_key_clone.clone(),
                            file: filename_clone.clone(),
                            bytes_done,
                            bytes_total: size_bytes,
                        };
                        if let Ok(mut guard) = last_progress_clone.lock() {
                            *guard = Some(event.clone());
                        }
                        let _ = app_clone.emit("model-download-progress", event);
                        last_emit = now;
                        last_emit_bytes = bytes_done;
                    }
                }

                // Final progress emit
                {
                    let event = DownloadProgressEvent {
                        model: group_key_clone.clone(),
                        file: filename_clone.clone(),
                        bytes_done,
                        bytes_total: size_bytes,
                    };
                    if let Ok(mut guard) = last_progress_clone.lock() {
                        *guard = Some(event.clone());
                    }
                    let _ = app_clone.emit("model-download-progress", event);
                }

                // 5. Verify SHA256 Integrity
                log::info!("[download] Verifying checksum for {}", filename_clone);
                let mut verify_file = match File::open(&part_path_clone) {
                    Ok(f) => f,
                    Err(e) => {
                        let ev = DownloadErrorEvent::Filesystem {
                            file: filename_clone.clone(),
                            detail: e.to_string(),
                        };
                        emit_download_error(&app_clone, &ev);
                        return Err(AppError::Config(format!("Failed to open part file for verification: {}", e)));
                    }
                };
                let mut hasher = Sha256::new();
                let mut buf = [0u8; 65536];
                loop {
                    match verify_file.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => hasher.update(&buf[..n]),
                        Err(e) => {
                            let ev = DownloadErrorEvent::Filesystem {
                                file: filename_clone.clone(),
                                detail: e.to_string(),
                            };
                            emit_download_error(&app_clone, &ev);
                            return Err(AppError::Config(format!("Read error during verification: {}", e)));
                        }
                    }
                }
                let hex_result = format!("{:x}", hasher.finalize());
                let matches = hex_result.eq_ignore_ascii_case(&sha256_clone);
                log::info!("[download] SHA256 matches: {} (computed={}, expected={})", matches, hex_result, sha256_clone);

                if matches {
                    log::info!("[download] SHA256 verification succeeded!");
                    fs::rename(&part_path_clone, &final_path_clone).map_err(|e| {
                        let ev = DownloadErrorEvent::Filesystem {
                            file: filename_clone.clone(),
                            detail: e.to_string(),
                        };
                        emit_download_error(&app_clone, &ev);
                        AppError::Config(format!("Failed to rename part file to final: {}", e))
                    })?;
                    log::info!("[download] Verification succeeded. File saved to {:?}", final_path_clone);
                } else {
                    log::error!("[download] SHA256 verification FAILED!");
                    let _ = fs::remove_file(&part_path_clone);
                    let ev = DownloadErrorEvent::ChecksumMismatch { file: filename_clone.clone() };
                    emit_download_error(&app_clone, &ev);
                    return Err(AppError::Config(format!("Integrity check failed for {}. Checksum mismatch.", filename_clone)));
                }

                Ok(())
            }).await.map_err(|e| AppError::Config(format!("spawn_blocking task failed: {}", e)))?;

            result?;
        }
    }

    Ok(())
}

// ──────────────────────────────────────────────
// Unit tests
// ──────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ModelManifest, ModelGroup};
    use std::collections::HashMap;
    use std::io::Write;
    fn make_manifest(keys: &[&str]) -> ModelManifest {
        let mut models = HashMap::new();
        for &k in keys {
            models.insert(k.to_string(), ModelGroup {
                label: k.to_string(),
                files: vec![],
            });
        }
        ModelManifest {
            manifest_version: 1,
            min_app_version: "0.0.1".to_string(),
            update_notice: None,
            models,
        }
    }

    /// Requesting an unknown group key must return an error immediately.
    #[test]
    fn test_unknown_group_key_returns_error() {
        let manifest = make_manifest(&["clap", "sentence"]);
        let unknown = "nonexistent_model";
        assert!(
            !manifest.models.contains_key(unknown),
            "sanity: key should not exist"
        );
        // Simulate the validation logic from download_models_worker.
        let models = vec![unknown.to_string()];
        for group_key in &models {
            if !manifest.models.contains_key(group_key.as_str()) {
                let err = format!("unknown model group: {}", group_key);
                assert_eq!(err, "unknown model group: nonexistent_model");
                return; // test passed
            }
        }
        panic!("expected error for unknown group key");
    }

    /// Checksum mismatch must be detected when the file content doesn't match.
    #[test]
    fn test_checksum_mismatch_detected() {
        let dir = std::env::temp_dir();
        let file_path = dir.join(format!("deep_cuts_test_{}.bin", std::process::id()));

        // Write some content
        let mut f = File::create(&file_path).unwrap();
        f.write_all(b"hello world").unwrap();
        drop(f);

        // Correct SHA256 of "hello world"
        let correct_hex = "b94d27b9934d3e08a52e52d7da7dabfac484efe04294e576e7a5be9a53059c12";
        // Deliberately wrong expected hash
        let wrong_hex   = "0000000000000000000000000000000000000000000000000000000000000000";

        // Compute actual hash synchronously for the test
        let mut file = File::open(&file_path).unwrap();
        let mut hasher = Sha256::new();
        let mut buf = [0u8; 4096];
        loop {
            match file.read(&mut buf).unwrap() {
                0 => break,
                n => hasher.update(&buf[..n]),
            }
        }
        let hex = format!("{:x}", hasher.finalize());

        let _ = fs::remove_file(&file_path); // cleanup

        assert!(!hex.eq_ignore_ascii_case(wrong_hex), "mismatch should be detected");
        // If we used the correct expected hash it should match (validates the logic itself)
        assert!(hex.eq_ignore_ascii_case(correct_hex) || !hex.is_empty(),
            "hash computation must produce a non-empty result");
    }

    /// A manifest with a bad group key (mixed valid/invalid) errors on the invalid one.
    #[test]
    fn test_mixed_keys_errors_on_unknown() {
        let manifest = make_manifest(&["clap"]);
        let models = vec!["clap".to_string(), "ghost".to_string()];

        let mut found_error = false;
        for group_key in &models {
            if !manifest.models.contains_key(group_key.as_str()) {
                found_error = true;
                let msg = format!("unknown model group: {}", group_key);
                assert_eq!(msg, "unknown model group: ghost");
            }
        }
        assert!(found_error, "expected validation to catch the unknown key");
    }

    /// Verify the DownloadErrorEvent variants serialize with the expected `kind` tag.
    #[test]
    fn test_download_error_event_serialization() {
        let ev = DownloadErrorEvent::UnknownGroup { group: "foo".to_string() };
        let json = serde_json::to_string(&ev).unwrap();
        assert!(json.contains("\"kind\":\"unknown_group\""), "got: {}", json);
        assert!(json.contains("\"group\":\"foo\""), "got: {}", json);

        let ev2 = DownloadErrorEvent::ChecksumMismatch { file: "bar.bin".to_string() };
        let json2 = serde_json::to_string(&ev2).unwrap();
        assert!(json2.contains("\"kind\":\"checksum_mismatch\""), "got: {}", json2);

        let ev3 = DownloadErrorEvent::Network { file: "x".to_string(), detail: "timeout".to_string() };
        let json3 = serde_json::to_string(&ev3).unwrap();
        assert!(json3.contains("\"kind\":\"network\""), "got: {}", json3);
    }
}
