# DSP Feature Caching — Implemented Outcome (2026-06-07)

Doc-sync note for `dsp_recommendations.md` (which lives in the main checkout, not
this worktree branch). Feature (B) — caching intermediate DSP features — shipped on
this branch following the **[Claude, 10:57]** review corrections in `session.md`,
which override the original proposal where they differ.

## What landed

- **Onsets, not beats.** The proposal's `beat_onsets` is implemented as `onsets`,
  peak-picked from the per-frame spectral-flux envelope. No beat/downbeat phase is
  produced (BPM stays autocorrelation tempo only). `AudioAnalysisResult` gains
  `onsets: Vec<(f32, f32)>` = (time_seconds, normalised_strength).

- **Storage (hybrid).** Per the review: compact onset peak list → **SQLite**, fat
  chroma time-series → **sidecar**.
  - New DB column `tracks.onsets TEXT` (migration `src-tauri/migrations/32_dsp_onsets.sql`,
    registered after migration 30 in `database.rs::get_migrations()`), stored as compact
    JSON `{"times":[…],"strengths":[…]}`. Added to `AudioPass::SPEC.owned_columns` so the
    reset and sidecar round-trip are automatic.
  - Chroma time-series written to `.dc.json` under a new `dsp_features` key
    (`{chroma_time_step, chroma_times, chroma_series}`) via a new
    `scanner::sidecar::save_with_extra`. Not a DB column (avoids bloat). `restore` ignores
    the key — it is a read-only cache for Python eval scripts.

- **Chroma is emitted, not just "un-discarded".** The 10 s-block accumulation loop in
  `dsp.rs::analyze_key_and_bpm_joint` now collects per-FFT-frame chroma, then bins it to a
  **0.2 s** time-step, L1-normalised per frame.

- **Onset peak-picking method:** adaptive-threshold local-maximum picker over the
  ~23 ms-frame flux envelope. A frame is a peak if it is a local maximum (±2 frames),
  exceeds the global mean flux, and exceeds a ~0.5 s local-mean window plus 10% of the
  global max flux, with a ~0.05 s minimum spacing between peaks.

- **Times** for both features are seconds from the start of the analysis window (the
  centre 90 s crop used for key/BPM), not from the start of the track.

- `pass_version::AUDIO_ANALYSIS` bumped 1 → 2 so existing tracks re-run on the next
  pipeline pass. Re-running the full library is the user's call (not auto-triggered here).

## Not done (out of scope)

No boundary detection, no snapping, no structural-alignment change. Purely additive
feature persistence, as scoped.

## Files changed

- `src-tauri/src/dsp.rs` — `AudioAnalysisResult` + `JointAnalysis` structs, per-frame
  timeline collection in the analysis loop, `pick_onset_peaks`, `bin_chroma_series`, tests.
- `src-tauri/src/analysis/audio.rs` — write `onsets` column; write chroma series to sidecar
  via `save_with_extra`; `onsets` added to `owned_columns`.
- `src-tauri/src/scanner/sidecar.rs` — `save_with_extra`; `AUDIO_ANALYSIS` version 1 → 2.
- `src-tauri/src/database.rs` — register migration 32.
- `src-tauri/migrations/32_dsp_onsets.sql` — `ALTER TABLE tracks ADD COLUMN onsets TEXT;`
