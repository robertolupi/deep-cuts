#!/usr/bin/env python3
import os
import json
import sqlite3
import csv
import warnings
import numpy as np
import mir_eval
from pathlib import Path

warnings.filterwarnings("ignore", category=UserWarning)

DB_PATH = os.path.expanduser("~/Library/Application Support/com.rlupi.deep-cuts/deep_cuts.db")
REPO_ROOT = Path(__file__).resolve().parent.parent
VAL_TRACKS_PATH = REPO_ROOT / "doc/collab/sessions/2026-06-07-salami-eval-design/validation_tracks.json"
CSV_PATH = "/Users/rlupi/src/gh/Salami-dataset-used-in-music-structure-classification/New_salami_dataframe.csv"
JAMS_DIR = Path("/Volumes/Extreme Pro/Salami/annotations")

def load_onset_map():
    onset_map = {}
    if not os.path.exists(CSV_PATH):
        print(f"Warning: CSV not found at {CSV_PATH}. Using 0.0 offset for all tracks.")
        return onset_map
    with open(CSV_PATH, "r", encoding="utf-8") as f:
        reader = csv.DictReader(f, delimiter="\t")
        for row in reader:
            salami_id_str = row.get("salami_id")
            onset_str = row.get("onset_in_youtube")
            if salami_id_str and onset_str:
                try:
                    onset_map[int(salami_id_str)] = float(onset_str)
                except ValueError:
                    pass
    return onset_map

def parse_jams_boundaries_and_labels(jams_path, onset_offset, duration):
    if not jams_path.exists():
        return []

    try:
        with open(jams_path, "r", encoding="utf-8") as f:
            data = json.load(f)
    except Exception as e:
        print(f"Error reading JAMS {jams_path}: {e}")
        return []

    annotators_passes = []
    for ann in data.get("annotations", []):
        if ann.get("namespace") == "segment_salami_function":
            segments = []
            boundaries = set()
            for entry in ann.get("data", []):
                start = entry.get("time", 0.0) - onset_offset
                duration_seg = entry.get("duration", 0.0)
                end = start + duration_seg
                label = entry.get("value", "unknown")
                
                # Keep segment information
                segments.append({
                    "start": start,
                    "end": end,
                    "label": label
                })
                # Boundaries are segment transitions (start/end times within [0, duration])
                if 0.0 < start < duration:
                    boundaries.add(start)
                if 0.0 < end < duration:
                    boundaries.add(end)
            if segments:
                annotators_passes.append({
                    "segments": segments,
                    "boundaries": sorted(list(boundaries))
                })
    return annotators_passes

def intervals(bounds, dur):
    b = sorted(set([0.0] + [x for x in bounds if 0 < x < dur] + [float(dur)]))
    return mir_eval.util.boundaries_to_intervals(np.array(b))

def evaluate_boundaries_mir(ref_b, est_b, dur, win):
    try:
        p, r, f = mir_eval.segment.detection(
            intervals(ref_b, dur), intervals(est_b, dur), window=win, trim=True
        )
        return p, r, f
    except Exception:
        return 0.0, 0.0, 0.0

def evaluate_boundaries_greedy(pred_bounds, gt_bounds, tolerance):
    # Precision, Recall, F1
    if not pred_bounds and not gt_bounds:
        return 1.0, 1.0, 1.0
    if not pred_bounds or not gt_bounds:
        return 0.0, 0.0, 0.0

    tp = 0
    matched_gt = set()
    for pb in pred_bounds:
        # Find closest ground truth boundary within tolerance
        best_gt = None
        best_dist = tolerance
        for gb in gt_bounds:
            dist = abs(pb - gb)
            if dist <= best_dist:
                best_gt = gb
                best_dist = dist
        if best_gt is not None:
            tp += 1
            matched_gt.add(best_gt)

    precision = tp / len(pred_bounds)
    recall = len(matched_gt) / len(gt_bounds)
    f1 = 2 * precision * recall / (precision + recall) if (precision + recall) > 0 else 0.0
    return precision, recall, f1

def evaluate_boundaries(ref_b, est_b, dur, win, method="mir"):
    if method == "greedy":
        return evaluate_boundaries_greedy(est_b, ref_b, win)
    return evaluate_boundaries_mir(ref_b, est_b, dur, win)

def sym_boundary_f1(a, b, dur, win, method="mir"):
    _, _, f_ab = evaluate_boundaries(a, b, dur, win, method=method)
    _, _, f_ba = evaluate_boundaries(b, a, dur, win, method=method)
    return 0.5 * (f_ab + f_ba)

def evaluate_pairwise_clustering(pred_labels, gt_segments, duration, step=0.5):
    # Construct labels for sampled frames
    times = []
    t = 0.0
    while t <= duration:
        times.append(t)
        t += step

    n_frames = len(times)
    if n_frames < 2:
        return 1.0, 1.0, 1.0

    # Get prediction label per frame
    # pred_labels has exactly 16 elements
    bin_dur = duration / 16.0
    pred_frames = []
    for t in times:
        bin_idx = min(int(t / bin_dur), 15)
        pred_frames.append(pred_labels[bin_idx])

    # Get ground truth label per frame
    gt_frames = []
    for t in times:
        lbl = "silence"
        for seg in gt_segments:
            if seg["start"] <= t <= seg["end"]:
                lbl = seg["label"]
                break
        gt_frames.append(lbl)

    # Compute pairwise TP, FP, FN
    tp, fp, fn = 0, 0, 0
    for i in range(n_frames):
        for j in range(i + 1, n_frames):
            pred_same = (pred_frames[i] == pred_frames[j])
            gt_same = (gt_frames[i] == gt_frames[j])
            
            if pred_same and gt_same:
                tp += 1
            elif pred_same and not gt_same:
                fp += 1
            elif not pred_same and gt_same:
                fn += 1

    precision = tp / (tp + fp) if (tp + fp) > 0 else 0.0
    recall = tp / (tp + fn) if (tp + fn) > 0 else 0.0
    f1 = 2 * precision * recall / (precision + recall) if (precision + recall) > 0 else 0.0
    return precision, recall, f1

def project_jams_to_16_bins(gt_segments, duration):
    bin_dur = duration / 16.0
    proj_labels = []
    for b in range(16):
        t_center = (b + 0.5) * bin_dur
        lbl = "silence"
        for seg in gt_segments:
            if seg["start"] <= t_center <= seg["end"]:
                lbl = seg["label"]
                break
        proj_labels.append(lbl)
    
    # Derive boundaries from projected labels
    proj_boundaries = []
    for i in range(1, 16):
        if proj_labels[i-1] != proj_labels[i]:
            proj_boundaries.append(i * bin_dur)
    return proj_labels, proj_boundaries

def main():
    if not VAL_TRACKS_PATH.exists():
        print(f"Error: Validation tracks list not found at {VAL_TRACKS_PATH}. Run prepare_eval_splits.py first.")
        return

    with open(VAL_TRACKS_PATH, "r") as f:
        val_tracks = json.load(f)

    onset_map = load_onset_map()

    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()

    # Track metrics accumulators
    # 1. Full validation set
    full_f1_0_5 = []
    full_f1_3_0 = []
    full_pw_f1 = []
    full_grid_f1_0_5 = []
    full_grid_f1_3_0 = []
    full_limit_f1_0_5 = []
    full_limit_f1_3_0 = []
    
    # 2. Dual-annotator subset
    dual_f1_0_5 = []
    dual_f1_3_0 = []
    dual_pw_f1 = []
    dual_grid_f1_0_5 = []
    dual_grid_f1_3_0 = []
    dual_limit_f1_0_5 = []
    dual_limit_f1_3_0 = []
    dual_human_f1_0_5 = []
    dual_human_f1_3_0 = []

    for track in val_tracks:
        db_id = track["db_id"]
        salami_id = track["salami_id"]
        
        # Load from DB
        cursor.execute("SELECT duration_seconds, sax_alignment_segments FROM tracks WHERE id = ?", (db_id,))
        row = cursor.fetchone()
        if not row or not row[0] or not row[1]:
            continue
        
        duration = row[0]
        sax_segments_str = row[1]
        pred_labels = sax_segments_str.split(",")

        # Construct predicted boundary times (where label changes)
        bin_dur = duration / 16.0
        pred_boundaries = []
        for i in range(1, 16):
            if pred_labels[i-1] != pred_labels[i]:
                pred_boundaries.append(i * bin_dur)

        # Parse JAMS ground truth
        jams_path = JAMS_DIR / f"SALAMI_{salami_id}.jams"
        onset_offset = onset_map.get(salami_id, 0.0)
        gt_passes = parse_jams_boundaries_and_labels(jams_path, onset_offset, duration)

        if not gt_passes:
            continue

        # Evaluate against each annotator's pass and average them for the track
        track_f1_0_5 = []
        track_f1_3_0 = []
        track_pw_f1 = []
        track_grid_0_5 = []
        track_grid_3_0 = []
        track_limit_0_5 = []
        track_limit_3_0 = []
        
        for pass_info in gt_passes:
            # Boundary F1 at 0.5s and 3.0s (using mir_eval)
            _, _, f1_0_5 = evaluate_boundaries(pass_info["boundaries"], pred_boundaries, duration, 0.5)
            _, _, f1_3_0 = evaluate_boundaries(pass_info["boundaries"], pred_boundaries, duration, 3.0)
            
            # Pairwise Clustering F1
            _, _, pw_f1 = evaluate_pairwise_clustering(pred_labels, pass_info["segments"], duration)

            # Low-res projection
            proj_labels, proj_boundaries = project_jams_to_16_bins(pass_info["segments"], duration)
            
            # Pred vs. Projected GT
            _, _, f1_g_0_5 = evaluate_boundaries(proj_boundaries, pred_boundaries, duration, 0.5)
            _, _, f1_g_3_0 = evaluate_boundaries(proj_boundaries, pred_boundaries, duration, 3.0)
            
            # Projected GT vs. Continuous GT (quantization upper limit)
            _, _, f1_lim_0_5 = evaluate_boundaries(pass_info["boundaries"], proj_boundaries, duration, 0.5)
            _, _, f1_lim_3_0 = evaluate_boundaries(pass_info["boundaries"], proj_boundaries, duration, 3.0)

            track_f1_0_5.append(f1_0_5)
            track_f1_3_0.append(f1_3_0)
            track_pw_f1.append(pw_f1)
            track_grid_0_5.append(f1_g_0_5)
            track_grid_3_0.append(f1_g_3_0)
            track_limit_0_5.append(f1_lim_0_5)
            track_limit_3_0.append(f1_lim_3_0)

        # Average over passes for this track
        mean_f1_0_5 = np.mean(track_f1_0_5)
        mean_f1_3_0 = np.mean(track_f1_3_0)
        mean_pw_f1 = np.mean(track_pw_f1)
        mean_grid_0_5 = np.mean(track_grid_0_5)
        mean_grid_3_0 = np.mean(track_grid_3_0)
        mean_limit_0_5 = np.mean(track_limit_0_5)
        mean_limit_3_0 = np.mean(track_limit_3_0)

        # Append to full validation set
        full_f1_0_5.append(mean_f1_0_5)
        full_f1_3_0.append(mean_f1_3_0)
        full_pw_f1.append(mean_pw_f1)
        full_grid_f1_0_5.append(mean_grid_0_5)
        full_grid_f1_3_0.append(mean_grid_3_0)
        full_limit_f1_0_5.append(mean_limit_0_5)
        full_limit_f1_3_0.append(mean_limit_3_0)

        # Append to dual-annotator subset if applicable
        if len(gt_passes) == 2:
            dual_f1_0_5.append(mean_f1_0_5)
            dual_f1_3_0.append(mean_f1_3_0)
            dual_pw_f1.append(mean_pw_f1)
            dual_grid_f1_0_5.append(mean_grid_0_5)
            dual_grid_f1_3_0.append(mean_grid_3_0)
            dual_limit_f1_0_5.append(mean_limit_0_5)
            dual_limit_f1_3_0.append(mean_limit_3_0)
            
            # Human ceiling (consensus)
            h_0_5 = sym_boundary_f1(gt_passes[0]["boundaries"], gt_passes[1]["boundaries"], duration, 0.5)
            h_3_0 = sym_boundary_f1(gt_passes[0]["boundaries"], gt_passes[1]["boundaries"], duration, 3.0)
            dual_human_f1_0_5.append(h_0_5)
            dual_human_f1_3_0.append(h_3_0)

    conn.close()

    if not full_f1_0_5:
        print("No tracks were evaluated. Check that the analysis passes have finished and JAMS files exist.")
        return

    # Print Headline Results on Full Validation Set
    print("\n=== HEADLINE RESULTS (Full Validation Set) ===")
    print(f"Tracks Evaluated: {len(full_f1_0_5)} / {len(val_tracks)}")
    print("-" * 50)
    print("1. SAX Prediction vs. Original Continuous JAMS GT:")
    print(f"   Boundary F1-Score (±0.5s tolerance): {100 * np.mean(full_f1_0_5):.2f}%")
    print(f"   Boundary F1-Score (±3.0s tolerance): {100 * np.mean(full_f1_3_0):.2f}%")
    print(f"   Pairwise Clustering F1-Score:       {100 * np.mean(full_pw_f1):.2f}%")
    print("-" * 50)
    print("2. SAX Prediction vs. 16-Bin Projected JAMS GT (Classification on Grid):")
    print(f"   Boundary F1-Score (±0.5s tolerance): {100 * np.mean(full_grid_f1_0_5):.2f}%")
    print(f"   Boundary F1-Score (±3.0s tolerance): {100 * np.mean(full_grid_f1_3_0):.2f}%")
    print("-" * 50)
    print("3. Theoretical Upper Limit of 16-Bin Grid (Projected JAMS vs. Continuous JAMS):")
    print(f"   Boundary F1-Score (±0.5s tolerance): {100 * np.mean(full_limit_f1_0_5):.2f}%")
    print(f"   Boundary F1-Score (±3.0s tolerance): {100 * np.mean(full_limit_f1_3_0):.2f}%")
    print("=================================================")

    # Print Aligned Decompositions on Dual-Annotator Subset
    if dual_f1_0_5:
        mean_base_05 = 100 * np.mean(dual_f1_0_5)
        mean_base_30 = 100 * np.mean(dual_f1_3_0)
        mean_grid_05 = 100 * np.mean(dual_grid_f1_0_5)
        mean_grid_30 = 100 * np.mean(dual_grid_f1_3_0)
        mean_limit_05 = 100 * np.mean(dual_limit_f1_0_5)
        mean_limit_30 = 100 * np.mean(dual_limit_f1_3_0)
        mean_human_05 = 100 * np.mean(dual_human_f1_0_5)
        mean_human_30 = 100 * np.mean(dual_human_f1_3_0)

        print("\n=== ALIGNED DECOMPOSITIONS (Dual-Annotator Subset) ===")
        print(f"Tracks Evaluated: {len(dual_f1_0_5)}")
        print("-" * 60)
        print(f"  Tolerance:                           ±0.5s       ±3.0s")
        print(f"  Baseline Model F1:                  {mean_base_05:6.2f}%     {mean_base_30:6.2f}%")
        print(f"  16-bin GRID Ceiling (Limit):        {mean_limit_05:6.2f}%     {mean_limit_30:6.2f}%")
        print(f"  Human Consensus Ceiling:            {mean_human_05:6.2f}%     {mean_human_30:6.2f}%")
        print("-" * 60)
        
        # Decompositions
        # Model / GRID ceiling
        ratio_grid_05 = (mean_base_05 / mean_limit_05 * 100) if mean_limit_05 > 0 else 0.0
        ratio_grid_30 = (mean_base_30 / mean_limit_30 * 100) if mean_limit_30 > 0 else 0.0
        print(f"  Baseline / GRID ceiling:            {ratio_grid_05:5.1f}%     {ratio_grid_30:5.1f}%")
        
        # GRID / HUMAN ceiling
        ratio_human_grid_05 = (mean_limit_05 / mean_human_05 * 100) if mean_human_05 > 0 else 0.0
        ratio_human_grid_30 = (mean_limit_30 / mean_human_30 * 100) if mean_human_30 > 0 else 0.0
        print(f"  GRID / HUMAN ceiling:               {ratio_human_grid_05:5.1f}%     {ratio_human_grid_30:5.1f}%  (pure quantization debt)")
        
        # Model / HUMAN ceiling
        ratio_human_05 = (mean_base_05 / mean_human_05 * 100) if mean_human_05 > 0 else 0.0
        ratio_human_30 = (mean_base_30 / mean_human_30 * 100) if mean_human_30 > 0 else 0.0
        print(f"  Baseline / HUMAN ceiling:           {ratio_human_05:5.1f}%     {ratio_human_30:5.1f}%")
        print("=================================================")

if __name__ == "__main__":
    main()
