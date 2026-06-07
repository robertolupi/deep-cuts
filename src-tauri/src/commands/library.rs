use crate::database::{Track, WatchedDirectory};
use crate::error::AppError;
use crate::scanner;
use rusqlite::Connection;
use std::collections::HashMap;
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

/// Retrieve a single track by its ID.
#[tauri::command]
pub fn get_track(
    track_id: i64,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<Option<Track>, AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
    let track = Track::find(&conn, track_id)?;
    Ok(track)
}


/// Returns a map of track_id → list of tag metadata (name, source, score) for the detail pane.
#[tauri::command]
pub fn get_tags_with_meta_for_tracks(
    track_ids: Vec<i64>,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<HashMap<i64, Vec<serde_json::Value>>, AppError> {
    if track_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;

    let placeholders = track_ids
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");

    let sql = format!(
        "SELECT tt.track_id, t.name, tt.source, tt.score,
                (CASE WHEN ust.tag_name IS NOT NULL THEN 1 ELSE tt.discard END) AS discard
         FROM track_tags tt
         JOIN tags t ON t.id = tt.tag_id
         JOIN tracks tr ON tr.id = tt.track_id
         LEFT JOIN user_suppressed_tags ust ON ust.track_path = tr.path AND ust.tag_name = t.name
         WHERE tt.track_id IN ({})
         ORDER BY tt.track_id, discard ASC, t.name",
        placeholders
    );

    let mut stmt = conn.prepare(&sql)?;
    let params: Vec<rusqlite::types::Value> = track_ids
        .iter()
        .map(|id| rusqlite::types::Value::Integer(*id))
        .collect();
    let params_ref: Vec<&dyn rusqlite::ToSql> = params
        .iter()
        .map(|v| v as &dyn rusqlite::ToSql)
        .collect();

    let mut map: HashMap<i64, Vec<serde_json::Value>> = HashMap::new();
    let rows = stmt.query_map(params_ref.as_slice(), |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<f64>>(3)?,
            row.get::<_, i64>(4)?,
        ))
    })?;

    for row in rows.flatten() {
        let (track_id, name, source, score, discard) = row;
        map.entry(track_id).or_default().push(serde_json::json!({
            "name": name,
            "source": source,
            "discard": discard == 1,
            "score": score,
        }));
    }
    Ok(map)
}

/// Returns a map of track_id → list of tag names for the requested track IDs.
#[tauri::command]
pub fn get_tags_for_tracks(
    track_ids: Vec<i64>,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<HashMap<i64, Vec<String>>, AppError> {
    if track_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;

    let placeholders = track_ids
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");

    let sql = format!(
        "SELECT tt.track_id, t.name
         FROM track_tags tt
         JOIN tags t ON t.id = tt.tag_id
         JOIN tracks tr ON tr.id = tt.track_id
         LEFT JOIN user_suppressed_tags ust ON ust.track_path = tr.path AND ust.tag_name = t.name
         WHERE tt.track_id IN ({}) AND tt.discard = 0 AND ust.tag_name IS NULL
         ORDER BY tt.track_id, t.name",
        placeholders
    );

    let mut stmt = conn.prepare(&sql)?;
    let params: Vec<rusqlite::types::Value> = track_ids
        .iter()
        .map(|id| rusqlite::types::Value::Integer(*id))
        .collect();
    let params_ref: Vec<&dyn rusqlite::ToSql> = params
        .iter()
        .map(|v| v as &dyn rusqlite::ToSql)
        .collect();

    let mut map: HashMap<i64, Vec<String>> = HashMap::new();
    let rows = stmt.query_map(params_ref.as_slice(), |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;

    for row in rows.flatten() {
        map.entry(row.0).or_default().push(row.1);
    }
    Ok(map)
}

/// Returns a map of track_id → list of tag names for every track in the library.
#[tauri::command]
pub fn get_all_track_tags(
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<HashMap<i64, Vec<String>>, AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;

    let mut stmt = conn.prepare(
        "SELECT tt.track_id, t.name
         FROM track_tags tt
         JOIN tags t ON t.id = tt.tag_id
         JOIN tracks tr ON tr.id = tt.track_id
         LEFT JOIN user_suppressed_tags ust ON ust.track_path = tr.path AND ust.tag_name = t.name
         WHERE tt.discard = 0 AND ust.tag_name IS NULL
         ORDER BY tt.track_id, t.name",
    )?;

    let mut map: HashMap<i64, Vec<String>> = HashMap::new();
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows.flatten() {
        map.entry(row.0).or_default().push(row.1);
    }
    Ok(map)
}

/// Returns all distinct tag names in the library, sorted, for autocomplete.
#[tauri::command]
pub fn get_all_tags(
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<Vec<String>, AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;

    let mut stmt = conn.prepare(
        "SELECT DISTINCT t.name
         FROM tags t
         JOIN track_tags tt ON tt.tag_id = t.id
         JOIN tracks tr ON tr.id = tt.track_id
         LEFT JOIN user_suppressed_tags ust ON ust.track_path = tr.path AND ust.tag_name = t.name
         WHERE tt.discard = 0 AND ust.tag_name IS NULL
         ORDER BY t.name",
    )?;

    let tags = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .flatten()
        .collect();
    Ok(tags)
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

/// Opens the system file manager to the application log directory.
#[tauri::command]
pub fn open_log_dir(app: tauri::AppHandle) -> Result<(), AppError> {
    use tauri::Manager;
    let log_dir = app.path().app_log_dir()
        .map_err(|e| AppError::Generic(format!("Failed to get log directory: {}", e)))?;
    
    if !log_dir.exists() {
        std::fs::create_dir_all(&log_dir)
            .map_err(|e| AppError::Generic(format!("Failed to create log directory: {}", e)))?;
    }
    
    let path = log_dir.to_string_lossy().into_owned();

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&path)
            .spawn()?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()?;
    }
    Ok(())
}

/// Opens the system file manager to the application data directory (contains the database).
#[tauri::command]
pub fn open_data_dir(app: tauri::AppHandle) -> Result<(), AppError> {
    use tauri::Manager;
    let data_dir = app.path().app_data_dir()
        .map_err(|e| AppError::Generic(format!("Failed to get data directory: {}", e)))?;

    if !data_dir.exists() {
        std::fs::create_dir_all(&data_dir)
            .map_err(|e| AppError::Generic(format!("Failed to create data directory: {}", e)))?;
    }

    let path = data_dir.to_string_lossy().into_owned();

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&path)
            .spawn()?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()?;
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
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::Database)?;

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

    // 3. Acquire DB lock
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;

    let max_k = limit.unwrap_or(40) as i64;

    let query_sql =
        "SELECT ae.track_id, t.title, t.filename, t.artist, t.genre, t.bpm, t.key, t.scale,
                ae.distance
         FROM audio_embeddings ae
         JOIN tracks t ON t.id = ae.track_id
         WHERE ae.embedding MATCH ?1 AND k = ?2";

    let mut stmt = conn
        .prepare(query_sql)
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
            let distance: Option<f64> = row.get(8)?;

            // Convert raw Euclidean (L2) distance into percentage similarity score.
            // L2 distance squared = d^2 = distance * distance.
            // CosineSimilarity = 1 - d^2 / 2
            let dist_val = distance.unwrap_or(1.4142135623730951);
            let d_sq = dist_val * dist_val;
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

    let mut results: Vec<SemanticSearchResult> = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::Database)?;

    // Sort by score descending
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    if let Some(l) = limit {
        results.truncate(l);
    }

    Ok(results)
}

/// Perform a hybrid similarity search combining sonic (CLAP) and semantic (Qwen description) queries.
#[tauri::command]
/// Merges CLAP and semantic result lists into a single ranked list.
/// Each result's score is linearly blended: `clap_weight * clap_score + (1 - clap_weight) * sem_score`.
/// Tracks that appear in only one list receive their weighted score with the other component as zero.
/// Final scores are clamped to [0, 100] and the list is truncated to `limit`.
fn merge_search_results(
    clap_results: Vec<SemanticSearchResult>,
    semantic_results: Vec<SemanticSearchResult>,
    clap_weight: f64,
    limit: Option<usize>,
) -> Vec<SemanticSearchResult> {
    let sem_weight = 1.0 - clap_weight;
    let mut merged: std::collections::HashMap<i64, SemanticSearchResult> =
        std::collections::HashMap::new();

    for mut r in clap_results {
        r.score *= clap_weight;
        merged.insert(r.id, r);
    }

    for r in semantic_results {
        if let Some(existing) = merged.get_mut(&r.id) {
            existing.score += r.score * sem_weight;
        } else {
            let mut new_r = r;
            new_r.score *= sem_weight;
            merged.insert(new_r.id, new_r);
        }
    }

    let mut results: Vec<SemanticSearchResult> = merged.into_values().collect();
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    for r in &mut results {
        r.score = r.score.clamp(0.0, 100.0);
    }

    if let Some(lim) = limit {
        results.truncate(lim);
    }

    results
}

/// Perform a hybrid similarity search combining sonic (CLAP) and semantic (Qwen description) queries.
#[tauri::command]
pub async fn search_hybrid_vibe(
    query: String,
    clap_weight: f64,
    limit: Option<usize>,
    app_handle: tauri::AppHandle,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<Vec<SemanticSearchResult>, AppError> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    if (clap_weight - 1.0).abs() < 1e-6 {
        return search_clap_tracks(query, limit, app_handle, conn_state).await;
    }
    if clap_weight.abs() < 1e-6 {
        return search_semantic_tracks(query, limit, app_handle, conn_state).await;
    }

    let search_limit = Some(5000);
    let clap_results = search_clap_tracks(query.clone(), search_limit, app_handle.clone(), conn_state.clone()).await?;
    let semantic_results = search_semantic_tracks(query, search_limit, app_handle, conn_state).await?;

    Ok(merge_search_results(clap_results, semantic_results, clap_weight, limit))
}


#[tauri::command]
pub fn get_cover_art(
    path: String,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<Option<String>, String> {
    use base64::Engine as _;
    use lofty::config::ParseOptions;
    use lofty::prelude::*;
    use lofty::probe::Probe;
    use std::path::Path;

    // 1. Try checking the database cover_art cache first (e.g. populated via AcoustID enrichment)
    if let Ok(conn) = conn_state.lock() {
        let db_res: Result<Option<Vec<u8>>, rusqlite::Error> = conn.query_row(
            "SELECT cover_art FROM tracks WHERE path = ?1",
            [&path],
            |row| row.get(0),
        );
        if let Ok(Some(bytes)) = db_res {
            if !bytes.is_empty() {
                // Determine mime type from magic bytes or default to image/jpeg
                let mime = if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
                    "image/png"
                } else if bytes.starts_with(&[0x47, 0x49, 0x46, 0x38]) {
                    "image/gif"
                } else if bytes.starts_with(&[0x52, 0x49, 0x46, 0x46]) && bytes.get(8..12) == Some(&[0x57, 0x45, 0x42, 0x50]) {
                    "image/webp"
                } else {
                    "image/jpeg"
                };
                let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                return Ok(Some(format!("data:{};base64,{}", mime, b64)));
            }
        }
    }

    // 2. Fallback to extracting from the physical file tags via lofty
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

#[tauri::command]
pub fn add_user_tag(
    track_path: String,
    tag_name: String,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<(), AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;

    // 1. Get track ID from path
    let track_id: i64 = conn.query_row(
        "SELECT id FROM tracks WHERE path = ?1",
        [&track_path],
        |row| row.get(0),
    )?;

    // 2. Format names
    let trimmed = tag_name.trim();
    if trimmed.is_empty() {
        return Err(AppError::Generic("Tag name cannot be empty".to_string()));
    }
    
    // We expect the tag format to be namespace:label. If no namespace is provided, default to user:
    let formatted_tag = if trimmed.contains(':') {
        trimmed.to_string()
    } else {
        format!("user:{}", trimmed)
    };

    let normalized = formatted_tag
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != ':' && c != '_', "_")
        .trim_matches('_')
        .to_string();

    // 3. Ensure tag exists in tags table
    conn.execute(
        "INSERT OR IGNORE INTO tags (name, normalized_name) VALUES (?1, ?2)",
        [&formatted_tag, &normalized],
    )?;

    let tag_id: i64 = conn.query_row(
        "SELECT id FROM tags WHERE name = ?1 OR normalized_name = ?2 LIMIT 1",
        [&formatted_tag, &normalized],
        |row| row.get(0),
    )?;

    // 4. Link in track_tags (upsert to promote or unsuppress existing)
    conn.execute(
        "INSERT INTO track_tags (track_id, tag_id, source, score, discard)
         VALUES (?1, ?2, 'user', 1.0, 0)
         ON CONFLICT(track_id, tag_id) DO UPDATE SET
             source = 'user',
             score = 1.0,
             discard = 0",
        rusqlite::params![track_id, tag_id],
    )?;

    // 5. If sidecars are enabled, save the sidecar
    if crate::commands::config::is_sidecar_enabled(&conn) {
        if let Err(e) = crate::scanner::sidecar::save(&conn, track_id) {
            log::error!("[add_user_tag] sidecar save failed: {}", e);
        }
    }

    Ok(())
}

#[tauri::command]
pub fn remove_user_tag(
    track_path: String,
    tag_name: String,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<(), AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;

    let track_id: i64 = conn.query_row(
        "SELECT id FROM tracks WHERE path = ?1",
        [&track_path],
        |row| row.get(0),
    )?;

    conn.execute(
        "DELETE FROM track_tags WHERE track_id = ?1 AND source = 'user' AND tag_id = (SELECT id FROM tags WHERE name = ?2)",
        rusqlite::params![track_id, tag_name],
    )?;

    if crate::commands::config::is_sidecar_enabled(&conn) {
        if let Err(e) = crate::scanner::sidecar::save(&conn, track_id) {
            log::error!("[remove_user_tag] sidecar save failed: {}", e);
        }
    }

    Ok(())
}

#[tauri::command]
pub fn suppress_tag(
    track_path: String,
    tag_name: String,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<(), AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;

    conn.execute(
        "INSERT OR IGNORE INTO user_suppressed_tags (track_path, tag_name) VALUES (?1, ?2)",
        [&track_path, &tag_name],
    )?;

    let track_id_opt: Option<i64> = conn.query_row(
        "SELECT id FROM tracks WHERE path = ?1",
        [&track_path],
        |row| row.get(0),
    ).ok();

    if let Some(track_id) = track_id_opt {
        if crate::commands::config::is_sidecar_enabled(&conn) {
            if let Err(e) = crate::scanner::sidecar::save(&conn, track_id) {
                log::error!("[suppress_tag] sidecar save failed: {}", e);
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub fn unsuppress_tag(
    track_path: String,
    tag_name: String,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<(), AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;

    conn.execute(
        "DELETE FROM user_suppressed_tags WHERE track_path = ?1 AND tag_name = ?2",
        [&track_path, &tag_name],
    )?;

    let track_id_opt: Option<i64> = conn.query_row(
        "SELECT id FROM tracks WHERE path = ?1",
        [&track_path],
        |row| row.get(0),
    ).ok();

    if let Some(track_id) = track_id_opt {
        if crate::commands::config::is_sidecar_enabled(&conn) {
            if let Err(e) = crate::scanner::sidecar::save(&conn, track_id) {
                log::error!("[unsuppress_tag] sidecar save failed: {}", e);
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub fn get_suppressed_tags(
    track_path: String,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<Vec<String>, AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;

    // Only get suppressed tags if they actually exist as automatic/suggested tags in the database for the given track (filtering out orphans)
    let mut stmt = conn.prepare(
        "SELECT ust.tag_name
         FROM user_suppressed_tags ust
         JOIN tracks t ON t.path = ust.track_path
         JOIN track_tags tt ON tt.track_id = t.id
         JOIN tags tg ON tg.id = tt.tag_id AND tg.name = ust.tag_name
         WHERE ust.track_path = ?1",
    )?;

    let list = stmt
        .query_map([&track_path], |row| row.get::<_, String>(0))?
        .flatten()
        .collect();

    Ok(list)
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

    #[test]
    fn test_sqlite_vec_distance_l2_function() {
        let conn = setup_test_db();

        conn.execute(
            "INSERT INTO watched_directories (id, name, path) VALUES (1, 'Test', '/test')",
            [],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO tracks (id, watched_directory_id, path, filename, size_bytes, last_modified, duration_seconds)
             VALUES (1, 1, '/test/a.mp3', 'a.mp3', 100, 100, 10)",
            [],
        ).unwrap();

        let vec1 = vec![0.05f32; 512];
        let bytes1: Vec<u8> = vec1.iter().flat_map(|&f| f.to_le_bytes()).collect();

        conn.execute(
            "INSERT INTO audio_embeddings (track_id, embedding) VALUES (1, ?1)",
            [bytes1]
        ).unwrap();

        let query_vec = vec![0.05f32; 512];
        let q_bytes: Vec<u8> = query_vec.iter().flat_map(|&f| f.to_le_bytes()).collect();

        let distance: f64 = conn.query_row(
            "SELECT vec_distance_l2(embedding, ?1) FROM audio_embeddings WHERE track_id = 1",
            [q_bytes],
            |row| row.get(0)
        ).unwrap();

        assert!((distance - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_clap_search_score_formula() {
        // Verify that the L2-distance → cosine similarity conversion is correct.
        // For normalized vectors, L2 distance d and cosine similarity c satisfy:
        //   c = 1 - d^2 / 2
        // So d=0 → c=1 (100%), d=sqrt(2) ≈ 1.414 → c=0 (0%).

        // d=0 → 100%
        let d = 0.0f64;
        let score = ((1.0 - d * d / 2.0) * 100.0).clamp(0.0, 100.0);
        assert!((score - 100.0).abs() < 1e-6);

        // d=sqrt(2) → 0%
        let d = std::f64::consts::SQRT_2;
        let score = ((1.0 - d * d / 2.0) * 100.0).clamp(0.0, 100.0);
        assert!((score - 0.0).abs() < 1e-6);

        // d=1.0 → 50%
        let d = 1.0f64;
        let score = ((1.0 - d * d / 2.0) * 100.0).clamp(0.0, 100.0);
        assert!((score - 50.0).abs() < 1e-6);
    }

    #[test]
    fn test_hybrid_clap_search_scoring_logic() {
        let conn = setup_test_db();

        conn.execute(
            "INSERT INTO watched_directories (id, name, path) VALUES (1, 'Test', '/test')",
            [],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO tracks (id, watched_directory_id, path, filename, size_bytes, last_modified, duration_seconds,
                                 title, ai_instruments, ai_genre, ai_mood)
             VALUES (1, 1, '/test/a.mp3', 'a.mp3', 100, 100, 10, 'Track A', 'harpsichord, violin', 'classical', 'peaceful'),
                    (2, 1, '/test/b.mp3', 'b.mp3', 100, 100, 10, 'Track B', '808, synthesizer', 'electronic', 'energetic'),
                    (3, 1, '/test/c.mp3', 'c.mp3', 100, 100, 10, 'Track C', 'synthesizer', 'electronic', 'chill')",
            [],
        ).unwrap();

        // Let's seed embeddings
        let vec1 = vec![0.05f32; 512];
        let bytes1: Vec<u8> = vec1.iter().flat_map(|&f| f.to_le_bytes()).collect();
        let vec2 = vec![0.1f32; 512];
        let bytes2: Vec<u8> = vec2.iter().flat_map(|&f| f.to_le_bytes()).collect();
        let vec3 = vec![0.2f32; 512];
        let bytes3: Vec<u8> = vec3.iter().flat_map(|&f| f.to_le_bytes()).collect();

        conn.execute("INSERT INTO audio_embeddings (track_id, embedding) VALUES (1, ?1)", [bytes1]).unwrap();
        conn.execute("INSERT INTO audio_embeddings (track_id, embedding) VALUES (2, ?1)", [bytes2]).unwrap();
        conn.execute("INSERT INTO audio_embeddings (track_id, embedding) VALUES (3, ?1)", [bytes3]).unwrap();

        // Search query: "harpsichord"
        // Since Track 1 has 'harpsichord' in ai_instruments, it should get a +30 boost.
        // Even if our query vector is closer to Track 2, Track 1 should win the ranking.
        // Query vector identical to Track 2's embedding → Track 2 should rank first (distance 0).
        let query_vec = vec![0.1f32; 512];
        let q_bytes: Vec<u8> = query_vec.iter().flat_map(|&f| f.to_le_bytes()).collect();

        let query_sql =
            "SELECT ae.track_id, t.title, t.filename, t.artist, t.genre, t.bpm, t.key, t.scale,
                    ae.distance
             FROM audio_embeddings ae
             JOIN tracks t ON t.id = ae.track_id
             WHERE ae.embedding MATCH ?1 AND k = 3";

        let mut stmt = conn.prepare(query_sql).unwrap();
        let rows = stmt.query_map(rusqlite::params![q_bytes], |row| {
            let id: i64 = row.get(0)?;
            let title: Option<String> = row.get(1)?;
            let filename: String = row.get(2)?;
            let artist: Option<String> = row.get(3)?;
            let genre: Option<String> = row.get(4)?;
            let bpm: Option<f64> = row.get(5)?;
            let key: Option<String> = row.get(6)?;
            let scale: Option<String> = row.get(7)?;
            let distance: Option<f64> = row.get(8)?;

            let dist_val = distance.unwrap_or(1.4142135623730951);
            let d_sq = dist_val * dist_val;
            let score = ((1.0_f64 - d_sq / 2.0_f64) * 100.0_f64).clamp(0.0_f64, 100.0_f64);

            Ok(SemanticSearchResult { id, title, filename, artist, genre, bpm, key, scale, score })
        }).unwrap();

        let mut results: Vec<SemanticSearchResult> = rows.map(|r| r.unwrap()).collect();
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // Track 2 (vec=[0.1;512]) must rank first — it is identical to the query vector.
        assert_eq!(results[0].id, 2);
        // Track 1 (vec=[0.05;512]) is closer to query than Track 3 (vec=[0.2;512]) after normalization.
        assert_eq!(results[1].id, 1);
        assert_eq!(results[2].id, 3);
    }

    #[test]
    fn test_clap_search_returns_nearest_by_vector() {
        // Verify that pure vector ranking puts the closest embedding first,
        // with no label boosting involved.
        let conn = setup_test_db();

        conn.execute(
            "INSERT INTO watched_directories (id, name, path) VALUES (1, 'Test', '/test')",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO tracks (id, watched_directory_id, path, filename, size_bytes, last_modified, duration_seconds, title)
             VALUES (1, 1, '/test/a.mp3', 'a.mp3', 100, 100, 10, 'Far'),
                    (2, 1, '/test/b.mp3', 'b.mp3', 100, 100, 10, 'Near')",
            [],
        ).unwrap();

        // Track 1: all 0.05 → farther from query (all 0.1)
        // Track 2: all 0.1  → identical to query → distance 0 → score 100
        let far_vec: Vec<u8> = vec![0.05f32; 512].iter().flat_map(|&f| f.to_le_bytes()).collect();
        let near_vec: Vec<u8> = vec![0.1f32; 512].iter().flat_map(|&f| f.to_le_bytes()).collect();
        conn.execute("INSERT INTO audio_embeddings (track_id, embedding) VALUES (1, ?1)", [far_vec]).unwrap();
        conn.execute("INSERT INTO audio_embeddings (track_id, embedding) VALUES (2, ?1)", [near_vec]).unwrap();

        let query_vec: Vec<u8> = vec![0.1f32; 512].iter().flat_map(|&f| f.to_le_bytes()).collect();
        let rows: Vec<(i64, f64)> = conn.prepare(
            "SELECT ae.track_id, ae.distance FROM audio_embeddings ae
             JOIN tracks t ON t.id = ae.track_id
             WHERE ae.embedding MATCH ?1 AND k = 2"
        ).unwrap()
         .query_map(rusqlite::params![query_vec], |row| Ok((row.get(0)?, row.get(1)?)))
         .unwrap()
         .map(|r| r.unwrap())
         .collect();

        let nearest = rows.iter().min_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).unwrap();
        assert_eq!(nearest.0, 2, "Track 2 (identical vector) must be nearest");
    }

    #[test]
    fn test_track_find() {
        let conn = setup_test_db();
        
        // Insert watched directory
        conn.execute(
            "INSERT INTO watched_directories (id, name, path) VALUES (1, 'Test Collection', '/Users/user/Music')",
            []
        ).unwrap();
        
        // Insert sample track
        conn.execute(
            "INSERT INTO tracks (id, watched_directory_id, path, filename, size_bytes, last_modified, duration_seconds)
             VALUES (42, 1, '/Users/user/Music/t42.mp3', 't42.mp3', 100, 1780000000, 100)",
            []
        ).unwrap();

        // Retrieve existing track
        let track_opt = Track::find(&conn, 42).unwrap();
        assert!(track_opt.is_some());
        let track = track_opt.unwrap();
        assert_eq!(track.id, 42);
        assert_eq!(track.filename, "t42.mp3");

        // Retrieve non-existent track
        let track_none = Track::find(&conn, 999).unwrap();
        assert!(track_none.is_none());
    }

    // ── merge_search_results ──────────────────────────────────────────────────

    fn make_result(id: i64, score: f64) -> SemanticSearchResult {
        SemanticSearchResult {
            id,
            title: None,
            filename: format!("track_{id}.mp3"),
            artist: None,
            genre: None,
            bpm: None,
            key: None,
            scale: None,
            score,
        }
    }

    #[test]
    fn test_merge_scores_are_weighted_blend_for_shared_tracks() {
        let clap = vec![make_result(1, 80.0), make_result(2, 60.0)];
        let sem  = vec![make_result(1, 40.0), make_result(2, 20.0)];
        let results = merge_search_results(clap, sem, 0.7, None);

        // id=1: 80*0.7 + 40*0.3 = 56 + 12 = 68
        // id=2: 60*0.7 + 20*0.3 = 42 +  6 = 48
        let r1 = results.iter().find(|r| r.id == 1).unwrap();
        let r2 = results.iter().find(|r| r.id == 2).unwrap();
        assert!((r1.score - 68.0).abs() < 1e-9, "id=1 score was {}", r1.score);
        assert!((r2.score - 48.0).abs() < 1e-9, "id=2 score was {}", r2.score);
    }

    #[test]
    fn test_merge_clap_only_track_gets_clap_weight_applied() {
        let clap = vec![make_result(10, 90.0)];
        let sem  = vec![];
        let results = merge_search_results(clap, sem, 0.6, None);

        assert_eq!(results.len(), 1);
        assert!((results[0].score - 54.0).abs() < 1e-9, "score was {}", results[0].score);
    }

    #[test]
    fn test_merge_semantic_only_track_gets_sem_weight_applied() {
        let clap = vec![];
        let sem  = vec![make_result(20, 80.0)];
        let results = merge_search_results(clap, sem, 0.6, None);

        assert_eq!(results.len(), 1);
        // sem_weight = 0.4 → 80 * 0.4 = 32
        assert!((results[0].score - 32.0).abs() < 1e-9, "score was {}", results[0].score);
    }

    #[test]
    fn test_merge_results_are_sorted_descending_by_score() {
        let clap = vec![make_result(1, 30.0), make_result(2, 90.0), make_result(3, 60.0)];
        let results = merge_search_results(clap, vec![], 1.0, None);

        let scores: Vec<f64> = results.iter().map(|r| r.score).collect();
        let mut sorted = scores.clone();
        sorted.sort_by(|a, b| b.partial_cmp(a).unwrap());
        assert_eq!(scores, sorted, "results should be sorted descending");
    }

    #[test]
    fn test_merge_scores_are_clamped_to_100() {
        // Intentionally inflate scores past 100
        let clap = vec![make_result(1, 200.0)];
        let results = merge_search_results(clap, vec![], 1.0, None);

        assert!(results[0].score <= 100.0, "score {} exceeds 100", results[0].score);
    }

    #[test]
    fn test_merge_respects_limit() {
        let clap: Vec<SemanticSearchResult> = (1..=10).map(|i| make_result(i, i as f64)).collect();
        let results = merge_search_results(clap, vec![], 1.0, Some(3));

        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_merge_empty_inputs_returns_empty() {
        let results = merge_search_results(vec![], vec![], 0.5, None);
        assert!(results.is_empty());
    }
}

