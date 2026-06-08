use ort::session::Session;
use ort::{inputs, value::Tensor};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use rustfft::{num_complex::Complex, FftPlanner};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use tokenizers::{Tokenizer, TruncationDirection, TruncationParams, TruncationStrategy};

// ── CLAP mel spectrogram constants ────────────────────────────────────────────

const CLAP_N_FFT: usize = 1024;
const CLAP_HOP: usize = 480;
const CLAP_N_MELS: usize = 64;
const CLAP_N_BINS: usize = CLAP_N_FFT / 2 + 1; // 513
const CLAP_SR: u32 = 48_000;
const CLAP_MAX_FRAMES: usize = 1000; // floor(10 s × 48000 / 480) = 1000
const CLAP_10S_SAMPLES: usize = 480_000; // 10 × 48000

// ── Thread-safe ONNX session ──────────────────────────────────────────────────

static SESSION_CLAP_AUDIO: Mutex<Option<Session>> = Mutex::new(None);

// ── CLAP mel filterbank (64 × 513 float32, loaded from clap_mel_weights.bin) ─

static CLAP_MEL_FILTERBANK: OnceLock<Result<Vec<f32>, String>> = OnceLock::new();

// ── Model path resolution ─────────────────────────────────────────────────────

/// Dynamically resolves the path of a model file.
/// Checks Tauri resource bundle, sandboxed App Data directory, and dev fallbacks.
pub fn get_model_path(model_filename: &str, app: Option<&tauri::AppHandle>) -> PathBuf {
    use tauri::Manager;
    if let Some(app) = app {
        // 1. User-configured model directory from app_settings.model_path (prioritized)
        if let Some(model_dir) = configured_model_dir(app) {
            let path = model_dir.join(model_filename);
            log::info!("[embeddings] Resolved get_model_path for {} in custom folder: {:?}", model_filename, path);
            return path;
        }

        // 2. Tauri resource bundle
        if let Ok(res_dir) = app.path().resource_dir() {
            let path = res_dir.join("models").join(model_filename);
            if path.exists() {
                return path;
            }
        }
        // 3. Sandboxed App Data directory
        if let Ok(app_dir) = app.path().app_data_dir() {
            let path = app_dir.join("models").join(model_filename);
            if path.exists() {
                return path;
            }
        }
    }

    // 4. Dev fallbacks (run from project root or src-tauri/)
    let dev_path = Path::new("models").join(model_filename);
    if dev_path.exists() {
        return dev_path;
    }
    let dev_path_parent = Path::new("../models").join(model_filename);
    if dev_path_parent.exists() {
        return dev_path_parent;
    }

    PathBuf::from(model_filename)
}

fn configured_model_dir(app: &tauri::AppHandle) -> Option<PathBuf> {
    use tauri::Manager;
    
    // Try managed state first to prevent SQLite locking busy errors
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
                    return Some(PathBuf::from(trimmed));
                }
            }
        }
    }

    // Fallback to manual open if managed state is not available
    if let Ok(app_dir) = app.path().app_data_dir() {
        let db_path = app_dir.join("deep_cuts.db");
        if db_path.exists() {
            if let Ok(conn) = rusqlite::Connection::open(db_path) {
                let value: Option<String> = conn
                    .query_row(
                        "SELECT value FROM app_settings WHERE key = 'model_path'",
                        [],
                        |row| row.get(0),
                    )
                    .ok()
                    .flatten();
                if let Some(val) = value {
                    let trimmed = val.trim();
                    if !trimmed.is_empty() {
                        return Some(PathBuf::from(trimmed));
                    }
                }
            }
        }
    }
    None
}

// ── Session management ────────────────────────────────────────────────────────

/// Configures (or reconfigures) the CLAP ONNX session with the given threading parameters.
/// Called once at the start of each analysis pipeline run.
pub fn configure_session(
    _use_coreml: bool,
    intra_threads: usize,
    app: Option<&tauri::AppHandle>,
) -> Result<(), String> {
    let path = get_model_path("clap_audio_encoder.onnx", app);
    if !path.exists() {
        return Err(format!(
            "CLAP audio encoder model missing: {:?}. Run tools/export_clap_onnx.py first.",
            path
        ));
    }

    let session = Session::builder()
        .map_err(|e| format!("ORT builder error: {}", e))?
        .with_intra_threads(intra_threads)
        .and_then(|b| b.with_inter_threads(1))
        .and_then(|b| b.with_config_entry("session.use_mmap", "0"))
        .map_err(|e| format!("Failed to configure CLAP session: {}", e))?
        .commit_from_file(&path)
        .map_err(|e| format!("Failed to load clap_audio_encoder.onnx: {}", e))?;

    let mut guard = SESSION_CLAP_AUDIO
        .lock()
        .map_err(|e| format!("Failed to acquire session lock: {}", e))?;
    *guard = Some(session);
    Ok(())
}

// ── CLAP mel filterbank ───────────────────────────────────────────────────────

fn get_clap_mel_filterbank(_app: Option<&tauri::AppHandle>) -> Result<&'static Vec<f32>, String> {
    match CLAP_MEL_FILTERBANK.get_or_init(|| {
        let bytes = include_bytes!("clap_mel_weights.bin");
        let expected = CLAP_N_MELS * CLAP_N_BINS * 4;
        if bytes.len() != expected {
            return Err(format!(
                "Corrupt CLAP mel weights: expected {} bytes, got {}",
                expected,
                bytes.len()
            ));
        }
        Ok(bytes
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
            .collect())
    }) {
        Ok(fb) => Ok(fb),
        Err(e) => Err(e.clone()),
    }
}

// ── Resampling ────────────────────────────────────────────────────────────────

/// Resamples mono audio from `from_sr` to `to_sr` using a high-quality sinc resampler.
fn resample_audio(audio: &[f32], from_sr: u32, to_sr: u32) -> Result<Vec<f32>, String> {
    if from_sr == to_sr {
        return Ok(audio.to_vec());
    }

    let ratio = to_sr as f64 / from_sr as f64;
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };
    let chunk_size = audio.len().max(1);
    let mut resampler = SincFixedIn::<f32>::new(ratio, 2.0, params, chunk_size, 1)
        .map_err(|e| format!("Failed to create resampler: {}", e))?;

    let input = vec![audio.to_vec()];
    let output = resampler
        .process(&input, None)
        .map_err(|e| format!("Resampling failed: {}", e))?;

    Ok(output.into_iter().next().unwrap_or_default())
}

// ── CLAP mel spectrogram ──────────────────────────────────────────────────────

/// Computes the CLAP log-mel spectrogram from 48 kHz mono audio.
///
/// Matches the librosa-based `ClapFeatureExtractor._np_extract_fbank_features`:
/// - Reflect-padded STFT (n_fft=1024, hop=480, periodic Hann)
/// - Power mel filterbank (64 bands, pre-computed weights from clap_mel_weights.bin)
/// - power_to_db (ref=1, amin=1e-10, top_db=80)
/// - Per-clip mean/std normalisation
///
/// Returns a flat Vec<f32> of shape (CLAP_MAX_FRAMES × CLAP_N_MELS) in row-major order,
/// suitable for wrapping into tensor shape [1, 1, CLAP_MAX_FRAMES, CLAP_N_MELS].
fn compute_clap_log_mel(audio_48k: &[f32], mel_filterbank: &[f32]) -> Result<Vec<f32>, String> {
    let n = audio_48k.len();
    let pad = CLAP_N_FFT / 2; // 512

    // Centre-reflect pad (numpy mode='reflect')
    // Left:  padded[i]         = audio[pad - i]     for i in 0..pad
    // Copy:  padded[pad..pad+n] = audio
    // Right: padded[pad+n+i]   = audio[n-2-i]        for i in 0..pad
    let mut padded = vec![0.0f32; n + 2 * pad];
    padded[pad..pad + n].copy_from_slice(audio_48k);
    for i in 0..pad.min(n.saturating_sub(1)) {
        padded[i] = audio_48k[pad - i];
        padded[pad + n + i] = audio_48k[n - 2 - i];
    }

    // Periodic Hann window: w[k] = 0.5 * (1 − cos(2π·k / N_FFT))
    let hann: Vec<f32> = (0..CLAP_N_FFT)
        .map(|k| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * k as f32 / CLAP_N_FFT as f32).cos()))
        .collect();

    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(CLAP_N_FFT);
    let mut fft_buf = vec![Complex::new(0.0f32, 0.0f32); CLAP_N_FFT];

    // n_frames = 1 + n / hop  (with centre padding the formula simplifies to this)
    let n_frames = 1 + n / CLAP_HOP; // 1001 for 10 s audio

    let mut mel_spec = vec![0.0f32; n_frames * CLAP_N_MELS];
    let mut power = vec![0.0f32; CLAP_N_BINS];

    for frame_idx in 0..n_frames {
        let start = frame_idx * CLAP_HOP;

        for i in 0..CLAP_N_FFT {
            let s = if start + i < padded.len() {
                padded[start + i]
            } else {
                0.0
            };
            fft_buf[i].re = s * hann[i];
            fft_buf[i].im = 0.0;
        }

        fft.process(&mut fft_buf);

        for k in 0..CLAP_N_BINS {
            let re = fft_buf[k].re;
            let im = fft_buf[k].im;
            power[k] = re * re + im * im;
        }

        for m in 0..CLAP_N_MELS {
            let mut sum = 0.0f32;
            let offset = m * CLAP_N_BINS;
            for k in 0..CLAP_N_BINS {
                sum += power[k] * mel_filterbank[offset + k];
            }
            mel_spec[frame_idx * CLAP_N_MELS + m] = sum;
        }
    }

    // power_to_db: 10 * log10(max(1e-10, power)), then clip to max − 80 dB
    // No per-clip normalisation — the model expects raw dB values as produced
    // by the HuggingFace ClapFeatureExtractor (mean ≈ -10, std ≈ 10).
    for v in mel_spec.iter_mut() {
        *v = 10.0 * v.max(1e-10f32).log10();
    }
    let max_db = mel_spec.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    for v in mel_spec.iter_mut() {
        *v = v.max(max_db - 80.0);
    }

    // Truncate to CLAP_MAX_FRAMES and pack into output buffer
    let out_frames = n_frames.min(CLAP_MAX_FRAMES);
    let mut output = vec![0.0f32; CLAP_MAX_FRAMES * CLAP_N_MELS];
    for f in 0..out_frames {
        for m in 0..CLAP_N_MELS {
            output[f * CLAP_N_MELS + m] = mel_spec[f * CLAP_N_MELS + m];
        }
    }

    Ok(output)
}

// ── Public API ────────────────────────────────────────────────────────────────

const DEFAULT_CLAP_WINDOW_PCTS: [f64; 3] = [0.25, 0.50, 0.75];

/// Returns the single highest-energy window center from the waveform profile, as a fraction [0,1].
/// Falls back to 0.5 (midpoint) if waveform data is absent or all-zero.
pub fn select_best_energy_window_pct(waveform_data: Option<&str>) -> f64 {
    let Some(data) = waveform_data else {
        return 0.5;
    };
    let Ok(waveform) = serde_json::from_str::<Vec<f32>>(data) else {
        return 0.5;
    };
    let best = waveform
        .iter()
        .enumerate()
        .filter(|(_, v)| v.is_finite())
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    match best {
        Some((idx, _)) => (idx as f64 + 0.5) / waveform.len() as f64,
        None => 0.5,
    }
}

/// Selects three CLAP window centers from the audio-analysis waveform profile.
///
/// Strategy (applied in order):
/// 1. Trim leading/trailing low-energy bins (intros, outros, silence) so they
///    are never candidates.
/// 2. Compute the coefficient of variation (CV = σ/μ) of the trimmed envelope.
///    - CV < 0.25 → flat-loudness (brickwall mastering): use temporal spread
///      at 15 / 50 / 85 % of the track body.
///    - CV ≥ 0.25 → dynamic track: pick the loudest bin from each of the three
///      energy terciles (low / mid / high), each separated by at least 10 s.
pub fn select_clap_window_pcts(waveform_data: Option<&str>, duration_seconds: i64) -> [f64; 3] {
    let Some(waveform_data) = waveform_data else {
        return DEFAULT_CLAP_WINDOW_PCTS;
    };
    let Ok(waveform) = serde_json::from_str::<Vec<f32>>(waveform_data) else {
        return DEFAULT_CLAP_WINDOW_PCTS;
    };

    let finite: Vec<(usize, f32)> = waveform
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, v)| v.is_finite() && *v > 0.0)
        .collect();

    if finite.len() < 3 {
        return DEFAULT_CLAP_WINDOW_PCTS;
    }

    let bin_count = waveform.len();
    let min_sep_bins = if duration_seconds > 0 {
        ((10.0 / duration_seconds as f64) * bin_count as f64)
            .ceil()
            .max(1.0) as usize
    } else {
        (bin_count / 12).max(1)
    };

    // ── Step 1: trim low-energy tails ────────────────────────────────────────
    // Threshold = 20th percentile of finite values.
    let mut sorted_vals: Vec<f32> = finite.iter().map(|(_, v)| *v).collect();
    sorted_vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let low_threshold = sorted_vals[sorted_vals.len() / 5];

    // Walk inward from each end to find where the energy rises above threshold.
    let first_active = (0..bin_count)
        .find(|&i| waveform.get(i).copied().unwrap_or(0.0) > low_threshold)
        .unwrap_or(0);
    let last_active = (0..bin_count)
        .rev()
        .find(|&i| waveform.get(i).copied().unwrap_or(0.0) > low_threshold)
        .unwrap_or(bin_count - 1);

    let candidates: Vec<(usize, f32)> = finite
        .iter()
        .copied()
        .filter(|(i, _)| *i >= first_active && *i <= last_active)
        .collect();

    if candidates.len() < 3 {
        return DEFAULT_CLAP_WINDOW_PCTS;
    }

    // ── Step 2: compute CV on the trimmed body ────────────────────────────────
    let vals: Vec<f32> = candidates.iter().map(|(_, v)| *v).collect();
    let mean = vals.iter().sum::<f32>() / vals.len() as f32;
    let variance = vals.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / vals.len() as f32;
    let cv = variance.sqrt() / mean;

    // ── Step 3: select windows ────────────────────────────────────────────────
    let pct = |idx: usize| (idx as f64 + 0.5) / bin_count as f64;

    if cv < 0.25 {
        // Flat-loudness: temporal spread anchored to the trimmed body.
        let body_start = pct(first_active);
        let body_end = pct(last_active);
        let span = body_end - body_start;
        return [
            body_start + span * 0.15,
            body_start + span * 0.50,
            body_start + span * 0.85,
        ];
    }

    // Dynamic: energy tercile selection with minimum spacing.
    let mut sorted_candidates = candidates.clone();
    sorted_candidates.sort_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap());
    let n = sorted_candidates.len();
    let t1 = sorted_candidates[n / 3].1;
    let t2 = sorted_candidates[2 * n / 3].1;

    let mut low_tercile: Vec<(usize, f32)> =
        candidates.iter().copied().filter(|(_, v)| *v <= t1).collect();
    let mut mid_tercile: Vec<(usize, f32)> =
        candidates.iter().copied().filter(|(_, v)| *v > t1 && *v <= t2).collect();
    let mut high_tercile: Vec<(usize, f32)> =
        candidates.iter().copied().filter(|(_, v)| *v > t2).collect();

    // Within each tercile, prefer the loudest bin that satisfies spacing.
    for tercile in [&mut low_tercile, &mut mid_tercile, &mut high_tercile] {
        tercile.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap());
    }

    let mut selected: Vec<usize> = Vec::with_capacity(3);
    for tercile in [&low_tercile, &mid_tercile, &high_tercile] {
        for (idx, _) in tercile {
            if selected.iter().all(|p| idx.abs_diff(*p) >= min_sep_bins) {
                selected.push(*idx);
                break;
            }
        }
    }

    if selected.len() < 3 {
        return DEFAULT_CLAP_WINDOW_PCTS;
    }

    selected.sort_unstable();
    [pct(selected[0]), pct(selected[1]), pct(selected[2])]
}

/// Decodes the full file, resamples to 48 kHz, then extracts a 10 s window centred at `pct`.
pub fn preprocess_window_at_pct(
    path: &str,
    pct: f64,
    app: Option<&tauri::AppHandle>,
) -> Result<Vec<f32>, String> {
    let (audio, sample_rate) = crate::dsp::decode_audio_to_mono(path)?;
    let audio_48k = resample_audio(&audio, sample_rate, CLAP_SR)?;

    let center = (audio_48k.len() as f64 * pct) as usize;
    let (start, end) = if audio_48k.len() <= CLAP_10S_SAMPLES {
        (0, audio_48k.len())
    } else {
        let half = CLAP_10S_SAMPLES / 2;
        let mut start = center.saturating_sub(half);
        let mut end = start + CLAP_10S_SAMPLES;
        if end > audio_48k.len() {
            end = audio_48k.len();
            start = end - CLAP_10S_SAMPLES;
        }
        (start, end)
    };
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

/// Runs ONNX inference on pre-computed mel features (thread-safe, concurrent reads).
/// Requires `configure_session` to have been called first.
pub fn run_clap_inference_only(mel_flat: Vec<f32>) -> Result<Vec<f32>, String> {
    let input_t = Tensor::from_array(([1usize, 1, CLAP_MAX_FRAMES, CLAP_N_MELS], mel_flat))
        .map_err(|e| e.to_string())?;

    let mut guard = SESSION_CLAP_AUDIO
        .lock()
        .map_err(|e| format!("Failed to acquire session lock: {}", e))?;

    let session = guard
        .as_mut()
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

/// Runs 3 ONNX inferences on pre-computed mel windows and returns the L2-normalised mean embedding.
pub fn run_clap_inference_pooled(mels: [Vec<f32>; 3]) -> Result<Vec<f32>, String> {
    let [mel_25, mel_50, mel_75] = mels;
    let v1 = run_clap_inference_only(mel_25)?;
    let v2 = run_clap_inference_only(mel_50)?;
    let v3 = run_clap_inference_only(mel_75)?;

    let mut v_mean = vec![0.0f32; 512];
    for i in 0..512 {
        v_mean[i] = (v1[i] + v2[i] + v3[i]) / 3.0;
    }

    let l2_norm = v_mean.iter().map(|&x| x * x).sum::<f32>().sqrt();
    if l2_norm > 1e-8 {
        Ok(v_mean.iter().map(|&x| x / l2_norm).collect())
    } else {
        Ok(v_mean)
    }
}

/// @concept CLAP
/// Convenience wrapper: full pipeline for a single track. Only used in tests.
#[cfg(test)]
pub fn run_clap_audio_embed(
    path: &str,
    app: Option<&tauri::AppHandle>,
) -> Result<Vec<f32>, String> {
    // Ensure a session exists (lazy-init with 1 intra-thread for single-track use)
    {
        let guard = SESSION_CLAP_AUDIO
            .lock()
            .map_err(|e| format!("Failed to acquire session lock: {}", e))?;
        if guard.is_none() {
            drop(guard);
            configure_session(false, 1, app)?;
        }
    }
    let mels = [
        preprocess_window_at_pct(path, 0.25, app)?,
        preprocess_window_at_pct(path, 0.50, app)?,
        preprocess_window_at_pct(path, 0.75, app)?,
    ];
    run_clap_inference_pooled(mels)
}

// ── Sentence Embedding (all-MiniLM-L6-v2) ──────────────────────────────────────

static SESSION_SENTENCE: OnceLock<Result<Mutex<Session>, String>> = OnceLock::new();
static SENTENCE_TOKENIZER: OnceLock<Result<Tokenizer, String>> = OnceLock::new();

fn load_sentence_session(
    model_file: &str,
    app: Option<&tauri::AppHandle>,
) -> Result<Mutex<Session>, String> {
    let path = get_model_path(model_file, app);
    if !path.exists() {
        return Err(format!(
            "Embedding model missing: {:?}. Please download it first.",
            path
        ));
    }
    let builder = Session::builder().map_err(|e| format!("ORT builder error: {}", e))?;

    let mut configured = builder
        .with_intra_threads(1)
        .and_then(|b| b.with_inter_threads(1))
        .and_then(|b| b.with_config_entry("session.use_mmap", "0"))
        .map_err(|e| format!("Failed to configure threading: {}", e))?;

    configured
        .commit_from_file(&path)
        .map(Mutex::new)
        .map_err(|e| format!("Failed to load {}: {}", model_file, e))
}

fn get_sentence_session(app: Option<&tauri::AppHandle>) -> Result<&'static Mutex<Session>, String> {
    match SESSION_SENTENCE.get_or_init(|| load_sentence_session("all-minilm-l6-v2.onnx", app)) {
        Ok(s) => Ok(s),
        Err(e) => Err(e.clone()),
    }
}

fn load_tokenizer(
    tokenizer_file: &str,
    max_length: usize,
    app: Option<&tauri::AppHandle>,
) -> Result<Tokenizer, String> {
    let path = get_model_path(tokenizer_file, app);
    let mut tokenizer = Tokenizer::from_file(path.to_str().unwrap_or(""))
        .map_err(|e| format!("Failed to load tokenizer {:?}: {}", path, e))?;
    tokenizer
        .with_truncation(Some(TruncationParams {
            max_length,
            strategy: TruncationStrategy::LongestFirst,
            direction: TruncationDirection::Right,
            stride: 0,
        }))
        .map_err(|e| format!("Tokenizer truncation config failed: {}", e))?;
    Ok(tokenizer)
}

fn get_sentence_tokenizer(app: Option<&tauri::AppHandle>) -> Result<&'static Tokenizer, String> {
    match SENTENCE_TOKENIZER
        .get_or_init(|| load_tokenizer("all-minilm-l6-v2-tokenizer.json", 512, app))
    {
        Ok(t) => Ok(t),
        Err(e) => Err(e.clone()),
    }
}

/// Encodes `text` into (input_ids, attention_mask, token_type_ids) as i64 vectors.
fn tokenize(tokenizer: &Tokenizer, text: &str) -> Result<(Vec<i64>, Vec<i64>, Vec<i64>), String> {
    let enc = tokenizer
        .encode(text, true)
        .map_err(|e| format!("Tokenisation failed: {}", e))?;

    let ids: Vec<i64> = enc.get_ids().iter().map(|&v| v as i64).collect();
    let mask: Vec<i64> = enc.get_attention_mask().iter().map(|&v| v as i64).collect();
    let type_ids: Vec<i64> = enc.get_type_ids().iter().map(|&v| v as i64).collect();
    Ok((ids, mask, type_ids))
}

/// @concept SentenceEmbeddings
/// Generates a 384-d L2-normalised sentence embedding using all-MiniLM-L6-v2.
pub fn run_sentence_embed(text: &str, app: Option<&tauri::AppHandle>) -> Result<Vec<f32>, String> {
    let tokenizer = get_sentence_tokenizer(app)?;
    let (ids, mask, type_ids) = tokenize(tokenizer, text)?;
    let seq_len = ids.len();

    let ids_t = Tensor::from_array(([1, seq_len], ids)).map_err(|e| e.to_string())?;
    let mask_t = Tensor::from_array(([1, seq_len], mask)).map_err(|e| e.to_string())?;
    let type_t = Tensor::from_array(([1, seq_len], type_ids)).map_err(|e| e.to_string())?;

    let session_mutex = get_sentence_session(app)?;
    let mut session = session_mutex.lock().map_err(|e| e.to_string())?;

    let outputs = session
        .run(inputs![
            "input_ids"      => ids_t,
            "attention_mask" => mask_t,
            "token_type_ids" => type_t
        ])
        .map_err(|e| format!("MiniLM inference failed: {}", e))?;

    let out = outputs
        .get("sentence_embedding")
        .ok_or("MiniLM output 'sentence_embedding' missing")?;

    let (_, data) = out
        .try_extract_tensor::<f32>()
        .map_err(|e| format!("Failed to extract MiniLM output: {}", e))?;

    let embedding: Vec<f32> = data.iter().copied().take(384).collect();

    // L2 normalization
    let l2_norm = embedding.iter().map(|&x| x * x).sum::<f32>().sqrt();
    if l2_norm > 1e-8 {
        Ok(embedding.iter().map(|&x| x / l2_norm).collect())
    } else {
        Ok(embedding)
    }
}

// ── CLAP Text Embedding (laion/clap-htsat-unfused text encoder) ────────────────

static SESSION_CLAP_TEXT: OnceLock<Result<Mutex<Session>, String>> = OnceLock::new();
static CLAP_TEXT_TOKENIZER: OnceLock<Result<Tokenizer, String>> = OnceLock::new();

fn load_clap_text_session(
    model_file: &str,
    app: Option<&tauri::AppHandle>,
) -> Result<Mutex<Session>, String> {
    let path = get_model_path(model_file, app);
    if !path.exists() {
        return Err(format!(
            "CLAP text encoder model missing: {:?}. Run tools/export_clap_onnx.py first.",
            path
        ));
    }
    let builder = Session::builder().map_err(|e| format!("ORT builder error: {}", e))?;

    let mut configured = builder
        .with_intra_threads(1)
        .and_then(|b| b.with_inter_threads(1))
        .map_err(|e| format!("Failed to configure CLAP text threading: {}", e))?;

    configured
        .commit_from_file(&path)
        .map(Mutex::new)
        .map_err(|e| format!("Failed to load {}: {}", model_file, e))
}

fn get_clap_text_session(app: Option<&tauri::AppHandle>) -> Result<&'static Mutex<Session>, String> {
    match SESSION_CLAP_TEXT.get_or_init(|| load_clap_text_session("clap_text_encoder.onnx", app)) {
        Ok(s) => Ok(s),
        Err(e) => Err(e.clone()),
    }
}

fn get_clap_text_tokenizer(app: Option<&tauri::AppHandle>) -> Result<&'static Tokenizer, String> {
    match CLAP_TEXT_TOKENIZER
        .get_or_init(|| load_tokenizer("clap-tokenizer.json", 512, app))
    {
        Ok(t) => Ok(t),
        Err(e) => Err(e.clone()),
    }
}

/// @concept CLAP
/// Generates a 512-d L2-normalised CLAP text embedding using clap_text_encoder.onnx.
pub fn run_clap_text_embed(text: &str, app: Option<&tauri::AppHandle>) -> Result<Vec<f32>, String> {
    let tokenizer = get_clap_text_tokenizer(app)?;
    let (ids, mask, _type_ids) = tokenize(tokenizer, text)?;
    let seq_len = ids.len();

    let ids_t = Tensor::from_array(([1, seq_len], ids)).map_err(|e| e.to_string())?;
    let mask_t = Tensor::from_array(([1, seq_len], mask)).map_err(|e| e.to_string())?;

    let session_mutex = get_clap_text_session(app)?;
    let mut session = session_mutex.lock().map_err(|e| e.to_string())?;

    let outputs = session
        .run(inputs![
            "input_ids"      => ids_t,
            "attention_mask" => mask_t
        ])
        .map_err(|e| format!("CLAP text inference failed: {}", e))?;

    let out = outputs
        .get("text_embedding")
        .ok_or("CLAP text output 'text_embedding' missing")?;

    let (_, data) = out
        .try_extract_tensor::<f32>()
        .map_err(|e| format!("Failed to extract CLAP text output: {}", e))?;

    let embedding: Vec<f32> = data.iter().copied().take(512).collect();

    // L2 normalization
    let l2_norm = embedding.iter().map(|&x| x * x).sum::<f32>().sqrt();
    if l2_norm > 1e-8 {
        Ok(embedding.iter().map(|&x| x / l2_norm).collect())
    } else {
        Ok(embedding)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        format!("{}/tests/fixtures/{}", manifest, name)
    }

    #[test]
    fn test_clap_audio_embed_returns_512d_vector() {
        let path = fixture("(From Zombie) Re_ Brain Supply Issue.mp3");
        let result = run_clap_audio_embed(&path, None).expect("CLAP embed failed");
        assert_eq!(result.len(), 512, "expected 512-d embedding");
        assert!(
            result.iter().all(|v| v.is_finite()),
            "embedding contains non-finite values"
        );
        assert!(
            result.iter().any(|&v| v != 0.0),
            "embedding is all-zero (model likely failed)"
        );
    }

    #[test]
    fn test_clap_audio_embed_is_approximately_unit_length() {
        let path = fixture("(From Zombie) Re_ Brain Supply Issue.mp3");
        let result = run_clap_audio_embed(&path, None).expect("CLAP embed failed");
        let norm_sq: f32 = result.iter().map(|&v| v * v).sum();
        // The model outputs L2-normalised embeddings; allow ±5% tolerance
        assert!(
            (norm_sq - 1.0).abs() < 0.05,
            "embedding norm² = {:.4}, expected ~1.0",
            norm_sq
        );
    }

    #[test]
    fn test_compute_clap_log_mel_dimensions() {
        let filterbank = get_clap_mel_filterbank(None).expect("failed to load filterbank weights");
        let sr = 48000;
        let duration_secs = 1;
        let signal: Vec<f32> = (0..sr * duration_secs)
            .map(|i| (i as f32 * 2.0 * std::f32::consts::PI * 440.0 / sr as f32).sin())
            .collect();

        let mel_flat =
            compute_clap_log_mel(&signal, filterbank).expect("spectrogram extraction failed");
        assert_eq!(mel_flat.len(), CLAP_MAX_FRAMES * CLAP_N_MELS);
        assert!(mel_flat.iter().all(|&v| v.is_finite()));
    }

    #[test]
    fn test_select_clap_window_pcts_tercile_dynamic() {
        // High CV waveform: three clear energy peaks separated by quiet sections.
        // low tercile peak at bin 2, mid at bin 6, high at bin 10 — all spaced.
        let wf: Vec<f32> = vec![
            0.1, 0.1, 0.4, 0.1, 0.1, 0.1, 0.7, 0.1, 0.1, 0.1, 1.0, 0.1,
        ];
        let waveform = serde_json::to_string(&wf).unwrap();
        let pcts = select_clap_window_pcts(Some(&waveform), 120);
        // Should pick one from each tercile, sorted by position.
        assert_eq!(pcts.len(), 3);
        assert!(pcts[0] < pcts[1] && pcts[1] < pcts[2]);
        // High-tercile bin (10) must be one of the picks.
        assert!(pcts.iter().any(|&p| (p - 10.5 / 12.0).abs() < 1e-9));
    }

    #[test]
    fn test_select_clap_window_pcts_flat_temporal_spread() {
        // Flat waveform (CV < 0.25): all bins nearly equal → temporal spread.
        let wf: Vec<f32> = vec![0.5; 20];
        let waveform = serde_json::to_string(&wf).unwrap();
        let pcts = select_clap_window_pcts(Some(&waveform), 200);
        // With a fully flat body the three picks should be evenly spread.
        assert!(pcts[0] < pcts[1] && pcts[1] < pcts[2]);
        // Middle pick should be near 50 %.
        assert!((pcts[1] - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_select_clap_window_pcts_trims_quiet_tails() {
        // Quiet intro (bins 0-1) and outro (bins 10-11), loud body in between.
        let mut wf = vec![0.01f32; 12];
        wf[2] = 0.4; wf[3] = 0.5; wf[4] = 0.8;
        wf[5] = 0.6; wf[6] = 0.7; wf[7] = 0.9;
        wf[8] = 0.5; wf[9] = 0.4;
        let waveform = serde_json::to_string(&wf).unwrap();
        let pcts = select_clap_window_pcts(Some(&waveform), 120);
        // No pick should land in the silent intro (bins 0-1 → pct < 2/12)
        // or silent outro (bins 10-11 → pct > 10/12).
        assert!(pcts.iter().all(|&p| p > 2.0 / 12.0 && p < 11.0 / 12.0));
    }

    #[test]
    fn test_select_clap_window_pcts_falls_back_for_bad_waveform() {
        assert_eq!(
            select_clap_window_pcts(Some("not json"), 120),
            DEFAULT_CLAP_WINDOW_PCTS
        );
        assert_eq!(
            select_clap_window_pcts(Some("[0,0,0]"), 120),
            DEFAULT_CLAP_WINDOW_PCTS
        );
        assert_eq!(select_clap_window_pcts(None, 120), DEFAULT_CLAP_WINDOW_PCTS);
    }

    #[test]
    fn test_clap_text_embed_returns_512d_vector() {
        let result = run_clap_text_embed("heavy techno beat", None).expect("CLAP text embed failed");
        assert_eq!(result.len(), 512, "expected 512-d embedding");
        assert!(
            result.iter().all(|v| v.is_finite()),
            "embedding contains non-finite values"
        );
        let norm_sq: f32 = result.iter().map(|&v| v * v).sum();
        assert!((norm_sq - 1.0).abs() < 1e-4, "embedding norm² = {}, expected ~1.0", norm_sq);
    }
}
