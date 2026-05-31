use rusqlite::Connection;
use std::sync::Mutex;

const DESCRIPTION_EMBEDDING_DIM: usize = 384;

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
    candidate_clap: &[f32],
    candidate_description: Option<&[f32]>,
    clap_weight: f64,
) -> Option<f64> {
    let norm_seed_clap = l2_normalize(seed_clap);
    let norm_candidate_clap = l2_normalize(candidate_clap);
    let clap_distance_sq = l2_distance_sq(&norm_seed_clap, &norm_candidate_clap)?;

    if let (Some(seed_description), Some(candidate_description)) =
        (seed_description, candidate_description)
    {
        let norm_seed_description = l2_normalize(seed_description);
        let norm_candidate_description = l2_normalize(candidate_description);
        if let Some(description_distance_sq) =
            l2_distance_sq(&norm_seed_description, &norm_candidate_description)
        {
            let description_weight = 1.0 - clap_weight;
            return Some(
                ((clap_weight * clap_weight * clap_distance_sq)
                    + (description_weight * description_weight * description_distance_sq))
                    .sqrt(),
            );
        }
    }

    Some(clap_distance_sq.sqrt())
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

#[derive(Debug, PartialEq)]
struct EffectiveProjectionConfig {
    clap_weight: f64,
}

fn effective_projection_config(
    clap_weight: Option<f64>,
    algorithm: &str,
    n_neighbors: i32,
    min_dist: f64,
    perplexity: f64,
) -> EffectiveProjectionConfig {
    // rag-umap exposes no tuning surface here yet. Keep accepted UI parameters
    // intentionally ignored until alternate projection algorithms are implemented.
    let _ = (algorithm, n_neighbors, min_dist, perplexity);
    EffectiveProjectionConfig {
        clap_weight: clap_weight.unwrap_or(0.5),
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
                t.genre, t.bpm, t.key, t.scale
         FROM track_coords tc
         JOIN tracks t ON t.id = tc.track_id
         WHERE (t.detected_genre IS NULL OR t.detected_genre NOT LIKE 'Non-Music%')"
    } else {
        "SELECT tc.track_id, tc.x, tc.y,
                t.watched_directory_id, t.title, t.filename, t.artist,
                t.genre, t.bpm, t.key, t.scale
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

    let (seed_clap_blob, seed_description_blob): (Vec<u8>, Option<Vec<u8>>) = conn
        .query_row(
            "SELECT ae.embedding, de.embedding
             FROM audio_embeddings ae
             LEFT JOIN description_embeddings de ON de.track_id = ae.track_id
             WHERE ae.track_id = ?1",
            [track_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| "Track has no CLAP embedding yet — run analysis first.".to_string())?;
    let seed_clap = bytes_to_floats(&seed_clap_blob);
    let seed_description = seed_description_blob
        .as_ref()
        .map(|blob| bytes_to_floats(blob));

    let mut rows: Vec<AudioSimilarityResult> = if let Some(dir_id) = directory_id {
        let mut stmt = conn
            .prepare(
                "SELECT t.id, ae.embedding, de.embedding, t.title, t.artist, t.bpm, t.key, t.scale
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
                let candidate_clap = bytes_to_floats(&candidate_clap_blob);
                let candidate_description = candidate_description_blob
                    .as_ref()
                    .map(|blob| bytes_to_floats(blob));
                let distance = blended_embedding_distance(
                    &seed_clap,
                    seed_description.as_deref(),
                    &candidate_clap,
                    candidate_description.as_deref(),
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
            .filter_map(|r| r.ok())
            .filter(|r| r.distance.is_finite())
            .collect()
    } else {
        let mut stmt = conn
            .prepare(
                "SELECT t.id, ae.embedding, de.embedding, t.title, t.artist, t.bpm, t.key, t.scale
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
                let candidate_clap = bytes_to_floats(&candidate_clap_blob);
                let candidate_description = candidate_description_blob
                    .as_ref()
                    .map(|blob| bytes_to_floats(blob));
                let distance = blended_embedding_distance(
                    &seed_clap,
                    seed_description.as_deref(),
                    &candidate_clap,
                    candidate_description.as_deref(),
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
            .filter_map(|r| r.ok())
            .filter(|r| r.distance.is_finite())
            .collect()
    };

    rows.sort_by(|a, b| {
        a.distance
            .partial_cmp(&b.distance)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });
    rows.truncate(20);
    Ok(rows)
}

/// Runs UMAP on CLAP audio embeddings (and optionally blends description embeddings)
/// and persists the 2D coordinates in `track_coords`. Emits `projection-updated` when done.
/// When `music_only` is true, tracks classified as Non-Music by Essentia are excluded from
/// the projection, matching the frontend `musicOnly` filter signal.
#[tauri::command]
pub async fn recompute_projection(
    music_only: bool,
    clap_weight: Option<f64>,
    algorithm: String,
    n_neighbors: i32,
    min_dist: f64,
    perplexity: f64,
    app: tauri::AppHandle,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<usize, String> {
    use tauri::Emitter;
    let effective_config =
        effective_projection_config(clap_weight, &algorithm, n_neighbors, min_dist, perplexity);

    // Collect CLAP and description embeddings, optionally excluding non-music tracks
    let (track_ids, blended_vectors) = {
        let conn = conn_state.lock().map_err(|e| e.to_string())?;
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
                let id: i64 = row.get(0)?;
                let clap_blob: Vec<u8> = row.get(1)?;
                let desc_blob_opt: Option<Vec<u8>> = row.get(2)?;
                Ok((id, clap_blob, desc_blob_opt))
            })
            .map_err(|e| e.to_string())?;
        let mut ids = Vec::new();
        let mut vecs = Vec::new();
        let blend_weight = effective_config.clap_weight;

        for row in rows.filter_map(|r| r.ok()) {
            let (id, clap_blob, desc_blob_opt) = row;
            let clap_embed = bytes_to_floats(&clap_blob);
            let desc_embed_opt = desc_blob_opt.map(|b| bytes_to_floats(&b));

            let blended =
                blended_projection_vector(&clap_embed, desc_embed_opt.as_deref(), blend_weight);

            ids.push(id);
            vecs.push(blended);
        }
        (ids, vecs)
    };

    if blended_vectors.is_empty() {
        return Err(
            "No tracks with CLAP embeddings found. Run the analysis pipeline first.".to_string(),
        );
    }

    let n = blended_vectors.len();
    let coords: Vec<(f64, f64)> = if n < 4 {
        // Too few points for UMAP — spread evenly on a horizontal line
        (0..n)
            .map(|i| {
                let x = if n > 1 {
                    i as f64 / (n - 1) as f64 * 100.0
                } else {
                    50.0
                };
                (x, 50.0)
            })
            .collect()
    } else {
        let raw = rag_umap::convert_to_2d(blended_vectors)
            .map_err(|e| format!("UMAP projection failed: {:?}", e))?;
        standardize_to_100(
            &raw.iter()
                .map(|v| (v[0] as f64, v[1] as f64))
                .collect::<Vec<_>>(),
        )
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
                    "INSERT INTO track_coords (track_id, x, y, music_only) VALUES (?1, ?2, ?3, ?4)",
                )
                .map_err(|e| e.to_string())?;
            let music_only_int: i64 = if music_only { 1 } else { 0 };
            for (i, &(x, y)) in coords.iter().enumerate() {
                ins.execute(rusqlite::params![track_ids[i], x, y, music_only_int])
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

        assert_eq!(default_umap, requested_tsne);
        assert_eq!(default_umap.clap_weight, 0.7);
        assert_eq!(
            effective_projection_config(None, "pca", 5, 0.0, 100.0).clap_weight,
            0.5,
        );
    }

    #[test]
    fn test_blended_embedding_distance_uses_description_when_available() {
        let seed_clap = vec![1.0, 0.0];
        let candidate_clap = vec![1.0, 0.0];
        let seed_description = vec![1.0, 0.0];
        let candidate_description = vec![0.0, 1.0];

        let distance = blended_embedding_distance(
            &seed_clap,
            Some(&seed_description),
            &candidate_clap,
            Some(&candidate_description),
            0.5,
        )
        .unwrap();

        assert!((distance - 0.70710678118).abs() < 1e-6);
    }

    #[test]
    fn test_blended_embedding_distance_falls_back_to_clap_without_description_pair() {
        let seed_clap = vec![1.0, 0.0];
        let candidate_clap = vec![0.0, 1.0];
        let seed_description = vec![1.0, 0.0];

        let distance = blended_embedding_distance(
            &seed_clap,
            Some(&seed_description),
            &candidate_clap,
            None,
            0.5,
        )
        .unwrap();

        assert!((distance - 2.0_f64.sqrt()).abs() < 1e-6);
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
}
