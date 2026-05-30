# BPM Correction Passes — Design Document

## Problem

The native DSP BPM detector frequently produces half-tempo or double-tempo values:
- Many tracks cluster at 207–208 BPM (detector ceiling), real tempo ~103 BPM
- Classical pieces detected at 220+ BPM, real tempo ~110 BPM
- Some tracks detected below 70 BPM that should be doubled
- Garbage values (e.g. -799) from non-music content (audiobooks, speech)

Genre context is essential: 170 BPM is correct for drum & bass but wrong for downtempo.

## Approach

Two sequential correction passes using progressively more precise genre information:

| Pass | Priority | Genre source | Runs after |
|------|----------|-------------|------------|
| `bpm_correction` | 15 | `tracks.genre` (metadata tag, coarse) | `audio_analysis` (10) |
| `bpm_refinement` | 55 | `tracks.detected_genre` (Essentia Discogs-400, precise) | `essentia` (50) |

Each pass updates `tracks.bpm` in place. The first pass uses coarse metadata genre
to fix obvious outliers. The second refines using the Essentia classifier's 400-class
Discogs taxonomy, which has the precision needed to distinguish e.g.
`Electronic---Drum n Bass` (155–185 BPM) from `Electronic---Downtempo` (55–100 BPM).

Note: this means `bpm_refinement` depends on the Essentia classifier pass being ported
to deep-cuts (it exists in the music-intelligence prototype). The Qwen `ai_genre` field
is intentionally **not** used for BPM correction — free-text genre is less reliable than
Essentia's fixed taxonomy for this purpose.

## Database Changes

```sql
ALTER TABLE tracks ADD COLUMN bpm_raw REAL;  -- original detector output, never updated
```

`bpm_raw` is written once by `audio_analysis` alongside `bpm`. Subsequent passes only
touch `bpm`. It is not shown in the UI — debug/diagnostics only.

## Correction Algorithm

```
fn correct_bpm(bpm: f64, range: (f64, f64)) -> f64:
    let (min, max) = range
    if bpm <= 0 or bpm is garbage (< 20 or > 300):
        return NULL           -- unrecoverable, null out
    loop:
        if bpm > max: bpm /= 2.0
        elif bpm < min: bpm *= 2.0
        else: break
        if bpm < 20 or bpm > 300: return NULL  -- diverged
    return round(bpm, 1)
```

Applied iteratively so extreme values (e.g. 414 BPM → 207 → 103) converge correctly.

## Genre BPM Ranges

### Pass 1 — metadata genre (coarse iTunes-style tags)

`tracks.genre` contains iTunes-style tags: "Pop", "Rock", "Classical", "Hip Hop/Rap",
"Audiobook", "iTunes U", "Unclassifiable", etc. These are too coarse to distinguish
e.g. doom metal from thrash, so Pass 1 only acts on cases where even a broad genre
label gives high confidence — primarily to catch obvious doublings and to immediately
NULL non-music content without waiting for Essentia.

Matched case-insensitively. First match wins.

| Keywords | Range (BPM) | Rationale |
|----------|-------------|-----------|
| `audiobook`, `spoken`, `podcast`, `comedy` | → NULL | Non-music, no valid BPM |
| `classical`, `orchestral`, `opera` | 40–200 | Wide but catches 220+ doublings |
| `jazz` | 60–220 | Very wide, only catches gross errors |
| `hip.?hop`, `rap` | 60–115 | iTunes "Hip Hop/Rap" is reliably slow |
| `reggae`, `ska`, `dub` | 55–105 | |
| *(everything else: pop, rock, country, world, etc.)* | no correction | Too ambiguous — defer to Pass 2 |

Pass 1 is intentionally conservative. A "Pop" or "Rock" track at 207 BPM is left
alone here — it might be Judas Priest. Pass 2 will correct it with certainty once
Essentia provides `Electronic---House` or `Rock---Heavy Metal`.

### Pass 2 — `detected_genre` (Essentia Discogs-400, precise)

Same algorithm, same range table, but matching against the full `Parent---Subgenre`
string (e.g. `Electronic---Drum n Bass`, `Rock---Doom Metal`). Match on the subgenre
first; fall back to parent category if no subgenre entry exists.

If `detected_genre` is NULL (essentia pass failed or skipped), this pass is a no-op.

#### Non-Music handling

The Discogs-400 taxonomy includes a `Non-Music` parent class
(`Non-Music---Audiobook`, `Non-Music---Spoken Word`, etc.). When `detected_genre`
starts with `Non-Music`, set `bpm = NULL` — no meaningful tempo for speech/sound
effects. This complements Qwen's `MUSIC: no` detection but works independently.

#### Selected subgenre ranges (Discogs-400)

| Discogs-400 class | Range (BPM) |
|-------------------|-------------|
| `Electronic---Drum n Bass` | 155–185 |
| `Electronic---Jungle` | 155–175 |
| `Electronic---Gabber` / `Electronic---Speedcore` | 160–300 |
| `Electronic---Hardcore` / `Electronic---Hardstyle` | 145–175 |
| `Electronic---Dubstep` | 130–145 |
| `Electronic---Techno` / `Electronic---Hard Techno` | 130–160 |
| `Electronic---Trance` / `Electronic---Psy-Trance` | 130–160 |
| `Electronic---House` / `Electronic---Deep House` | 118–135 |
| `Electronic---Downtempo` / `Electronic---Trip Hop` | 55–100 |
| `Electronic---Ambient` / `Electronic---Drone` | 40–90 |
| `Hip Hop---Trap` | 60–90 (half-time at 140) |
| `Hip Hop---*` (all others) | 70–115 |
| `Rock---Doom Metal` / `Rock---Funeral Doom Metal` / `Rock---Sludge Metal` | 40–80 |
| `Rock---Death Metal` / `Rock---Black Metal` / `Rock---Thrash` | 80–220 |
| `Rock---Grindcore` / `Rock---Powerviolence` | 100–260 |
| `Rock---Progressive Metal` / `Rock---Post-Metal` | 70–180 |
| `Rock---*` (general metal/rock) | 70–180 |
| `Classical---Baroque` / `Classical---Renaissance` | 50–160 |
| `Classical---*` | 40–200 |
| `Jazz---*` | 60–240 |
| `Reggae---*` | 55–100 |
| `Folk, World, & Country---*` | 60–160 |
| `Non-Music---*` | → NULL |

## Pipeline Sequencing

```
audio_analysis (10)  — writes bpm → bpm_raw (copy on first write), bpm
bpm_correction (15)  — reads tracks.genre (metadata), updates bpm
clap (20)            — reads corrected bpm
qwen (30)            — reads corrected bpm for its own prompt
description_embed (40)
essentia (50)        — writes detected_genre (Discogs-400)
bpm_refinement (55)  — reads detected_genre, updates bpm again; NULLs bpm for Non-Music
```

Note: Essentia runs after Qwen so the heavy passes (llama-server + Essentia models)
don't overlap in memory. This ordering also means `bpm_refinement` has the most
accurate genre information available before anything downstream uses `bpm`.

## `reset_pass` Behaviour

- `reset_pass("bpm_correction")` → resets only `bpm_correction` pass rows to pending;
  does **not** restore `bpm` from `bpm_raw` (re-running the pass will recompute from `bpm_raw`)
- `reset_pass("bpm_refinement")` → same, resets only `bpm_refinement` rows
- `reset_pass("audio_analysis")` → resets `bpm` and `bpm_raw` to NULL along with
  all downstream passes

## New Rust Code Location

All logic lives in a new `src-tauri/src/bpm.rs`:

```rust
pub fn genre_bpm_range(genre: Option<&str>) -> (f64, f64)
pub fn correct_bpm(raw: Option<f64>, genre: Option<&str>) -> Option<f64>
```

Called from `analysis.rs` in the `bpm_correction` and `bpm_refinement` phase handlers.
