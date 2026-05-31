use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct WatchedDirectory {
    pub id: i64,
    pub name: String,
    pub path: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct Track {
    pub id: i64,
    pub watched_directory_id: i64,
    pub path: String,
    pub filename: String,
    pub size_bytes: i64,
    pub last_modified: i64, // Unix epoch integer

    // Audio properties
    pub duration_seconds: i64,
    pub sample_rate: Option<i64>,
    pub bitrate: Option<i64>,
    pub channels: Option<i64>,
    pub bit_depth: Option<i64>,

    // Metadata tags
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<i64>,
    pub track_number: Option<i64>,
    pub track_total: Option<i64>,
    pub disc_number: Option<i64>,
    pub disc_total: Option<i64>,
    pub album_artist: Option<String>,
    pub composer: Option<String>,
    pub comment: Option<String>,
    pub bpm: Option<f64>,
    pub lyrics: Option<String>,

    // Analysis results (written by the audio_analysis pass)
    pub waveform_data: Option<String>,
    pub key: Option<String>,
    pub scale: Option<String>,
    pub key_strength: Option<f64>,
    pub loudness_lufs: Option<f64>,
    pub loudness_range: Option<f64>,
    pub silence_regions: Option<String>,
    pub has_long_silence: i64,

    // Essentia classifier results
    pub detected_genre: Option<String>,
    pub detected_vocal: Option<String>,
    pub detected_vocal_confidence: Option<f64>,
    pub mood_happy: Option<f64>,
    pub mood_sad: Option<f64>,
    pub mood_aggressive: Option<f64>,
    pub mood_relaxed: Option<f64>,
    pub mood_party: Option<f64>,
    pub mood_acoustic: Option<f64>,
    pub mood_electronic: Option<f64>,

    // Qwen2-Audio listener results
    pub is_music: Option<i64>,
    pub ai_genre: Option<String>,
    pub ai_mood: Option<String>,
    pub ai_instruments: Option<String>,
    pub description: Option<String>,
}

impl WatchedDirectory {
    pub fn find_all(conn: &Connection) -> Result<Vec<Self>, rusqlite::Error> {
        let mut stmt =
            conn.prepare("SELECT id, name, path FROM watched_directories ORDER BY id DESC")?;
        let rows = stmt.query_map([], |row| {
            Ok(Self {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
            })
        })?;
        let mut list = Vec::new();
        for row in rows {
            list.push(row?);
        }
        Ok(list)
    }

    pub fn insert(&self, conn: &Connection) -> Result<(), rusqlite::Error> {
        conn.execute(
            "INSERT INTO watched_directories (name, path) VALUES (?1, ?2)",
            [&self.name, &self.path],
        )?;
        Ok(())
    }

    pub fn delete(conn: &Connection, id: i64) -> Result<(), rusqlite::Error> {
        conn.execute("DELETE FROM watched_directories WHERE id = ?1", [id])?;
        Ok(())
    }
}

impl Track {
    pub fn find_all(conn: &Connection) -> Result<Vec<Self>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT id, watched_directory_id, path, filename, size_bytes, last_modified,
                    duration_seconds, sample_rate, bitrate, channels, bit_depth,
                    title, artist, album, genre, year, track_number, track_total,
                    disc_number, disc_total, album_artist, composer, comment, bpm, lyrics,
                    waveform_data, key, scale, key_strength, loudness_lufs, loudness_range,
                    silence_regions, has_long_silence,
                    detected_genre, detected_vocal, detected_vocal_confidence,
                    mood_happy, mood_sad, mood_aggressive, mood_relaxed,
                    mood_party, mood_acoustic, mood_electronic,
                    is_music, ai_genre, ai_mood, ai_instruments, description
             FROM tracks ORDER BY artist ASC, album ASC, track_number ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Self {
                id: row.get(0)?,
                watched_directory_id: row.get(1)?,
                path: row.get(2)?,
                filename: row.get(3)?,
                size_bytes: row.get(4)?,
                last_modified: row.get(5)?,
                duration_seconds: row.get(6)?,
                sample_rate: row.get(7)?,
                bitrate: row.get(8)?,
                channels: row.get(9)?,
                bit_depth: row.get(10)?,
                title: row.get(11)?,
                artist: row.get(12)?,
                album: row.get(13)?,
                genre: row.get(14)?,
                year: row.get(15)?,
                track_number: row.get(16)?,
                track_total: row.get(17)?,
                disc_number: row.get(18)?,
                disc_total: row.get(19)?,
                album_artist: row.get(20)?,
                composer: row.get(21)?,
                comment: row.get(22)?,
                bpm: row.get(23)?,
                lyrics: row.get(24)?,
                waveform_data: row.get(25)?,
                key: row.get(26)?,
                scale: row.get(27)?,
                key_strength: row.get(28)?,
                loudness_lufs: row.get(29)?,
                loudness_range: row.get(30)?,
                silence_regions: row.get(31)?,
                has_long_silence: row.get(32)?,
                detected_genre: row.get(33)?,
                detected_vocal: row.get(34)?,
                detected_vocal_confidence: row.get(35)?,
                mood_happy: row.get(36)?,
                mood_sad: row.get(37)?,
                mood_aggressive: row.get(38)?,
                mood_relaxed: row.get(39)?,
                mood_party: row.get(40)?,
                mood_acoustic: row.get(41)?,
                mood_electronic: row.get(42)?,
                is_music: row.get(43)?,
                ai_genre: row.get(44)?,
                ai_mood: row.get(45)?,
                ai_instruments: row.get(46)?,
                description: row.get(47)?,
            })
        })?;
        let mut list = Vec::new();
        for row in rows {
            list.push(row?);
        }
        Ok(list)
    }

    pub fn count(conn: &Connection) -> Result<i64, rusqlite::Error> {
        conn.query_row("SELECT COUNT(*) FROM tracks", [], |row| row.get(0))
    }
}

pub mod pass_status {
    pub const PENDING: i64 = 0;
    pub const IN_PROGRESS: i64 = 1;
    pub const DONE: i64 = 2;
    pub const FAILED: i64 = 3;
}

pub struct DbManager {
    db_path: PathBuf,
}

impl DbManager {
    /// Resolves standard sandbox OS storage folders and defines database file path.
    pub fn new(app_handle: &AppHandle) -> Self {
        // Resolve: ~/Library/Application Support/com.rlupi.deep-cuts/
        let app_data_dir = app_handle
            .path()
            .app_data_dir()
            .expect("Failed to resolve standard OS App Data Directory");

        // Ensure path exists
        if !app_data_dir.exists() {
            fs::create_dir_all(&app_data_dir).expect("Failed to create App Data Directory");
        }

        let db_path = app_data_dir.join("deep_cuts.db");
        DbManager { db_path }
    }

    /// Connects to SQLite and performs DB migrations up to the latest schema.
    pub fn connect_and_migrate(&self) -> Result<Connection, Box<dyn std::error::Error>> {
        let mut conn = Connection::open(&self.db_path)?;

        // WAL mode: allows one writer + concurrent readers without file-level locking.
        // Must be set before migrations so the journal file is created correctly.
        conn.execute_batch("PRAGMA journal_mode = WAL;")?;

        // Retry for up to 5 seconds if another connection holds a write lock.
        conn.busy_timeout(std::time::Duration::from_secs(5))?;

        // Enable foreign key constraints
        conn.execute("PRAGMA foreign_keys = ON;", [])?;

        // Run migrations up to latest version
        let migrations = get_migrations();
        migrations.to_latest(&mut conn)?;

        Ok(conn)
    }
}

/// Returns the schema migrations vector chronologically.
pub fn get_migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(include_str!("../migrations/01_app_settings.sql")),
        M::up(include_str!("../migrations/02_watched_directories.sql")),
        M::up(include_str!("../migrations/03_tracks.sql")),
        M::up(include_str!("../migrations/04_track_passes.sql")),
        M::up(include_str!("../migrations/05_audio_embeddings.sql")),
        M::up(include_str!("../migrations/06_track_coords.sql")),
        M::up(include_str!("../migrations/07_essentia_columns.sql")),
        M::up(include_str!("../migrations/08_track_passes_version.sql")),
        M::up(include_str!("../migrations/09_bpm_raw.sql")),
        M::up(include_str!("../migrations/10_qwen_columns.sql")),
        M::up(include_str!("../migrations/11_description_embeddings.sql")),
        M::up(include_str!("../migrations/12_silence_columns.sql")),
        M::up(include_str!("../migrations/13_track_coords_music_only.sql")),
        M::up(include_str!("../migrations/14_track_coords_algorithm.sql")),
        M::up(include_str!("../migrations/15_track_passes_raw_result.sql")),
    ])
}

#[cfg(test)]
pub fn setup_test_db() -> Connection {
    // Dynamically load the C-based sqlite-vec extension globally before booting the in-memory test database
    unsafe {
        let _ = rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    }
    let mut conn = Connection::open_in_memory().unwrap();
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    conn.execute("PRAGMA foreign_keys = ON;", []).unwrap();
    let migrations = get_migrations();
    migrations.to_latest(&mut conn).unwrap();
    conn
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_migrations_boot_successfully() {
        let conn = setup_test_db();

        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .unwrap();
        let table_names: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        assert!(table_names.contains(&"app_settings".to_string()));
        assert!(table_names.contains(&"watched_directories".to_string()));
        assert!(table_names.contains(&"tracks".to_string()));
        assert!(table_names.contains(&"track_passes".to_string()));

        // Migrations must seed default theme setting
        let theme: String = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = 'theme'",
                [],
                |row| row.get(0),
            )
            .expect("theme setting missing after migrations");
        assert_eq!(theme, "system");

        // Verify CRUD on watched_directories
        conn.execute(
            "INSERT INTO watched_directories (name, path) VALUES ('My Music', '/Users/rlupi/Music')",
            [],
        ).unwrap();

        let dir: WatchedDirectory = conn
            .query_row(
                "SELECT id, name, path FROM watched_directories WHERE name = 'My Music'",
                [],
                |row| {
                    Ok(WatchedDirectory {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        path: row.get(2)?,
                    })
                },
            )
            .unwrap();
        assert_eq!(dir.name, "My Music");
        assert_eq!(dir.path, "/Users/rlupi/Music");

        // Verify CRUD on tracks
        conn.execute(
            "INSERT INTO tracks (
                watched_directory_id, path, filename, size_bytes, last_modified,
                duration_seconds, sample_rate, bitrate, channels, bit_depth,
                title, artist, album, genre, year, track_number
            ) VALUES (
                ?1, '/Users/rlupi/Music/song.mp3', 'song.mp3', 5000000, 1780000000,
                180, 44100, 320, 2, 16,
                'My Song', 'My Artist', 'My Album', 'Rock', 2026, 3
            )",
            rusqlite::params![dir.id],
        )
        .unwrap();

        let track: Track = conn
            .query_row(
                "SELECT id, watched_directory_id, path, filename, size_bytes, last_modified,
                    duration_seconds, sample_rate, bitrate, channels, bit_depth,
                    title, artist, album, genre, year, track_number, track_total,
                    disc_number, disc_total, album_artist, composer, comment, bpm, lyrics,
                    waveform_data, key, scale, key_strength, loudness_lufs, loudness_range,
                    silence_regions, has_long_silence,
                    detected_genre, detected_vocal, detected_vocal_confidence,
                    mood_happy, mood_sad, mood_aggressive, mood_relaxed,
                    mood_party, mood_acoustic, mood_electronic,
                    is_music, ai_genre, ai_mood, ai_instruments, description
             FROM tracks WHERE title = 'My Song'",
                [],
                |row| {
                    Ok(Track {
                        id: row.get(0)?,
                        watched_directory_id: row.get(1)?,
                        path: row.get(2)?,
                        filename: row.get(3)?,
                        size_bytes: row.get(4)?,
                        last_modified: row.get(5)?,
                        duration_seconds: row.get(6)?,
                        sample_rate: row.get(7)?,
                        bitrate: row.get(8)?,
                        channels: row.get(9)?,
                        bit_depth: row.get(10)?,
                        title: row.get(11)?,
                        artist: row.get(12)?,
                        album: row.get(13)?,
                        genre: row.get(14)?,
                        year: row.get(15)?,
                        track_number: row.get(16)?,
                        track_total: row.get(17)?,
                        disc_number: row.get(18)?,
                        disc_total: row.get(19)?,
                        album_artist: row.get(20)?,
                        composer: row.get(21)?,
                        comment: row.get(22)?,
                        bpm: row.get(23)?,
                        lyrics: row.get(24)?,
                        waveform_data: row.get(25)?,
                        key: row.get(26)?,
                        scale: row.get(27)?,
                        key_strength: row.get(28)?,
                        loudness_lufs: row.get(29)?,
                        loudness_range: row.get(30)?,
                        silence_regions: row.get(31)?,
                        has_long_silence: row.get(32)?,
                        detected_genre: row.get(33)?,
                        detected_vocal: row.get(34)?,
                        detected_vocal_confidence: row.get(35)?,
                        mood_happy: row.get(36)?,
                        mood_sad: row.get(37)?,
                        mood_aggressive: row.get(38)?,
                        mood_relaxed: row.get(39)?,
                        mood_party: row.get(40)?,
                        mood_acoustic: row.get(41)?,
                        mood_electronic: row.get(42)?,
                        is_music: row.get(43)?,
                        ai_genre: row.get(44)?,
                        ai_mood: row.get(45)?,
                        ai_instruments: row.get(46)?,
                        description: row.get(47)?,
                    })
                },
            )
            .unwrap();

        assert_eq!(track.watched_directory_id, dir.id);
        assert_eq!(track.path, "/Users/rlupi/Music/song.mp3");
        assert_eq!(track.filename, "song.mp3");
        assert_eq!(track.size_bytes, 5000000);
        assert_eq!(track.last_modified, 1780000000);
        assert_eq!(track.duration_seconds, 180);
        assert_eq!(track.sample_rate, Some(44100));
        assert_eq!(track.bitrate, Some(320));
        assert_eq!(track.channels, Some(2));
        assert_eq!(track.bit_depth, Some(16));
        assert_eq!(track.title, Some("My Song".to_string()));
        assert_eq!(track.artist, Some("My Artist".to_string()));
        assert_eq!(track.album, Some("My Album".to_string()));
        assert_eq!(track.genre, Some("Rock".to_string()));
        assert_eq!(track.year, Some(2026));
        assert_eq!(track.track_number, Some(3));
        assert_eq!(track.track_total, None);
        assert_eq!(track.lyrics, None);
        assert_eq!(track.waveform_data, None);
        assert_eq!(track.key, None);
        assert_eq!(track.scale, None);
        assert_eq!(track.key_strength, None);
        assert_eq!(track.loudness_lufs, None);
        assert_eq!(track.loudness_range, None);
        assert_eq!(track.silence_regions, None);
        assert_eq!(track.has_long_silence, 0);
    }
}
