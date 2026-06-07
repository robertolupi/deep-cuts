#!/usr/bin/env python3
import os
import json
import sqlite3
import csv
import warnings
from pathlib import Path
import numpy as np
import mir_eval

warnings.filterwarnings("ignore", category=UserWarning)

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

# ---------- boundary evaluation algorithms ----------

def evaluate_boundaries_greedy(pred_bounds, gt_bounds, tolerance):
    """Fast internal greedy nearest-match scorer (sanity check only)."""
    if not pred_bounds and not gt_bounds:
        return 1.0, 1.0, 1.0
    if not pred_bounds or not gt_bounds:
        return 0.0, 0.0, 0.0

    tp = 0
    matched_gt = set()
    for pb in pred_bounds:
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

evaluate_boundaries = evaluate_boundaries_greedy


def intervals(bounds, dur):
    """Convert boundary list to interval representation required by mir_eval."""
    b = sorted(set([0.0] + [x for x in bounds if 0 < x < dur] + [float(dur)]))
    return mir_eval.util.boundaries_to_intervals(np.array(b))

def evaluate_boundaries_mir(ref_b, est_b, dur, win):
    """Benchmark-standard boundary evaluator using mir_eval bipartite matching."""
    _, _, f = mir_eval.segment.detection(
        intervals(ref_b, dur), intervals(est_b, dur), window=win, trim=True)
    return f

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

    # Get prediction label per frame (16-bin grid)
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

def sym_boundary_f1(a, b, dur, win):
    """Symmetric human inter-annotator agreement (mean A1->A2 and A2->A1)."""
    f_ab = evaluate_boundaries_mir(a, b, dur, win)
    f_ba = evaluate_boundaries_mir(b, a, dur, win)
    return 0.5 * (f_ab + f_ba)

def main():
    if not VAL_TRACKS_PATH.exists():
        print(f"Error: Validation tracks list not found at {VAL_TRACKS_PATH}. Run prepare_eval_splits.py first.")
        return

    with open(VAL_TRACKS_PATH, "r") as f:
        val_tracks = json.load(f)

    onset_map = load_onset_map()

    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()

    total_tracks_evaluated = 0
    
    # mir_eval aggregates (±3.0s and ±0.5s)
    f1_3_0_base, f1_3_0_ref, f1_3_0_grid, f1_3_0_hum = [], [], [], []
    f1_0_5_base, f1_0_5_ref, f1_0_5_grid, f1_0_5_hum = [], [], [], []
    
    # Greedy aggregates (±3.0s) for sanity checks
    greedy_3_0_base, greedy_3_0_ref, greedy_3_0_hum = [], [], []

    # Per-track normalized metrics
    norm_3_0 = []
    norm_0_5 = []
    n_zero_human_3_0 = 0
    n_zero_human_0_5 = 0

    # Local novelty-augment setup for 'refined' candidate boundary list
    from refine_salami_boundaries import baseline_boundaries, ranked_novelty_peaks, augment_with_peaks

    for track in val_tracks:
        db_id = track["db_id"]
        salami_id = track["salami_id"]
        
        # Load from DB
        cursor.execute("SELECT duration_seconds, waveform_data, sax_alignment_segments FROM tracks WHERE id = ?", (db_id,))
        row = cursor.fetchone()
        if not row or not row[0] or not row[2]:
            continue
        
        duration, waveform_json, sax_segments_str = row
        labels = sax_segments_str.split(",")
        if len(labels) != 16:
            continue

        base_bounds = baseline_boundaries(labels, duration)
        refined_bounds = augment_with_peaks(base_bounds, ranked_novelty_peaks(waveform_json, duration), 8, 5.0)

        # Parse JAMS ground truth
        jams_path = JAMS_DIR / f"SALAMI_{salami_id}.jams"
        onset_offset = onset_map.get(salami_id, 0.0)
        gt_passes = parse_jams_boundaries_and_labels(jams_path, onset_offset, duration)

        if len(gt_passes) != 2: # Limit to dual-annotator subset to keep ceilings aligned
            continue

        # Evaluate passes
        pass_base_3, pass_ref_3, pass_grid_3 = [], [], []
        pass_base_05, pass_ref_05, pass_grid_05 = [], [], []
        
        pass_gr_base_3, pass_gr_ref_3 = [], []

        for p in gt_passes:
            # mir_eval @3.0s
            pass_base_3.append(evaluate_boundaries_mir(p["boundaries"], base_bounds, duration, 3.0))
            pass_ref_3.append(evaluate_boundaries_mir(p["boundaries"], refined_bounds, duration, 3.0))
            
            # mir_eval @0.5s
            pass_base_05.append(evaluate_boundaries_mir(p["boundaries"], base_bounds, duration, 0.5))
            pass_ref_05.append(evaluate_boundaries_mir(p["boundaries"], refined_bounds, duration, 0.5))

            # Greedy @3.0s
            _, _, fg_b = evaluate_boundaries_greedy(base_bounds, p["boundaries"], 3.0)
            _, _, fg_r = evaluate_boundaries_greedy(refined_bounds, p["boundaries"], 3.0)
            pass_gr_base_3.append(fg_b)
            pass_gr_ref_3.append(fg_r)

            # Grid ceiling
            _, proj_bounds = project_jams_to_16_bins(p["segments"], duration)
            pass_grid_3.append(evaluate_boundaries_mir(p["boundaries"], proj_bounds, duration, 3.0))
            pass_grid_05.append(evaluate_boundaries_mir(p["boundaries"], proj_bounds, duration, 0.5))

        # Human agreement ceiling (symmetric)
        h_3 = sym_boundary_f1(gt_passes[0]["boundaries"], gt_passes[1]["boundaries"], duration, 3.0)
        h_05 = sym_boundary_f1(gt_passes[0]["boundaries"], gt_passes[1]["boundaries"], duration, 0.5)

        avg_ref_3 = np.mean(pass_ref_3)
        avg_ref_05 = np.mean(pass_ref_05)

        # Track-level normalization (cap at 1.5)
        if h_3 > 0.05:
            norm_3_0.append(min(avg_ref_3 / h_3, 1.5))
        else:
            n_zero_human_3_0 += 1

        if h_05 > 0.05:
            norm_0_5.append(min(avg_ref_05 / h_05, 1.5))
        else:
            n_zero_human_0_5 += 1

        # Accumulate
        f1_3_0_base.append(np.mean(pass_base_3))
        f1_3_0_ref.append(avg_ref_3)
        f1_3_0_grid.append(np.mean(pass_grid_3))
        f1_3_0_hum.append(h_3)

        f1_0_5_base.append(np.mean(pass_base_05))
        f1_0_5_ref.append(avg_ref_05)
        f1_0_5_grid.append(np.mean(pass_grid_05))
        f1_0_5_hum.append(h_05)

        greedy_3_0_base.append(np.mean(pass_gr_base_3))
        greedy_3_0_ref.append(np.mean(pass_gr_ref_3))
        
        _, _, h_gr_3 = evaluate_boundaries_greedy(gt_passes[0]["boundaries"], gt_passes[1]["boundaries"], 3.0)
        greedy_3_0_hum.append(h_gr_3)

        total_tracks_evaluated += 1

    conn.close()

    if total_tracks_evaluated == 0:
        print("No tracks were evaluated.")
        return

    # Print Headline results under mir_eval (optimal matching)
    print(f"\nBenchmark-Standard mir_eval Boundary Evaluation  (N = {total_tracks_evaluated} dual-annotator tracks)")
    print("=" * 75)
    print(f"{'Metric':<22}{'Baseline':>12}{'Refined':>12}{'Grid Limit':>14}{'Human Ceiling':>15}")
    print("-" * 75)
    print(f"{'Boundary F1 (±3.0s)':<22}{np.mean(f1_3_0_base)*100:>11.2f}%{np.mean(f1_3_0_ref)*100:>11.2f}%{np.mean(f1_3_0_grid)*100:>13.2f}%{np.mean(f1_3_0_hum)*100:>14.2f}%")
    print(f"{'Boundary F1 (±0.5s)':<22}{np.mean(f1_0_5_base)*100:>11.2f}%{np.mean(f1_0_5_ref)*100:>11.2f}%{np.mean(f1_0_5_grid)*100:>13.2f}%{np.mean(f1_0_5_hum)*100:>14.2f}%")
    print("-" * 75)
    
    # Print Decomposition
    ref_3 = np.mean(f1_3_0_ref)
    grid_3 = np.mean(f1_3_0_grid)
    hum_3 = np.mean(f1_3_0_hum)
    ref_05 = np.mean(f1_0_5_ref)
    grid_05 = np.mean(f1_0_5_grid)
    hum_05 = np.mean(f1_0_5_hum)

    print(f"Refined / GRID ceiling  (±3.0s) : {(ref_3 / grid_3)*100:6.1f}%  (detector saturation score)")
    print(f"Refined / GRID ceiling  (±0.5s) : {(ref_05 / grid_05)*100:6.1f}%")
    print(f"GRID / HUMAN ceiling    (±3.0s) : {(grid_3 / hum_3)*100:6.1f}%  (pure quantization loss)")
    print(f"Refined / HUMAN ceiling (±3.0s) : {(ref_3 / hum_3)*100:6.1f}%  (ratio of means)")
    print(f"Per-track normalized mean (±3.0s): {np.mean(norm_3_0)*100:6.1f}%  (mean model/human, cap 1.5, n={len(norm_3_0)})")
    print(f"Per-track normalized mean (±0.5s): {np.mean(norm_0_5)*100:6.1f}%  (mean model/human, cap 1.5, n={len(norm_0_5)})")
    print("=" * 75)

    # Fast internal sanity check (greedy vs mir_eval)
    print("\n[Sanity Check] Greedy matching vs mir_eval (±3s):")
    print(f"  Greedy: baseline={np.mean(greedy_3_0_base)*100:.2f}%, refined={np.mean(greedy_3_0_ref)*100:.2f}%, human={np.mean(greedy_3_0_hum)*100:.2f}%")
    print(f"  mir_eval: baseline={np.mean(f1_3_0_base)*100:.2f}%, refined={np.mean(f1_3_0_ref)*100:.2f}%, human={np.mean(f1_3_0_hum)*100:.2f}%")

if __name__ == "__main__":
    main()
