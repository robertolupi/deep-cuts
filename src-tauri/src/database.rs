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
        M::up(
            "CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT OR IGNORE INTO app_settings (key, value) VALUES ('theme', 'system');"
        ),
        M::up(
            "CREATE TABLE IF NOT EXISTS watched_directories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                path TEXT NOT NULL UNIQUE
            );"
        ),
        M::up(
            "CREATE TABLE IF NOT EXISTS tracks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                watched_directory_id INTEGER NOT NULL,
                path TEXT NOT NULL UNIQUE,
                filename TEXT NOT NULL,
                size_bytes INTEGER NOT NULL,
                last_modified INTEGER NOT NULL,
                duration_seconds INTEGER NOT NULL,
                sample_rate INTEGER,
                bitrate INTEGER,
                channels INTEGER,
                bit_depth INTEGER,
                title TEXT,
                artist TEXT,
                album TEXT,
                genre TEXT,
                year INTEGER,
                track_number INTEGER,
                track_total INTEGER,
                disc_number INTEGER,
                disc_total INTEGER,
                album_artist TEXT,
                composer TEXT,
                comment TEXT,
                bpm INTEGER,
                lyrics TEXT,
                FOREIGN KEY(watched_directory_id) REFERENCES watched_directories(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_tracks_path ON tracks(path);
            CREATE INDEX IF NOT EXISTS idx_tracks_directory ON tracks(watched_directory_id);"
        ),
        M::up(
            "ALTER TABLE tracks ADD COLUMN waveform_data TEXT;
            ALTER TABLE tracks ADD COLUMN key TEXT;
            ALTER TABLE tracks ADD COLUMN scale TEXT;
            ALTER TABLE tracks ADD COLUMN key_strength REAL;
            ALTER TABLE tracks ADD COLUMN loudness_lufs REAL;
            ALTER TABLE tracks ADD COLUMN loudness_range REAL;
            CREATE TABLE IF NOT EXISTS track_passes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                track_id INTEGER NOT NULL,
                pass_name TEXT NOT NULL,
                priority INTEGER NOT NULL DEFAULT 0,
                status INTEGER NOT NULL DEFAULT 0,
                log TEXT,
                result TEXT,
                duration_ms INTEGER,
                last_run_at TEXT,
                FOREIGN KEY(track_id) REFERENCES tracks(id) ON DELETE CASCADE,
                UNIQUE(track_id, pass_name)
            );
            CREATE INDEX IF NOT EXISTS idx_track_passes_status ON track_passes(status);
            CREATE INDEX IF NOT EXISTS idx_track_passes_track ON track_passes(track_id);"
        ),
        M::up(
            "CREATE VIRTUAL TABLE IF NOT EXISTS audio_embeddings USING vec0(
                track_id INTEGER PRIMARY KEY,
                embedding float[512]
            );"
        ),
        M::up(
            "CREATE TABLE IF NOT EXISTS track_coords (
                track_id INTEGER PRIMARY KEY,
                x        REAL NOT NULL,
                y        REAL NOT NULL,
                FOREIGN KEY(track_id) REFERENCES tracks(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_track_coords_track ON track_coords(track_id);"
        ),
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

        let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table'").unwrap();
        let table_names: Vec<String> = stmt.query_map([], |row| row.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        assert!(table_names.contains(&"app_settings".to_string()));
        assert!(table_names.contains(&"watched_directories".to_string()));
        assert!(table_names.contains(&"tracks".to_string()));
        assert!(table_names.contains(&"track_passes".to_string()));

        // Migrations must seed default theme setting
        let theme: String = conn.query_row(
            "SELECT value FROM app_settings WHERE key = 'theme'",
            [],
            |row| row.get(0),
        ).expect("theme setting missing after migrations");
        assert_eq!(theme, "system");

        // Verify CRUD on watched_directories
        conn.execute(
            "INSERT INTO watched_directories (name, path) VALUES ('My Music', '/Users/rlupi/Music')",
            [],
        ).unwrap();

        let dir: WatchedDirectory = conn.query_row(
            "SELECT id, name, path FROM watched_directories WHERE name = 'My Music'",
            [],
            |row| {
                Ok(WatchedDirectory {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    path: row.get(2)?,
                })
            },
        ).unwrap();
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
        ).unwrap();

        let track: Track = conn.query_row(
            "SELECT id, watched_directory_id, path, filename, size_bytes, last_modified,
                    duration_seconds, sample_rate, bitrate, channels, bit_depth,
                    title, artist, album, genre, year, track_number, track_total,
                    disc_number, disc_total, album_artist, composer, comment, bpm, lyrics,
                    waveform_data, key, scale, key_strength, loudness_lufs, loudness_range
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
                })
            },
        ).unwrap();

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
    }
}
