use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use ort::session::Session;
use ort::{inputs, value::Tensor};
use rustfft::{num_complex::Complex, FftPlanner};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

// ── CLAP mel spectrogram constants ────────────────────────────────────────────

const CLAP_N_FFT: usize = 1024;
const CLAP_HOP: usize = 480;
const CLAP_N_MELS: usize = 64;
const CLAP_N_BINS: usize = CLAP_N_FFT / 2 + 1; // 513
const CLAP_SR: u32 = 48_000;
const CLAP_MAX_FRAMES: usize = 1000; // floor(10 s × 48000 / 480) = 1000
const CLAP_10S_SAMPLES: usize = 480_000; // 10 × 48000

// ── Lazy-loaded ONNX session ──────────────────────────────────────────────────

static SESSION_CLAP_AUDIO: OnceLock<Result<Mutex<Session>, String>> = OnceLock::new();

// ── CLAP mel filterbank (64 × 513 float32, loaded from clap_mel_weights.bin) ─

static CLAP_MEL_FILTERBANK: OnceLock<Result<Vec<f32>, String>> = OnceLock::new();

// ── Model path resolution ─────────────────────────────────────────────────────

/// Dynamically resolves the path of a model file.
/// Checks Tauri resource bundle, sandboxed App Data directory, and dev fallbacks.
pub fn get_model_path(model_filename: &str, app: Option<&tauri::AppHandle>) -> PathBuf {
    use tauri::Manager;
    if let Some(app) = app {
        // 1. Tauri resource bundle
        if let Ok(res_dir) = app.path().resource_dir() {
            let path = res_dir.join("models").join(model_filename);
            if path.exists() {
                return path;
            }
        }
        // 2. Sandboxed App Data directory
        if let Ok(app_dir) = app.path().app_data_dir() {
            let path = app_dir.join("models").join(model_filename);
            if path.exists() {
                return path;
            }
        }
    }

    // 3. Dev fallbacks (run from project root or src-tauri/)
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

// ── Session helpers ───────────────────────────────────────────────────────────

fn load_clap_audio_session(app: Option<&tauri::AppHandle>) -> Result<Mutex<Session>, String> {
    let path = get_model_path("clap_audio_encoder.onnx", app);
    if !path.exists() {
        return Err(format!(
            "CLAP audio encoder model missing: {:?}. Run tools/export_clap_onnx.py first.",
            path
        ));
    }
    let mut builder = Session::builder()
        .map_err(|e| format!("ORT builder error: {}", e))?
        .with_intra_threads(1)
        .and_then(|b| b.with_inter_threads(1))
        .map_err(|e| format!("Failed to configure CLAP threading: {}", e))?;

    builder
        .commit_from_file(&path)
        .map(Mutex::new)
        .map_err(|e| format!("Failed to load clap_audio_encoder.onnx: {}", e))
}

fn get_clap_audio_session(app: Option<&tauri::AppHandle>) -> Result<&'static Mutex<Session>, String> {
    match SESSION_CLAP_AUDIO.get_or_init(|| load_clap_audio_session(app)) {
        Ok(s) => Ok(s),
        Err(e) => Err(e.clone()),
    }
}

// ── CLAP mel filterbank ───────────────────────────────────────────────────────

fn get_clap_mel_filterbank(app: Option<&tauri::AppHandle>) -> Result<&'static Vec<f32>, String> {
    match CLAP_MEL_FILTERBANK.get_or_init(|| {
        let path = get_model_path("clap_mel_weights.bin", app);
        if !path.exists() {
            return Err(format!(
                "CLAP mel weights missing: {:?}. Run tools/export_clap_onnx.py first.",
                path
            ));
        }
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
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
    let mut resampler =
        SincFixedIn::<f32>::new(ratio, 2.0, params, chunk_size, 1)
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
    for v in mel_spec.iter_mut() {
        *v = 10.0 * v.max(1e-10f32).log10();
    }
    let max_db = mel_spec.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    for v in mel_spec.iter_mut() {
        *v = v.max(max_db - 80.0);
    }

    // Per-clip normalisation: (x − μ) / (σ + 1e-6)
    let mean = mel_spec.iter().sum::<f32>() / mel_spec.len() as f32;
    let var = mel_spec.iter().map(|&x| (x - mean) * (x - mean)).sum::<f32>()
        / mel_spec.len() as f32;
    let std_dev = var.sqrt();
    for v in mel_spec.iter_mut() {
        *v = (*v - mean) / (std_dev + 1e-6);
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

/// Generates a 512-d L2-normalised CLAP audio embedding for a music file.
///
/// Decodes the file, resamples to 48 kHz, extracts a 10-second centre window,
/// computes the CLAP log-mel spectrogram, and runs the ONNX audio encoder.
///
/// Requires `models/clap_audio_encoder.onnx` and `models/clap_mel_weights.bin`
/// (generate with `tools/export_clap_onnx.py`).
pub fn run_clap_audio_embed(
    path: &str,
    app: Option<&tauri::AppHandle>,
) -> Result<Vec<f32>, String> {
    // 1. Decode and resample to 48 kHz
    let (audio, sample_rate) = crate::dsp::decode_audio_to_mono(path)?;
    let audio_48k = resample_audio(&audio, sample_rate, CLAP_SR)?;

    // 2. Extract 10-second window centred on the track midpoint
    let mid = audio_48k.len() / 2;
    let half = CLAP_10S_SAMPLES / 2;
    let start = mid.saturating_sub(half);
    let end = (start + CLAP_10S_SAMPLES).min(audio_48k.len());
    let mut window = audio_48k[start..end].to_vec();

    // Tile-pad short clips rather than zero-pad, to avoid DC bias
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

    // 3. CLAP log-mel spectrogram → flat (CLAP_MAX_FRAMES × CLAP_N_MELS)
    let mel_filterbank = get_clap_mel_filterbank(app)?;
    let mel_flat = compute_clap_log_mel(&window, mel_filterbank)?;

    // 4. Wrap into tensor [1, 1, CLAP_MAX_FRAMES, CLAP_N_MELS] and run ONNX encoder
    let input_t = Tensor::from_array(
        ([1usize, 1, CLAP_MAX_FRAMES, CLAP_N_MELS], mel_flat),
    )
    .map_err(|e| e.to_string())?;

    let session_mutex = get_clap_audio_session(app)?;
    let mut session = session_mutex
        .lock()
        .map_err(|e| format!("Failed to lock CLAP audio session: {}", e))?;

    let outputs = session
        .run(inputs!["input_features" => input_t])
        .map_err(|e| format!("CLAP audio inference failed: {}", e))?;

    let out = outputs
        .get("audio_embedding")
        .ok_or("CLAP audio output 'audio_embedding' missing")?;

    let (_, data) = out
        .try_extract_tensor::<f32>()
        .map_err(|e| format!("Failed to extract CLAP audio output: {}", e))?;

    Ok(data.iter().copied().take(512).collect())
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

        let mel_flat = compute_clap_log_mel(&signal, filterbank).expect("spectrogram extraction failed");
        assert_eq!(mel_flat.len(), CLAP_MAX_FRAMES * CLAP_N_MELS);
        assert!(mel_flat.iter().all(|&v| v.is_finite()));
    }
}
