# Strategy Specification: 3-Window Mean Pooling for CLAP Quality Improvement
### Enhancing Acoustic Similarity & UMAP Clustering Robustness

This document is a standalone technical specification for upgrading the `clap` embedding quality in the `deep-cuts` application. It details the transition from a single 10-second center window to a **3-Window Temporal Mean Pooling** architecture, providing the core rationale, mathematical formulas, and the Rust code blueprint.

---

## 1. Rationale: The "Single Window" Limitation

Currently, the CLAP analyzer extracts a single 10-second window centered exactly at the midpoint of a song. While this is a fast and simple heuristic, it suffers from significant structural limitations:

1. **Structural Blindspots**: A song is a dynamic story. The exact midpoint might land on an instrumental breakdown, a quiet vocal bridge, a transition, or even silence, mischaracterizing the entire track's acoustic signature.
2. **Acoustic Drift**: High-energy dance tracks might have slow, ambient intros or long minimal bridges. A single window cannot capture both the energetic chorus and the progressive instrumentation.
3. **Misleading Cluster Placements**: If the midpoint of an energetic electronic song is a quiet breakdown, the UMAP algorithm will plot it in the "Ambient / Down-tempo" cluster, leading to inaccurate UMAP projections and misleading KNN similarity recommendations.

### The Solution: 3-Window Temporal Mean Pooling
By extracting and analyzing **three separate 10-second windows** across the song's duration (at **25%**, **50%**, and **75%** marks) and blending their embeddings, we capture a much more representative, comprehensive, and robust acoustic fingerprint of the entire track.

---

## 2. Mathematical Design

For any given song, the pipeline generates a single, unified 512-dimensional embedding vector $V_{\text{final}}$ by combining the embeddings of the three target windows:

```
[Song Timeline]
0% ----------- 25% (V1) ----------- 50% (V2) ----------- 75% (V3) ----------- 100%
                |                   |                   |
           Inference 1         Inference 2         Inference 3
                |                   |                   |
            [512-d V1]          [512-d V2]          [512-d V3]
                \                   |                   /
                 \                  |                  /
                  Element-wise Arithmetic Mean (V_mean)
                                    |
                            L2 Normalization
                                    |
                               [512-d V_final]
```

### The Formulas:

1. **Acoustic Inference**:
   Run the CLAP encoder on each resampled window to obtain three 512-dimensional vectors:
   $$V_1, V_2, V_3 \in \mathbb{R}^{512}$$

2. **Mean Pooling (Element-wise Average)**:
   Combine the vectors by taking their arithmetic mean:
   $$V_{\text{mean}} = \frac{V_1 + V_2 + V_3}{3}$$

3. **L2-Normalization (Re-normalization)**:
   Since vector similarity metrics in `sqlite-vec` (specifically cosine similarity and L2 Euclidean distance) expect unit vectors to guarantee accurate distance calculations, we **must re-normalize** the averaged vector back to unit length:
   $$V_{\text{final}} = \frac{V_{\text{mean}}}{\|V_{\text{mean}}\|_2} = \frac{V_{\text{mean}}}{\sqrt{\sum_{i=1}^{512} (V_{\text{mean}, i})^2}}$$

---

## 3. Seamless Backward Compatibility

An elegant feature of the **3-Window Mean Pooling** strategy is **100% backward compatibility**:
* **Database Compatibility**: The output vector $V_{\text{final}}$ is still exactly a single 512-dimensional, L2-normalized vector. It fits directly into the existing `audio_embeddings` SQLite virtual table without *any* changes to the schema.
* **UI & UMAP Compatibility**: Since every song still maps to a single 512-d vector, the UMAP coordinate calculation and the Svelte canvas drawing loop require zero modifications. 
* **Performance Synergy**: By combining this strategy with the **Container-Level Seeking** optimization, the three decodes (at 25%, 50%, and 75%) are executed in milliseconds. The only added cost is the extra ONNX passes.

---

## 4. Technical Rust Blueprint

### Refactoring `src-tauri/src/embeddings.rs`
We implement the 3-window extraction and vector pooling math directly inside `embeddings.rs`:

```rust
/// Helper to extract, seek-decode, and preprocess a specific window at a percentage offset
pub fn preprocess_window_at_pct(
    path: &str,
    pct: f64,
    app: Option<&tauri::AppHandle>,
) -> Result<Vec<f32>, String> {
    // 1. Decode with seeking to target percentage offset
    let (audio, sample_rate) = crate::dsp::decode_audio_at_percentage_with_seeking(path, pct)?;
    let audio_48k = resample_audio(&audio, sample_rate, CLAP_SR)?;

    // 2. Extract 10-second slice starting at sought position
    let end = CLAP_10S_SAMPLES.min(audio_48k.len());
    let mut window = audio_48k[..end].to_vec();

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

/// Computes the high-quality, mean-pooled 512-d embedding across 3 windows
pub fn run_clap_audio_embed_pooled_3_windows(
    path: &str,
    app: Option<&tauri::AppHandle>,
) -> Result<Vec<f32>, String> {
    // 1. Compute 3 independent mel spectrograms in parallel (at 25%, 50%, and 75%)
    let mel_25 = preprocess_window_at_pct(path, 0.25, app)?;
    let mel_50 = preprocess_window_at_pct(path, 0.50, app)?;
    let mel_75 = preprocess_window_at_pct(path, 0.75, app)?;

    // 2. Run sequential model inferences (NPU offloaded or CPU parallel)
    let v1 = run_clap_inference_only(mel_25)?;
    let v2 = run_clap_inference_only(mel_50)?;
    let v3 = run_clap_inference_only(mel_75)?;

    // 3. Compute arithmetic mean: V_mean = (V1 + V2 + V3) / 3
    let mut v_mean = vec![0.0f32; 512];
    for i in 0..512 {
        v_mean[i] = (v1[i] + v2[i] + v3[i]) / 3.0;
    }

    // 4. Compute L2 Norm: ||V_mean||
    let sum_squares: f32 = v_mean.iter().map(|&x| x * x).sum();
    let l2_norm = sum_squares.sqrt();

    // 5. L2 Re-normalization: V_final = V_mean / ||V_mean||
    let v_final: Vec<f32> = if l2_norm > 1e-8 {
        v_mean.iter().map(|&x| x / l2_norm).collect()
    } else {
        // Fallback in case of division by zero
        v_mean
    };

    Ok(v_final)
}
```

This 3-window mean-pooling specification guarantees a highly representative acoustic analysis that eliminates midpoint anomalies, significantly sharpening the accuracy of similarity recommendations and UMAP clusters.
