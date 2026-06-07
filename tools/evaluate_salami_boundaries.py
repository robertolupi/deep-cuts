#!/usr/bin/env python3
import os
import json
import sqlite3
import csv
from pathlib import Path

DB_PATH = os.path.expanduser("~/Library/Application Support/com.rlupi.deep-cuts/deep_cuts.db")
VAL_TRACKS_PATH = Path("/Users/rlupi/src/deep-cuts/doc/collab/sessions/2026-06-07-salami-eval-design/validation_tracks.json")
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

def evaluate_boundaries(pred_bounds, gt_bounds, tolerance):
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

    print(f"Loaded {len(val_tracks)} validation tracks.")
    onset_map = load_onset_map()

    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()

    total_tracks_evaluated = 0
    # Original baseline metrics (Pred vs. Continuous GT)
    total_f1_0_5 = 0.0
    total_f1_3_0 = 0.0
    total_pairwise_f1 = 0.0

    # Low-res projection metrics
    # 1. How well Pred matches the 16-bin Projected GT (Evaluating classification on grid)
    total_f1_grid_0_5 = 0.0
    total_f1_grid_3_0 = 0.0
    
    # 2. How well 16-bin Projected GT matches Continuous GT (Quantization error upper limit)
    total_f1_limit_0_5 = 0.0
    total_f1_limit_3_0 = 0.0

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
        track_f1_0_5 = 0.0
        track_f1_3_0 = 0.0
        track_pw_f1 = 0.0
        
        track_f1_grid_0_5 = 0.0
        track_f1_grid_3_0 = 0.0
        track_f1_limit_0_5 = 0.0
        track_f1_limit_3_0 = 0.0
        
        for pass_info in gt_passes:
            # Boundary F1 at 0.5s and 3.0s
            _, _, f1_0_5 = evaluate_boundaries(pred_boundaries, pass_info["boundaries"], 0.5)
            _, _, f1_3_0 = evaluate_boundaries(pred_boundaries, pass_info["boundaries"], 3.0)
            
            # Pairwise Clustering F1
            _, _, pw_f1 = evaluate_pairwise_clustering(pred_labels, pass_info["segments"], duration)

            # Low-res projection
            proj_labels, proj_boundaries = project_jams_to_16_bins(pass_info["segments"], duration)
            
            # Pred vs. Projected GT
            _, _, f1_g_0_5 = evaluate_boundaries(pred_boundaries, proj_boundaries, 0.5)
            _, _, f1_g_3_0 = evaluate_boundaries(pred_boundaries, proj_boundaries, 3.0)
            
            # Projected GT vs. Continuous GT (quantization upper limit)
            _, _, f1_lim_0_5 = evaluate_boundaries(proj_boundaries, pass_info["boundaries"], 0.5)
            _, _, f1_lim_3_0 = evaluate_boundaries(proj_boundaries, pass_info["boundaries"], 3.0)

            track_f1_0_5 += f1_0_5
            track_f1_3_0 += f1_3_0
            track_pw_f1 += pw_f1
            
            track_f1_grid_0_5 += f1_g_0_5
            track_f1_grid_3_0 += f1_g_3_0
            track_f1_limit_0_5 += f1_lim_0_5
            track_f1_limit_3_0 += f1_lim_3_0

        n_passes = len(gt_passes)
        total_f1_0_5 += (track_f1_0_5 / n_passes)
        total_f1_3_0 += (track_f1_3_0 / n_passes)
        total_pairwise_f1 += (track_pw_f1 / n_passes)
        
        total_f1_grid_0_5 += (track_f1_grid_0_5 / n_passes)
        total_f1_grid_3_0 += (track_f1_grid_3_0 / n_passes)
        total_f1_limit_0_5 += (track_f1_limit_0_5 / n_passes)
        total_f1_limit_3_0 += (track_f1_limit_3_0 / n_passes)
        
        total_tracks_evaluated += 1

    conn.close()

    if total_tracks_evaluated == 0:
        print("No tracks were evaluated. Check that the analysis passes have finished and JAMS files exist.")
        return

    avg_f1_0_5 = (total_f1_0_5 / total_tracks_evaluated) * 100
    avg_f1_3_0 = (total_f1_3_0 / total_tracks_evaluated) * 100
    avg_pw_f1 = (total_pairwise_f1 / total_tracks_evaluated) * 100
    
    avg_grid_f1_0_5 = (total_f1_grid_0_5 / total_tracks_evaluated) * 100
    avg_grid_f1_3_0 = (total_f1_grid_3_0 / total_tracks_evaluated) * 100
    avg_limit_f1_0_5 = (total_f1_limit_0_5 / total_tracks_evaluated) * 100
    avg_limit_f1_3_0 = (total_f1_limit_3_0 / total_tracks_evaluated) * 100

    print("\n=== Baseline SAX Alignment Evaluation Results ===")
    print(f"Tracks Evaluated: {total_tracks_evaluated} / {len(val_tracks)}")
    print("-" * 50)
    print("1. SAX Prediction vs. Original Continuous JAMS GT:")
    print(f"   Boundary F1-Score (±0.5s tolerance): {avg_f1_0_5:.2f}%")
    print(f"   Boundary F1-Score (±3.0s tolerance): {avg_f1_3_0:.2f}%")
    print(f"   Pairwise Clustering F1-Score:       {avg_pw_f1:.2f}%")
    print("-" * 50)
    print("2. SAX Prediction vs. 16-Bin Projected JAMS GT (Classification on Grid):")
    print(f"   Boundary F1-Score (±0.5s tolerance): {avg_grid_f1_0_5:.2f}%")
    print(f"   Boundary F1-Score (±3.0s tolerance): {avg_grid_f1_3_0:.2f}%")
    print("-" * 50)
    print("3. Theoretical Upper Limit of 16-Bin Grid (Projected JAMS vs. Continuous JAMS):")
    print(f"   Boundary F1-Score (±0.5s tolerance): {avg_limit_f1_0_5:.2f}%")
    print(f"   Boundary F1-Score (±3.0s tolerance): {avg_limit_f1_3_0:.2f}%")
    print("=================================================")

if __name__ == "__main__":
    main()
