use super::fs::DiscoveredFile;
use lofty::config::{ParseOptions, ParsingMode};
use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::ItemKey;
use rayon::prelude::*;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ParsedAudioTags {
    pub watched_directory_id: i64,
    pub path: String,
    pub filename: String,
    pub size_bytes: i64,
    pub last_modified: i64,

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
    pub bpm: Option<i64>,
    pub lyrics: Option<String>,
}

pub fn parse_single_file(file: &DiscoveredFile) -> Result<ParsedAudioTags, Box<dyn std::error::Error + Send + Sync>> {
    let path = Path::new(&file.path);
    let options = ParseOptions::new().parsing_mode(ParsingMode::Relaxed);
    
    // Initialize properties with default values to ensure files are indexed even if lofty fails
    let mut duration_seconds = 0;
    let mut sample_rate = None;
    let mut bitrate = None;
    let mut bit_depth = None;
    let mut channels = None;

    let mut title = None;
    let mut artist = None;
    let mut album = None;
    let mut genre = None;
    let mut year = None;
    let mut track_number = None;
    let mut track_total = None;
    let mut disc_number = None;
    let mut disc_total = None;
    let mut album_artist = None;
    let mut composer = None;
    let mut comment = None;
    let mut bpm = None;
    let mut lyrics = None;

    // Try to open and read using lofty, but fail gracefully to prevent skipping files
    if let Ok(probe) = Probe::open(path) {
        if let Ok(tagged_file) = probe.options(options).read() {
            let properties = tagged_file.properties();
            duration_seconds = properties.duration().as_secs() as i64;
            sample_rate = properties.sample_rate().map(|sr| sr as i64);
            bitrate = properties.audio_bitrate().map(|br| br as i64);
            bit_depth = properties.bit_depth().map(|v| v as i64);
            channels = properties.channels().map(|v| v as i64);

            for tag in tagged_file.tags() {
                if title.is_none() { title = tag.title().map(|s| s.into_owned()); }
                if artist.is_none() { artist = tag.artist().map(|s| s.into_owned()); }
                if album.is_none() { album = tag.album().map(|s| s.into_owned()); }
                if genre.is_none() { genre = tag.genre().map(|s| s.into_owned()); }
                if year.is_none() { year = tag.year().map(|v| v as i64); }
                if track_number.is_none() { track_number = tag.track().map(|v| v as i64); }
                if track_total.is_none() { track_total = tag.track_total().map(|v| v as i64); }
                if disc_number.is_none() { disc_number = tag.disk().map(|v| v as i64); }
                if disc_total.is_none() { disc_total = tag.disk_total().map(|v| v as i64); }
                if comment.is_none() { comment = tag.comment().map(|s| s.into_owned()); }
                if album_artist.is_none() { album_artist = tag.get_string(&ItemKey::AlbumArtist).map(|s| s.to_owned()); }
                if composer.is_none() { composer = tag.get_string(&ItemKey::Composer).map(|s| s.to_owned()); }
                if lyrics.is_none() { lyrics = tag.get_string(&ItemKey::Lyrics).map(|s| s.to_owned()); }
                if bpm.is_none() { bpm = tag.get_string(&ItemKey::IntegerBpm).and_then(|s| s.trim().parse::<i64>().ok()); }
            }
        } else {
            eprintln!("Warning: lofty failed to read tags for '{}'. Indexing with empty metadata.", file.path);
        }
    } else {
        eprintln!("Warning: lofty failed to probe file '{}'. Indexing with empty metadata.", file.path);
    }

    Ok(ParsedAudioTags {
        watched_directory_id: 0, // Will be updated by orchestrator
        path: file.path.clone(),
        filename: file.filename.clone(),
        size_bytes: file.size_bytes,
        last_modified: file.last_modified,
        duration_seconds,
        sample_rate,
        bitrate,
        channels,
        bit_depth,
        title,
        artist,
        album,
        genre,
        year,
        track_number,
        track_total,
        disc_number,
        disc_total,
        album_artist,
        composer,
        comment,
        bpm,
        lyrics,
    })
}

/// Safely parses a single file, catching and logging any errors to return None.
pub fn parse_single_file_safe(file: &DiscoveredFile) -> Option<ParsedAudioTags> {
    match parse_single_file(file) {
        Ok(tags) => Some(tags),
        Err(e) => {
            eprintln!("Warning: Failed to parse metadata for file '{}': {:?}", file.path, e);
            None
        }
    }
}

/// Parses metadata for a list of files in parallel using all available CPU threads (via Rayon).
pub fn parse_multiple_files_parallel(files: &[DiscoveredFile]) -> Vec<ParsedAudioTags> {
    files.par_iter()
        .filter_map(|file| parse_single_file_safe(file))
        .collect()
}
