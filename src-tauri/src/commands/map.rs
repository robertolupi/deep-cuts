use rusqlite::Connection;
use std::sync::Mutex;

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

fn standardize_to_100(coords: &[(f64, f64)]) -> Vec<(f64, f64)> {
    if coords.is_empty() {
        return Vec::new();
    }
    let x_min = coords.iter().map(|p| p.0).fold(f64::MAX, f64::min);
    let x_max = coords.iter().map(|p| p.0).fold(f64::MIN, f64::max);
    let y_min = coords.iter().map(|p| p.1).fold(f64::MAX, f64::min);
    let y_max = coords.iter().map(|p| p.1).fold(f64::MIN, f64::max);
    let xs = if x_max == x_min { 1.0 } else { x_max - x_min };
    let ys = if y_max == y_min { 1.0 } else { y_max - y_min };
    coords
        .iter()
        .map(|&(x, y)| ((x - x_min) / xs * 100.0, (y - y_min) / ys * 100.0))
        .collect()
}

/// Returns the stored 2D UMAP coordinates joined with basic track metadata.
#[tauri::command]
pub fn get_projection_coordinates(
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
/// by L2 distance in the CLAP audio embedding space.
#[tauri::command]
pub fn search_similar_tracks_audio(
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
        let mut s = conn
            .prepare("SELECT id FROM tracks WHERE watched_directory_id = ?1")
            .map_err(|e| e.to_string())?;
        let rows = s
            .query_map([dir_id], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        rows.filter_map(|r| r.ok()).collect()
    } else {
        let mut s = conn
            .prepare("SELECT id FROM tracks")
            .map_err(|e| e.to_string())?;
        let rows = s.query_map([], |r| r.get(0)).map_err(|e| e.to_string())?;
        rows.filter_map(|r| r.ok()).collect()
    };

    let k = if directory_id.is_some() {
        500i64
    } else {
        (valid_ids.len() + 1) as i64
    };
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
        .query_map(rusqlite::params![blob], |row| {
            Ok(AudioSimilarityResult {
                id: row.get(0)?,
                distance: row.get(1)?,
                title: row.get(2)?,
                artist: row.get(3)?,
                bpm: row.get(4)?,
                key: row.get(5)?,
                scale: row.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut list: Vec<AudioSimilarityResult> = rows
        .filter_map(|r| r.ok())
        .filter(|r| r.id != track_id && valid_ids.contains(&r.id))
        .collect();
    list.truncate(20);
    Ok(list)
}

/// Runs UMAP on all CLAP audio embeddings (and optionally blends description embeddings)
/// and persists the 2D coordinates in `track_coords`. Emits `projection-updated` when done.
#[tauri::command]
pub async fn recompute_projection(
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

    // Collect all CLAP and description embeddings
    let (track_ids, blended_vectors) = {
        let conn = conn_state.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT t.id, ae.embedding, de.embedding
                 FROM tracks t
                 JOIN audio_embeddings ae ON ae.track_id = t.id
                 LEFT JOIN description_embeddings de ON de.track_id = t.id",
            )
            .map_err(|e| e.to_string())?;
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

            let blended = if let Some(desc_embed) = desc_embed_opt {
                let norm_clap = l2_normalize(&clap_embed);
                let norm_desc = l2_normalize(&desc_embed);

                let mut vec = Vec::with_capacity(norm_clap.len() + norm_desc.len());
                for &x in &norm_clap {
                    vec.push(x * blend_weight as f32);
                }
                for &x in &norm_desc {
                    vec.push(x * (1.0 - blend_weight) as f32);
                }
                vec
            } else {
                l2_normalize(&clap_embed)
            };

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

    // Persist inside a transaction
    {
        let mut conn = conn_state.lock().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM track_coords", [])
            .map_err(|e| e.to_string())?;
        {
            let mut ins = tx
                .prepare("INSERT INTO track_coords (track_id, x, y) VALUES (?1, ?2, ?3)")
                .map_err(|e| e.to_string())?;
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
        let coords = vec![(10.0, 5.0), (20.0, 15.0)];
        let standardized = standardize_to_100(&coords);
        assert_eq!(standardized[0], (0.0, 0.0));
        assert_eq!(standardized[1], (100.0, 100.0));
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
}
