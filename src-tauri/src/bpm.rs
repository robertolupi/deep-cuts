use rusqlite::Connection;

/// BPM correction utilities for the `bpm_correction` and `bpm_refinement` passes.
///
/// The DSP detector frequently outputs half- or double-tempo values (e.g. 206.7 BPM
/// is the detector ceiling for ~103 BPM tracks). Knowing the genre lets us define a
/// plausible range as a Gaussian profile (centroid and spread) and select the multiplier
/// (raw, half, or double) that maximizing the probability score.

/// Returns the expected BPM profile `(centroid_bpm, std_dev)` for a given genre string,
/// or `None` if the genre is unknown / too ambiguous to correct.
///
/// `genre` may be:
/// - A coarse iTunes-style tag for Pass 1 (`"Classical"`, `"Hip-Hop/Rap"`, …)
/// - A Discogs-400 `Parent---Subgenre` string for Pass 2 (`"Electronic---House"`, …)
///
/// Matching is case-insensitive. Subgenre is tried first; parent falls back.
pub fn genre_bpm_profile(genre: &str) -> Option<(f64, f64)> {
    let g = genre.to_lowercase();

    // --- Non-music: NULL out entirely (signal via (0,0) sentinel) ---
    if g.starts_with("non-music")
        || g.contains("audiobook")
        || g.contains("spoken")
        || g.contains("podcast")
        || g.contains("comedy")
        || g.contains("dialogue")
        || g.contains("interview")
        || g.contains("monolog")
        || g.contains("radioplay")
        || g.contains("religious")
        || g.contains("poetry")
    {
        return Some((0.0, 0.0)); // sentinel: NULL out bpm
    }

    // --- Electronic subgenres (most specific first) ---
    if g.contains("drum n bass") || g.contains("drum & bass") || g.contains("drumfunk") {
        return Some((170.0, 8.0));
    }
    if g.contains("jungle") {
        return Some((165.0, 6.0));
    }
    if g.contains("gabber") || g.contains("speedcore") {
        return Some((210.0, 40.0));
    }
    if g.contains("hardcore") || g.contains("hardstyle") || g.contains("hard techno") {
        return Some((160.0, 8.0));
    }
    if g.contains("dubstep") {
        return Some((140.0, 5.0));
    }
    if g.contains("techno") || g.contains("trance") || g.contains("psy-trance") {
        return Some((140.0, 8.0));
    }
    if g.contains("house")
        || g.contains("deep house")
        || g.contains("tech house")
        || g.contains("dance-pop")
        || g.contains("euro house")
    {
        return Some((126.0, 5.0));
    }
    if g.contains("downtempo")
        || g.contains("trip hop")
        || g.contains("chillout")
        || g.contains("new age")
        || g.contains("acid jazz")
    {
        return Some((80.0, 12.0));
    }
    if g.contains("ambient") || g.contains("drone") || g.contains("dark ambient") {
        return Some((70.0, 15.0));
    }
    if g.contains("synth-pop") || g.contains("electropop") || g.contains("electroclash") {
        return Some((120.0, 12.0));
    }
    if g.contains("breakbeat") || g.contains("big beat") || g.contains("nu skool breaks") {
        return Some((130.0, 8.0));
    }
    if g.contains("disco") || g.contains("italo-disco") {
        return Some((120.0, 7.0));
    }
    // Electronic parent catch-all
    if g.starts_with("electronic") {
        return Some((120.0, 18.0));
    }

    // --- Hip Hop ---
    if g.contains("trap") {
        return Some((75.0, 8.0)); // trap is typically written at half-time
    }
    if g.contains("hip hop")
        || g.contains("hip-hop")
        || g.contains("rap")
        || g.contains("rnb")
        || g.contains("r&b")
        || g.contains("r'n'b")
    {
        return Some((92.5, 12.0));
    }

    // --- Rock / Metal subgenres ---
    if g.contains("doom metal") || g.contains("funeral doom") || g.contains("sludge metal") {
        return Some((60.0, 10.0));
    }
    if g.contains("grindcore") || g.contains("powerviolence") {
        return Some((180.0, 40.0));
    }
    if g.contains("death metal") || g.contains("black metal") || g.contains("thrash") {
        return Some((150.0, 35.0));
    }
    if g.contains("progressive metal") || g.contains("post-metal") || g.contains("post metal") {
        return Some((110.0, 25.0));
    }
    // Rock / Metal parent catch-all
    if g.starts_with("rock") {
        return Some((110.0, 25.0));
    }

    // --- Classical ---
    if g.contains("baroque") || g.contains("renaissance") {
        return Some((95.0, 25.0));
    }
    if g.contains("classical")
        || g.contains("orchestral")
        || g.contains("opera")
        || g.contains("romantic")
        || g.contains("impressionist")
        || g.contains("modern")
        || g.contains("contemporary")
        || g.contains("score")
        || g.contains("soundtrack")
    {
        return Some((100.0, 35.0));
    }

    // --- Jazz ---
    if g.contains("jazz") || g.contains("bebop") || g.contains("swing") {
        return Some((120.0, 45.0));
    }

    // --- Reggae / Ska / Dub ---
    if g.contains("reggae") || g.contains("ska") || g.contains("dub") || g.contains("dancehall") {
        return Some((75.0, 12.0));
    }

    // --- Folk / World / Country ---
    if g.contains("folk")
        || g.contains("country")
        || g.contains("world")
        || g.contains("latin")
        || g.contains("forró")
        || g.contains("forro")
        || g.contains("laïkó")
        || g.contains("laiko")
        || g.contains("éntekhno")
        || g.contains("celtic")
        || g.contains("bluegrass")
        || g.contains("gospel")
    {
        return Some((100.0, 25.0));
    }

    // --- Blues / Funk / Soul ---
    if g.contains("blues") || g.contains("funk") || g.contains("soul") || g.contains("r&b") {
        return Some((100.0, 20.0));
    }

    // --- Pop ---
    if g == "pop" {
        return Some((110.0, 15.0));
    }

    None // Too ambiguous — leave BPM unchanged
}

/// Applies Gaussian fuzzy probability profile to raw `bpm` candidate.
/// Evaluates raw, half, and double tempos, choosing the highest probability option.
/// Returns `CorrectResult::Null` if the highest probability is below 0.10.
pub fn correct_bpm(bpm: Option<f64>, genre: Option<&str>) -> CorrectResult {
    let Some(raw) = bpm else {
        return CorrectResult::Unchanged;
    };

    let Some(genre_str) = genre else {
        return CorrectResult::Unchanged;
    };

    let Some((centroid, spread)) = genre_bpm_profile(genre_str) else {
        return CorrectResult::Unchanged;
    };

    // Non-music sentinel
    if (centroid, spread) == (0.0, 0.0) {
        return CorrectResult::Null;
    }

    // Garbage raw values outside all possible tempos
    if raw < 20.0 || raw > 300.0 {
        return CorrectResult::Null;
    }

    // Gaussian probability scoring function
    let score = |x: f64| -> f64 {
        if x < 20.0 || x > 300.0 {
            return 0.0;
        }
        (-((x - centroid).powi(2)) / (2.0 * spread.powi(2))).exp()
    };

    let v_raw = raw;
    let v_half = raw / 2.0;
    let v_double = raw * 2.0;

    let s_raw = score(v_raw);
    let s_half = score(v_half);
    let s_double = score(v_double);

    let mut best_v = v_raw;
    let mut best_score = s_raw;

    if s_half > best_score {
        best_v = v_half;
        best_score = s_half;
    }
    if s_double > best_score {
        best_v = v_double;
        best_score = s_double;
    }

    // If maximum probability is below 10%, we discard it
    if best_score < 0.10 {
        return CorrectResult::Null;
    }

    let corrected = (best_v * 10.0).round() / 10.0; // round to 1 decimal place
    if (corrected - raw).abs() < 0.05 {
        CorrectResult::Unchanged
    } else {
        CorrectResult::Corrected(corrected)
    }
}

/// Abstracted reset helper for all BPM passes.
pub fn reset_bpm_data(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "UPDATE tracks SET bpm = bpm_raw WHERE bpm_raw IS NOT NULL",
        [],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Abstracted save helper for all BPM passes.
pub fn save_bpm_result(conn: &Connection, track_id: i64, result: &CorrectResult) -> Result<(), String> {
    match result {
        CorrectResult::Corrected(new_bpm) => {
            conn.execute(
                "UPDATE tracks SET bpm = ?1 WHERE id = ?2",
                rusqlite::params![new_bpm, track_id],
            ).map_err(|e| e.to_string())?;
        }
        CorrectResult::Null => {
            conn.execute(
                "UPDATE tracks SET bpm = NULL WHERE id = ?1",
                rusqlite::params![track_id],
            ).map_err(|e| e.to_string())?;
        }
        CorrectResult::Unchanged => {}
    }
    Ok(())
}

/// Abstracted logging helper for all BPM passes.
pub fn format_bpm_result_json(bpm_raw: Option<f64>, genre: Option<&str>, result: &CorrectResult) -> String {
    match result {
        CorrectResult::Corrected(v) => serde_json::json!({
            "bpm_raw": bpm_raw,
            "genre": genre,
            "result": "corrected",
            "rule": if bpm_raw.map_or(false, |b| b > *v) { "halved" } else { "doubled" },
            "bpm_corrected": v,
        }),
        CorrectResult::Unchanged => serde_json::json!({
            "bpm_raw": bpm_raw,
            "genre": genre,
            "result": "unchanged",
        }),
        CorrectResult::Null => serde_json::json!({
            "bpm_raw": bpm_raw,
            "genre": genre,
            "result": "nulled",
        }),
    }.to_string()
}

#[derive(Debug, PartialEq)]
pub enum CorrectResult {
    /// BPM already in range — no update needed.
    Unchanged,
    /// BPM corrected to a new value.
    Corrected(f64),
    /// Non-music or diverged — set bpm to NULL.
    Null,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_halves_double_tempo() {
        // 206.7 BPM Classical Baroque → should halve to ~103.35 (rounds to 103.4)
        assert_eq!(
            correct_bpm(Some(206.7), Some("Classical---Baroque")),
            CorrectResult::Corrected(103.4)
        );
    }

    #[test]
    fn test_doubles_half_tempo() {
        // 65 BPM Electronic House → should double to 130
        assert_eq!(
            correct_bpm(Some(65.0), Some("Electronic---House")),
            CorrectResult::Corrected(130.0)
        );
    }

    #[test]
    fn test_in_range_unchanged() {
        assert_eq!(
            correct_bpm(Some(128.0), Some("Electronic---House")),
            CorrectResult::Unchanged
        );
        assert_eq!(
            correct_bpm(Some(170.0), Some("Electronic---Drum n Bass")),
            CorrectResult::Unchanged
        );
    }

    #[test]
    fn test_non_music_nulled() {
        assert_eq!(
            correct_bpm(Some(207.0), Some("Non-Music---Audiobook")),
            CorrectResult::Null
        );
        assert_eq!(
            correct_bpm(Some(207.0), Some("Non-Music---Spoken Word")),
            CorrectResult::Null
        );
        assert_eq!(
            correct_bpm(Some(100.0), Some("Non-Music---Radioplay")),
            CorrectResult::Null
        );
    }

    #[test]
    fn test_garbage_bpm_nulled() {
        assert_eq!(
            correct_bpm(Some(5.0), Some("Rock---Hard Rock")),
            CorrectResult::Null
        );
        assert_eq!(
            correct_bpm(Some(350.0), Some("Electronic---House")),
            CorrectResult::Null
        );
    }

    #[test]
    fn test_none_bpm_unchanged() {
        assert_eq!(
            correct_bpm(None, Some("Classical---Baroque")),
            CorrectResult::Unchanged
        );
    }

    #[test]
    fn test_unknown_genre_unchanged() {
        assert_eq!(
            correct_bpm(Some(207.0), Some("Polka---Oompa")),
            CorrectResult::Unchanged
        );
        assert_eq!(correct_bpm(Some(207.0), None), CorrectResult::Unchanged);
    }

    #[test]
    fn test_single_step_only() {
        // 414 > 300 → Null (garbage, before any correction)
        assert_eq!(
            correct_bpm(Some(414.0), Some("Classical---Baroque")),
            CorrectResult::Null
        );
        // 207 → 103.5 for Rock (one halve, lands near centroid 110)
        assert_eq!(
            correct_bpm(Some(207.0), Some("Rock---Hard Rock")),
            CorrectResult::Corrected(103.5)
        );
        // 200 for DnB (centroid 170, spread 8): 200/2=100. score(100) is exp(-(100-170)^2 / 128) = 0.0 < 0.10 → Null
        assert_eq!(
            correct_bpm(Some(200.0), Some("Electronic---Drum n Bass")),
            CorrectResult::Null
        );
        // 200 for House (centroid 126, spread 5): 200/2=100. score(100) is exp(-676 / 50) = 0.0 < 0.10 → Null
        assert_eq!(
            correct_bpm(Some(200.0), Some("Electronic---House")),
            CorrectResult::Null
        );
    }

    #[test]
    fn test_doom_metal_low_range() {
        assert_eq!(
            correct_bpm(Some(120.0), Some("Rock---Doom Metal")),
            CorrectResult::Corrected(60.0)
        );
        assert_eq!(
            correct_bpm(Some(60.0), Some("Rock---Doom Metal")),
            CorrectResult::Unchanged
        );
    }

    #[test]
    fn test_dnb_high_range() {
        assert_eq!(
            correct_bpm(Some(85.0), Some("Electronic---Drum n Bass")),
            CorrectResult::Corrected(170.0)
        );
        assert_eq!(
            correct_bpm(Some(170.0), Some("Electronic---Drum n Bass")),
            CorrectResult::Unchanged
        );
    }
}
