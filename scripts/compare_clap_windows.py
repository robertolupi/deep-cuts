#!/usr/bin/env python3
"""
Compare CLAP window selection algorithms for a given track.

Usage:
    python3 scripts/compare_clap_windows.py <track_id> [output_dir]

Produces:
    <output_dir>/track_<id>_current.wav   — 3 × 10 s windows (current: top-3 loudest spaced)
    <output_dir>/track_<id>_adaptive.wav  — 3 × 10 s windows (proposed: tercile for dynamic
                                            tracks, temporal spread for flat-loudness tracks)
"""

import json
import math
import os
import statistics
import subprocess
import sys
import tempfile

DB = os.path.expanduser(
    "~/Library/Application Support/com.rlupi.deep-cuts/deep_cuts.db"
)
WINDOW_SEC = 10.0
DEFAULT_PCTS = [0.2, 0.5, 0.8]

# Tracks with CV below this threshold are considered flat-loudness (modern mastering)
FLAT_CV_THRESHOLD = 0.25


# ── algorithm implementations ─────────────────────────────────────────────────

def waveform_cv(waveform):
    """Coefficient of variation of finite positive waveform values."""
    vals = [v for v in waveform if math.isfinite(v) and v > 0]
    if len(vals) < 2:
        return 0.0
    mean = statistics.mean(vals)
    return statistics.stdev(vals) / mean if mean > 0 else 0.0


def select_current(waveform, duration_seconds):
    """Top-3 loudest bins with minimum spacing of ~10 s between them."""
    if not waveform or all(not math.isfinite(v) or v <= 0 for v in waveform):
        return DEFAULT_PCTS[:]
    bin_count = len(waveform)
    min_sep = _min_sep(bin_count, duration_seconds)
    ranked = sorted(
        [(i, v) for i, v in enumerate(waveform) if math.isfinite(v)],
        key=lambda x: -x[1],
    )
    selected = []
    for idx, _ in ranked:
        if all(abs(idx - p) >= min_sep for p in selected):
            selected.append(idx)
        if len(selected) == 3:
            break
    for idx, _ in ranked:
        if idx not in selected:
            selected.append(idx)
        if len(selected) == 3:
            break
    if len(selected) < 3:
        return DEFAULT_PCTS[:]
    selected.sort()
    return [(s + 0.5) / bin_count for s in selected]


def select_adaptive(waveform, duration_seconds):
    """Tercile for dynamic tracks; temporal spread for flat-loudness tracks."""
    cv = waveform_cv(waveform)
    if cv < FLAT_CV_THRESHOLD:
        return select_temporal_spread(waveform, duration_seconds, cv)
    else:
        return select_tercile(waveform, duration_seconds)


def select_tercile(waveform, duration_seconds):
    """One representative window per energy tercile (low / mid / high),
    with minimum spacing constraint."""
    if not waveform or all(not math.isfinite(v) or v <= 0 for v in waveform):
        return DEFAULT_PCTS[:]
    bin_count = len(waveform)
    min_sep = _min_sep(bin_count, duration_seconds)
    finite_vals = sorted(v for v in waveform if math.isfinite(v) and v > 0)
    if len(finite_vals) < 3:
        return DEFAULT_PCTS[:]
    t1 = finite_vals[len(finite_vals) // 3]
    t2 = finite_vals[2 * len(finite_vals) // 3]
    bins = [(i, v) for i, v in enumerate(waveform) if math.isfinite(v)]
    low  = sorted([(i, v) for i, v in bins if v <= t1],       key=lambda x: -x[1])
    mid  = sorted([(i, v) for i, v in bins if t1 < v <= t2],  key=lambda x: -x[1])
    high = sorted([(i, v) for i, v in bins if v > t2],        key=lambda x: -x[1])
    picks = []
    for tercile in [low, mid, high]:
        for idx, _ in tercile:
            if all(abs(idx - p) >= min_sep for p in picks):
                picks.append(idx)
                break
    if len(picks) < 3:
        return DEFAULT_PCTS[:]
    picks.sort()
    return [(p + 0.5) / bin_count for p in picks]


def select_temporal_spread(waveform, duration_seconds, cv):
    """Three evenly-spaced windows at 15%, 50%, 85% for flat-loudness tracks."""
    # Fixed anchors that avoid intros/outros while covering the track body
    return [0.15, 0.50, 0.85]


def _min_sep(bin_count, duration_seconds):
    sep = (
        math.ceil((10.0 / duration_seconds) * bin_count)
        if duration_seconds > 0
        else max(bin_count // 12, 1)
    )
    return max(sep, 1)


# ── helpers ───────────────────────────────────────────────────────────────────

def query_track(track_id):
    result = subprocess.run(
        [
            "sqlite3", DB,
            ".mode json",
            f"SELECT path, duration_seconds, waveform_data "
            f"FROM tracks WHERE id = {track_id};",
        ],
        capture_output=True,
        text=True,
        check=True,
    )
    rows = json.loads(result.stdout)
    if not rows:
        raise SystemExit(f"Track {track_id} not found in database.")
    return rows[0]


def extract_window(path, center_pct, duration_seconds, out_wav):
    """Extract a WINDOW_SEC clip centred at center_pct using ffmpeg."""
    center = center_pct * duration_seconds
    start = max(0.0, center - WINDOW_SEC / 2)
    start = min(start, max(0.0, duration_seconds - WINDOW_SEC))
    subprocess.run(
        [
            "ffmpeg", "-y",
            "-ss", str(start),
            "-i", path,
            "-t", str(WINDOW_SEC),
            "-ar", "44100",
            "-ac", "2",
            out_wav,
        ],
        capture_output=True,
        check=True,
    )


def concatenate_wavs(wav_files, out_wav):
    """Concatenate wav_files into out_wav using ffmpeg concat demuxer."""
    with tempfile.NamedTemporaryFile(
        mode="w", suffix=".txt", delete=False
    ) as flist:
        for w in wav_files:
            flist.write(f"file '{w}'\n")
        flist_path = flist.name
    try:
        subprocess.run(
            [
                "ffmpeg", "-y",
                "-f", "concat",
                "-safe", "0",
                "-i", flist_path,
                "-c", "copy",
                out_wav,
            ],
            capture_output=True,
            check=True,
        )
    finally:
        os.unlink(flist_path)


def build_excerpt(label, path, pcts, duration_seconds, out_wav):
    with tempfile.TemporaryDirectory() as tmp:
        clips = []
        for i, pct in enumerate(pcts):
            clip = os.path.join(tmp, f"clip_{i}.wav")
            extract_window(path, pct, duration_seconds, clip)
            clips.append(clip)
            time_s = pct * duration_seconds
            print(f"  [{label}] window {i+1}: {pct:.2f} → {time_s/60:.1f} min ({time_s:.0f} s)")
        concatenate_wavs(clips, out_wav)
    print(f"  → {out_wav}")


# ── main ──────────────────────────────────────────────────────────────────────

def main():
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(1)

    track_id = int(sys.argv[1])
    out_dir = sys.argv[2] if len(sys.argv) > 2 else "."
    os.makedirs(out_dir, exist_ok=True)

    row = query_track(track_id)
    path = row["path"]
    duration = row["duration_seconds"] or 0
    waveform = json.loads(row["waveform_data"]) if row["waveform_data"] else []

    cv = waveform_cv(waveform)
    mode = "temporal-spread" if cv < FLAT_CV_THRESHOLD else "tercile"

    print(f"\nTrack {track_id}: {os.path.basename(path)}")
    print(f"  Duration: {duration//60}:{duration%60:02d}  |  Waveform bins: {len(waveform)}  |  CV: {cv:.3f}  |  adaptive mode: {mode}\n")

    cur_pcts = select_current(waveform, duration)
    adp_pcts = select_adaptive(waveform, duration)

    build_excerpt(
        "current",
        path,
        cur_pcts,
        duration,
        os.path.join(out_dir, f"track_{track_id}_current.wav"),
    )
    print()
    build_excerpt(
        "adaptive",
        path,
        adp_pcts,
        duration,
        os.path.join(out_dir, f"track_{track_id}_adaptive.wav"),
    )
    print("\nDone.")


if __name__ == "__main__":
    main()
