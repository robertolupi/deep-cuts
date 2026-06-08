use crate::error::AppError;
use rusqlite::Connection;
use std::sync::Mutex;
use std::time::SystemTime;

/// @concept Playlists
/// @skill add-ipc-command
/// Playlist represents a user-created collection of audio tracks.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct PlaylistTrack {
    pub playlist_id: i64,
    pub track_id: Option<i64>,
    pub position: i32,
    pub cached_title: String,
    pub cached_artist: String,
    
    // Joined track columns
    pub path: Option<String>,
    pub filename: Option<String>,
    pub duration_seconds: Option<f64>,
    pub bpm: Option<f64>,
    pub key: Option<String>,
    pub scale: Option<String>,
    pub size_bytes: Option<i64>,
    pub last_modified: Option<i64>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct SavedSearch {
    pub id: i64,
    pub name: String,
    pub query_json: String,
    pub schema_version: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Playlists Command Handlers
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_playlists(
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<Vec<Playlist>, AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
        
    let mut stmt = conn.prepare("SELECT id, name, created_at, updated_at FROM playlists ORDER BY name ASC")?;
    let rows = stmt.query_map([], |row| {
        Ok(Playlist {
            id: row.get(0)?,
            name: row.get(1)?,
            created_at: row.get(2)?,
            updated_at: row.get(3)?,
        })
    })?;
    
    let playlists: Vec<Playlist> = rows.collect::<Result<Vec<_>, _>>()?;
    Ok(playlists)
}

#[tauri::command]
pub fn create_playlist(
    conn_state: tauri::State<'_, Mutex<Connection>>,
    name: String,
) -> Result<i64, AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
        
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::Generic("Playlist name cannot be empty".to_string()));
    }
    
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
        
    conn.execute(
        "INSERT INTO playlists (name, created_at, updated_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![trimmed, now, now],
    )?;
    
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn delete_playlist(
    conn_state: tauri::State<'_, Mutex<Connection>>,
    id: i64,
) -> Result<(), AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
        
    conn.execute("DELETE FROM playlists WHERE id = ?1", [id])?;
    Ok(())
}

#[tauri::command]
pub fn rename_playlist(
    conn_state: tauri::State<'_, Mutex<Connection>>,
    id: i64,
    new_name: String,
) -> Result<(), AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
        
    let trimmed = new_name.trim();
    if trimmed.is_empty() {
        return Err(AppError::Generic("Playlist name cannot be empty".to_string()));
    }
    
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
        
    conn.execute(
        "UPDATE playlists SET name = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![trimmed, now, id],
    )?;
    
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Playlist Tracks Command Handlers
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_playlist_tracks(
    conn_state: tauri::State<'_, Mutex<Connection>>,
    playlist_id: i64,
) -> Result<Vec<PlaylistTrack>, AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
        
    let mut stmt = conn.prepare(
        "SELECT pt.playlist_id, pt.track_id, pt.position, pt.cached_title, pt.cached_artist,
                t.path, t.filename, t.duration_seconds, t.bpm, t.key, t.scale, t.size_bytes, t.last_modified
         FROM playlist_tracks pt
         LEFT JOIN tracks t ON t.id = pt.track_id
         WHERE pt.playlist_id = ?1
         ORDER BY pt.position ASC"
    )?;
    
    let rows = stmt.query_map([playlist_id], |row| {
        Ok(PlaylistTrack {
            playlist_id: row.get(0)?,
            track_id: row.get(1)?,
            position: row.get(2)?,
            cached_title: row.get(3)?,
            cached_artist: row.get(4)?,
            path: row.get(5)?,
            filename: row.get(6)?,
            duration_seconds: row.get(7)?,
            bpm: row.get(8)?,
            key: row.get(9)?,
            scale: row.get(10)?,
            size_bytes: row.get(11)?,
            last_modified: row.get(12)?,
        })
    })?;
    
    let tracks: Vec<PlaylistTrack> = rows.collect::<Result<Vec<_>, _>>()?;
    Ok(tracks)
}

#[tauri::command]
pub fn add_tracks_to_playlist(
    conn_state: tauri::State<'_, Mutex<Connection>>,
    playlist_id: i64,
    track_ids: Vec<i64>,
) -> Result<(), AppError> {
    let mut conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
        
    if track_ids.is_empty() {
        return Ok(());
    }
    
    // Check next insert position
    let next_pos: i32 = conn.query_row(
        "SELECT COALESCE(MAX(position) + 1, 0) FROM playlist_tracks WHERE playlist_id = ?1",
        [playlist_id],
        |row| row.get(0)
    ).unwrap_or(0);
    
    let tx = conn.transaction()?;
    
    for (offset, track_id) in track_ids.into_iter().enumerate() {
        let position = next_pos + offset as i32;
        
        // Fetch track title, artist and filename for cached tombstoning metadata
        let track_details: (Option<String>, Option<String>, String) = tx.query_row(
            "SELECT title, artist, filename FROM tracks WHERE id = ?1",
            [track_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        )?;
        
        let cached_title = track_details.0.unwrap_or(track_details.2);
        let cached_artist = track_details.1.unwrap_or_else(|| "Unknown Artist".to_string());
        
        tx.execute(
            "INSERT INTO playlist_tracks (playlist_id, track_id, position, cached_title, cached_artist)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![playlist_id, track_id, position, cached_title, cached_artist],
        )?;
    }
    
    tx.commit()?;
    
    // Update playlist updated_at timestamp
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    conn.execute("UPDATE playlists SET updated_at = ?1 WHERE id = ?2", rusqlite::params![now, playlist_id])?;
    
    Ok(())
}

#[tauri::command]
pub fn remove_track_from_playlist(
    conn_state: tauri::State<'_, Mutex<Connection>>,
    playlist_id: i64,
    position: i32,
) -> Result<(), AppError> {
    let mut conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
        
    let tx = conn.transaction()?;
    
    tx.execute(
        "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND position = ?2",
        rusqlite::params![playlist_id, position],
    )?;
    
    // Close the gap in positions
    tx.execute(
        "UPDATE playlist_tracks SET position = position - 1 
         WHERE playlist_id = ?1 AND position > ?2",
        rusqlite::params![playlist_id, position],
    )?;
    
    tx.commit()?;
    
    // Update playlist updated_at timestamp
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    conn.execute("UPDATE playlists SET updated_at = ?1 WHERE id = ?2", rusqlite::params![now, playlist_id])?;
    
    Ok(())
}

#[tauri::command]
pub fn reorder_playlist_track(
    conn_state: tauri::State<'_, Mutex<Connection>>,
    playlist_id: i64,
    from_pos: i32,
    to_pos: i32,
) -> Result<(), AppError> {
    let mut conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
    if from_pos == to_pos {
        return Ok(());
    }
    
    struct MinTrack {
        position: i32,
        track_id: Option<i64>,
    }
    
    // Retrieve all tracks ordered by position in a nested scope to drop immutable borrows
    let mut tracks: Vec<MinTrack> = {
        let mut stmt = conn.prepare(
            "SELECT position, track_id FROM playlist_tracks WHERE playlist_id = ?1 ORDER BY position ASC"
        )?;
        
        let rows = stmt.query_map([playlist_id], |row| {
            Ok(MinTrack {
                position: row.get(0)?,
                track_id: row.get(1)?,
            })
        })?;
        
        rows.collect::<Result<Vec<_>, _>>()?
    };

    if from_pos < 0 || from_pos >= tracks.len() as i32 || to_pos < 0 || to_pos >= tracks.len() as i32 {
        return Err(AppError::Generic("Invalid reorder index bounds".to_string()));
    }
    
    // Reorder in memory vector
    let item = tracks.remove(from_pos as usize);
    tracks.insert(to_pos as usize, item);
    
    // Apply changes in a single SQL transaction using a UNIQUE-conflict-safe negative position buffer
    let tx = conn.transaction()?;
    
    // 1. Shift all positions temporarily to negative values to avoid UNIQUE primary key constraints
    for (i, track) in tracks.iter().enumerate() {
        tx.execute(
            "UPDATE playlist_tracks SET position = ?1 
             WHERE playlist_id = ?2 AND position = ?3 AND COALESCE(track_id, -1) = ?4",
            rusqlite::params![-(i as i32) - 1, playlist_id, track.position, track.track_id.unwrap_or(-1)],
        )?;
    }
    
    // 2. Restore all negative positions to positive final offsets
    tx.execute(
        "UPDATE playlist_tracks SET position = -position - 1 
         WHERE playlist_id = ?1 AND position < 0",
        [playlist_id],
    )?;
    
    tx.commit()?;
    
    // Update playlist updated_at timestamp
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    conn.execute("UPDATE playlists SET updated_at = ?1 WHERE id = ?2", rusqlite::params![now, playlist_id])?;
    
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Saved Searches Command Handlers
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_saved_searches(
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<Vec<SavedSearch>, AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
        
    let mut stmt = conn.prepare("SELECT id, name, query_json, schema_version, created_at, updated_at FROM saved_searches ORDER BY name ASC")?;
    let rows = stmt.query_map([], |row| {
        Ok(SavedSearch {
            id: row.get(0)?,
            name: row.get(1)?,
            query_json: row.get(2)?,
            schema_version: row.get(3)?,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
        })
    })?;
    
    let searches: Vec<SavedSearch> = rows.collect::<Result<Vec<_>, _>>()?;
    Ok(searches)
}

#[tauri::command]
pub fn create_saved_search(
    conn_state: tauri::State<'_, Mutex<Connection>>,
    name: String,
    query_json: String,
) -> Result<i64, AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
        
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::Generic("Saved search name cannot be empty".to_string()));
    }
    
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
        
    conn.execute(
        "INSERT INTO saved_searches (name, query_json, schema_version, created_at, updated_at) 
         VALUES (?1, ?2, 1, ?3, ?4)",
        rusqlite::params![trimmed, query_json, now, now],
    )?;
    
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn delete_saved_search(
    conn_state: tauri::State<'_, Mutex<Connection>>,
    id: i64,
) -> Result<(), AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
        
    conn.execute("DELETE FROM saved_searches WHERE id = ?1", [id])?;
    Ok(())
}

#[tauri::command]
pub fn update_saved_search(
    conn_state: tauri::State<'_, Mutex<Connection>>,
    id: i64,
    query_json: String,
) -> Result<(), AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
        
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
        
    conn.execute(
        "UPDATE saved_searches SET query_json = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![query_json, now, id],
    )?;
    
    Ok(())
}

#[tauri::command]
pub fn get_playlists_for_track(
    conn_state: tauri::State<'_, Mutex<Connection>>,
    track_id: i64,
) -> Result<Vec<Playlist>, AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
        
    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, p.created_at, p.updated_at
         FROM playlists p
         JOIN playlist_tracks pt ON pt.playlist_id = p.id
         WHERE pt.track_id = ?1
         ORDER BY p.name ASC"
    )?;
    let rows = stmt.query_map([track_id], |row| {
        Ok(Playlist {
            id: row.get(0)?,
            name: row.get(1)?,
            created_at: row.get(2)?,
            updated_at: row.get(3)?,
        })
    })?;
    
    let playlists: Vec<Playlist> = rows.collect::<Result<Vec<_>, _>>()?;
    Ok(playlists)
}

#[tauri::command]
pub fn remove_track_from_playlist_by_id(
    conn_state: tauri::State<'_, Mutex<Connection>>,
    playlist_id: i64,
    track_id: i64,
) -> Result<(), AppError> {
    let mut conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
        
    let tx = conn.transaction()?;
    
    // Get all positions for this track in this playlist (in case it was added multiple times)
    let positions: Vec<i32> = {
        let mut stmt = tx.prepare(
            "SELECT position FROM playlist_tracks WHERE playlist_id = ?1 AND track_id = ?2 ORDER BY position DESC"
        )?;
        let rows = stmt.query_map(rusqlite::params![playlist_id, track_id], |row| row.get(0))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    for pos in positions {
        tx.execute(
            "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND position = ?2",
            rusqlite::params![playlist_id, pos],
        )?;
        
        tx.execute(
            "UPDATE playlist_tracks SET position = position - 1 
             WHERE playlist_id = ?1 AND position > ?2",
            rusqlite::params![playlist_id, pos],
        )?;
    }
    
    tx.commit()?;
    
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    conn.execute("UPDATE playlists SET updated_at = ?1 WHERE id = ?2", rusqlite::params![now, playlist_id])?;
    
    Ok(())
}

#[derive(serde::Deserialize, Clone, Debug)]
pub struct M3UTrackInfo {
    pub path: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub duration_seconds: Option<f64>,
}

#[tauri::command]
pub fn export_m3u_playlist(tracks: Vec<M3UTrackInfo>) -> Result<bool, AppError> {
    if tracks.is_empty() {
        return Err(AppError::Generic("No tracks to export".to_string()));
    }
    
    // Open save file dialog
    let file = rfd::FileDialog::new()
        .set_title("Export M3U Playlist")
        .add_filter("M3U Playlist", &["m3u"])
        .set_file_name("playlist.m3u")
        .save_file();
        
    if let Some(path_buf) = file {
        use std::io::Write;
        let mut f = std::fs::File::create(&path_buf)?;
        
        writeln!(f, "#EXTM3U")?;
        for t in tracks {
            let duration = t.duration_seconds.unwrap_or(0.0).round() as i64;
            let artist = t.artist.as_deref().unwrap_or("Unknown Artist");
            let title = t.title.as_deref().unwrap_or("Unknown Title");
            
            writeln!(f, "#EXTINF:{},{} - {}", duration, artist, title)?;
            writeln!(f, "{}", t.path)?;
        }
        
        Ok(true)
    } else {
        Ok(false)
    }
}
