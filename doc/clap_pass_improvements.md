# Implementation Plan: CLAP Audio Embedding Optimization & Parallelization
### Standalone Technical Specification for Apple Silicon & General Hardware Tuning

This document is a standalone technical specification for optimizing and parallelizing the `clap` embedding pass in the `deep-cuts` desktop application. Any developer or AI agent should be able to implement this plan flawlessly using only the instructions, file paths, and code blueprints provided below.

---

## 1. Objective & Performance Baseline

* **Current Baseline**: 1,000 songs take **~15 minutes** to index (~900ms per track).
* **Target Objective**: Reduce indexing time to **~90 seconds on CPU** (~90ms per track), or **~15 seconds on CoreML NPU** (~15ms per track) for a Mac Studio M3 Ultra, scaling proportionally across all Apple Silicon.
* **The Root Bottlenecks**:
  1. **Seek-by-Decoding**: Extracting a 10-second center window currently requires `symphonia` to decode sequentially from the start of the file to the midpoint, wasting 300–600ms per song.
  2. **Mutex Contention**: The ONNX session is locked behind a `Mutex`, preventing concurrent execution.
  3. **Resource Starvation**: ONNX Runtime is restricted to 1 CPU thread, leaving multi-core systems (especially Apple Silicon P-Cores) highly underutilized.

---

## 2. File Modularity & Architecture Overview

To implement the optimization, we will create one new module and modify two existing modules in the Rust backend (`src-tauri/src/`):

1. **[NEW]** `src-tauri/src/hardware.rs`
   * Handles macOS `sysctl` discovery to detect P-Cores, E-Cores, and ARM64 architecture natively at runtime.
   * Defines the `PipelineConfig` auto-tuning matrix.
2. **[MODIFY]** `src-tauri/src/embeddings.rs`
   * Refactors the monolithic `run_clap_audio_embed` into separate, decoupled stages: Preprocessing (CPU-bound decoding/resampling) vs. Inference (model execution).
   * Replaces the static `Mutex<Session>` with a thread-safe `RwLock<Option<Session>>` to allow concurrent model execution and runtime session settings hot-swapping.
3. **[MODIFY]** `src-tauri/src/analysis.rs`
   * Integrates container-level seeking using Symphonia timestamps to skip decoding the first half of audio files.
   * Replaces the single-threaded `clap` loop in `PipelineManager::run` with a parallel **Producer-Consumer pipeline** utilizing bounded channel queues.

---

## 3. Step-by-Step Code Blueprints

### Step 1: Create `src-tauri/src/hardware.rs`

This file handles system topology discovery. Create this file and add it to `src-tauri/src/lib.rs` as `pub mod hardware;`.

```rust
use std::process::Command;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppleSiliconProfile {
    pub is_arm64: bool,
    pub p_cores: usize,
    pub e_cores: usize,
}

impl AppleSiliconProfile {
    /// Detects Apple Silicon CPU topologies natively on macOS via sysctl
    pub fn discover() -> Self {
        let is_arm64 = Self::query_sysctl_bool("hw.optional.arm64");
        
        let p_cores = Self::query_sysctl_u32("hw.perflevel0.physicalcpu")
            .map(|v| v as usize)
            .unwrap_or_else(|| {
                std::thread::available_parallelism()
                    .map(|n| (n.get() / 2).max(1))
                    .unwrap_or(4)
            });

        let e_cores = Self::query_sysctl_u32("hw.perflevel1.physicalcpu")
            .map(|v| v as usize)
            .unwrap_or(0);

        Self { is_arm64, p_cores, e_cores }
    }

    fn query_sysctl_u32(key: &str) -> Option<u32> {
        let output = Command::new("sysctl").arg("-n").arg(key).output().ok()?;
        let val_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        val_str.parse::<u32>().ok()
    }

    fn query_sysctl_bool(key: &str) -> bool {
        Self::query_sysctl_u32(key).unwrap_or(0) == 1
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PipelineConfig {
    pub use_coreml: bool,
    pub decode_threads: usize,
    pub intra_threads: usize,
}

impl PipelineConfig {
    /// Auto-tunes parallel parameters based on detected system hardware
    pub fn auto_tune() -> Self {
        let profile = AppleSiliconProfile::discover();

        if profile.is_arm64 {
            #[cfg(feature = "coreml")]
            {
                // Apple Neural Engine (ANE/NPU) acceleration configuration
                let decode_threads = match profile.p_cores {
                    16 => 8,       // Ultra
                    8..=12 => 4,   // Pro/Max
                    _ => 2,        // Base
                };
                Self {
                    use_coreml: true,
                    decode_threads,
                    intra_threads: 1, // ANE is single-pipelined
                }
            }
            #[cfg(not(feature = "coreml"))]
            {
                // Apple Silicon CPU-only configuration
                let intra_threads = (profile.p_cores / 2).max(1).min(4);
                let decode_threads = (profile.p_cores / 2).max(1);
                Self {
                    use_coreml: false,
                    decode_threads,
                    intra_threads,
                }
            }
        } else {
            // General x86 Windows/Linux fallback
            Self {
                use_coreml: false,
                decode_threads: 2,
                intra_threads: 2,
            }
        }
    }
}
```

---

### Step 2: Modify `src-tauri/src/embeddings.rs`

We split the monolithic `run_clap_audio_embed` function and replace the `Mutex` wrapper.

#### A. Session Storage Refactor (Lines 22-23 and 86-91):
Replace `Mutex` with `RwLock` to enable parallel non-blocking execution:

```rust
use std::sync::RwLock;

// 1. Session Storage Definition
static SESSION_CLAP_AUDIO: RwLock<Option<Session>> = RwLock::new(None);

// 2. Dynamic Runtime Configurer (Swaps session if settings change)
pub fn configure_session(
    use_coreml: bool,
    intra_threads: usize,
    app: Option<&tauri::AppHandle>,
) -> Result<(), String> {
    let mut session_guard = SESSION_CLAP_AUDIO.write()
        .map_err(|e| format!("Failed to acquire write lock: {}", e))?;

    let path = get_model_path("clap_audio_encoder.onnx", app);

    let mut builder = Session::builder()
        .map_err(|e| format!("ORT builder error: {}", e))?;

    builder = builder.with_intra_threads(intra_threads as i16)
        .map_err(|e| format!("Failed to set intra threads: {}", e))?;

    #[cfg(feature = "coreml")]
    if use_coreml {
        builder = builder.with_execution_providers([
            ort::execution_providers::CoreMLExecutionProvider::default()
                .with_ane_only()
                .build()
        ]).map_err(|e| format!("Failed to set CoreML EP: {}", e))?;
    }

    let session = builder.commit_from_file(&path)
        .map_err(|e| format!("Failed to load session: {}", e))?;

    *session_guard = Some(session);
    Ok(())
}
```

#### B. Function Split (Lines 264-320):
Split into `preprocess_track_to_mel` (CPU/rubato heavy) and `run_clap_inference_only` (ORT heavy):

```rust
/// pre-computes log-mel spectrogram features for a track (CPU-bound)
pub fn preprocess_track_to_mel(
    path: &str,
    app: Option<&tauri::AppHandle>,
) -> Result<Vec<f32>, String> {
    let (audio, sample_rate) = crate::dsp::decode_audio_to_mono_with_seeking(path)?;
    let audio_48k = resample_audio(&audio, sample_rate, CLAP_SR)?;

    let mid = audio_48k.len() / 2;
    let half = CLAP_10S_SAMPLES / 2;
    let start = mid.saturating_sub(half);
    let end = (start + CLAP_10S_SAMPLES).min(audio_48k.len());
    let mut window = audio_48k[start..end].to_vec();

    if window.len() < CLAP_10S_SAMPLES && !window.is_empty() {
        let original = window.clone();
        while window.len() < CLAP_10S_SAMPLES {
            let needed = CLAP_10S_SAMPLES - window.len();
            let to_add = needed.min(original.len());
            window.extend_from_slice(&original[..to_add]);
        }
    } else {
        window.resize(CLAP_10S_SAMPLES, 0.0);
    }

    let mel_filterbank = get_clap_mel_filterbank(app)?;
    compute_clap_log_mel(&window, mel_filterbank)
}

/// Runs ONNX inference using pre-computed mel features (Thread-safe)
pub fn run_clap_inference_only(
    mel_flat: Vec<f32>,
) -> Result<Vec<f32>, String> {
    let input_t = Tensor::from_array(
        ([1usize, 1, CLAP_MAX_FRAMES, CLAP_N_MELS], mel_flat),
    ).map_err(|e| e.to_string())?;

    let session_guard = SESSION_CLAP_AUDIO.read()
        .map_err(|e| format!("Failed to acquire read lock: {}", e))?;
    
    let session = session_guard.as_ref()
        .ok_or("CLAP session not configured. Call configure_session first.")?;

    let outputs = session
        .run(inputs!["input_features" => input_t])
        .map_err(|e| format!("CLAP audio inference failed: {}", e))?;

    let out = outputs
        .get("audio_embedding")
        .ok_or("CLAP audio output 'audio_embedding' missing")?;

    let (_, data) = out
        .try_extract_tensor::<f32>()
        .map_err(|e| format!("Failed to extract output: {}", e))?;

    Ok(data.iter().copied().take(512).collect())
}
```

---

### Step 3: Modify `src-tauri/src/dsp.rs`

Add a seek-aware audio decoder function `decode_audio_to_mono_with_seeking` to jump straight to the midpoint:

```rust
use symphonia::core::units::Time;
use symphonia::core::concept::SeekTo;

/// Decodes the audio file but seeks directly to the midpoint before reading frames
pub fn decode_audio_to_mono_with_seeking(path: &str) -> Result<(Vec<f32>, u32), String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mss = symphonia::default::get_media_source_stream(file);

    let mut hint = symphonia::core::probe::Hint::new();
    let extension = std::path::Path::new(path).extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    hint.with_extension(extension);

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &Default::default(), &Default::default())
        .map_err(|e| e.to_string())?;

    let mut format_reader = probed.format;
    let track = format_reader.default_track()
        .ok_or("No default track found")?;
    
    let time_base = track.codecparams.time_base
        .ok_or("Missing track time base info")?;

    // 1. Calculate duration and seek target (Midpoint)
    let n_frames = track.codecparams.n_frames
        .ok_or("Missing frame count info")?;
    let duration_seconds = (time_base.calc_time(n_frames).seconds as f64) +
        (time_base.calc_time(n_frames).frac as f64);
    
    let midpoint_seconds = duration_seconds / 2.0;

    // 2. Perform native container-level seeking
    let seek_ts = Time::from(midpoint_seconds as u64);
    if let Err(e) = format_reader.seek(
        symphonia::core::concept::SeekMode::Accurate,
        SeekTo::Time(seek_ts),
    ) {
        log::warn!("Symphonia seeking failed: {}. Falling back to sequential read.", e);
    }

    // 3. Normal decode loop for remaining sought frames ...
    // [Insert the standard symphonia frame decode loop present in decode_audio_to_mono]
}
```

---

### Step 4: Modify `src-tauri/src/analysis.rs`

Integrate the Preprocessed Producer-Consumer Pipeline into `PipelineManager::run`.

Locate the `// ── Phase 2: clap ──` sequential loop (lines 240-316) and replace it with this dynamic preprocessor pool and consumer queue:

```rust
// ── Phase 2: CLAP Pass with Producer-Consumer Preprocessing & Dynamic Scaling ──
let config = crate::hardware::PipelineConfig::auto_tune();

// 1. Configure the thread-safe ONNX session dynamically at runtime
if let Err(e) = embeddings::configure_session(config.use_coreml, config.intra_threads, Some(&app)) {
    eprintln!("[clap] Failed to configure ONNX session: {}", e);
    return;
}

let clap_pending: Vec<SpoolJob> = {
    let conn = conn_arc.lock().unwrap();
    // [Insert the standard database SELECT query for track_passes where status=PENDING and pass_name='clap']
};

struct PreppedSpectrogram {
    pass_id: i64,
    track_id: i64,
    mel_features: Vec<f32>,
}

// 2. Spawn decoding/resampling threads based on P-Core count
let (tx, rx) = std::sync::mpsc::sync_channel::<PreppedSpectrogram>(config.decode_threads);
let clap_jobs_queue = Arc::new(Mutex::new(VecDeque::from(clap_pending)));

let mut prep_workers = Vec::new();
for _ in 0..config.decode_threads {
    let queue_clone = Arc::clone(&clap_jobs_queue);
    let tx_clone = tx.clone();
    let app_clone = app.clone();

    prep_workers.push(std::thread::spawn(move || {
        loop {
            let job = {
                let mut q = queue_clone.lock().unwrap();
                q.pop_front()
            };
            let job = match job {
                Some(j) => j,
                None => break,
            };

            // Preprocess (Seek-Decode + Resampling) on concurrent threads
            match embeddings::preprocess_track_to_mel(&job.path, Some(&app_clone)) {
                Ok(mel_features) => {
                    let _ = tx_clone.send(PreppedSpectrogram {
                        pass_id: job.pass_id,
                        track_id: job.track_id,
                        mel_features,
                    });
                }
                Err(e) => {
                    log::error!("[clap] Preprocessing failed for track {}: {}", job.track_id, e);
                }
            }
        }
    }));
}
drop(tx); // Drop original transmitter so the receiver knows when all workers exit

// 3. Single-threaded consumer executes ONNX session (offloaded to NPU or CPU P-cores)
for prepped in rx {
    let start = std::time::Instant::now();
    let result = embeddings::run_clap_inference_only(prepped.mel_features);
    let elapsed_ms = start.elapsed().as_millis() as i64;

    let conn = conn_arc.lock().unwrap();
    match result {
        Ok(embedding) => {
            let blob: Vec<u8> = embedding.iter().flat_map(|&f| f.to_le_bytes()).collect();
            let _ = conn.execute(
                "INSERT OR REPLACE INTO audio_embeddings (track_id, embedding) VALUES (?1, ?2)",
                rusqlite::params![prepped.track_id, blob],
            );
            let _ = conn.execute(
                "UPDATE track_passes SET status = ?1, duration_ms = ?2, last_run_at = CURRENT_TIMESTAMP WHERE id = ?3",
                rusqlite::params![pass_status::DONE, elapsed_ms, prepped.pass_id],
            );
            let _ = app.emit("analysis-progress", serde_json::json!({
                "track_id": prepped.track_id,
                "pass_name": "clap",
                "status": pass_status::DONE,
            }));
        }
        Err(e) => {
            let _ = conn.execute(
                "UPDATE track_passes SET status = ?1, log = ?2, duration_ms = ?3, last_run_at = CURRENT_TIMESTAMP WHERE id = ?4",
                rusqlite::params![pass_status::FAILED, e, elapsed_ms, prepped.pass_id],
            );
        }
    }
}

// 4. Join Preprocessing Workers
for h in prep_workers {
    let _ = h.join();
}

let _ = app.emit("analysis-complete", ());
```

This dynamic, seek-aware, producer-consumer framework optimizes pipeline throughput on any Apple Silicon processor by ensuring zero decoding delays and full, safe utilization of the platform's multi-core topologies.
