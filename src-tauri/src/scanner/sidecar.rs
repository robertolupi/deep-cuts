use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const SUFFIX: &str = ".dc.json";

/// Current algorithm/model version for each analysis pass.
/// Bump the version constant when the model or algorithm changes in a way that
/// makes previously-cached results stale. restore() will skip any pass whose
/// sidecar version is lower than the current constant, forcing re-inference.
pub mod pass_version {
    pub const AUDIO_ANALYSIS: u32 = 1;
    pub const CLAP: u32 = 1;
    pub const ESSENTIA: u32 = 1;
    pub const BPM_CORRECTION: u32 = 1;
    pub const BPM_REFINEMENT: u32 = 1;
    pub const QWEN: u32 = 1;
    pub const DESCRIPTION_EMBED: u32 = 1;
}

/// ML-derived fields persisted alongside each audio file.
/// When adding a new analysis pass, add its output fields here and update:
///   - save()    — add to the SELECT query and struct construction
///   - restore() — add to the UPDATE statement
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SidecarMlMetadata {
    // --- audio_analysis pass ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bpm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_strength: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loudness_lufs: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loudness_range: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waveform_data: Option<String>,
    // --- clap pass (stored separately as a blob, serialised here as Vec<f32>) ---
    /// 512-d L2-normalised CLAP audio embedding. Persisted to avoid expensive ONNX re-inference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clap_embedding: Option<Vec<f32>>,
    // --- essentia pass ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected_genre: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected_vocal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected_vocal_confidence: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mood_happy: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mood_sad: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mood_aggressive: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mood_relaxed: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mood_party: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mood_acoustic: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mood_electronic: Option<f64>,
    // --- qwen pass ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_music: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_genre: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_mood: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_instruments: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    // --- description_embed pass ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_embedding: Option<Vec<f32>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SidecarData {
    /// Schema version for the sidecar format itself (not pass versions).
    pub version: i32,
    /// Per-pass algorithm/model version at the time this sidecar was written.
    /// Missing entries (e.g. from old sidecars) are treated as version 0, which
    /// is always lower than any current constant, so the pass is re-run.
    #[serde(default)]
    pub pass_versions: HashMap<String, u32>,
    #[serde(default)]
    pub ml_metadata: SidecarMlMetadata,
}

pub fn path_for(track_path: &str) -> String {
    format!("{}{}", track_path, SUFFIX)
}

/// Reads and parses the sidecar file for `track_path`. Returns None on any error
/// (missing file, invalid JSON, wrong shape) so callers can treat absence as a no-op.
pub fn load(track_path: &str) -> Option<SidecarData> {
    let content = std::fs::read_to_string(path_for(track_path)).ok()?;
    serde_json::from_str(&content).ok()
}

/// Writes a sidecar file next to `track_id`'s audio file, persisting its ML metadata.
pub fn save(conn: &Connection, track_id: i64) -> Result<(), Box<dyn std::error::Error>> {
    let path: String =
        conn.query_row("SELECT path FROM tracks WHERE id = ?1", [track_id], |row| {
            row.get(0)
        })?;

    let mut ml_metadata: SidecarMlMetadata = conn.query_row(
        "SELECT bpm, key, scale, key_strength, loudness_lufs, loudness_range, waveform_data,
                detected_genre, detected_vocal, detected_vocal_confidence,
                mood_happy, mood_sad, mood_aggressive, mood_relaxed,
                mood_party, mood_acoustic, mood_electronic,
                is_music, ai_genre, ai_mood, ai_instruments, description
         FROM tracks WHERE id = ?1",
        [track_id],
        |row| {
            Ok(SidecarMlMetadata {
                bpm: row.get(0)?,
                key: row.get(1)?,
                scale: row.get(2)?,
                key_strength: row.get(3)?,
                loudness_lufs: row.get(4)?,
                loudness_range: row.get(5)?,
                waveform_data: row.get(6)?,
                clap_embedding: None,
                detected_genre: row.get(7)?,
                detected_vocal: row.get(8)?,
                detected_vocal_confidence: row.get(9)?,
                mood_happy: row.get(10)?,
                mood_sad: row.get(11)?,
                mood_aggressive: row.get(12)?,
                mood_relaxed: row.get(13)?,
                mood_party: row.get(14)?,
                mood_acoustic: row.get(15)?,
                mood_electronic: row.get(16)?,
                is_music: row.get(17)?,
                ai_genre: row.get(18)?,
                ai_mood: row.get(19)?,
                ai_instruments: row.get(20)?,
                description: row.get(21)?,
                description_embedding: None,
            })
        },
    )?;

    // Read CLAP embedding blob from audio_embeddings and deserialise to Vec<f32>
    let clap_blob: Option<Vec<u8>> = conn
        .query_row(
            "SELECT embedding FROM audio_embeddings WHERE track_id = ?1",
            [track_id],
            |row| row.get(0),
        )
        .ok();
    if let Some(blob) = clap_blob {
        let floats: Vec<f32> = blob
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        if !floats.is_empty() {
            ml_metadata.clap_embedding = Some(floats);
        }
    }

    // Read DESCRIPTION embedding blob from description_embeddings and deserialise to Vec<f32>
    let desc_blob: Option<Vec<u8>> = conn
        .query_row(
            "SELECT embedding FROM description_embeddings WHERE track_id = ?1",
            [track_id],
            |row| row.get(0),
        )
        .ok();
    if let Some(blob) = desc_blob {
        let floats: Vec<f32> = blob
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        if !floats.is_empty() {
            ml_metadata.description_embedding = Some(floats);
        }
    }

    // Record the pass_version for every DONE pass directly from the DB column.
    let mut pass_versions: HashMap<String, u32> = HashMap::new();
    let mut stmt = conn.prepare(
        "SELECT pass_name, pass_version FROM track_passes WHERE track_id = ?1 AND status = 2",
    )?;
    let rows: Vec<(String, u32)> = stmt
        .query_map([track_id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();
    for (pass_name, version) in rows {
        pass_versions.insert(pass_name, version);
    }

    let sidecar = SidecarData {
        version: 1,
        pass_versions,
        ml_metadata,
    };
    let json = serde_json::to_string_pretty(&sidecar)?;
    std::fs::write(path_for(&path), json)?;
    Ok(())
}

/// Returns the sidecar-recorded version for `pass`, or 0 if absent (old sidecar → force re-run).
fn sidecar_pass_version(sidecar: &SidecarData, pass: &str) -> u32 {
    *sidecar.pass_versions.get(pass).unwrap_or(&0)
}

/// Reads the sidecar for `track_path` and restores its ML fields into the tracks table.
/// Each pass is only restored if the sidecar's recorded version matches the current
/// constant in `pass_version`. A missing entry (old sidecar) is treated as version 0
/// and the pass is skipped, allowing the pipeline to re-run it fresh.
pub fn restore(
    conn: &Connection,
    track_id: i64,
    track_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(sidecar) = load(track_path) else {
        return Ok(());
    };

    let m = &sidecar.ml_metadata;

    // --- audio_analysis ---
    if sidecar_pass_version(&sidecar, "audio_analysis") >= pass_version::AUDIO_ANALYSIS {
        conn.execute(
            "UPDATE tracks SET
                bpm = COALESCE(?1, bpm),
                key = COALESCE(?2, key),
                scale = COALESCE(?3, scale),
                key_strength = COALESCE(?4, key_strength),
                loudness_lufs = COALESCE(?5, loudness_lufs),
                loudness_range = COALESCE(?6, loudness_range),
                waveform_data = COALESCE(?7, waveform_data)
             WHERE id = ?8",
            rusqlite::params![
                m.bpm,
                m.key,
                m.scale,
                m.key_strength,
                m.loudness_lufs,
                m.loudness_range,
                m.waveform_data,
                track_id,
            ],
        )?;
        conn.execute(
            "UPDATE track_passes SET status = 2, pass_version = ?1, last_run_at = CURRENT_TIMESTAMP
             WHERE track_id = ?2 AND pass_name = 'audio_analysis'",
            rusqlite::params![pass_version::AUDIO_ANALYSIS, track_id],
        )?;
    }

    // --- clap ---
    if sidecar_pass_version(&sidecar, "clap") >= pass_version::CLAP {
        if let Some(floats) = &m.clap_embedding {
            if !floats.is_empty() {
                let blob: Vec<u8> = floats.iter().flat_map(|&f| f.to_le_bytes()).collect();
                conn.execute(
                    "INSERT OR REPLACE INTO audio_embeddings (track_id, embedding) VALUES (?1, ?2)",
                    rusqlite::params![track_id, blob],
                )?;
                conn.execute(
                    "UPDATE track_passes SET status = 2, pass_version = ?1, last_run_at = CURRENT_TIMESTAMP
                     WHERE track_id = ?2 AND pass_name = 'clap'",
                    rusqlite::params![pass_version::CLAP, track_id],
                )?;
            }
        }
    }

    // --- essentia ---
    if sidecar_pass_version(&sidecar, "essentia") >= pass_version::ESSENTIA {
        conn.execute(
            "UPDATE tracks SET
                detected_genre             = COALESCE(?1,  detected_genre),
                detected_vocal             = COALESCE(?2,  detected_vocal),
                detected_vocal_confidence  = COALESCE(?3,  detected_vocal_confidence),
                mood_happy                 = COALESCE(?4,  mood_happy),
                mood_sad                   = COALESCE(?5,  mood_sad),
                mood_aggressive            = COALESCE(?6,  mood_aggressive),
                mood_relaxed               = COALESCE(?7,  mood_relaxed),
                mood_party                 = COALESCE(?8,  mood_party),
                mood_acoustic              = COALESCE(?9,  mood_acoustic),
                mood_electronic            = COALESCE(?10, mood_electronic)
             WHERE id = ?11",
            rusqlite::params![
                m.detected_genre,
                m.detected_vocal,
                m.detected_vocal_confidence,
                m.mood_happy,
                m.mood_sad,
                m.mood_aggressive,
                m.mood_relaxed,
                m.mood_party,
                m.mood_acoustic,
                m.mood_electronic,
                track_id,
            ],
        )?;
        conn.execute(
            "UPDATE track_passes SET status = 2, pass_version = ?1, last_run_at = CURRENT_TIMESTAMP
             WHERE track_id = ?2 AND pass_name = 'essentia'",
            rusqlite::params![pass_version::ESSENTIA, track_id],
        )?;
    }

    // --- qwen ---
    if sidecar_pass_version(&sidecar, "qwen") >= pass_version::QWEN {
        conn.execute(
            "UPDATE tracks SET
                is_music = COALESCE(?1, is_music),
                ai_genre = COALESCE(?2, ai_genre),
                ai_mood = COALESCE(?3, ai_mood),
                ai_instruments = COALESCE(?4, ai_instruments),
                description = COALESCE(?5, description)
             WHERE id = ?6",
            rusqlite::params![
                m.is_music,
                m.ai_genre,
                m.ai_mood,
                m.ai_instruments,
                m.description,
                track_id,
            ],
        )?;
        conn.execute(
            "UPDATE track_passes SET status = 2, pass_version = ?1, last_run_at = CURRENT_TIMESTAMP
             WHERE track_id = ?2 AND pass_name = 'qwen'",
            rusqlite::params![pass_version::QWEN, track_id],
        )?;
    }

    // --- description_embed ---
    if sidecar_pass_version(&sidecar, "description_embed") >= pass_version::DESCRIPTION_EMBED {
        if let Some(floats) = &m.description_embedding {
            if !floats.is_empty() {
                let blob: Vec<u8> = floats.iter().flat_map(|&f| f.to_le_bytes()).collect();
                conn.execute(
                    "INSERT OR REPLACE INTO description_embeddings (track_id, embedding) VALUES (?1, ?2)",
                    rusqlite::params![track_id, blob],
                )?;
                conn.execute(
                    "UPDATE track_passes SET status = 2, pass_version = ?1, last_run_at = CURRENT_TIMESTAMP
                     WHERE track_id = ?2 AND pass_name = 'description_embed'",
                    rusqlite::params![pass_version::DESCRIPTION_EMBED, track_id],
                )?;
            }
        }
    }

    Ok(())
}

/// Writes sidecar files for every track in the database. Returns the count of files written.
pub fn export_all(conn: &Connection) -> Result<usize, Box<dyn std::error::Error>> {
    let mut stmt = conn.prepare("SELECT id FROM tracks")?;
    let ids: Vec<i64> = stmt
        .query_map([], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    let mut count = 0;
    for id in ids {
        if save(conn, id).is_ok() {
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::setup_test_db;
    use std::path::PathBuf;

    fn temp_track(label: &str) -> (PathBuf, String) {
        let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        dir.push(format!("target/sidecar_test_{}", label));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("song.mp3");
        std::fs::write(&path, b"dummy").unwrap();
        let path_str = path.to_string_lossy().into_owned();
        (dir, path_str)
    }

    fn cleanup(dir: &PathBuf, track_path: &str) {
        let _ = std::fs::remove_file(track_path);
        let _ = std::fs::remove_file(path_for(track_path));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_save_creates_dc_json_file() {
        let (dir, track_path) = temp_track("save");
        let conn = setup_test_db();

        conn.execute(
            "INSERT INTO watched_directories (id, name, path) VALUES (1, 'T', ?1)",
            [dir.to_string_lossy().as_ref()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks (id, watched_directory_id, path, filename, size_bytes, last_modified, duration_seconds) \
             VALUES (1, 1, ?1, 'song.mp3', 5, 0, 0)",
            [&track_path],
        )
        .unwrap();

        save(&conn, 1).unwrap();

        let sidecar_file = path_for(&track_path);
        assert!(
            std::path::Path::new(&sidecar_file).exists(),
            "expected .dc.json file at {}",
            sidecar_file
        );

        cleanup(&dir, &track_path);
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let (dir, track_path) = temp_track("roundtrip");
        let conn = setup_test_db();

        conn.execute(
            "INSERT INTO watched_directories (id, name, path) VALUES (1, 'T', ?1)",
            [dir.to_string_lossy().as_ref()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks (id, watched_directory_id, path, filename, size_bytes, last_modified, duration_seconds,
                                 bpm, key, scale, key_strength, loudness_lufs, loudness_range, waveform_data) \
             VALUES (1, 1, ?1, 'song.mp3', 5, 0, 180, 128.5, 'G', 'minor', 0.87, -14.2, 6.1, '[0.1,0.2]')",
            [&track_path],
        )
        .unwrap();
        // Mark audio_analysis as DONE with the current pass_version
        conn.execute(
            "INSERT INTO track_passes (track_id, pass_name, priority, status, pass_version) \
             VALUES (1, 'audio_analysis', 10, 2, ?1)",
            rusqlite::params![pass_version::AUDIO_ANALYSIS],
        )
        .unwrap();

        save(&conn, 1).unwrap();
        let loaded = load(&track_path).expect("sidecar should be loadable after save");
        assert_eq!(loaded.version, 1);
        let m = &loaded.ml_metadata;
        assert_eq!(m.bpm, Some(128.5));
        assert_eq!(m.key.as_deref(), Some("G"));
        assert_eq!(m.scale.as_deref(), Some("minor"));
        assert_eq!(m.key_strength, Some(0.87));
        assert_eq!(m.loudness_lufs, Some(-14.2));
        assert_eq!(m.loudness_range, Some(6.1));
        assert_eq!(m.waveform_data.as_deref(), Some("[0.1,0.2]"));
        // pass_versions should record audio_analysis at the current constant
        assert_eq!(
            loaded.pass_versions.get("audio_analysis").copied(),
            Some(pass_version::AUDIO_ANALYSIS)
        );

        cleanup(&dir, &track_path);
    }

    #[test]
    fn test_restore_writes_ml_fields_back() {
        let (dir, track_path) = temp_track("restore_fields");
        let conn = setup_test_db();

        conn.execute(
            "INSERT INTO watched_directories (id, name, path) VALUES (1, 'T', ?1)",
            [dir.to_string_lossy().as_ref()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks (id, watched_directory_id, path, filename, size_bytes, last_modified, duration_seconds,
                                 bpm, key, scale, key_strength, loudness_lufs, loudness_range, waveform_data) \
             VALUES (1, 1, ?1, 'song.mp3', 5, 0, 180, 120.0, 'C', 'major', 0.9, -12.0, 5.0, '[0.5]')",
            [&track_path],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO track_passes (track_id, pass_name, priority, status, pass_version) \
             VALUES (1, 'audio_analysis', 10, 2, ?1)",
            rusqlite::params![pass_version::AUDIO_ANALYSIS],
        )
        .unwrap();

        // Save to disk, then wipe DB fields and the pass row, then restore from disk
        save(&conn, 1).unwrap();
        conn.execute(
            "UPDATE tracks SET bpm = NULL, key = NULL, scale = NULL, key_strength = NULL,
             loudness_lufs = NULL, loudness_range = NULL, waveform_data = NULL WHERE id = 1",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE track_passes SET status = 0 WHERE track_id = 1 AND pass_name = 'audio_analysis'",
            [],
        ).unwrap();
        restore(&conn, 1, &track_path).unwrap();

        let (bpm, key, scale): (Option<f64>, Option<String>, Option<String>) = conn
            .query_row("SELECT bpm, key, scale FROM tracks WHERE id = 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert_eq!(bpm, Some(120.0));
        assert_eq!(key.as_deref(), Some("C"));
        assert_eq!(scale.as_deref(), Some("major"));

        // Pass should be marked DONE with the current pass_version after restore
        let (status, version): (i64, u32) = conn.query_row(
            "SELECT status, pass_version FROM track_passes WHERE track_id = 1 AND pass_name = 'audio_analysis'",
            [], |r| Ok((r.get(0)?, r.get(1)?)),
        ).unwrap();
        assert_eq!(
            status, 2,
            "audio_analysis pass should be DONE after restore"
        );
        assert_eq!(
            version,
            pass_version::AUDIO_ANALYSIS,
            "restored pass_version should match current constant"
        );

        cleanup(&dir, &track_path);
    }

    #[test]
    fn test_restore_skips_stale_pass_version() {
        let (dir, track_path) = temp_track("stale_version");

        // Write a sidecar with pass_version 0 — below every current constant
        let stale =
            r#"{"version":1,"pass_versions":{"audio_analysis":0},"ml_metadata":{"bpm":99.0}}"#;
        std::fs::write(path_for(&track_path), stale).unwrap();

        let conn = setup_test_db();
        conn.execute(
            "INSERT INTO watched_directories (id, name, path) VALUES (1, 'T', ?1)",
            [dir.to_string_lossy().as_ref()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks (id, watched_directory_id, path, filename, size_bytes, last_modified, duration_seconds) \
             VALUES (1, 1, ?1, 'song.mp3', 5, 0, 0)",
            [&track_path],
        ).unwrap();
        conn.execute(
            "INSERT INTO track_passes (track_id, pass_name, priority, status, pass_version) \
             VALUES (1, 'audio_analysis', 10, 0, 0)",
            [],
        )
        .unwrap();

        restore(&conn, 1, &track_path).unwrap();

        // BPM should NOT have been restored — sidecar version was stale
        let bpm: Option<f64> = conn
            .query_row("SELECT bpm FROM tracks WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(bpm, None, "stale pass version should prevent restore");

        // Pass should remain PENDING
        let status: i64 = conn.query_row(
            "SELECT status FROM track_passes WHERE track_id = 1 AND pass_name = 'audio_analysis'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(status, 0, "stale pass should remain pending for re-run");

        cleanup(&dir, &track_path);
    }

    #[test]
    fn test_restore_old_sidecar_without_pass_versions() {
        let (dir, track_path) = temp_track("old_sidecar");

        // Old format: no pass_versions field at all
        let old_format = r#"{"version":1,"ml_metadata":{"bpm":120.0,"key":"A","scale":"major"}}"#;
        std::fs::write(path_for(&track_path), old_format).unwrap();

        let conn = setup_test_db();
        conn.execute(
            "INSERT INTO watched_directories (id, name, path) VALUES (1, 'T', ?1)",
            [dir.to_string_lossy().as_ref()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks (id, watched_directory_id, path, filename, size_bytes, last_modified, duration_seconds) \
             VALUES (1, 1, ?1, 'song.mp3', 5, 0, 0)",
            [&track_path],
        ).unwrap();

        // Should not panic; missing pass_versions → all passes treated as version 0 → skipped
        let result = restore(&conn, 1, &track_path);
        assert!(result.is_ok());

        let bpm: Option<f64> = conn
            .query_row("SELECT bpm FROM tracks WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            bpm, None,
            "old sidecar without pass_versions should not restore any fields"
        );

        cleanup(&dir, &track_path);
    }

    #[test]
    fn test_corrupt_sidecar_returns_none() {
        let (dir, track_path) = temp_track("corrupt");

        let cases = [
            ("truncated", "{\"version\":1,\"ml_metadata\":{"),
            ("plain_text", "not json"),
            ("empty", ""),
        ];

        for (label, content) in cases {
            std::fs::write(path_for(&track_path), content).unwrap();
            assert!(
                load(&track_path).is_none(),
                "[{label}] expected None for corrupt sidecar"
            );
        }

        let _ = std::fs::remove_file(path_for(&track_path));
        cleanup(&dir, &track_path);
    }

    #[test]
    fn test_restore_is_safe_on_missing_sidecar() {
        let (dir, track_path) = temp_track("no_sidecar");
        let conn = setup_test_db();

        conn.execute(
            "INSERT INTO watched_directories (id, name, path) VALUES (1, 'T', ?1)",
            [dir.to_string_lossy().as_ref()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks (id, watched_directory_id, path, filename, size_bytes, last_modified, duration_seconds) \
             VALUES (1, 1, ?1, 'song.mp3', 5, 0, 0)",
            [&track_path],
        )
        .unwrap();

        // No sidecar file exists — restore must return Ok without panicking.
        let result = restore(&conn, 1, &track_path);
        assert!(result.is_ok());

        cleanup(&dir, &track_path);
    }

    #[test]
    fn test_export_all_writes_one_file_per_track() {
        let (dir, track_path) = temp_track("export_all");
        let conn = setup_test_db();

        conn.execute(
            "INSERT INTO watched_directories (id, name, path) VALUES (1, 'T', ?1)",
            [dir.to_string_lossy().as_ref()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks (id, watched_directory_id, path, filename, size_bytes, last_modified, duration_seconds) \
             VALUES (1, 1, ?1, 'song.mp3', 5, 0, 0)",
            [&track_path],
        )
        .unwrap();

        let count = export_all(&conn).unwrap();
        assert_eq!(count, 1);
        assert!(std::path::Path::new(&path_for(&track_path)).exists());

        cleanup(&dir, &track_path);
    }
}
