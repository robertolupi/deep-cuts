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
        if let Ok(id) = conn.query_row("SELECT id FROM tracks WHERE path = ?1", [path], |row| {
            row.get::<_, i64>(0)
        }) {
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

    // vec0 virtual tables don't support FK cascades, so sweep orphans manually.
    // This catches rows left by any deletion path (individual track removal,
    // remove_watched_directory, etc.).
    let _ = conn.execute(
        "DELETE FROM audio_embeddings WHERE track_id NOT IN (SELECT id FROM tracks)",
        [],
    );
    let _ = conn.execute(
        "DELETE FROM description_embeddings WHERE track_id NOT IN (SELECT id FROM tracks)",
        [],
    );

    Ok(deleted_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::setup_test_db;

    #[test]
    fn test_upsert_and_cache_queries() {
        let mut conn = setup_test_db();

        conn.execute(
            "INSERT INTO watched_directories (id, name, path) VALUES (1, 'Test Collection', '/Users/user/Music')",
            [],
        ).unwrap();

        let track = ParsedAudioTags {
            watched_directory_id: 1,
            path: "/Users/user/Music/song1.mp3".to_string(),
            filename: "song1.mp3".to_string(),
            size_bytes: 4096,
            last_modified: 1700000000,
            duration_seconds: 120,
            sample_rate: Some(44100),
            bitrate: Some(320),
            channels: Some(2),
            bit_depth: Some(16),
            title: Some("Song One".to_string()),
            artist: Some("Artist".to_string()),
            album: Some("Album".to_string()),
            genre: Some("Pop".to_string()),
            year: Some(2026),
            track_number: Some(1),
            track_total: None,
            disc_number: None,
            disc_total: None,
            album_artist: None,
            composer: None,
            comment: None,
            bpm: None,
            lyrics: None,
        };

        upsert_tracks_transactional(&mut conn, &[track.clone()]).unwrap();

        let cached = get_cached_track_details(&conn, "/Users/user/Music/song1.mp3").unwrap();
        assert_eq!(cached, (4096, 1700000000));

        assert!(get_cached_track_details(&conn, "/Users/user/Music/unknown.mp3").is_none());

        let ids = get_track_ids_by_paths(&conn, &["/Users/user/Music/song1.mp3"]);
        assert!(ids.contains_key("/Users/user/Music/song1.mp3"));
        let track_id = ids["/Users/user/Music/song1.mp3"];
        assert!(track_id > 0);
    }

    #[test]
    fn test_reconcile_deleted_tracks() {
        let mut conn = setup_test_db();

        conn.execute(
            "INSERT INTO watched_directories (id, name, path) VALUES (1, 'Collection', '/Users/user/Music')",
            [],
        ).unwrap();

        let tracks = vec![
            ParsedAudioTags {
                watched_directory_id: 1,
                path: "/Users/user/Music/stay.mp3".to_string(),
                filename: "stay.mp3".to_string(),
                size_bytes: 1000,
                last_modified: 1700000000,
                duration_seconds: 150,
                sample_rate: Some(44100),
                bitrate: Some(320),
                channels: Some(2),
                bit_depth: Some(16),
                title: Some("Stay".to_string()),
                artist: None,
                album: None,
                genre: None,
                year: None,
                track_number: None,
                track_total: None,
                disc_number: None,
                disc_total: None,
                album_artist: None,
                composer: None,
                comment: None,
                bpm: None,
                lyrics: None,
            },
            ParsedAudioTags {
                watched_directory_id: 1,
                path: "/Users/user/Music/delete_me.mp3".to_string(),
                filename: "delete_me.mp3".to_string(),
                size_bytes: 2000,
                last_modified: 1700000000,
                duration_seconds: 180,
                sample_rate: Some(44100),
                bitrate: Some(320),
                channels: Some(2),
                bit_depth: Some(16),
                title: Some("Delete Me".to_string()),
                artist: None,
                album: None,
                genre: None,
                year: None,
                track_number: None,
                track_total: None,
                disc_number: None,
                disc_total: None,
                album_artist: None,
                composer: None,
                comment: None,
                bpm: None,
                lyrics: None,
            },
        ];

        upsert_tracks_transactional(&mut conn, &tracks).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);

        let mut active_paths = HashSet::new();
        active_paths.insert("/Users/user/Music/stay.mp3".to_string());

        let deleted = reconcile_deleted_tracks(&mut conn, 1, &active_paths).unwrap();
        assert_eq!(deleted, 1);

        let remaining_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(remaining_count, 1);

        let stay_exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM tracks WHERE path = '/Users/user/Music/stay.mp3')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(stay_exists);
    }
}
