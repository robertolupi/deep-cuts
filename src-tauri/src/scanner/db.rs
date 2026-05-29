use super::metadata::ParsedAudioTags;
use rusqlite::Connection;
use std::collections::HashSet;

/// Retrieves the size_bytes and last_modified UNIX time for a path to verify caching status.
pub fn get_cached_track_details(conn: &Connection, path: &str) -> Option<(i64, i64)> {
    conn.query_row(
        "SELECT size_bytes, last_modified FROM tracks WHERE path = ?1",
        [path],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .ok()
}

/// Transactionally inserts or updates a list of track metadata in high-speed batches.
pub fn upsert_tracks_transactional(
    conn: &mut Connection,
    tracks: &[ParsedAudioTags],
) -> Result<(), rusqlite::Error> {
    if tracks.is_empty() {
        return Ok(());
    }

    let tx = conn.transaction()?;

    {
        let mut stmt = tx.prepare(
            "INSERT INTO tracks (
                watched_directory_id, path, filename, size_bytes, last_modified,
                duration_seconds, sample_rate, bitrate, channels, bit_depth,
                title, artist, album, genre, year, track_number, track_total,
                disc_number, disc_total, album_artist, composer, comment, bpm, lyrics
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24
            )
            ON CONFLICT(path) DO UPDATE SET
                watched_directory_id = excluded.watched_directory_id,
                filename = excluded.filename,
                size_bytes = excluded.size_bytes,
                last_modified = excluded.last_modified,
                duration_seconds = excluded.duration_seconds,
                sample_rate = excluded.sample_rate,
                bitrate = excluded.bitrate,
                channels = excluded.channels,
                bit_depth = excluded.bit_depth,
                title = excluded.title,
                artist = excluded.artist,
                album = excluded.album,
                genre = excluded.genre,
                year = excluded.year,
                track_number = excluded.track_number,
                track_total = excluded.track_total,
                disc_number = excluded.disc_number,
                disc_total = excluded.disc_total,
                album_artist = excluded.album_artist,
                composer = excluded.composer,
                comment = excluded.comment,
                bpm = excluded.bpm,
                lyrics = excluded.lyrics",
        )?;

        for track in tracks {
            stmt.execute(rusqlite::params![
                track.watched_directory_id,
                track.path,
                track.filename,
                track.size_bytes,
                track.last_modified,
                track.duration_seconds,
                track.sample_rate,
                track.bitrate,
                track.channels,
                track.bit_depth,
                track.title,
                track.artist,
                track.album,
                track.genre,
                track.year,
                track.track_number,
                track.track_total,
                track.disc_number,
                track.disc_total,
                track.album_artist,
                track.composer,
                track.comment,
                track.bpm,
                track.lyrics,
            ])?;
        }
    }

    tx.commit()?;
    Ok(())
}

/// Returns a map of path → track ID for the given paths, used by the sidecar restore step.
pub fn get_track_ids_by_paths(
    conn: &Connection,
    paths: &[&str],
) -> std::collections::HashMap<String, i64> {
    let mut map = std::collections::HashMap::new();
    for path in paths {
        if let Ok(id) = conn.query_row(
            "SELECT id FROM tracks WHERE path = ?1",
            [path],
            |row| row.get::<_, i64>(0),
        ) {
            map.insert(path.to_string(), id);
        }
    }
    map
}

/// Prunes track records from the database that no longer physically exist in the scanned directory.
pub fn reconcile_deleted_tracks(
    conn: &mut Connection,
    directory_id: i64,
    active_paths: &HashSet<String>,
) -> Result<usize, rusqlite::Error> {
    let tx = conn.transaction()?;

    let mut to_delete = Vec::new();
    {
        let mut stmt = tx.prepare("SELECT path FROM tracks WHERE watched_directory_id = ?1")?;
        let db_paths = stmt.query_map([directory_id], |row| row.get::<_, String>(0))?;
        for path_res in db_paths {
            if let Ok(path) = path_res {
                if !active_paths.contains(&path) {
                    to_delete.push(path);
                }
            }
        }
    }

    let deleted_count = to_delete.len();
    if deleted_count > 0 {
        let mut del_stmt = tx.prepare("DELETE FROM tracks WHERE path = ?1")?;
        for path in to_delete {
            del_stmt.execute([path])?;
        }
    }

    tx.commit()?;
    Ok(deleted_count)
}
