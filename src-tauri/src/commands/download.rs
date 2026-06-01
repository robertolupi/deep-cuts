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

pub struct DownloadState {
    pub is_running: Arc<AtomicBool>,
    pub cancel_flag: Arc<AtomicBool>,
}

impl Default for DownloadState {
    fn default() -> Self {
        Self {
            is_running: Arc::new(AtomicBool::new(false)),
            cancel_flag: Arc::new(AtomicBool::new(false)),
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

    let cancel_flag = state.cancel_flag.clone();
    let is_running = state.is_running.clone();

    tauri::async_runtime::spawn(async move {
        let result = download_models_worker(app.clone(), models, custom_url_base, custom_manifest, cancel_flag).await;
        is_running.store(false, Ordering::SeqCst);

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
) -> Result<(), AppError> {
    let manifest = match custom_manifest {
        Some(json) => ModelManifest::parse(&json)
            .map_err(|e| AppError::Config(format!("Failed to parse custom manifest: {}", e)))?,
        None => ModelManifest::fallback(),
    };
    let dest_dir = get_model_destination_dir(&app);
    log::info!("[download] Target models folder: {:?}", dest_dir);
    fs::create_dir_all(&dest_dir).map_err(|e| AppError::Config(format!("Failed to create models folder: {}", e)))?;

    for group_key in &models {
        if let Some(group) = manifest.models.get(group_key) {
            for file in &group.files {
                if cancel_flag.load(Ordering::SeqCst) {
                    return Err(AppError::Config("Download cancelled".to_string()));
                }

                let final_path = dest_dir.join(&file.filename);
                let part_path = dest_dir.join(format!("{}.part", file.filename));

                // 1. Check if the final file is already there and valid
                if final_path.exists() {
                    if verify_sha256(&final_path, &file.sha256).await {
                        log::info!("[download] File already exists and verified: {}", file.filename);
                        let _ = app.emit("model-download-progress", DownloadProgressEvent {
                            model: group_key.clone(),
                            file: file.filename.clone(),
                            bytes_done: file.size_bytes,
                            bytes_total: file.size_bytes,
                        });
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
                let mut offset = 0;
                if part_path.exists() {
                    if let Ok(metadata) = fs::metadata(&part_path) {
                        offset = metadata.len();
                    }
                }

                log::info!("[download] Downloading from URL: {}, offset: {}", url, offset);

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
                        return Err(AppError::Config(format!("HTTP request failed for {}: {}", file.filename, e)));
                    }
                };

                let is_partial = resp.status() == 206;
                let mut file_handle = if is_partial && offset > 0 {
                    log::info!("[download] Server supports Range. Appending to part file.");
                    fs::OpenOptions::new()
                        .write(true)
                        .append(true)
                        .open(&part_path)
                        .map_err(|e| AppError::Config(format!("Failed to open part file for append: {}", e)))?
                } else {
                    log::info!("[download] Server doesn't support Range or start from zero. Overwriting part file.");
                    offset = 0;
                    fs::OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .open(&part_path)
                        .map_err(|e| AppError::Config(format!("Failed to create part file: {}", e)))?
                };

                let mut reader = resp.into_reader();
                let mut chunk = [0; 65536];
                let mut bytes_done = offset;
                let mut last_emit = std::time::Instant::now();
                let mut last_emit_bytes = bytes_done;

                loop {
                    if cancel_flag.load(Ordering::SeqCst) {
                        return Err(AppError::Config("Download cancelled".to_string()));
                    }

                    let n = match reader.read(&mut chunk) {
                        Ok(0) => break,
                        Ok(n) => n,
                        Err(e) => return Err(AppError::Config(format!("Network read failed: {}", e))),
                    };

                    file_handle.write_all(&chunk[..n])
                        .map_err(|e| AppError::Config(format!("Disk write failed: {}", e)))?;

                    bytes_done += n as u64;

                    let now = std::time::Instant::now();
                    if now.duration_since(last_emit).as_millis() >= 100 || bytes_done - last_emit_bytes >= 1024 * 1024 {
                        let _ = app.emit("model-download-progress", DownloadProgressEvent {
                            model: group_key.clone(),
                            file: file.filename.clone(),
                            bytes_done,
                            bytes_total: file.size_bytes,
                        });
                        last_emit = now;
                        last_emit_bytes = bytes_done;
                    }
                }

                // Final progress emit
                let _ = app.emit("model-download-progress", DownloadProgressEvent {
                    model: group_key.clone(),
                    file: file.filename.clone(),
                    bytes_done,
                    bytes_total: file.size_bytes,
                });

                // 4. Verify SHA256 Integrity
                log::info!("[download] Verifying checksum for {}", file.filename);
                if verify_sha256(&part_path, &file.sha256).await {
                    log::info!("[download] SHA256 verification succeeded!");
                    fs::rename(&part_path, &final_path)
                        .map_err(|e| AppError::Config(format!("Failed to rename part file to final: {}", e)))?;
                    log::info!("[download] Verification succeeded. File saved to {:?}", final_path);
                } else {
                    log::error!("[download] SHA256 verification FAILED!");
                    let _ = fs::remove_file(&part_path);
                    return Err(AppError::Config(format!("Integrity check failed for {}. Checksum mismatch.", file.filename)));
                }
            }
        }
    }

    Ok(())
}
