use rusqlite::Connection;
use std::sync::Mutex;
use faer::Mat;

const DESCRIPTION_EMBEDDING_DIM: usize = 384;

/// @concept SpectralMap
/// @skill add-ipc-command
/// MappedTrackPoint represents a 2D-projected audio track with metadata and classifier features.
#[derive(serde::Serialize)]
pub struct MappedTrackPoint {
    pub id: i64,
    pub x: f64,
    pub y: f64,
    pub watched_directory_id: i64,
    pub title: Option<String>,
    pub filename: String,
    pub artist: Option<String>,
    pub genre: Option<String>,
    pub bpm: Option<f64>,
    pub key: Option<String>,
    pub scale: Option<String>,
    pub algorithm: Option<String>,
    pub mood_happy: Option<f64>,
    pub mood_sad: Option<f64>,
    pub mood_aggressive: Option<f64>,
    pub mood_relaxed: Option<f64>,
    pub mood_party: Option<f64>,
    pub mood_acoustic: Option<f64>,
    pub mood_electronic: Option<f64>,
    pub structure_cluster_id: Option<i64>,
}

#[derive(serde::Serialize)]
pub struct AudioSimilarityResult {
    pub id: i64,
    pub distance: f64,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub bpm: Option<f64>,
    pub key: Option<String>,
    pub scale: Option<String>,
}

fn bytes_to_floats(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

fn l2_normalize(vec: &[f32]) -> Vec<f32> {
    let norm = vec.iter().map(|&x| x * x).sum::<f32>().sqrt();
    if norm == 0.0 {
        vec.to_vec()
    } else {
        vec.iter().map(|&x| x / norm).collect()
    }
}

fn l2_distance_sq(a: &[f32], b: &[f32]) -> Option<f64> {
    if a.len() != b.len() {
        return None;
    }
    Some(
        a.iter()
            .zip(b.iter())
            .map(|(&x, &y)| {
                let delta = x as f64 - y as f64;
                delta * delta
            })
            .sum(),
    )
}

fn blended_embedding_distance(
    seed_clap: &[f32],
    seed_description: Option<&[f32]>,
    seed_sax: Option<&str>,
    candidate_clap: &[f32],
    candidate_description: Option<&[f32]>,
    candidate_sax: Option<&str>,
    clap_weight: f64,
) -> Option<f64> {
    let norm_seed_clap = l2_normalize(seed_clap);
    let norm_candidate_clap = l2_normalize(candidate_clap);
    let clap_distance_sq = l2_distance_sq(&norm_seed_clap, &norm_candidate_clap)?;

    // SAX structural distance — blended in with a fixed 15% weight when both strings are present.
    // This is additive on top of the acoustic distance so it doesn't dominate.
    let sax_distance_sq = match (seed_sax, candidate_sax) {
        (Some(a), Some(b)) => {
            crate::analysis::sax::sax_mindist(a, b).map(|d| d * d).unwrap_or(0.0)
        }
        _ => 0.0,
    };
    const SAX_WEIGHT: f64 = 0.15;
    let acoustic_weight = 1.0 - SAX_WEIGHT;

    let base_distance_sq = acoustic_weight * acoustic_weight * clap_distance_sq
        + SAX_WEIGHT * SAX_WEIGHT * sax_distance_sq;

    if let (Some(seed_description), Some(candidate_description)) =
        (seed_description, candidate_description)
    {
        let norm_seed_description = l2_normalize(seed_description);
        let norm_candidate_description = l2_normalize(candidate_description);
        if let Some(description_distance_sq) =
            l2_distance_sq(&norm_seed_description, &norm_candidate_description)
        {
            let description_weight = 1.0 - clap_weight;
            let acoustic_clap_weight = clap_weight * acoustic_weight;
            return Some(
                ((acoustic_clap_weight * acoustic_clap_weight * clap_distance_sq)
                    + (SAX_WEIGHT * SAX_WEIGHT * sax_distance_sq)
                    + (description_weight * description_weight * description_distance_sq))
                    .sqrt(),
            );
        }
    }

    Some(base_distance_sq.sqrt())
}

fn blended_projection_vector(
    clap: &[f32],
    description: Option<&[f32]>,
    clap_weight: f64,
) -> Vec<f32> {
    let norm_clap = l2_normalize(clap);
    let norm_description = description
        .map(l2_normalize)
        .unwrap_or_else(|| vec![0.0; DESCRIPTION_EMBEDDING_DIM]);
    let description_weight = 1.0 - clap_weight;

    let mut vec = Vec::with_capacity(norm_clap.len() + norm_description.len());
    for &x in &norm_clap {
        vec.push(x * clap_weight as f32);
    }
    for &x in &norm_description {
        vec.push(x * description_weight as f32);
    }
    vec
}

fn compute_pca_2d(blended_vectors: &[Vec<f32>]) -> Result<Vec<(f64, f64)>, String> {
    let n = blended_vectors.len();
    if n == 0 {
        return Ok(Vec::new());
    }
    let d = blended_vectors[0].len();

    // 1. Calculate column means
    let mut column_means = vec![0.0f64; d];
    for row in blended_vectors {
        for (j, &val) in row.iter().enumerate() {
            column_means[j] += val as f64;
        }
    }
    for mean in &mut column_means {
        *mean /= n as f64;
    }

    // 2. Build centered matrix Mat<f64>
    let mat = Mat::from_fn(n, d, |i, j| {
        blended_vectors[i][j] as f64 - column_means[j]
    });

    // 3. Compute Thin SVD
    let svd = mat.as_ref().thin_svd()
        .map_err(|e| format!("SVD projection failed to converge: {:?}", e))?;

    // 4. Projection
    let u = svd.U();
    let s = svd.S().column_vector();

    let s_vals: Vec<f64> = s.iter().copied().collect();
    if s_vals.len() < 2 {
        return Err("Not enough dimensions or singular values for 2D PCA projection.".to_string());
    }

    let s0 = s_vals[0];
    let s1 = s_vals[1];

    let col0: Vec<f64> = u.col(0).iter().copied().collect();
    let col1: Vec<f64> = u.col(1).iter().copied().collect();

    let projected: Vec<(f64, f64)> = (0..n)
        .map(|i| (col0[i] * s0, col1[i] * s1))
        .collect();

    Ok(projected)
}

#[derive(Debug, PartialEq)]
struct EffectiveProjectionConfig {
    clap_weight: f64,
    algorithm: String,
}

fn effective_projection_config(
    clap_weight: Option<f64>,
    algorithm: &str,
    n_neighbors: i32,
    min_dist: f64,
    perplexity: f64,
) -> EffectiveProjectionConfig {
    // UMAP parameters n_neighbors, min_dist, perplexity are ignored by rag-umap for now
    let _ = (n_neighbors, min_dist, perplexity);
    EffectiveProjectionConfig {
        clap_weight: clap_weight.unwrap_or(0.5),
        algorithm: algorithm.to_string(),
    }
}

/// Returns the value at percentile `p` (0–100) of a pre-sorted slice.
fn percentile_value(sorted: &[f64], p: f64) -> f64 {
    let idx = ((sorted.len() - 1) as f64 * p / 100.0).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

/// Normalises raw UMAP coordinates into the [0, 100] canvas range using
/// percentile-clipped (p1–p99) scaling. The 1st and 99th percentiles
/// anchor the scale so that extreme outliers are clamped to the canvas
/// edges rather than compressing the dense cluster into a small region.
fn standardize_to_100(coords: &[(f64, f64)]) -> Vec<(f64, f64)> {
    if coords.is_empty() {
        return Vec::new();
    }
    let mut xs: Vec<f64> = coords.iter().map(|p| p.0).collect();
    let mut ys: Vec<f64> = coords.iter().map(|p| p.1).collect();
    xs.sort_by(|a, b| a.total_cmp(b));
    ys.sort_by(|a, b| a.total_cmp(b));

    let x_lo = percentile_value(&xs, 1.0);
    let x_hi = percentile_value(&xs, 99.0);
    let y_lo = percentile_value(&ys, 1.0);
    let y_hi = percentile_value(&ys, 99.0);

    let x_span = if x_hi == x_lo { 1.0 } else { x_hi - x_lo };
    let y_span = if y_hi == y_lo { 1.0 } else { y_hi - y_lo };

    coords
        .iter()
        .map(|&(x, y)| {
            let nx = ((x - x_lo) / x_span * 100.0).clamp(0.0, 100.0);
            let ny = ((y - y_lo) / y_span * 100.0).clamp(0.0, 100.0);
            (nx, ny)
        })
        .collect()
}

/// Returns the stored 2D UMAP coordinates joined with basic track metadata.
/// When `music_only` is true, tracks classified as Non-Music by Essentia are excluded,
/// matching the frontend `musicOnly` filter signal.
#[tauri::command]
pub fn get_projection_coordinates(
    music_only: bool,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<Vec<MappedTrackPoint>, String> {
    let conn = conn_state.lock().map_err(|e| e.to_string())?;
    let sql = if music_only {
        "SELECT tc.track_id, tc.x, tc.y,
                t.watched_directory_id, t.title, t.filename, t.artist,
                COALESCE(t.detected_genre, t.genre), t.bpm, t.key, t.scale, tc.algorithm,
                t.mood_happy, t.mood_sad, t.mood_aggressive, t.mood_relaxed, t.mood_party, t.mood_acoustic, t.mood_electronic,
                t.structure_cluster_id
         FROM track_coords tc
         JOIN tracks t ON t.id = tc.track_id
         WHERE (t.detected_genre IS NULL OR t.detected_genre NOT LIKE 'Non-Music%')"
    } else {
        "SELECT tc.track_id, tc.x, tc.y,
                t.watched_directory_id, t.title, t.filename, t.artist,
                COALESCE(t.detected_genre, t.genre), t.bpm, t.key, t.scale, tc.algorithm,
                t.mood_happy, t.mood_sad, t.mood_aggressive, t.mood_relaxed, t.mood_party, t.mood_acoustic, t.mood_electronic,
                t.structure_cluster_id
         FROM track_coords tc
         JOIN tracks t ON t.id = tc.track_id"
    };
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(MappedTrackPoint {
                id: row.get(0)?,
                x: row.get(1)?,
                y: row.get(2)?,
                watched_directory_id: row.get(3)?,
                title: row.get(4)?,
                filename: row.get(5)?,
                artist: row.get(6)?,
                genre: row.get(7)?,
                bpm: row.get(8)?,
                key: row.get(9)?,
                scale: row.get(10)?,
                algorithm: row.get(11)?,
                mood_happy: row.get(12)?,
                mood_sad: row.get(13)?,
                mood_aggressive: row.get(14)?,
                mood_relaxed: row.get(15)?,
                mood_party: row.get(16)?,
                mood_acoustic: row.get(17)?,
                mood_electronic: row.get(18)?,
                structure_cluster_id: row.get(19)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.map(|r| r.map_err(|e| e.to_string())).collect()
}

/// KNN similarity search: given a seed track_id, returns the N nearest tracks
/// by blended acoustic/semantic embedding distance where semantic embeddings exist.
#[tauri::command]
pub fn search_similar_tracks_audio(
    track_id: i64,
    directory_id: Option<i64>,
    clap_weight: Option<f64>,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<Vec<AudioSimilarityResult>, String> {
    let conn = conn_state.lock().map_err(|e| e.to_string())?;
    let blend_weight = clap_weight.unwrap_or(0.5);

    let (seed_clap_blob, seed_description_blob, seed_sax): (Vec<u8>, Option<Vec<u8>>, Option<String>) = conn
        .query_row(
            "SELECT ae.embedding, de.embedding, t.waveform_sax
             FROM audio_embeddings ae
             JOIN tracks t ON t.id = ae.track_id
             LEFT JOIN description_embeddings de ON de.track_id = ae.track_id
             WHERE ae.track_id = ?1",
            [track_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|_| "Track has no CLAP embedding yet — run analysis first.".to_string())?;
    let seed_clap = bytes_to_floats(&seed_clap_blob);
    let seed_description = seed_description_blob
        .as_ref()
        .map(|blob| bytes_to_floats(blob));

    let mut rows: Vec<AudioSimilarityResult> = if let Some(dir_id) = directory_id {
        let mut stmt = conn
            .prepare(
                "SELECT t.id, ae.embedding, de.embedding, t.title, t.artist, t.bpm, t.key, t.scale, t.waveform_sax
                 FROM tracks t
                 JOIN audio_embeddings ae ON ae.track_id = t.id
                 LEFT JOIN description_embeddings de ON de.track_id = t.id
                 WHERE t.watched_directory_id = ?1 AND t.id != ?2",
            )
            .map_err(|e| e.to_string())?;
        let mapped = stmt
            .query_map(rusqlite::params![dir_id, track_id], |row| {
                let candidate_clap_blob: Vec<u8> = row.get(1)?;
                let candidate_description_blob: Option<Vec<u8>> = row.get(2)?;
                let candidate_sax: Option<String> = row.get(8)?;
                let candidate_clap = bytes_to_floats(&candidate_clap_blob);
                let candidate_description = candidate_description_blob
                    .as_ref()
                    .map(|blob| bytes_to_floats(blob));
                let distance = blended_embedding_distance(
                    &seed_clap,
                    seed_description.as_deref(),
                    seed_sax.as_deref(),
                    &candidate_clap,
                    candidate_description.as_deref(),
                    candidate_sax.as_deref(),
                    blend_weight,
                )
                .unwrap_or(f64::INFINITY);
                Ok(AudioSimilarityResult {
                    id: row.get(0)?,
                    distance,
                    title: row.get(3)?,
                    artist: row.get(4)?,
                    bpm: row.get(5)?,
                    key: row.get(6)?,
                    scale: row.get(7)?,
                })
            })
            .map_err(|e| e.to_string())?;
        mapped
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|r| r.distance.is_finite())
            .collect()
    } else {
        let mut stmt = conn
            .prepare(
                "SELECT t.id, ae.embedding, de.embedding, t.title, t.artist, t.bpm, t.key, t.scale, t.waveform_sax
                 FROM tracks t
                 JOIN audio_embeddings ae ON ae.track_id = t.id
                 LEFT JOIN description_embeddings de ON de.track_id = t.id
                 WHERE t.id != ?1",
            )
            .map_err(|e| e.to_string())?;
        let mapped = stmt
            .query_map([track_id], |row| {
                let candidate_clap_blob: Vec<u8> = row.get(1)?;
                let candidate_description_blob: Option<Vec<u8>> = row.get(2)?;
                let candidate_sax: Option<String> = row.get(8)?;
                let candidate_clap = bytes_to_floats(&candidate_clap_blob);
                let candidate_description = candidate_description_blob
                    .as_ref()
                    .map(|blob| bytes_to_floats(blob));
                let distance = blended_embedding_distance(
                    &seed_clap,
                    seed_description.as_deref(),
                    seed_sax.as_deref(),
                    &candidate_clap,
                    candidate_description.as_deref(),
                    candidate_sax.as_deref(),
                    blend_weight,
                )
                .unwrap_or(f64::INFINITY);
                Ok(AudioSimilarityResult {
                    id: row.get(0)?,
                    distance,
                    title: row.get(3)?,
                    artist: row.get(4)?,
                    bpm: row.get(5)?,
                    key: row.get(6)?,
                    scale: row.get(7)?,
                })
            })
            .map_err(|e| e.to_string())?;
        mapped
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|r| r.distance.is_finite())
            .collect()
    };

    rows.sort_by(|a, b| {
        a.distance
            .partial_cmp(&b.distance)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });
    rows.truncate(50);
    Ok(rows)
}

#[derive(serde::Serialize)]
pub struct DuplicatePair {
    pub id_a: i64,
    pub id_b: i64,
    pub title_a: Option<String>,
    pub title_b: Option<String>,
    pub artist_a: Option<String>,
    pub artist_b: Option<String>,
    pub filename_a: String,
    pub filename_b: String,
    pub path_a: String,
    pub path_b: String,
    pub distance: f64,
}

/// Computes pairwise CLAP cosine similarity across all analysed tracks and returns
/// pairs whose L2 distance is at or below `threshold`, sorted ascending by distance.
///
/// For L2-normalised vectors: dist(i,j) = sqrt(2 − 2·dot(i,j)).
/// Complexity: O(n² · dim) time, O(n · dim) memory for the normalised matrix.
/// Emits `duplicate-scan-progress` per row-block and `duplicate-scan-done` on finish.
#[tauri::command]
pub async fn find_duplicate_pairs(
    threshold: f64,
    app: tauri::AppHandle,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<Vec<DuplicatePair>, String> {
    use tauri::Emitter;

    // Collect raw rows (id, title, artist, filename, path, blob) while holding the lock,
    // then decode blobs after releasing it so the MutexGuard doesn't cross an await.
    type RawRow = (i64, Option<String>, Option<String>, String, String, Vec<u8>);
    let raw_rows: Vec<RawRow> = {
        let conn = conn_state.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT t.id, t.title, t.artist, t.filename, t.path, ae.embedding
                 FROM tracks t
                 JOIN audio_embeddings ae ON ae.track_id = t.id",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Vec<u8>>(5)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?
    };

    struct TrackEmb {
        id: i64,
        title: Option<String>,
        artist: Option<String>,
        filename: String,
        path: String,
        clap: Vec<f32>,
    }

    let tracks: Vec<TrackEmb> = raw_rows
        .into_iter()
        .map(|(id, title, artist, filename, path, blob)| TrackEmb {
            id,
            title,
            artist,
            filename,
            path,
            clap: bytes_to_floats(&blob),
        })
        .filter(|t| !t.clap.is_empty())
        .collect();

    let n = tracks.len();
    if n == 0 {
        return Ok(Vec::new());
    }

    app.emit("duplicate-scan-progress", serde_json::json!({ "stage": "normalising", "n": n })).ok();

    // Build L2-normalised matrix A (n × dim, f64).
    // Peak memory: n × dim × 8 bytes (10K tracks × 512 dim ≈ 40 MB).
    let dim = tracks[0].clap.len();
    let a = Mat::<f64>::from_fn(n, dim, |i, j| {
        let row = &tracks[i].clap;
        let norm: f32 = row.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm == 0.0 { 0.0 } else { (row[j] / norm) as f64 }
    });

    // Blocked matmul: each chunk computes sim_chunk = A[chunk] @ Aᵀ (faer SIMD path).
    // For L2-normalised vectors: dist(i,j) = sqrt(2 − 2·cosim).
    // BLOCK=512 keeps the sim chunk at ≤ 512 × n × 8 bytes ≈ 40 MB at n=10K.
    const BLOCK: usize = 512;
    let threshold_sq = (threshold * threshold).min(2.0);
    let mut pairs: Vec<DuplicatePair> = Vec::new();

    for chunk_start in (0..n).step_by(BLOCK) {
        let chunk_end = (chunk_start + BLOCK).min(n);
        let chunk_rows = chunk_end - chunk_start;

        // sim_chunk[ci, j] = dot(A[chunk_start+ci], A[j])  shape: chunk_rows × n
        let sim_chunk = a.as_ref().subrows(chunk_start, chunk_rows) * a.as_ref().transpose();

        for ci in 0..chunk_rows {
            let i = chunk_start + ci;
            for j in (i + 1)..n {
                let cosim = sim_chunk[(ci, j)].clamp(-1.0, 1.0);
                let dist_sq = (2.0 - 2.0 * cosim).max(0.0);
                if dist_sq <= threshold_sq {
                    pairs.push(DuplicatePair {
                        id_a: tracks[i].id,
                        id_b: tracks[j].id,
                        title_a: tracks[i].title.clone(),
                        title_b: tracks[j].title.clone(),
                        artist_a: tracks[i].artist.clone(),
                        artist_b: tracks[j].artist.clone(),
                        filename_a: tracks[i].filename.clone(),
                        filename_b: tracks[j].filename.clone(),
                        path_a: tracks[i].path.clone(),
                        path_b: tracks[j].path.clone(),
                        distance: dist_sq.sqrt(),
                    });
                }
            }
        }
        app.emit("duplicate-scan-progress", serde_json::json!({
            "stage": "computing",
            "done": chunk_end,
            "total": n,
        })).ok();
    }

    pairs.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));
    app.emit("duplicate-scan-done", serde_json::json!({ "count": pairs.len() })).ok();
    Ok(pairs)
}


// ── Harmonic Circle of Fifths and Spring Layout Helpers ──────────────────────

struct SpringNode {
    anchor_x: f64,
    anchor_y: f64,
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
}

fn key_to_camelot(key: &str, scale: &str) -> Option<(u32, bool)> {
    let is_minor = scale.trim().to_lowercase() == "minor" || key.ends_with('m');
    let base_key = key.trim().trim_end_matches('m').trim_end_matches("Min").trim_end_matches("Maj").trim();

    let norm_key = match base_key {
        "C" => "C",
        "C#" | "Db" => "C#",
        "D" => "D",
        "D#" | "Eb" => "Eb",
        "E" => "E",
        "F" => "F",
        "F#" | "Gb" => "F#",
        "G" => "G",
        "G#" | "Ab" => "Ab",
        "A" => "A",
        "A#" | "Bb" => "Bb",
        "B" | "Cb" => "B",
        _ => return None,
    };

    if is_minor {
        let hour = match norm_key {
            "Ab" => 1,
            "Eb" => 2,
            "Bb" => 3,
            "F"  => 4,
            "C"  => 5,
            "G"  => 6,
            "D"  => 7,
            "A"  => 8,
            "E"  => 9,
            "B"  => 10,
            "F#" => 11,
            "C#" => 12,
            _ => return None,
        };
        Some((hour, true))
    } else {
        let hour = match norm_key {
            "B"  => 1,
            "F#" => 2,
            "C#" => 3,
            "Ab" => 4,
            "Eb" => 5,
            "Bb" => 6,
            "F"  => 7,
            "C"  => 8,
            "G"  => 9,
            "D"  => 10,
            "A"  => 11,
            "E"  => 12,
            _ => return None,
        };
        Some((hour, false))
    }
}

fn deterministic_genre_jitter(genre: &str) -> (f64, f64) {
    if genre.trim().is_empty() {
        return (0.0, 0.0);
    }
    let mut hash: u32 = 5381;
    for c in genre.bytes() {
        hash = ((hash << 5).wrapping_add(hash)).wrapping_add(c as u32);
    }
    let val_angle = ((hash & 0xFFFF) as f64 / 65535.0) * 2.0 - 1.0;
    let val_radial = (((hash >> 16) & 0xFFFF) as f64 / 65535.0) * 2.0 - 1.0;
    
    (val_angle * 0.04, val_radial * 1.2)
}

fn run_spring_layout(nodes: &mut [SpringNode], iterations: usize) {
    let k_spring = 0.32;
    let k_repulsion = 0.8;
    let damping = 0.70;
    let min_dist_threshold = 1.6;

    for _ in 0..iterations {
        let mut fx = vec![0.0; nodes.len()];
        let mut fy = vec![0.0; nodes.len()];

        for i in 0..nodes.len() {
            let ax = nodes[i].anchor_x - nodes[i].x;
            let ay = nodes[i].anchor_y - nodes[i].y;
            fx[i] += ax * k_spring;
            fy[i] += ay * k_spring;

            for j in 0..nodes.len() {
                if i == j {
                    continue;
                }
                let mut dx = nodes[i].x - nodes[j].x;
                let mut dy = nodes[i].y - nodes[j].y;
                let mut dist_sq = dx * dx + dy * dy;
                
                if dist_sq < 1e-4 {
                    dx = 0.1 * (i as f64 - j as f64).signum();
                    dy = 0.1;
                    dist_sq = dx * dx + dy * dy;
                }
                let dist = dist_sq.sqrt();
                if dist < min_dist_threshold {
                    let force = k_repulsion / (dist_sq + 0.1);
                    fx[i] += (dx / dist) * force;
                    fy[i] += (dy / dist) * force;
                }
            }
        }

        for i in 0..nodes.len() {
            nodes[i].vx = (nodes[i].vx + fx[i]) * damping;
            nodes[i].vy = (nodes[i].vy + fy[i]) * damping;
            nodes[i].x += nodes[i].vx;
            nodes[i].y += nodes[i].vy;
            nodes[i].x = nodes[i].x.clamp(2.0, 98.0);
            nodes[i].y = nodes[i].y.clamp(2.0, 98.0);
        }
    }
}

fn compute_harmonic_layout(
    conn: &Connection,
    music_only: bool,
) -> Result<(Vec<i64>, Vec<(f64, f64)>), String> {
    let sql = if music_only {
        "SELECT id, COALESCE(detected_genre, genre), bpm, key, scale FROM tracks
         WHERE (detected_genre IS NULL OR detected_genre NOT LIKE 'Non-Music%')"
    } else {
        "SELECT id, COALESCE(detected_genre, genre), bpm, key, scale FROM tracks"
    };
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            row.get::<_, Option<f64>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    }).map_err(|e| e.to_string())?;

    let mut ids = Vec::new();
    let mut genres = Vec::new();
    let mut bpms: Vec<Option<f64>> = Vec::new();
    let mut keys: Vec<Option<String>> = Vec::new();
    let mut scales: Vec<Option<String>> = Vec::new();
    for r in rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())? {
        ids.push(r.0); genres.push(r.1); bpms.push(r.2); keys.push(r.3); scales.push(r.4);
    }

    if ids.is_empty() {
        return Err("No tracks found to compute projection.".to_string());
    }

    let mut nodes: Vec<SpringNode> = Vec::with_capacity(ids.len());
    for i in 0..ids.len() {
        let bpm_val = bpms[i].unwrap_or(120.0);
        let radial_base = 20.0 + ((bpm_val.clamp(50.0, 200.0) - 50.0) / 150.0) * 26.0;

        let (theta_base, radial_mode) = if let (Some(k), Some(s)) = (&keys[i], &scales[i]) {
            if let Some((hour, is_minor)) = key_to_camelot(k, s) {
                let rad_mode = if is_minor { -1.5 } else { 1.5 };
                (hour as f64 * (2.0 * std::f64::consts::PI / 12.0), rad_mode)
            } else {
                (0.0, 0.0)
            }
        } else {
            (0.0, 0.0)
        };

        let (jitter_theta, jitter_radial) = deterministic_genre_jitter(&genres[i]);
        let (init_x, init_y) = if keys[i].is_some() {
            let theta = theta_base + jitter_theta;
            let r = (radial_base + radial_mode + jitter_radial).clamp(5.0, 48.0);
            (50.0 + r * theta.cos(), 50.0 + r * theta.sin())
        } else {
            // Keyless tracks cluster at center shifted deterministic by genre
            let r = (jitter_radial.abs() + 2.0).min(6.0);
            (50.0 + r * jitter_theta.cos(), 50.0 + r * jitter_theta.sin())
        };

        nodes.push(SpringNode { anchor_x: init_x, anchor_y: init_y, x: init_x, y: init_y, vx: 0.0, vy: 0.0 });
    }

    run_spring_layout(&mut nodes, 4);
    Ok((ids, nodes.iter().map(|n| (n.x, n.y)).collect()))
}

fn compute_essentia_layout(
    conn: &Connection,
    music_only: bool,
) -> Result<(Vec<i64>, Vec<(f64, f64)>), String> {
    let sql = if music_only {
        "SELECT id, COALESCE(detected_genre, genre), mood_happy, mood_party, mood_electronic, mood_aggressive, mood_sad, mood_relaxed, mood_acoustic
         FROM tracks WHERE (detected_genre IS NULL OR detected_genre NOT LIKE 'Non-Music%')"
    } else {
        "SELECT id, COALESCE(detected_genre, genre), mood_happy, mood_party, mood_electronic, mood_aggressive, mood_sad, mood_relaxed, mood_acoustic
         FROM tracks"
    };
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| {
        let id: i64 = row.get(0)?;
        let genre: Option<String> = row.get(1)?;
        let happy:      f64 = row.get::<_, Option<f64>>(2)?.unwrap_or(0.0);
        let party:      f64 = row.get::<_, Option<f64>>(3)?.unwrap_or(0.0);
        let electronic: f64 = row.get::<_, Option<f64>>(4)?.unwrap_or(0.0);
        let aggressive: f64 = row.get::<_, Option<f64>>(5)?.unwrap_or(0.0);
        let sad:        f64 = row.get::<_, Option<f64>>(6)?.unwrap_or(0.0);
        let relaxed:    f64 = row.get::<_, Option<f64>>(7)?.unwrap_or(0.0);
        let acoustic:   f64 = row.get::<_, Option<f64>>(8)?.unwrap_or(0.0);
        Ok((id, genre.unwrap_or_default(), [happy, party, electronic, aggressive, sad, relaxed, acoustic]))
    }).map_err(|e| e.to_string())?;

    let mut ids = Vec::new();
    let mut genres = Vec::new();
    let mut moods: Vec<[f64; 7]> = Vec::new();
    for r in rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())? {
        ids.push(r.0); genres.push(r.1); moods.push(r.2);
    }

    if ids.is_empty() {
        return Err("No tracks found to compute projection.".to_string());
    }

    // Radar chart vertices: Happy at top (-pi/2), then clockwise
    let theta_step = 2.0 * std::f64::consts::PI / 7.0;
    let theta_start = -std::f64::consts::PI / 2.0;
    let angles: Vec<f64> = (0..7).map(|i| theta_start + i as f64 * theta_step).collect();

    let mut nodes: Vec<SpringNode> = Vec::with_capacity(ids.len());
    for i in 0..ids.len() {
        let mut sum_x = 0.0f64;
        let mut sum_y = 0.0f64;
        let mut sum_val = 0.0f64;
        for j in 0..7 {
            let val = moods[i][j];
            sum_x += val * angles[j].cos();
            sum_y += val * angles[j].sin();
            sum_val += val;
        }

        let (init_x, init_y) = if sum_val > 1e-5 {
            let centroid_r = ((sum_x / 7.0).powi(2) + (sum_y / 7.0).powi(2)).sqrt();
            let theta = (sum_y / 7.0).atan2(sum_x / 7.0);
            let (jitter_theta, jitter_radial) = deterministic_genre_jitter(&genres[i]);
            let r = (centroid_r * 110.0 + jitter_radial).clamp(1.5, 48.0);
            (50.0 + r * (theta + jitter_theta).cos(), 50.0 + r * (theta + jitter_theta).sin())
        } else {
            let (jitter_theta, jitter_radial) = deterministic_genre_jitter(&genres[i]);
            let r = (jitter_radial.abs() + 2.0).min(6.0);
            (50.0 + r * jitter_theta.cos(), 50.0 + r * jitter_theta.sin())
        };

        nodes.push(SpringNode { anchor_x: init_x, anchor_y: init_y, x: init_x, y: init_y, vx: 0.0, vy: 0.0 });
    }

    run_spring_layout(&mut nodes, 4);
    Ok((ids, nodes.iter().map(|n| (n.x, n.y)).collect()))
}

fn compute_genre_wheel_layout(
    conn: &Connection,
    music_only: bool,
) -> Result<(Vec<i64>, Vec<(f64, f64)>), String> {
    let sql = if music_only {
        "SELECT id, COALESCE(detected_genre, genre), bpm, key, scale FROM tracks
         WHERE (detected_genre IS NULL OR detected_genre NOT LIKE 'Non-Music%')"
    } else {
        "SELECT id, COALESCE(detected_genre, genre), bpm, key, scale FROM tracks"
    };
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            row.get::<_, Option<f64>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    }).map_err(|e| e.to_string())?;

    let mut ids = Vec::new();
    let mut genres = Vec::new();
    let mut bpms: Vec<Option<f64>> = Vec::new();
    let mut keys: Vec<Option<String>> = Vec::new();
    let mut scales: Vec<Option<String>> = Vec::new();
    for r in rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())? {
        ids.push(r.0); genres.push(r.1); bpms.push(r.2); keys.push(r.3); scales.push(r.4);
    }

    if ids.is_empty() {
        return Err("No tracks found to compute projection.".to_string());
    }

    let mut unique_genres: Vec<String> = genres.iter()
        .map(|g| g.trim().to_lowercase())
        .filter(|g| !g.is_empty())
        .collect();
    unique_genres.sort();
    unique_genres.dedup();
    let num_genres = unique_genres.len().max(1);

    let mut nodes: Vec<SpringNode> = Vec::with_capacity(ids.len());
    for i in 0..ids.len() {
        let norm_genre = genres[i].trim().to_lowercase();
        let genre_idx = unique_genres.binary_search(&norm_genre).unwrap_or(0);
        let theta_base = genre_idx as f64 * (2.0 * std::f64::consts::PI / num_genres as f64);

        let bpm_val = bpms[i].unwrap_or(120.0);
        let radial_base = 20.0 + ((bpm_val.clamp(50.0, 200.0) - 50.0) / 150.0) * 26.0;

        let (jitter_theta, jitter_radial) = if let (Some(k), Some(s)) = (&keys[i], &scales[i]) {
            if let Some((hour, is_minor)) = key_to_camelot(k, s) {
                let rad_mode = if is_minor { -1.0 } else { 1.0 };
                let theta_shift = (hour as f64 - 6.5) * 0.012;
                (theta_shift, rad_mode)
            } else {
                (0.0, 0.0)
            }
        } else {
            (0.0, 0.0)
        };

        let r = (radial_base + jitter_radial).clamp(5.0, 48.0);
        let theta = theta_base + jitter_theta;
        let init_x = 50.0 + r * theta.cos();
        let init_y = 50.0 + r * theta.sin();

        nodes.push(SpringNode { anchor_x: init_x, anchor_y: init_y, x: init_x, y: init_y, vx: 0.0, vy: 0.0 });
    }

    run_spring_layout(&mut nodes, 4);
    Ok((ids, nodes.iter().map(|n| (n.x, n.y)).collect()))
}

fn compute_hybrid_layout(
    conn: &Connection,
    music_only: bool,
    clap_weight: f64,
    algorithm: &str,
) -> Result<(Vec<i64>, Vec<(f64, f64)>), String> {
    let sql = if music_only {
        "SELECT t.id, ae.embedding, de.embedding
         FROM tracks t
         JOIN audio_embeddings ae ON ae.track_id = t.id
         LEFT JOIN description_embeddings de ON de.track_id = t.id
         WHERE (t.detected_genre IS NULL OR t.detected_genre NOT LIKE 'Non-Music%')"
    } else {
        "SELECT t.id, ae.embedding, de.embedding
         FROM tracks t
         JOIN audio_embeddings ae ON ae.track_id = t.id
         LEFT JOIN description_embeddings de ON de.track_id = t.id"
    };
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, Option<Vec<u8>>>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut ids = Vec::new();
    let mut blended_vectors = Vec::new();
    for (id, clap_blob, desc_blob_opt) in rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())? {
        let clap_embed = bytes_to_floats(&clap_blob);
        let desc_embed_opt = desc_blob_opt.map(|b| bytes_to_floats(&b));
        let blended = blended_projection_vector(&clap_embed, desc_embed_opt.as_deref(), clap_weight);
        ids.push(id);
        blended_vectors.push(blended);
    }

    if blended_vectors.is_empty() {
        return Err("No tracks with CLAP embeddings found. Run the analysis pipeline first.".to_string());
    }

    let n = blended_vectors.len();
    let coords: Vec<(f64, f64)> = if n < 4 {
        (0..n)
            .map(|i| {
                let x = if n > 1 { i as f64 / (n - 1) as f64 * 100.0 } else { 50.0 };
                (x, 50.0)
            })
            .collect()
    } else {
        match algorithm {
            "pca" => standardize_to_100(&compute_pca_2d(&blended_vectors)?),
            _ => {
                let raw = rag_umap::convert_to_2d(blended_vectors)
                    .map_err(|e| format!("UMAP projection failed: {:?}", e))?;
                standardize_to_100(&raw.iter().map(|v| (v[0] as f64, v[1] as f64)).collect::<Vec<_>>())
            }
        }
    };
    Ok((ids, coords))
}

/// Runs UMAP/PCA or Direct geometric mapping on tracks based on the chosen mode,
/// and persists the 2D coordinates in `track_coords`. Emits `projection-updated` when done.
#[tauri::command]
pub async fn recompute_projection(
    music_only: bool,
    clap_weight: Option<f64>,
    algorithm: String,
    n_neighbors: i32,
    min_dist: f64,
    perplexity: f64,
    projection_mode: Option<String>,
    app: tauri::AppHandle,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<usize, String> {
    use tauri::Emitter;
    let effective_config =
        effective_projection_config(clap_weight, &algorithm, n_neighbors, min_dist, perplexity);

    let mode = projection_mode.unwrap_or_else(|| "hybrid".to_string());

    let (track_ids, coords) = {
        let conn = conn_state.lock().map_err(|e| e.to_string())?;
        match mode.as_str() {
            "harmonic"    => compute_harmonic_layout(&conn, music_only)?,
            "essentia"    => compute_essentia_layout(&conn, music_only)?,
            "genre_wheel" => compute_genre_wheel_layout(&conn, music_only)?,
            _             => compute_hybrid_layout(&conn, music_only, effective_config.clap_weight, &effective_config.algorithm)?,
        }
    };

    // Persist inside a transaction, recording the music_only scope per row
    {
        let mut conn = conn_state.lock().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM track_coords", [])
            .map_err(|e| e.to_string())?;
        {
            let mut ins = tx
                .prepare(
                    "INSERT INTO track_coords (track_id, x, y, music_only, algorithm) VALUES (?1, ?2, ?3, ?4, ?5)",
                )
                .map_err(|e| e.to_string())?;
            let music_only_int: i64 = if music_only { 1 } else { 0 };
            for (i, &(x, y)) in coords.iter().enumerate() {
                ins.execute(rusqlite::params![
                    track_ids[i],
                    x,
                    y,
                    music_only_int,
                    effective_config.algorithm
                ])
                .map_err(|e| e.to_string())?;
            }
        }
        tx.commit().map_err(|e| e.to_string())?;
    }

    let _ = app.emit("projection-updated", ());
    Ok(coords.len())
}

#[cfg(test)]
mod math_tests {
    use super::*;

    #[test]
    fn test_bytes_to_floats() {
        let floats = vec![1.0f32, -2.5f32, 0.0f32];
        let bytes: Vec<u8> = floats.iter().flat_map(|&f| f.to_le_bytes()).collect();
        let converted = bytes_to_floats(&bytes);
        assert_eq!(converted, floats);
    }

    #[test]
    fn test_l2_normalize_standard() {
        let vec = vec![3.0f32, 4.0f32];
        let normalized = l2_normalize(&vec);
        assert!((normalized[0] - 0.6).abs() < 1e-5);
        assert!((normalized[1] - 0.8).abs() < 1e-5);
    }

    #[test]
    fn test_l2_normalize_zero_vector() {
        let vec = vec![0.0f32, 0.0f32];
        let normalized = l2_normalize(&vec);
        assert_eq!(normalized, vec![0.0f32, 0.0f32]);
    }

    #[test]
    fn test_standardize_to_100_empty() {
        let coords: Vec<(f64, f64)> = vec![];
        let standardized = standardize_to_100(&coords);
        assert!(standardized.is_empty());
    }

    #[test]
    fn test_standardize_to_100_scaling() {
        // With 2 points p1 == min and p99 == max, so behaviour matches old min/max.
        let coords = vec![(10.0, 5.0), (20.0, 15.0)];
        let standardized = standardize_to_100(&coords);
        assert_eq!(standardized[0], (0.0, 0.0));
        assert_eq!(standardized[1], (100.0, 100.0));
    }

    #[test]
    fn test_standardize_to_100_outliers_are_clamped() {
        // 100 points: 2 extreme outliers bracketing 98 points in [0, 97].
        // Under min/max the cluster maps to only ~50–55 % of the canvas.
        // Under p1-p99 the cluster should fill 0–100 and outliers clamp to edges.
        let mut coords = vec![(-1000.0_f64, -1000.0_f64), (1000.0, 1000.0)];
        for i in 0..98_i64 {
            coords.push((i as f64, i as f64));
        }
        let result = standardize_to_100(&coords);

        // Every output value must stay within [0, 100].
        for &(x, y) in &result {
            assert!((0.0..=100.0).contains(&x), "x={x} out of bounds");
            assert!((0.0..=100.0).contains(&y), "y={y} out of bounds");
        }

        // The top of the main cluster (input 97) should map to 100.0,
        // proving the cluster fills the canvas rather than being squashed.
        let top_cluster = result
            .iter()
            .zip(coords.iter())
            .find(|(_, &(cx, _))| cx == 97.0)
            .map(|(&(rx, _), _)| rx)
            .expect("point at x=97 must exist");
        assert!(
            (top_cluster - 100.0).abs() < 1e-9,
            "top of cluster should map to 100.0, got {top_cluster}"
        );
    }

    #[test]
    fn test_standardize_to_100_single_point() {
        let coords = vec![(10.0, 5.0)];
        let standardized = standardize_to_100(&coords);
        assert_eq!(standardized[0], (0.0, 0.0));
    }

    #[test]
    fn test_projection_request_parameters_are_currently_ignored_except_blend_weight() {
        let default_umap = effective_projection_config(Some(0.7), "umap", 20, 0.1, 30.0);
        let requested_tsne = effective_projection_config(Some(0.7), "tsne", 90, 0.8, 5.0);

        assert_eq!(default_umap.clap_weight, requested_tsne.clap_weight);
        assert_eq!(default_umap.algorithm, "umap");
        assert_eq!(requested_tsne.algorithm, "tsne");
        assert_eq!(default_umap.clap_weight, 0.7);
        assert_eq!(
            effective_projection_config(None, "pca", 5, 0.0, 100.0).clap_weight,
            0.5,
        );
    }

    #[test]
    fn test_compute_pca_2d_returns_two_dimensions() {
        let vectors = vec![
            vec![1.0, 2.0, 3.0, 4.0],
            vec![4.0, 3.0, 2.0, 1.0],
            vec![5.0, 6.0, 7.0, 8.0],
            vec![8.0, 7.0, 6.0, 5.0],
        ];
        let coords = compute_pca_2d(&vectors).unwrap();
        assert_eq!(coords.len(), 4);
        for &(x, y) in &coords {
            assert!(x.is_finite());
            assert!(y.is_finite());
        }
    }

    #[test]
    fn test_blended_embedding_distance_uses_description_when_available() {
        let seed_clap = vec![1.0, 0.0];
        let candidate_clap = vec![1.0, 0.0];
        let seed_description = vec![1.0, 0.0];
        let candidate_description = vec![0.0, 1.0];

        // No SAX strings — SAX contributes 0, so result driven by CLAP+description blend.
        let distance = blended_embedding_distance(
            &seed_clap,
            Some(&seed_description),
            None,
            &candidate_clap,
            Some(&candidate_description),
            None,
            0.5,
        )
        .unwrap();

        // clap identical → clap_distance_sq=0; description orthogonal → desc_distance_sq=2
        // acoustic_clap_weight = 0.5 * 0.85 = 0.425; description_weight = 0.5; SAX_WEIGHT = 0.15
        // sqrt(0 + 0 + 0.5^2 * 2) = sqrt(0.5) ≈ 0.70710678
        assert!((distance - 0.70710678118).abs() < 1e-4);
    }

    #[test]
    fn test_blended_embedding_distance_falls_back_to_clap_without_description_pair() {
        let seed_clap = vec![1.0, 0.0];
        let candidate_clap = vec![0.0, 1.0];
        let seed_description = vec![1.0, 0.0];

        // No SAX, no candidate description — falls back to base_distance_sq only.
        // clap orthogonal → clap_distance_sq = 2; acoustic_weight = 0.85
        // base_distance_sq = 0.85^2 * 2 = 1.445; sqrt ≈ 1.2021
        let distance = blended_embedding_distance(
            &seed_clap,
            Some(&seed_description),
            None,
            &candidate_clap,
            None,
            None,
            0.5,
        )
        .unwrap();

        assert!((distance - (0.85_f64 * 0.85 * 2.0_f64).sqrt()).abs() < 1e-6);
    }

    #[test]
    fn test_blended_projection_vector_zero_pads_missing_description() {
        let clap = vec![3.0, 4.0];
        let with_description = blended_projection_vector(&clap, Some(&vec![1.0; 384]), 0.5);
        let without_description = blended_projection_vector(&clap, None, 0.5);

        assert_eq!(with_description.len(), 386);
        assert_eq!(without_description.len(), 386);
        assert!(without_description[2..].iter().all(|&v| v == 0.0));
    }

    // ── key_to_camelot ────────────────────────────────────────────────────────

    #[test]
    fn test_key_to_camelot_all_major_hours() {
        // Camelot wheel: 12 major keys, each with its expected hour (1–12)
        let cases = [
            ("B",  "major", 1u32),
            ("F#", "major", 2),
            ("C#", "major", 3),
            ("Ab", "major", 4),
            ("Eb", "major", 5),
            ("Bb", "major", 6),
            ("F",  "major", 7),
            ("C",  "major", 8),
            ("G",  "major", 9),
            ("D",  "major", 10),
            ("A",  "major", 11),
            ("E",  "major", 12),
        ];
        for (key, scale, hour) in cases {
            assert_eq!(
                key_to_camelot(key, scale),
                Some((hour, false)),
                "major key {key} should be hour {hour}"
            );
        }
    }

    #[test]
    fn test_key_to_camelot_all_minor_hours() {
        let cases = [
            ("Ab", "minor", 1u32),
            ("Eb", "minor", 2),
            ("Bb", "minor", 3),
            ("F",  "minor", 4),
            ("C",  "minor", 5),
            ("G",  "minor", 6),
            ("D",  "minor", 7),
            ("A",  "minor", 8),
            ("E",  "minor", 9),
            ("B",  "minor", 10),
            ("F#", "minor", 11),
            ("C#", "minor", 12),
        ];
        for (key, scale, hour) in cases {
            assert_eq!(
                key_to_camelot(key, scale),
                Some((hour, true)),
                "minor key {key} should be hour {hour}"
            );
        }
    }

    #[test]
    fn test_key_to_camelot_enharmonic_equivalents() {
        // Db == C#, Gb == F#, G# == Ab  (all should return the same result)
        assert_eq!(key_to_camelot("Db", "major"), key_to_camelot("C#", "major"));
        assert_eq!(key_to_camelot("Gb", "major"), key_to_camelot("F#", "major"));
        assert_eq!(key_to_camelot("G#", "minor"), key_to_camelot("Ab", "minor"));
        assert_eq!(key_to_camelot("D#", "minor"), key_to_camelot("Eb", "minor"));
        assert_eq!(key_to_camelot("A#", "minor"), key_to_camelot("Bb", "minor"));
        assert_eq!(key_to_camelot("Cb", "major"), key_to_camelot("B", "major"));
    }

    #[test]
    fn test_key_to_camelot_minor_suffix_overrides_scale_param() {
        // "Am" should be treated as minor regardless of the scale argument
        assert_eq!(key_to_camelot("Am", "major"), Some((8, true)));
        assert_eq!(key_to_camelot("Am", "minor"), Some((8, true)));
        // "G#m" likewise
        assert_eq!(key_to_camelot("G#m", "major"), Some((1, true)));
    }

    #[test]
    fn test_key_to_camelot_unknown_returns_none() {
        assert_eq!(key_to_camelot("X", "major"), None);
        assert_eq!(key_to_camelot("", "minor"), None);
        assert_eq!(key_to_camelot("H", "major"), None);
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    use rusqlite::Connection;

    fn make_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE tracks (
                id INTEGER PRIMARY KEY,
                detected_genre TEXT,
                genre TEXT,
                bpm REAL,
                key TEXT,
                scale TEXT,
                mood_happy REAL,
                mood_sad REAL,
                mood_aggressive REAL,
                mood_relaxed REAL,
                mood_party REAL,
                mood_acoustic REAL,
                mood_electronic REAL
            );
            CREATE TABLE audio_embeddings (
                track_id INTEGER PRIMARY KEY,
                embedding BLOB NOT NULL
            );
            CREATE TABLE description_embeddings (
                track_id INTEGER PRIMARY KEY,
                embedding BLOB NOT NULL
            );",
        )
        .unwrap();
        conn
    }

    fn unit_clap_embedding() -> Vec<u8> {
        let dim = 512usize;
        let val = 1.0f32 / (dim as f32).sqrt();
        let floats = vec![val; dim];
        floats.iter().flat_map(|&f| f.to_le_bytes()).collect()
    }

    // ── empty DB returns error ────────────────────────────────────────────────

    #[test]
    fn test_harmonic_layout_empty_returns_error() {
        let conn = make_test_db();
        assert!(compute_harmonic_layout(&conn, false).is_err());
    }

    #[test]
    fn test_essentia_layout_empty_returns_error() {
        let conn = make_test_db();
        assert!(compute_essentia_layout(&conn, false).is_err());
    }

    #[test]
    fn test_genre_wheel_layout_empty_returns_error() {
        let conn = make_test_db();
        assert!(compute_genre_wheel_layout(&conn, false).is_err());
    }

    #[test]
    fn test_hybrid_layout_empty_returns_error() {
        let conn = make_test_db();
        assert!(compute_hybrid_layout(&conn, false, 0.5, "pca").is_err());
    }

    // ── music_only filter ─────────────────────────────────────────────────────

    #[test]
    fn test_music_only_excludes_non_music_tracks() {
        let conn = make_test_db();
        conn.execute_batch(
            "INSERT INTO tracks (id, detected_genre, bpm, key, scale) VALUES
             (1, NULL,            120.0, 'C',  'major'),
             (2, 'Non-Music: SFX', 90.0, 'G',  'minor'),
             (3, NULL,            100.0, 'A',  'major');",
        )
        .unwrap();

        let (ids_all, _)      = compute_harmonic_layout(&conn, false).unwrap();
        let (ids_music, _)    = compute_harmonic_layout(&conn, true).unwrap();

        assert_eq!(ids_all.len(), 3);
        assert_eq!(ids_music.len(), 2);
        assert!(!ids_music.contains(&2));
    }

    // ── coord count matches track count ───────────────────────────────────────

    #[test]
    fn test_layout_coord_count_matches_track_count() {
        let conn = make_test_db();
        conn.execute_batch(
            "INSERT INTO tracks (id, bpm, key, scale) VALUES
             (1, 120.0, 'C', 'major'),
             (2, 140.0, 'G', 'minor'),
             (3,  90.0, 'A', 'major');",
        )
        .unwrap();

        let (ids, coords) = compute_harmonic_layout(&conn, false).unwrap();
        assert_eq!(ids.len(), 3);
        assert_eq!(coords.len(), 3);
    }

    // ── spring layout keeps coords in [2, 98] ─────────────────────────────────

    #[test]
    fn test_spring_layout_outputs_in_canvas_bounds() {
        let conn = make_test_db();
        // 10 tracks spread across keys and genres
        conn.execute_batch(
            "INSERT INTO tracks (id, genre, bpm, key, scale) VALUES
             (1,'Electronic',128.0,'C','major'),(2,'Jazz',95.0,'F#','minor'),
             (3,'Rock',140.0,'G','major'),(4,'Ambient',70.0,'Bb','minor'),
             (5,'Electronic',125.0,'D','major'),(6,'Jazz',100.0,'E','minor'),
             (7,'Rock',135.0,'Ab','major'),(8,'Ambient',75.0,'B','minor'),
             (9,'Classical',60.0,NULL,NULL),(10,'Classical',65.0,NULL,NULL);",
        )
        .unwrap();

        for (ids, coords) in [
            compute_harmonic_layout(&conn, false).unwrap(),
            compute_essentia_layout(&conn, false).unwrap(),
            compute_genre_wheel_layout(&conn, false).unwrap(),
        ] {
            assert_eq!(ids.len(), 10);
            for &(x, y) in &coords {
                assert!(x >= 2.0 && x <= 98.0, "x={x} out of [2,98]");
                assert!(y >= 2.0 && y <= 98.0, "y={y} out of [2,98]");
            }
        }
    }

    // ── harmonic: keyed tracks placed away from center ────────────────────────

    #[test]
    fn test_harmonic_keyed_tracks_further_from_center_than_keyless() {
        let conn = make_test_db();
        conn.execute_batch(
            "INSERT INTO tracks (id, bpm, key, scale) VALUES
             (1, 120.0, 'C', 'major'),
             (2, 120.0, 'G', 'minor'),
             (3, 120.0, NULL, NULL),
             (4, 120.0, NULL, NULL);",
        )
        .unwrap();

        let (ids, coords) = compute_harmonic_layout(&conn, false).unwrap();
        let by_id: std::collections::HashMap<i64, (f64, f64)> =
            ids.into_iter().zip(coords).collect();

        let dist = |(x, y): (f64, f64)| ((x - 50.0).powi(2) + (y - 50.0).powi(2)).sqrt();

        let keyed_min   = [by_id[&1], by_id[&2]].iter().copied().map(dist).fold(f64::INFINITY, f64::min);
        let keyless_max = [by_id[&3], by_id[&4]].iter().copied().map(dist).fold(0.0f64, f64::max);

        assert!(
            keyed_min > keyless_max,
            "keyed tracks should be further from center than keyless ones \
             (keyed_min={keyed_min:.2}, keyless_max={keyless_max:.2})"
        );
    }

    // ── essentia: zero-mood tracks cluster near center ─────────────────────────

    #[test]
    fn test_essentia_zero_mood_tracks_near_center() {
        let conn = make_test_db();
        // Two tracks with strong moods, two with all zeros
        conn.execute_batch(
            "INSERT INTO tracks (id, mood_happy, mood_party, mood_electronic,
                mood_aggressive, mood_sad, mood_relaxed, mood_acoustic) VALUES
             (1, 0.9, 0.8, 0.7, 0.1, 0.1, 0.1, 0.1),
             (2, 0.1, 0.1, 0.1, 0.9, 0.8, 0.7, 0.6),
             (3, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
             (4, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);",
        )
        .unwrap();

        let (ids, coords) = compute_essentia_layout(&conn, false).unwrap();
        let by_id: std::collections::HashMap<i64, (f64, f64)> =
            ids.into_iter().zip(coords).collect();

        let dist = |(x, y): (f64, f64)| ((x - 50.0).powi(2) + (y - 50.0).powi(2)).sqrt();

        let mood_min   = [by_id[&1], by_id[&2]].iter().copied().map(dist).fold(f64::INFINITY, f64::min);
        let no_mood_max = [by_id[&3], by_id[&4]].iter().copied().map(dist).fold(0.0f64, f64::max);

        assert!(
            no_mood_max < mood_min,
            "zero-mood tracks should be closer to center than tracks with strong moods \
             (no_mood_max={no_mood_max:.2}, mood_min={mood_min:.2})"
        );
    }

    // ── hybrid: linear fallback for n < 4 ────────────────────────────────────

    #[test]
    fn test_hybrid_linear_fallback_for_small_n() {
        let conn = make_test_db();
        let blob = unit_clap_embedding();
        conn.execute("INSERT INTO tracks (id) VALUES (1)", []).unwrap();
        conn.execute("INSERT INTO audio_embeddings (track_id, embedding) VALUES (1, ?1)", [&blob]).unwrap();

        let (ids, coords) = compute_hybrid_layout(&conn, false, 1.0, "umap").unwrap();
        assert_eq!(ids.len(), 1);
        assert_eq!(coords[0], (50.0, 50.0));
    }

    // ── deterministic_genre_jitter ────────────────────────────────────────────

    #[test]
    fn test_genre_jitter_is_deterministic_and_stable() {
        // Same input → same output every call
        assert_eq!(
            deterministic_genre_jitter("Electronic"),
            deterministic_genre_jitter("Electronic")
        );
        // Different genres → different jitter
        assert_ne!(
            deterministic_genre_jitter("Electronic"),
            deterministic_genre_jitter("Ambient")
        );
    }

    #[test]
    fn test_genre_jitter_empty_string_is_zero() {
        assert_eq!(deterministic_genre_jitter(""), (0.0, 0.0));
    }

    #[test]
    fn test_genre_jitter_output_is_within_mathematical_bounds() {
        // val_angle  = (hash & 0xFFFF) / 65535.0 * 2.0 - 1.0  → [-1, 1], then * 0.04 → [-0.04, 0.04]
        // val_radial = same raw → [-1, 1], then * 1.2          → [-1.2, 1.2]
        let genres = ["Electronic", "Jazz", "Classical", "Hip-Hop", "Ambient", "Metal", "Pop"];
        for g in genres {
            let (angle, radial) = deterministic_genre_jitter(g);
            assert!(
                angle >= -0.04 && angle <= 0.04,
                "angle jitter {angle} out of [-0.04, 0.04] for genre {g}"
            );
            assert!(
                radial >= -1.2 && radial <= 1.2,
                "radial jitter {radial} out of [-1.2, 1.2] for genre {g}"
            );
        }
    }

    // ── run_spring_layout ─────────────────────────────────────────────────────

    #[test]
    fn test_spring_layout_single_node_no_panic() {
        let mut nodes = vec![
            SpringNode { anchor_x: 50.0, anchor_y: 50.0, x: 50.0, y: 50.0, vx: 0.0, vy: 0.0 },
        ];
        run_spring_layout(&mut nodes, 20);
        assert!(nodes[0].x.is_finite() && nodes[0].y.is_finite());
    }

    #[test]
    fn test_spring_layout_spreads_overlapping_nodes() {
        let mut nodes = vec![
            SpringNode { anchor_x: 50.0, anchor_y: 50.0, x: 50.0, y: 50.0, vx: 0.0, vy: 0.0 },
            SpringNode { anchor_x: 50.0, anchor_y: 50.0, x: 50.0, y: 50.0, vx: 0.0, vy: 0.0 },
        ];
        run_spring_layout(&mut nodes, 10);

        let dx = nodes[0].x - nodes[1].x;
        let dy = nodes[0].y - nodes[1].y;
        let dist = (dx * dx + dy * dy).sqrt();
        assert!(dist > 0.5, "Nodes should have moved apart. Dist: {dist}");
    }

    #[test]
    fn test_spring_layout_all_nodes_stay_in_bounds() {
        // 8 nodes at various positions — clamping must keep all within [2, 98]
        let anchors: &[(f64, f64)] = &[
            (10.0, 10.0), (90.0, 90.0), (50.0, 50.0), (50.0, 50.0),
            (1.0,  1.0),  (99.0, 99.0), (30.0, 70.0), (70.0, 30.0),
        ];
        let mut nodes: Vec<SpringNode> = anchors.iter().map(|&(ax, ay)| SpringNode {
            anchor_x: ax, anchor_y: ay, x: ax, y: ay, vx: 0.0, vy: 0.0,
        }).collect();

        run_spring_layout(&mut nodes, 50);

        for n in &nodes {
            assert!(n.x >= 2.0 && n.x <= 98.0, "x={} out of [2, 98]", n.x);
            assert!(n.y >= 2.0 && n.y <= 98.0, "y={} out of [2, 98]", n.y);
            assert!(n.x.is_finite() && n.y.is_finite());
        }
    }

    #[test]
    fn test_spring_layout_anchor_pull_keeps_nodes_near_anchor() {
        // With well-separated anchors and no neighbours close enough to repel,
        // nodes should settle within a reasonable distance of their anchors.
        let mut nodes = vec![
            SpringNode { anchor_x: 20.0, anchor_y: 20.0, x: 20.0, y: 20.0, vx: 0.0, vy: 0.0 },
            SpringNode { anchor_x: 80.0, anchor_y: 80.0, x: 80.0, y: 80.0, vx: 0.0, vy: 0.0 },
        ];
        run_spring_layout(&mut nodes, 100);

        for (i, n) in nodes.iter().enumerate() {
            let dist = ((n.x - n.anchor_x).powi(2) + (n.y - n.anchor_y).powi(2)).sqrt();
            assert!(
                dist < 5.0,
                "node {i} ended up {dist:.2} units from its anchor — anchor pull too weak"
            );
        }
    }
}
