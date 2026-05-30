use ort::session::Session;
use ort::{inputs, value::Tensor};
use serde::{Deserialize, Serialize};
/// Essentia Discogs-Effnet classifier — genre, mood, and voice/instrumental.
///
/// Architecture:
///   1. `discogs-effnet-bsdynamic-1.onnx`  — shared feature extractor
///      input  : `melspectrogram`  [n_patches, 128, 96]
///      output : `embeddings`      [n_patches, 1280]
///   2. Per-task head ONNX models (genre, mood_*, voice_instrumental)
///      input  : `embeddings`      [n_patches, 1280]
///      output : `activations`     [n_patches, n_classes]
///      Predictions are averaged across patches.
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::embeddings::get_model_path;

// ── Session caches (initialised once per process) ─────────────────────────────

static BASE_SESSION: OnceLock<Result<Mutex<Session>, String>> = OnceLock::new();
static HEAD_SESSIONS: OnceLock<HashMap<String, Result<Mutex<Session>, String>>> = OnceLock::new();
static LABELS_CACHE: OnceLock<HashMap<String, Result<Vec<String>, String>>> = OnceLock::new();

// ── Public result type ────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct ClassifierResult {
    pub genre: Option<String>,
    pub vocal: Option<String>,
    pub vocal_confidence: Option<f64>,
    pub mood_happy: Option<f64>,
    pub mood_sad: Option<f64>,
    pub mood_aggressive: Option<f64>,
    pub mood_relaxed: Option<f64>,
    pub mood_party: Option<f64>,
    pub mood_acoustic: Option<f64>,
    pub mood_electronic: Option<f64>,
}

// ── Label file helpers ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct LabelsJson {
    classes: Vec<String>,
}

fn label_filename(key: &str) -> Option<&'static str> {
    match key {
        "effnet_discogs" => Some("discogs-effnet-bsdynamic-1.json"),
        "genre_discogs400" => Some("genre_discogs400-discogs-effnet-1.json"),
        "mood_happy" => Some("mood_happy-discogs-effnet-1.json"),
        "mood_sad" => Some("mood_sad-discogs-effnet-1.json"),
        "mood_aggressive" => Some("mood_aggressive-discogs-effnet-1.json"),
        "mood_relaxed" => Some("mood_relaxed-discogs-effnet-1.json"),
        "mood_party" => Some("mood_party-discogs-effnet-1.json"),
        "mood_acoustic" => Some("mood_acoustic-discogs-effnet-1.json"),
        "mood_electronic" => Some("mood_electronic-discogs-effnet-1.json"),
        "voice_instrumental" => Some("voice_instrumental-discogs-effnet-1.json"),
        _ => None,
    }
}

fn load_labels(key: &str, app: Option<&tauri::AppHandle>) -> Result<Vec<String>, String> {
    let cache = LABELS_CACHE.get_or_init(HashMap::new);
    if let Some(res) = cache.get(key) {
        return res.clone();
    }
    let fname = label_filename(key).ok_or_else(|| format!("Unknown labels key: {key}"))?;
    let path = get_model_path(fname, app);
    if !path.exists() {
        return Err(format!("Labels JSON missing: {path:?}"));
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("Cannot read {path:?}: {e}"))?;
    let parsed: LabelsJson =
        serde_json::from_str(&text).map_err(|e| format!("Cannot parse {path:?}: {e}"))?;
    Ok(parsed.classes)
}

// ── Session loaders ───────────────────────────────────────────────────────────

fn get_base_session(app: Option<&tauri::AppHandle>) -> Result<&'static Mutex<Session>, String> {
    let res = BASE_SESSION.get_or_init(|| {
        let path = get_model_path("discogs-effnet-bsdynamic-1.onnx", app);
        if !path.exists() {
            return Err(format!("discogs-effnet model missing: {path:?}"));
        }
        Session::builder()
            .map_err(|e| format!("Session builder error: {e}"))?
            .with_intra_threads(1)
            .and_then(|b| b.with_inter_threads(1))
            .map_err(|e| format!("Thread config error: {e}"))?
            .commit_from_file(&path)
            .map(Mutex::new)
            .map_err(|e| format!("Failed to load base Effnet session: {e}"))
    });
    res.as_ref().map_err(|e| e.clone())
}

fn head_filename(key: &str) -> Option<&'static str> {
    match key {
        "genre_discogs400" => Some("genre_discogs400-discogs-effnet-1.onnx"),
        "mood_happy" => Some("mood_happy-discogs-effnet-1.onnx"),
        "mood_sad" => Some("mood_sad-discogs-effnet-1.onnx"),
        "mood_aggressive" => Some("mood_aggressive-discogs-effnet-1.onnx"),
        "mood_relaxed" => Some("mood_relaxed-discogs-effnet-1.onnx"),
        "mood_party" => Some("mood_party-discogs-effnet-1.onnx"),
        "mood_acoustic" => Some("mood_acoustic-discogs-effnet-1.onnx"),
        "mood_electronic" => Some("mood_electronic-discogs-effnet-1.onnx"),
        "voice_instrumental" => Some("voice_instrumental-discogs-effnet-1.onnx"),
        _ => None,
    }
}

fn get_head_session(
    key: &str,
    app: Option<&tauri::AppHandle>,
) -> Result<&'static Mutex<Session>, String> {
    let cache = HEAD_SESSIONS.get_or_init(|| {
        let keys = [
            "genre_discogs400",
            "mood_happy",
            "mood_sad",
            "mood_aggressive",
            "mood_relaxed",
            "mood_party",
            "mood_acoustic",
            "mood_electronic",
            "voice_instrumental",
        ];
        let mut map = HashMap::new();
        for k in &keys {
            let fname = match head_filename(k) {
                Some(f) => f,
                None => continue,
            };
            let path = get_model_path(fname, app);
            let res = if !path.exists() {
                Err(format!("Head model missing: {path:?}"))
            } else {
                Session::builder()
                    .map_err(|e| format!("Session builder error: {e}"))
                    .and_then(|b| {
                        b.with_intra_threads(1)
                            .and_then(|b| b.with_inter_threads(1))
                            .map_err(|e| format!("Thread config error: {e}"))
                    })
                    .and_then(|mut b| {
                        b.commit_from_file(&path)
                            .map(Mutex::new)
                            .map_err(|e| format!("Failed to load head {k}: {e}"))
                    })
            };
            map.insert(k.to_string(), res);
        }
        map
    });
    match cache.get(key) {
        Some(Ok(s)) => Ok(s),
        Some(Err(e)) => Err(e.clone()),
        None => Err(format!("Head session '{key}' not in cache")),
    }
}

// ── Main inference entry point ────────────────────────────────────────────────

/// Runs the full Essentia Discogs-Effnet classifier pipeline on pre-extracted
/// log-mel spectrogram patches.
///
/// `patches` — flat `(128 × 96)` f32 vectors from `spectrogram::extract_patches`.
pub fn run_classifier_inference(
    patches: &[Vec<f32>],
    app: Option<&tauri::AppHandle>,
) -> Result<ClassifierResult, String> {
    if patches.is_empty() {
        return Err("No patches provided for inference".to_string());
    }

    const PATCH_SIZE: usize = 128;
    const N_BANDS: usize = 96;
    const EMBEDDING_DIM: usize = 1280;

    let n_patches = patches.len();

    // ── 1. Build flat input tensor [n_patches, 128, 96] ───────────────────────
    let mut input_data = Vec::with_capacity(n_patches * PATCH_SIZE * N_BANDS);
    for p in patches {
        input_data.extend_from_slice(p);
    }

    // ── 2. Base Effnet feature extraction ─────────────────────────────────────
    let base_mutex = get_base_session(app)?;
    let mut base = base_mutex
        .lock()
        .map_err(|e| format!("Base session lock error: {e}"))?;

    let input_tensor = Tensor::from_array(([n_patches, PATCH_SIZE, N_BANDS], input_data))
        .map_err(|e| format!("Failed to create input tensor: {e}"))?;

    let base_out = base
        .run(inputs!["melspectrogram" => input_tensor])
        .map_err(|e| format!("Base Effnet inference failed: {e}"))?;

    let emb_tensor = base_out
        .get("embeddings")
        .ok_or("Base Effnet output missing 'embeddings'")?;

    let (emb_shape_cow, emb_data) = emb_tensor
        .try_extract_tensor::<f32>()
        .map_err(|e| format!("Failed to extract embeddings: {e}"))?;

    let emb_shape = &**emb_shape_cow;
    if emb_shape.len() != 2
        || emb_shape[0] as usize != n_patches
        || emb_shape[1] as usize != EMBEDDING_DIM
    {
        return Err(format!(
            "Unexpected embedding shape: expected [{n_patches}, {EMBEDDING_DIM}], got {emb_shape:?}"
        ));
    }
    let embeddings: Vec<f32> = emb_data.to_vec();

    // ── 3. Helper: run a head and average probabilities across patches ─────────
    let run_head = |key: &str, input_node: &str, output_node: &str| -> Result<Vec<f32>, String> {
        let mutex = get_head_session(key, app)?;
        let mut session = mutex
            .lock()
            .map_err(|e| format!("Head '{key}' lock error: {e}"))?;

        let emb_tensor = Tensor::from_array(([n_patches, EMBEDDING_DIM], embeddings.clone()))
            .map_err(|e| format!("Failed to create embedding tensor: {e}"))?;

        let out = session
            .run(inputs![input_node => emb_tensor])
            .map_err(|e| format!("Head '{key}' inference failed: {e}"))?;

        let prob_tensor = out
            .get(output_node)
            .ok_or_else(|| format!("Head '{key}' missing output '{output_node}'"))?;

        let (shape_cow, prob_data) = prob_tensor
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Failed to extract head '{key}' output: {e}"))?;

        let shape = &**shape_cow;
        if shape.len() != 2 || shape[0] as usize != n_patches {
            return Err(format!("Unexpected head '{key}' output shape: {shape:?}"));
        }
        let n_classes = shape[1] as usize;

        let mut avg = vec![0.0f32; n_classes];
        for p in 0..n_patches {
            for c in 0..n_classes {
                avg[c] += prob_data[p * n_classes + c];
            }
        }
        for v in avg.iter_mut() {
            *v /= n_patches as f32;
        }
        Ok(avg)
    };

    // ── 4. Genre (Discogs-400) ────────────────────────────────────────────────
    let genre_probs = run_head(
        "genre_discogs400",
        "serving_default_model_Placeholder:0",
        "PartitionedCall:0",
    )?;
    let genre_labels = load_labels("genre_discogs400", app)?;
    let best_genre = genre_probs
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .and_then(|(i, _)| genre_labels.get(i).cloned());

    // ── 5. Voice / Instrumental ───────────────────────────────────────────────
    let vi_probs = run_head("voice_instrumental", "embeddings", "activations")?;
    let vi_labels = load_labels("voice_instrumental", app)?;
    let (best_vocal, vocal_confidence) = if vi_probs.len() >= 2 && vi_labels.len() >= 2 {
        let idx = if vi_probs[0] > vi_probs[1] { 0 } else { 1 };
        (
            Some(vi_labels[idx].to_lowercase()),
            Some((vi_probs[idx] as f64 * 10_000.0).round() / 10_000.0),
        )
    } else {
        (None, None)
    };

    // ── 6. Mood scores ────────────────────────────────────────────────────────
    let binary_mood = |key: &str, target: &str| -> Option<f64> {
        let probs = run_head(key, "embeddings", "activations").ok()?;
        let labels = load_labels(key, app).ok()?;
        let idx = labels.iter().position(|l| l == target)?;
        probs
            .get(idx)
            .map(|&v| (v as f64 * 10_000.0).round() / 10_000.0)
    };

    Ok(ClassifierResult {
        genre: best_genre,
        vocal: best_vocal,
        vocal_confidence,
        mood_happy: binary_mood("mood_happy", "happy"),
        mood_sad: binary_mood("mood_sad", "sad"),
        mood_aggressive: binary_mood("mood_aggressive", "aggressive"),
        mood_relaxed: binary_mood("mood_relaxed", "relaxed"),
        mood_party: binary_mood("mood_party", "party"),
        mood_acoustic: binary_mood("mood_acoustic", "acoustic"),
        mood_electronic: binary_mood("mood_electronic", "electronic"),
    })
}
