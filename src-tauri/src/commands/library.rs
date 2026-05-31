use crate::database::{Track, WatchedDirectory};
use crate::error::AppError;
use crate::scanner;
use rusqlite::Connection;
use std::sync::Mutex;

/// Spawns a native directory picker dialog using rfd and returns selected path.
#[tauri::command]
pub fn select_directory() -> Result<Option<String>, AppError> {
    let folder = rfd::FileDialog::new()
        .set_title("Select Music Folder")
        .pick_folder();

    if let Some(path_buf) = folder {
        Ok(Some(path_buf.to_string_lossy().into_owned()))
    } else {
        Ok(None)
    }
}

/// Retrieve all registered directories from database.
#[tauri::command]
pub fn get_watched_directories(
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<Vec<WatchedDirectory>, AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
    let dirs = WatchedDirectory::find_all(&conn)?;
    Ok(dirs)
}

/// Registers a new directory with path validation and unique checks.
#[tauri::command]
pub fn add_watched_directory(
    name: String,
    path: String,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<(), AppError> {
    let trimmed_name = name.trim();
    let trimmed_path = path.trim();

    if trimmed_name.is_empty() || trimmed_path.is_empty() {
        return Err(AppError::Generic(
            "Collection name and directory path cannot be empty.".to_string(),
        ));
    }

    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
    let dir = WatchedDirectory {
        id: 0,
        name: trimmed_name.to_string(),
        path: trimmed_path.to_string(),
    };
    dir.insert(&conn).map_err(|e| {
        let err_str = e.to_string();
        if err_str.contains("UNIQUE constraint failed") {
            AppError::Generic(
                "This folder path is already registered under another collection.".to_string(),
            )
        } else {
            AppError::Database(e)
        }
    })?;
    Ok(())
}

/// Unregisters a watched directory by ID.
#[tauri::command]
pub fn remove_watched_directory(
    id: i64,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<(), AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
    WatchedDirectory::delete(&conn, id)?;
    Ok(())
}

/// Queries the total number of track records in the database.
#[tauri::command]
pub fn get_track_count(conn_state: tauri::State<'_, Mutex<Connection>>) -> Result<i64, AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
    let count = Track::count(&conn)?;
    Ok(count)
}

/// Retrieve all indexed tracks from the database.
#[tauri::command]
pub fn get_tracks(conn_state: tauri::State<'_, Mutex<Connection>>) -> Result<Vec<Track>, AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
    let tracks = Track::find_all(&conn)?;
    Ok(tracks)
}

/// Writes a .dc.json sidecar file next to the given track's audio file.
#[tauri::command]
pub fn save_sidecar(
    track_id: i64,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<(), AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
    scanner::sidecar::save(&conn, track_id)?;
    Ok(())
}

/// Writes .dc.json sidecar files for every track in the database.
/// Returns the number of files written successfully.
#[tauri::command]
pub fn export_sidecars(conn_state: tauri::State<'_, Mutex<Connection>>) -> Result<usize, AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
    let count = scanner::sidecar::export_all(&conn)?;
    Ok(count)
}

/// Opens the system file manager and selects the given file.
/// macOS: open -R <path>  |  Windows: explorer /select,<path>  |  Linux: xdg-open <parent dir>
#[tauri::command]
pub fn reveal_in_finder(path: String) -> Result<(), AppError> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-R", &path])
            .spawn()?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(format!("/select,{}", path))
            .spawn()?;
    }
    #[cfg(target_os = "linux")]
    {
        let parent = std::path::Path::new(&path)
            .parent()
            .ok_or_else(|| AppError::Generic("Could not determine parent directory".to_string()))?;
        std::process::Command::new("xdg-open").arg(parent).spawn()?;
    }
    Ok(())
}

#[derive(serde::Serialize)]
pub struct SemanticSearchResult {
    pub id: i64,
    pub title: Option<String>,
    pub filename: String,
    pub artist: Option<String>,
    pub genre: Option<String>,
    pub bpm: Option<f64>,
    pub key: Option<String>,
    pub scale: Option<String>,
    pub score: f64,
}

/// Perform a local semantic vector search over description_embeddings table using sqlite-vec MATCH.
#[tauri::command]
pub async fn search_semantic_tracks(
    query: String,
    limit: Option<usize>,
    app_handle: tauri::AppHandle,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<Vec<SemanticSearchResult>, AppError> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    // 1. Generate sentence embedding for the query string using all-MiniLM-L6-v2 ONNX model
    let embedding = crate::embeddings::run_sentence_embed(trimmed, Some(&app_handle))
        .map_err(|e| AppError::Generic(format!("Failed to generate semantic embedding: {}", e)))?;

    // 2. Convert Vec<f32> embedding to a little-endian byte array Vec<u8>
    let bytes: Vec<u8> = embedding.iter().flat_map(|&f| f.to_le_bytes()).collect();

    // 3. Acquire DB lock and execute sqlite-vec MATCH vector query
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;

    let max_k = limit.unwrap_or(40) as i64;

    let mut stmt = conn
        .prepare(
            "SELECT de.track_id, t.title, t.filename, t.artist, t.genre, t.bpm, t.key, t.scale, de.distance
             FROM description_embeddings de
             JOIN tracks t ON t.id = de.track_id
             WHERE de.embedding MATCH ?1 AND k = ?2
             ORDER BY de.distance ASC",
        )
        .map_err(AppError::Database)?;

    let rows = stmt
        .query_map(rusqlite::params![bytes, max_k], |row| {
            let id: i64 = row.get(0)?;
            let title: Option<String> = row.get(1)?;
            let filename: String = row.get(2)?;
            let artist: Option<String> = row.get(3)?;
            let genre: Option<String> = row.get(4)?;
            let bpm: Option<f64> = row.get(5)?;
            let key: Option<String> = row.get(6)?;
            let scale: Option<String> = row.get(7)?;
            let distance: f64 = row.get(8)?;

            // Convert raw Euclidean (L2) distance into percentage similarity score.
            // L2 distance squared = d^2 = distance * distance.
            // CosineSimilarity = 1 - d^2 / 2
            let d_sq = distance * distance;
            let score = ((1.0_f64 - d_sq / 2.0_f64) * 100.0_f64).clamp(0.0_f64, 100.0_f64);

            Ok(SemanticSearchResult {
                id,
                title,
                filename,
                artist,
                genre,
                bpm,
                key,
                scale,
                score,
            })
        })
        .map_err(AppError::Database)?;

    let results: Vec<SemanticSearchResult> = rows
        .filter_map(|r| r.ok())
        .collect();

    Ok(results)
}

/// Perform a local sonic vector search over audio_embeddings table using sqlite-vec MATCH and CLAP text encoder.
#[tauri::command]
pub async fn search_clap_tracks(
    query: String,
    limit: Option<usize>,
    app_handle: tauri::AppHandle,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<Vec<SemanticSearchResult>, AppError> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    // 1. Generate CLAP text embedding for the query string
    let embedding = crate::embeddings::run_clap_text_embed(trimmed, Some(&app_handle))
        .map_err(|e| AppError::Generic(format!("Failed to generate CLAP text embedding: {}", e)))?;

    // 2. Convert Vec<f32> embedding to a little-endian byte array Vec<u8>
    let bytes: Vec<u8> = embedding.iter().flat_map(|&f| f.to_le_bytes()).collect();

    // 3. Acquire DB lock and execute sqlite-vec MATCH vector query on audio_embeddings
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;

    let max_k = limit.unwrap_or(40) as i64;

    let mut stmt = conn
        .prepare(
            "SELECT ae.track_id, t.title, t.filename, t.artist, t.genre, t.bpm, t.key, t.scale, ae.distance
             FROM audio_embeddings ae
             JOIN tracks t ON t.id = ae.track_id
             WHERE ae.embedding MATCH ?1 AND k = ?2
             ORDER BY ae.distance ASC",
        )
        .map_err(AppError::Database)?;

    let rows = stmt
        .query_map(rusqlite::params![bytes, max_k], |row| {
            let id: i64 = row.get(0)?;
            let title: Option<String> = row.get(1)?;
            let filename: String = row.get(2)?;
            let artist: Option<String> = row.get(3)?;
            let genre: Option<String> = row.get(4)?;
            let bpm: Option<f64> = row.get(5)?;
            let key: Option<String> = row.get(6)?;
            let scale: Option<String> = row.get(7)?;
            let distance: f64 = row.get(8)?;

            let d_sq = distance * distance;
            let score = ((1.0_f64 - d_sq / 2.0_f64) * 100.0_f64).clamp(0.0_f64, 100.0_f64);

            Ok(SemanticSearchResult {
                id,
                title,
                filename,
                artist,
                genre,
                bpm,
                key,
                scale,
                score,
            })
        })
        .map_err(AppError::Database)?;

    let results: Vec<SemanticSearchResult> = rows
        .filter_map(|r| r.ok())
        .collect();

    Ok(results)
}

#[tauri::command]
pub fn get_cover_art(path: String) -> Result<Option<String>, String> {
    use base64::Engine as _;
    use lofty::config::ParseOptions;
    use lofty::prelude::*;
    use lofty::probe::Probe;
    use std::path::Path;

    let tagged = Probe::open(Path::new(&path))
        .map_err(|e| e.to_string())?
        .options(ParseOptions::new())
        .read()
        .map_err(|e| e.to_string())?;

    for tag in tagged.tags() {
        for picture in tag.pictures() {
            let mime = picture
                .mime_type()
                .map(|m| m.as_str())
                .unwrap_or("image/jpeg");
            let b64 = base64::engine::general_purpose::STANDARD.encode(picture.data());
            return Ok(Some(format!("data:{};base64,{}", mime, b64)));
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::setup_test_db;

    #[test]
    fn test_similarity_score_clamping() {
        // d = 0 -> score = 100%
        let d = 0.0_f64;
        let score = ((1.0_f64 - (d * d) / 2.0_f64) * 100.0_f64).clamp(0.0_f64, 100.0_f64);
        assert_eq!(score, 100.0);

        // d = sqrt(2) -> score = 0%
        let d = 2.0f64.sqrt();
        let score = ((1.0_f64 - (d * d) / 2.0_f64) * 100.0_f64).clamp(0.0_f64, 100.0_f64);
        assert!((score - 0.0).abs() < 1e-9);

        // d = 2.0 -> score = 0% (clamped)
        let d = 2.0_f64;
        let score = ((1.0_f64 - (d * d) / 2.0_f64) * 100.0_f64).clamp(0.0_f64, 100.0_f64);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_sqlite_vec_match_query() {
        let conn = setup_test_db();
        
        // Insert a watched directory to satisfy the foreign key constraint
        conn.execute(
            "INSERT INTO watched_directories (id, name, path) VALUES (1, 'Test Collection', '/Users/user/Music')",
            []
        ).unwrap();
        
        // Insert sample tracks
        conn.execute(
            "INSERT INTO tracks (watched_directory_id, path, filename, size_bytes, last_modified, duration_seconds)
             VALUES (1, '/Users/user/Music/t1.mp3', 't1.mp3', 100, 1780000000, 100)",
            []
        ).unwrap();
        conn.execute(
            "INSERT INTO tracks (watched_directory_id, path, filename, size_bytes, last_modified, duration_seconds)
             VALUES (1, '/Users/user/Music/t2.mp3', 't2.mp3', 100, 1780000000, 100)",
            []
        ).unwrap();

        // Insert sample embeddings (384-dimensional)
        // Track 1: all 0.05
        let v1 = vec![0.05f32; 384];
        let bytes1: Vec<u8> = v1.iter().flat_map(|&f| f.to_le_bytes()).collect();
        conn.execute(
            "INSERT INTO description_embeddings (track_id, embedding) VALUES (1, ?1)",
            [bytes1]
        ).unwrap();

        // Track 2: all 0.5
        let v2 = vec![0.5f32; 384];
        let bytes2: Vec<u8> = v2.iter().flat_map(|&f| f.to_le_bytes()).collect();
        conn.execute(
            "INSERT INTO description_embeddings (track_id, embedding) VALUES (2, ?1)",
            [bytes2]
        ).unwrap();

        // Query using a vector identical to Track 1
        let query_vec = vec![0.05f32; 384];
        let q_bytes: Vec<u8> = query_vec.iter().flat_map(|&f| f.to_le_bytes()).collect();

        let mut stmt = conn.prepare(
            "SELECT de.track_id, de.distance
             FROM description_embeddings de
             WHERE de.embedding MATCH ?1 AND k = 2
             ORDER BY de.distance ASC"
        ).unwrap();

        let results: Vec<(i64, f64)> = stmt.query_map([q_bytes], |row| {
            Ok((row.get(0)?, row.get(1)?))
        }).unwrap().map(|r| r.unwrap()).collect();

        assert_eq!(results.len(), 2);
        // Track 1 should be closest with a distance of 0.0
        assert_eq!(results[0].0, 1);
        assert!((results[0].1 - 0.0).abs() < 1e-9);
        // Track 2 should be further
        assert_eq!(results[1].0, 2);
        assert!(results[1].1 > 0.0);
    }

    #[test]
    fn test_sqlite_vec_clap_match_query() {
        let conn = setup_test_db();

        // Setup prerequisites (watched directories + tracks) to prevent constraint errors
        conn.execute(
            "INSERT INTO watched_directories (id, name, path) VALUES (1, 'Test', '/test')",
            [],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO tracks (id, watched_directory_id, path, filename, size_bytes, last_modified, duration_seconds)
             VALUES (1, 1, '/test/a.mp3', 'a.mp3', 100, 100, 10),
                    (2, 1, '/test/b.mp3', 'b.mp3', 100, 100, 10)",
            [],
        ).unwrap();

        // Seed audio_embeddings (CLAP is 512-dimensional)
        let vec1 = vec![0.05f32; 512];
        let bytes1: Vec<u8> = vec1.iter().flat_map(|&f| f.to_le_bytes()).collect();

        let vec2 = vec![0.1f32; 512];
        let bytes2: Vec<u8> = vec2.iter().flat_map(|&f| f.to_le_bytes()).collect();

        conn.execute(
            "INSERT INTO audio_embeddings (track_id, embedding) VALUES (1, ?1)",
            [bytes1]
        ).unwrap();
        conn.execute(
            "INSERT INTO audio_embeddings (track_id, embedding) VALUES (2, ?1)",
            [bytes2]
        ).unwrap();

        // Query identical to Track 1
        let query_vec = vec![0.05f32; 512];
        let q_bytes: Vec<u8> = query_vec.iter().flat_map(|&f| f.to_le_bytes()).collect();

        let mut stmt = conn.prepare(
            "SELECT ae.track_id, ae.distance
             FROM audio_embeddings ae
             WHERE ae.embedding MATCH ?1 AND k = 2
             ORDER BY ae.distance ASC"
        ).unwrap();

        let results: Vec<(i64, f64)> = stmt.query_map([q_bytes], |row| {
            Ok((row.get(0)?, row.get(1)?))
        }).unwrap().map(|r| r.unwrap()).collect();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, 1);
        assert!((results[0].1 - 0.0).abs() < 1e-9);
        assert_eq!(results[1].0, 2);
        assert!(results[1].1 > 0.0);
    }
}
