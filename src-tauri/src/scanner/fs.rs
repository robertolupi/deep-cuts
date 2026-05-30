use std::fs;
use std::path::Path;
use std::time::SystemTime;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct DiscoveredFile {
    pub path: String,
    pub filename: String,
    pub size_bytes: i64,
    pub last_modified: i64, // Unix epoch timestamp
}

/// Checks if file has a supported audio extension (mp3, wav, flac, m4a, aiff, aif, ogg, oga, opus).
pub fn is_supported_audio(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy().to_lowercase();
        return ext_str == "mp3"
            || ext_str == "wav"
            || ext_str == "flac"
            || ext_str == "m4a"
            || ext_str == "aiff"
            || ext_str == "aif"
            || ext_str == "ogg"
            || ext_str == "oga"
            || ext_str == "opus";
    }
    false
}

/// Recursively scans a root directory path and returns a list of discovered files.
/// Returns Ok(None) if the directory is dismounted or inaccessible.
pub fn walk_directory(
    directory_path: &str,
) -> Result<Option<Vec<DiscoveredFile>>, Box<dyn std::error::Error + Send + Sync>> {
    let root_path = Path::new(directory_path);

    // Drive Dismount Safety Check:
    // If the path does not physically exist, return None to indicate the drive
    // is inaccessible. This prevents the scanner from pruning existing database records.
    if !root_path.exists() {
        return Ok(None);
    }

    let mut discovered = Vec::new();

    for entry in WalkDir::new(root_path).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() && is_supported_audio(path) {
            let abs_path = path.to_string_lossy().into_owned();

            let metadata = match fs::metadata(path) {
                Ok(m) => m,
                Err(_) => continue,
            };

            let size_bytes = metadata.len() as i64;

            let last_modified = metadata
                .modified()
                .unwrap_or_else(|_| SystemTime::now())
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            let filename = path
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Unknown".to_string());

            discovered.push(DiscoveredFile {
                path: abs_path,
                filename,
                size_bytes,
                last_modified,
            });
        }
    }

    Ok(Some(discovered))
}
