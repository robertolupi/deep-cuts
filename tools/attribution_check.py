#!/usr/bin/env python3
"""Attribution Check: Baseline + Refined-Only Candidate Ceiling.

Evaluates candidate ceiling when the candidate pool contains ONLY baseline and refined
boundaries (excluding SSM peaks) to attribute the fine-resolution headroom.
"""

import json
from pathlib import Path
import numpy as np

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent

# Load diagnostic results
json_path = SCRIPT_DIR / "diagnostic_salami_phase1_results.json"
if not json_path.exists():
    print(f"Error: run diagnostics first to write {json_path}")
    sys.exit(1)

from evaluate_salami_phase0 import DEFAULT_DB_PATH, DEFAULT_VALIDATION_SPLIT, calculate_crop_offset, load_track, to_absolute_time, _prepare_pass_for_mode, _score_boundaries
from evaluate_salami_boundaries import JAMS_DIR, load_onset_map, parse_jams_boundaries_and_labels
from evaluate_salami_phase1a import load_high_res_features
from diagnostic_salami_phase1 import compute_candidate_oracle_boundaries

with open(DEFAULT_VALIDATION_SPLIT, "r", encoding="utf-8") as f:
    split_entries = json.load(f)

# Load tracks and compute ceiling
tracks = []
skipped = []
onset_map = load_onset_map()
for entry in split_entries:
    try:
        db_id = entry["db_id"]
        salami_id = entry["salami_id"]
        path = entry.get("path")
        track = load_track(str(db_id), DEFAULT_DB_PATH)
        track["salami_id"] = salami_id
        track["path"] = path
        jams_path = JAMS_DIR / f"SALAMI_{salami_id}.jams"
        offset = onset_map.get(salami_id, 0.0)
        passes = parse_jams_boundaries_and_labels(jams_path, offset, track["duration"])
        if not passes:
            continue
        track["passes"] = passes
        tracks.append(track)
    except Exception:
        pass

# Seed and split exactly as diagnostic check
rng = np.random.default_rng(42)
shuffled_tracks = list(tracks)
rng.shuffle(shuffled_tracks)
split_idx = int(len(shuffled_tracks) * 0.8)
heldback_tracks = shuffled_tracks[split_idx:]

print(f"Loaded {len(heldback_tracks)} heldback tracks.")

f1s_05 = []
f1s_30 = []

for track in heldback_tracks:
    duration = float(track["duration"])
    offset = calculate_crop_offset(duration, 90.0)
    
    # Candidate pool: ONLY baseline + refined boundaries
    candidates = []
    candidates.extend(track["baseline_boundaries"])
    candidates.extend(track["refined_boundaries"])
    candidates = sorted(list(set(candidates)))
    
    track_f1s_05 = []
    track_f1s_30 = []
    for pass_info in track["passes"]:
        prepared, eval_duration, _ = _prepare_pass_for_mode(pass_info, "windowed", duration, 90.0)
        if prepared["segments"]:
            pred_ceiling_05 = compute_candidate_oracle_boundaries(prepared["boundaries"], candidates, 0.5)
            pred_ceiling_30 = compute_candidate_oracle_boundaries(prepared["boundaries"], candidates, 3.0)
            
            score_05 = _score_boundaries(prepared["boundaries"], pred_ceiling_05, eval_duration, 0.5)
            score_30 = _score_boundaries(prepared["boundaries"], pred_ceiling_30, eval_duration, 3.0)
            
            track_f1s_05.append(score_05["f1"])
            track_f1s_30.append(score_30["f1"])
            
    if track_f1s_05 and track_f1s_30:
        f1s_05.append(np.mean(track_f1s_05))
        f1s_30.append(np.mean(track_f1s_30))

print(f"Baseline+Refined-Only Ceiling F1@0.5s: {np.mean(f1s_05)*100:.2f}%")
print(f"Baseline+Refined-Only Ceiling F1@3.0s: {np.mean(f1s_30)*100:.2f}%")
