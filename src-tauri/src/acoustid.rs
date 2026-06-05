use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Manager, Emitter};
use rusqlite::Connection;
use tokio::sync::Semaphore;

// Global 1 req/sec rate limiter for MusicBrainz API
static MB_SEMAPHORE: OnceLock<Semaphore> = OnceLock::new();

fn get_mb_semaphore() -> &'static Semaphore {
    MB_SEMAPHORE.get_or_init(|| Semaphore::new(1))
}

// Default fallback AcoustID client API key
const DEFAULT_ACOUSTID_CLIENT_KEY: &str = "8Xa4u4ux";
fn user_agent() -> String {
    format!("DeepCuts/{} ( roberto.lupi@gmail.com )", env!("CARGO_PKG_VERSION"))
}

/// Resolves the AcoustID client API key at compile time from the ACOUSTID_CLIENT_KEY environment variable.
fn get_acoustid_client_key() -> &'static str {
    option_env!("ACOUSTID_CLIENT_KEY").unwrap_or(DEFAULT_ACOUSTID_CLIENT_KEY)
}

#[derive(serde::Deserialize, Debug)]
struct FpcalcOutput {
    duration: f64,
    fingerprint: String,
}

#[derive(serde::Deserialize, Debug)]
struct AcoustIdResponse {
    status: String,
    results: Option<Vec<AcoustIdResult>>,
}

#[derive(serde::Deserialize, Debug)]
struct AcoustIdResult {
    #[serde(rename = "score")]
    _score: f64,
    recordings: Option<Vec<AcoustIdRecording>>,
}

#[derive(serde::Deserialize, Debug)]
struct AcoustIdRecording {
    id: String,
}

#[derive(serde::Deserialize, Debug, serde::Serialize)]
struct MusicBrainzRecording {
    id: String,
    title: Option<String>,
    #[serde(rename = "artist-credit")]
    artist_credit: Option<Vec<ArtistCredit>>,
    releases: Option<Vec<MusicBrainzRelease>>,
    tags: Option<Vec<MusicBrainzTag>>,
}

#[derive(serde::Deserialize, Debug, serde::Serialize)]
struct ArtistCredit {
    name: String,
    joinphrase: Option<String>,
}

#[derive(serde::Deserialize, Debug, serde::Serialize)]
struct MusicBrainzRelease {
    id: String,
    title: Option<String>,
    date: Option<String>,
}

#[derive(serde::Deserialize, Debug, serde::Serialize)]
struct MusicBrainzTag {
    name: String,
    count: Option<i64>,
}

/// Resolves the absolute path of the bundled fpcalc sidecar binary.
fn get_fpcalc_path(app: &AppHandle) -> Option<PathBuf> {
    #[cfg(target_arch = "aarch64")]
    const TARGET_ARCH: &str = "aarch64";
    #[cfg(target_arch = "x86_64")]
    const TARGET_ARCH: &str = "x86_64";

    #[cfg(target_os = "macos")]
    const TARGET_OS: &str = "apple-darwin";
    #[cfg(target_os = "linux")]
    const TARGET_OS: &str = "unknown-linux-gnu";
    #[cfg(target_os = "windows")]
    const TARGET_OS: &str = "pc-windows-msvc";

    let triple = format!("{}-{}", TARGET_ARCH, TARGET_OS);
    let filename = if cfg!(target_os = "windows") {
        format!("fpcalc-{}.exe", triple)
    } else {
        format!("fpcalc-{}", triple)
    };

    // 1. Check next to the executable (production package where sidecar suffix is stripped or not)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let prod_filename = if cfg!(target_os = "windows") {
                "fpcalc.exe"
            } else {
                "fpcalc"
            };
            let p_prod = exe_dir.join(prod_filename);
            if p_prod.exists() {
                return Some(p_prod);
            }
            let p_triple = exe_dir.join(&filename);
            if p_triple.exists() {
                return Some(p_triple);
            }
        }
    }

    // 2. Check tauri resource dir (production package)
    if let Ok(res_dir) = app.path().resource_dir() {
        let p = res_dir.join("binaries").join(&filename);
        if p.exists() {
            return Some(p);
        }
    }

    // 3. Check local dev directories relative to repository root
    let dev_paths = vec![
        Path::new("src-tauri/binaries").join(&filename),
        Path::new("binaries").join(&filename),
        Path::new("../src-tauri/binaries").join(&filename),
    ];

    for p in dev_paths {
        if p.exists() {
            return Some(p);
        }
    }

    None
}

/// Runs the fingerprint calculation using the fpcalc sidecar.
fn run_fpcalc(fpcalc_path: &Path, file_path: &str) -> Result<FpcalcOutput, String> {
    log::info!("[acoustid] Invoking fpcalc on: {}", file_path);
    let output = std::process::Command::new(fpcalc_path)
        .arg("-json")
        .arg(file_path)
        .output()
        .map_err(|e| format!("Failed to execute fpcalc: {}", e))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("fpcalc failed: {}", err));
    }

    let parsed: FpcalcOutput = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse fpcalc JSON output: {}", e))?;

    Ok(parsed)
}

/// Core pipeline: fpcalc -> AcoustID -> MusicBrainz -> Cover Art Archive -> DB update.
pub async fn enrich_track(track_id: i64, force: bool, app: &AppHandle) -> Result<(), String> {
    // 1. Resolve SQLite connection from tauri state
    let db_state = app
        .try_state::<Mutex<Connection>>()
        .ok_or_else(|| "Database connection not registered in tauri state".to_string())?;

    // If the API key is using the default placeholder, skip requests to prevent invalid key 400 errors
    if get_acoustid_client_key() == DEFAULT_ACOUSTID_CLIENT_KEY {
        log::warn!("[acoustid] AcoustID client key is using the default placeholder. Skipping network queries.");
        return Err("AcoustID API key is missing. Please set ACOUSTID_CLIENT_KEY at compile time.".to_string());
    }

    // Check setting acoustid_enrichment_enabled
    let is_enabled = {
        let conn = db_state.lock().map_err(|e| e.to_string())?;
        let val: Option<String> = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = 'acoustid_enrichment_enabled'",
                [],
                |row| row.get(0),
            )
            .ok();
        val.unwrap_or_else(|| "silent".to_string())
    };

    if is_enabled == "never" {
        log::info!("[acoustid] Enrichment is disabled ('never'). Skipping track {}", track_id);
        return Ok(());
    }

    // Check current acoustid_status to prevent redundant lookups
    let (track_path, current_status) = {
        let conn = db_state.lock().map_err(|e| e.to_string())?;
        let res: Result<(String, Option<String>), rusqlite::Error> = conn.query_row(
            "SELECT path, acoustid_status FROM tracks WHERE id = ?1",
            [track_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        );
        match res {
            Ok(val) => val,
            Err(e) => return Err(format!("Track not found in DB: {}", e)),
        }
    };

    if !force {
        if let Some(status) = current_status {
            if status == "found" || status == "not_found" || status == "pending" {
                log::info!(
                    "[acoustid] Track {} already has status '{}'. Skipping lazy enrichment.",
                    track_id,
                    status
                );
                return Ok(());
            }
        }
    }

    // Mark track status as pending
    {
        let conn = db_state.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE tracks SET acoustid_status = 'pending' WHERE id = ?1",
            [track_id],
        )
        .map_err(|e| e.to_string())?;
    }

    // 2. Resolve fpcalc sidecar executable path
    let fpcalc_path = get_fpcalc_path(app)
        .ok_or_else(|| "Could not find bundled fpcalc executable!".to_string())?;

    // 3. Generate acoustic fingerprint
    let fp_res = run_fpcalc(&fpcalc_path, &track_path);
    let fp = match fp_res {
        Ok(f) => f,
        Err(err) => {
            log::error!("[acoustid] Fingerprinting failed: {}", err);
            let conn = db_state.lock().map_err(|e| e.to_string())?;
            let _ = conn.execute(
                "UPDATE tracks SET acoustid_status = 'error' WHERE id = ?1",
                [track_id],
            );
            return Err(err);
        }
    };

    log::info!(
        "[acoustid] Staged fingerprint successfully. Duration: {}s",
        fp.duration
    );

    // 4. Query AcoustID for recording MBID
    let acoustid_url = "https://api.acoustid.org/v2/lookup";
    let duration_secs = fp.duration.round() as i32;
    let form_data = format!(
        "client={}&duration={}&fingerprint={}&meta=recordings+compress",
        get_acoustid_client_key(), duration_secs, fp.fingerprint
    );

    let resp_res = ureq::post(acoustid_url)
        .set("Content-Type", "application/x-www-form-urlencoded")
        .send_string(&form_data);

    let resp = match resp_res {
        Ok(r) => r,
        Err(e) => {
            let error_msg = match e {
                ureq::Error::Status(code, response) => {
                    let body_str = response.into_string().unwrap_or_else(|_| "<failed to read body>".to_string());
                    format!("status code {}, response: {}", code, body_str)
                }
                other => other.to_string(),
            };
            log::error!("[acoustid] AcoustID API lookup failed: {}", error_msg);
            let conn = db_state.lock().map_err(|e| e.to_string())?;
            let _ = conn.execute(
                "UPDATE tracks SET acoustid_status = 'error' WHERE id = ?1",
                [track_id],
            );
            return Err(error_msg);
        }
    };

    let parsed: AcoustIdResponse = resp
        .into_json()
        .map_err(|e| format!("Failed to parse AcoustID response JSON: {}", e))?;

    if parsed.status != "ok" {
        let conn = db_state.lock().map_err(|e| e.to_string())?;
        let _ = conn.execute(
            "UPDATE tracks SET acoustid_status = 'error' WHERE id = ?1",
            [track_id],
        );
        return Err(format!("AcoustID status returned not ok: {}", parsed.status));
    }

    let mbid = match parsed.results {
        Some(ref res) if !res.is_empty() => {
            // Find recording in highest-confidence result
            let best_result = &res[0];
            match best_result.recordings {
                Some(ref recs) if !recs.is_empty() => recs[0].id.clone(),
                _ => {
                    log::info!("[acoustid] AcoustID match found but no MusicBrainz MBID linked.");
                    let conn = db_state.lock().map_err(|e| e.to_string())?;
                    conn.execute(
                        "UPDATE tracks SET acoustid_status = 'not_found' WHERE id = ?1",
                        [track_id],
                    )
                    .map_err(|e| e.to_string())?;
                    return Ok(());
                }
            }
        }
        _ => {
            log::info!("[acoustid] AcoustID returned no matches for track {}.", track_id);
            let conn = db_state.lock().map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE tracks SET acoustid_status = 'not_found' WHERE id = ?1",
                [track_id],
            )
            .map_err(|e| e.to_string())?;
            return Ok(());
        }
    };

    log::info!("[acoustid] Matched AcoustID successfully. MBID: {}", mbid);

    // 5. Query MusicBrainz recording endpoint with 1 req/sec rate limit compliance
    let mb_url = format!(
        "https://musicbrainz.org/ws/2/recording/{}?inc=artists+releases+tags&fmt=json",
        mbid
    );

    let mb_parsed = {
        // Critical section for rate limiter
        let _permit = get_mb_semaphore()
            .acquire()
            .await
            .map_err(|e| e.to_string())?;

        log::info!("[acoustid] Fetching rich metadata from MusicBrainz...");
        let mb_resp_res = ureq::get(&mb_url)
            .set("User-Agent", &user_agent())
            .call();

        // Hold permit and sleep to ensure spacing
        tokio::time::sleep(Duration::from_millis(1000)).await;

        let mb_resp = match mb_resp_res {
            Ok(r) => r,
            Err(e) => {
                log::error!("[acoustid] MusicBrainz fetch failed: {}", e);
                let conn = db_state.lock().map_err(|e| e.to_string())?;
                let _ = conn.execute(
                    "UPDATE tracks SET acoustid_status = 'error' WHERE id = ?1",
                    [track_id],
                );
                return Err(e.to_string());
            }
        };

        let parsed_rec: MusicBrainzRecording = mb_resp
            .into_json()
            .map_err(|e| format!("Failed to parse MusicBrainz JSON: {}", e))?;
        parsed_rec
    };

    // Extract rich fields
    let title_opt = mb_parsed.title.clone();
    
    let artist_opt = mb_parsed.artist_credit.as_ref().map(|credits| {
        let mut artist_str = String::new();
        for credit in credits {
            artist_str.push_str(&credit.name);
            if let Some(ref join) = credit.joinphrase {
                artist_str.push_str(join);
            }
        }
        artist_str
    });

    let (album_opt, release_mbid_opt, year_opt) = match mb_parsed.releases {
        Some(ref releases) if !releases.is_empty() => {
            let rel = &releases[0];
            let date_year = rel.date.as_ref().and_then(|d| {
                d.split('-').next().and_then(|y| y.parse::<i64>().ok())
            });
            (rel.title.clone(), Some(rel.id.clone()), date_year)
        }
        _ => (None, None, None),
    };

    let genre_opt = mb_parsed.tags.as_ref().and_then(|tags| {
        // Take the highest tag count
        if tags.is_empty() {
            None
        } else {
            Some(tags[0].name.clone())
        }
    });

    // 6. Try downloading cover art from Cover Art Archive if release exists
    let mut cover_art_blob: Option<Vec<u8>> = None;
    if let Some(ref release_mbid) = release_mbid_opt {
        // Rate-limited Cover Art Archive query
        let _permit = get_mb_semaphore()
            .acquire()
            .await
            .map_err(|e| e.to_string())?;

        let caa_url = format!("https://coverartarchive.org/release/{}/front-250", release_mbid);
        log::info!("[acoustid] Querying Cover Art Archive: {}", caa_url);

        let caa_resp_res = ureq::get(&caa_url)
            .set("User-Agent", &user_agent())
            .call();

        tokio::time::sleep(Duration::from_millis(1000)).await;

        if let Ok(caa_resp) = caa_resp_res {
            let mut bytes = Vec::new();
            if let Ok(_) = caa_resp.into_reader().read_to_end(&mut bytes) {
                if !bytes.is_empty() {
                    log::info!(
                        "[acoustid] Downloaded cover art successfully. Size: {} bytes",
                        bytes.len()
                    );
                    cover_art_blob = Some(bytes);
                }
            }
        } else {
            log::info!("[acoustid] Cover art not available or CAA returned error.");
        }
    }

    // 7. Silent auto-update phase: Overwrite local database missing fields
    {
        let conn = db_state.lock().map_err(|e| e.to_string())?;

        // Cache full MusicBrainz payload into enriched_metadata JSON
        let enriched_payload_json = serde_json::to_string(&mb_parsed).unwrap_or_default();

        log::info!("[acoustid] Merging resolved MusicBrainz fields into track {} DB...", track_id);
        
        // Execute silent updates
        conn.execute(
            "UPDATE tracks SET 
                title = COALESCE(title, ?1),
                artist = COALESCE(artist, ?2),
                album = COALESCE(album, ?3),
                genre = COALESCE(genre, ?4),
                year = COALESCE(year, ?5),
                cover_art = COALESCE(cover_art, ?6),
                musicbrainz_id = ?7,
                enriched_metadata = ?8,
                acoustid_status = 'found'
             WHERE id = ?9",
            rusqlite::params![
                title_opt,
                artist_opt,
                album_opt,
                genre_opt,
                year_opt,
                cover_art_blob,
                mbid,
                enriched_payload_json,
                track_id
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    // 8. Emit track-enriched event to notify Svelte UI
    app.emit("track-enriched", track_id)
        .map_err(|e: tauri::Error| e.to_string())?;

    log::info!(
        "[acoustid] Enrichment pipeline successfully completed for track {}!",
        track_id
    );

    Ok(())
}
