/// BPM correction utilities for the `bpm_correction` and `bpm_refinement` passes.
///
/// The DSP detector frequently outputs half- or double-tempo values (e.g. 206.7 BPM
/// is the detector ceiling for ~103 BPM tracks). Knowing the genre lets us define a
/// plausible range and iteratively halve/double until the value lands inside it.

/// Returns the expected BPM range `(min, max)` for a given genre string, or `None`
/// if the genre is unknown / too ambiguous to correct.
///
/// `genre` may be:
/// - A coarse iTunes-style tag for Pass 1 (`"Classical"`, `"Hip-Hop/Rap"`, …)
/// - A Discogs-400 `Parent---Subgenre` string for Pass 2 (`"Electronic---House"`, …)
///
/// Matching is case-insensitive. Subgenre is tried first; parent falls back.
pub fn genre_bpm_range(genre: &str) -> Option<(f64, f64)> {
    let g = genre.to_lowercase();

    // --- Non-music: NULL out entirely (signal via (0,0) sentinel) ---
    if g.starts_with("non-music") || g.contains("audiobook") || g.contains("spoken")
        || g.contains("podcast") || g.contains("comedy") || g.contains("dialogue")
        || g.contains("interview") || g.contains("monolog") || g.contains("radioplay")
        || g.contains("religious") || g.contains("poetry")
    {
        return Some((0.0, 0.0)); // sentinel: NULL out bpm
    }

    // --- Electronic subgenres (most specific first) ---
    if g.contains("drum n bass") || g.contains("drum & bass") || g.contains("drumfunk") {
        return Some((155.0, 185.0));
    }
    if g.contains("jungle") {
        return Some((155.0, 175.0));
    }
    if g.contains("gabber") || g.contains("speedcore") {
        return Some((160.0, 300.0));
    }
    if g.contains("hardcore") || g.contains("hardstyle") || g.contains("hard techno") {
        return Some((145.0, 175.0));
    }
    if g.contains("dubstep") {
        return Some((130.0, 145.0));
    }
    if g.contains("techno") || g.contains("trance") || g.contains("psy-trance") {
        return Some((130.0, 160.0));
    }
    if g.contains("house") || g.contains("deep house") || g.contains("tech house")
        || g.contains("dance-pop") || g.contains("euro house")
    {
        return Some((118.0, 138.0));
    }
    if g.contains("downtempo") || g.contains("trip hop") || g.contains("chillout")
        || g.contains("new age") || g.contains("acid jazz")
    {
        return Some((55.0, 100.0));
    }
    if g.contains("ambient") || g.contains("drone") || g.contains("dark ambient") {
        return Some((40.0, 90.0));
    }
    if g.contains("synth-pop") || g.contains("electropop") || g.contains("electroclash") {
        return Some((100.0, 145.0));
    }
    if g.contains("breakbeat") || g.contains("big beat") || g.contains("nu skool breaks") {
        return Some((120.0, 145.0));
    }
    if g.contains("disco") || g.contains("italo-disco") {
        return Some((110.0, 135.0));
    }
    // Electronic parent catch-all
    if g.starts_with("electronic") {
        return Some((90.0, 160.0));
    }

    // --- Hip Hop ---
    if g.contains("trap") {
        return Some((60.0, 90.0)); // trap is typically written at half-time
    }
    if g.contains("hip hop") || g.contains("hip-hop") || g.contains("rap")
        || g.contains("rnb") || g.contains("r&b") || g.contains("r'n'b")
    {
        return Some((70.0, 115.0));
    }

    // --- Rock / Metal subgenres ---
    if g.contains("doom metal") || g.contains("funeral doom") || g.contains("sludge metal") {
        return Some((40.0, 80.0));
    }
    if g.contains("grindcore") || g.contains("powerviolence") {
        return Some((100.0, 260.0));
    }
    if g.contains("death metal") || g.contains("black metal") || g.contains("thrash") {
        return Some((80.0, 220.0));
    }
    if g.contains("progressive metal") || g.contains("post-metal") || g.contains("post metal") {
        return Some((70.0, 180.0));
    }
    // Rock / Metal parent catch-all — wide enough not to flip correct values
    if g.starts_with("rock") {
        return Some((70.0, 180.0));
    }

    // --- Classical ---
    if g.contains("baroque") || g.contains("renaissance") {
        return Some((50.0, 160.0));
    }
    if g.contains("classical") || g.contains("orchestral") || g.contains("opera")
        || g.contains("romantic") || g.contains("impressionist") || g.contains("modern")
        || g.contains("contemporary") || g.contains("score") || g.contains("soundtrack")
    {
        return Some((40.0, 200.0));
    }

    // --- Jazz ---
    if g.contains("jazz") || g.contains("bebop") || g.contains("swing") {
        return Some((60.0, 240.0));
    }

    // --- Reggae / Ska / Dub ---
    if g.contains("reggae") || g.contains("ska") || g.contains("dub") || g.contains("dancehall") {
        return Some((55.0, 100.0));
    }

    // --- Folk / World / Country ---
    if g.contains("folk") || g.contains("country") || g.contains("world")
        || g.contains("latin") || g.contains("forró") || g.contains("forro")
        || g.contains("laïkó") || g.contains("laiko") || g.contains("éntekhno")
        || g.contains("celtic") || g.contains("bluegrass") || g.contains("gospel")
    {
        return Some((60.0, 160.0));
    }

    // --- Blues / Funk / Soul ---
    if g.contains("blues") || g.contains("funk") || g.contains("soul") || g.contains("r&b") {
        return Some((60.0, 145.0));
    }

    // --- Pop (coarse iTunes tag only — Discogs Pop subgenres handled above) ---
    if g == "pop" {
        return Some((80.0, 145.0));
    }

    None // Too ambiguous — leave BPM unchanged
}

/// Applies iterative halve/double correction to `bpm` until it falls within `range`.
/// Returns `None` if:
/// - `bpm` is `None`
/// - `genre` maps to the non-music sentinel `(0.0, 0.0)` → caller should NULL out bpm
/// - correction diverges outside 20–300 BPM
pub fn correct_bpm(bpm: Option<f64>, genre: Option<&str>) -> CorrectResult {
    let Some(raw) = bpm else {
        return CorrectResult::Unchanged;
    };

    let Some(genre_str) = genre else {
        return CorrectResult::Unchanged;
    };

    let Some(range) = genre_bpm_range(genre_str) else {
        return CorrectResult::Unchanged;
    };

    // Non-music sentinel
    if range == (0.0, 0.0) {
        return CorrectResult::Null;
    }

    let (min, max) = range;
    let mut v = raw;

    // Garbage values outside all possible tempos
    if v < 20.0 || v > 300.0 {
        return CorrectResult::Null;
    }

    loop {
        if v > max {
            v /= 2.0;
        } else if v < min {
            v *= 2.0;
        } else {
            break;
        }
        if v < 20.0 || v > 300.0 {
            return CorrectResult::Null;
        }
    }

    let corrected = (v * 10.0).round() / 10.0; // round to 1 decimal place
    if (corrected - raw).abs() < 0.05 {
        CorrectResult::Unchanged
    } else {
        CorrectResult::Corrected(corrected)
    }
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
        // 206.7 BPM Classical Baroque → should halve to ~103.3
        assert_eq!(correct_bpm(Some(206.7), Some("Classical---Baroque")), CorrectResult::Corrected(103.4));
    }

    #[test]
    fn test_doubles_half_tempo() {
        // 65 BPM Electronic House → should double to 130
        assert_eq!(correct_bpm(Some(65.0), Some("Electronic---House")), CorrectResult::Corrected(130.0));
    }

    #[test]
    fn test_in_range_unchanged() {
        assert_eq!(correct_bpm(Some(128.0), Some("Electronic---House")), CorrectResult::Unchanged);
        assert_eq!(correct_bpm(Some(170.0), Some("Electronic---Drum n Bass")), CorrectResult::Unchanged);
    }

    #[test]
    fn test_non_music_nulled() {
        assert_eq!(correct_bpm(Some(207.0), Some("Non-Music---Audiobook")), CorrectResult::Null);
        assert_eq!(correct_bpm(Some(207.0), Some("Non-Music---Spoken Word")), CorrectResult::Null);
        assert_eq!(correct_bpm(Some(100.0), Some("Non-Music---Radioplay")), CorrectResult::Null);
    }

    #[test]
    fn test_garbage_bpm_nulled() {
        assert_eq!(correct_bpm(Some(5.0), Some("Rock---Hard Rock")), CorrectResult::Null);
        assert_eq!(correct_bpm(Some(350.0), Some("Electronic---House")), CorrectResult::Null);
    }

    #[test]
    fn test_none_bpm_unchanged() {
        assert_eq!(correct_bpm(None, Some("Classical---Baroque")), CorrectResult::Unchanged);
    }

    #[test]
    fn test_unknown_genre_unchanged() {
        assert_eq!(correct_bpm(Some(207.0), Some("Polka---Oompa")), CorrectResult::Unchanged);
        assert_eq!(correct_bpm(Some(207.0), None), CorrectResult::Unchanged);
    }

    #[test]
    fn test_multi_step_correction() {
        // 414 → 207 → 103.5 for Baroque
        assert_eq!(correct_bpm(Some(414.0), Some("Classical---Baroque")), CorrectResult::Null); // 414 > 300 → Null
        // 207 → 103.5 for Rock
        assert_eq!(correct_bpm(Some(207.0), Some("Rock---Hard Rock")), CorrectResult::Corrected(103.5));
    }

    #[test]
    fn test_doom_metal_low_range() {
        assert_eq!(correct_bpm(Some(120.0), Some("Rock---Doom Metal")), CorrectResult::Corrected(60.0));
        assert_eq!(correct_bpm(Some(60.0), Some("Rock---Doom Metal")), CorrectResult::Unchanged);
    }

    #[test]
    fn test_dnb_high_range() {
        assert_eq!(correct_bpm(Some(85.0), Some("Electronic---Drum n Bass")), CorrectResult::Corrected(170.0));
        assert_eq!(correct_bpm(Some(170.0), Some("Electronic---Drum n Bass")), CorrectResult::Unchanged);
    }
}
