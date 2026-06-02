#!/usr/bin/env python3
"""
Evaluate Qwen-Audio metadata accuracy using CLAP text/audio embeddings.

Scans the user's ground-truth folder (~/Downloads/MP3 Songs), parses:
  - style.txt (ground truth Suno style prompt)
  - *.dc.json (app analysis output including Qwen description/tags & CLAP audio embedding)

Computes CLAP text embeddings for the ground-truth prompt and Qwen's description,
then calculates similarities to establish optimal validation thresholds.

Run from the repo root with the tools venv active:
  source tools/.venv/bin/activate
  python tools/evaluate_qwen_clap.py
"""

import os
import json
import glob
import numpy as np
import onnxruntime as ort
from tokenizers import Tokenizer

SONGS_DIR = os.path.expanduser("~/Downloads/MP3 Songs")
TOKENIZER_PATH = "models/clap-tokenizer.json"
MODEL_PATH = "models/clap_text_encoder.onnx"
OUTPUT_REPORT_PATH = "doc/qwen_eval_results.json"


def load_tokenizer_and_model():
    if not os.path.exists(TOKENIZER_PATH):
        raise FileNotFoundError(f"Missing {TOKENIZER_PATH}. Run download_models.py first.")
    if not os.path.exists(MODEL_PATH):
        raise FileNotFoundError(f"Missing {MODEL_PATH}. Run download_models.py first.")

    print(f"Loading CLAP tokenizer from {TOKENIZER_PATH}...")
    tokenizer = Tokenizer.from_file(TOKENIZER_PATH)
    tokenizer.enable_truncation(max_length=512)

    print(f"Loading CLAP text encoder from {MODEL_PATH}...")
    # Disable CPU EP logging and set thread counts to 1 to run fast in parallel / background
    sess_opts = ort.SessionOptions()
    sess_opts.intra_op_num_threads = 1
    sess_opts.inter_op_num_threads = 1
    session = ort.InferenceSession(MODEL_PATH, sess_opts)

    return tokenizer, session


def get_text_embedding(text, tokenizer, session):
    encoded = tokenizer.encode(text)
    input_ids = np.array([encoded.ids], dtype=np.int64)
    attention_mask = np.array([encoded.attention_mask], dtype=np.int64)

    outputs = session.run(["text_embedding"], {
        "input_ids": input_ids,
        "attention_mask": attention_mask
    })
    embedding = outputs[0][0]

    # L2 Normalization
    norm = np.linalg.norm(embedding)
    if norm > 1e-8:
        embedding = embedding / norm
    return embedding


def scan_dataset():
    """Scans SONGS_DIR for folders containing style.txt and *.dc.json."""
    print(f"Scanning directories in: {SONGS_DIR}...")
    folders = glob.glob(os.path.join(SONGS_DIR, "*"))
    dataset = []

    for folder in folders:
        if not os.path.isdir(folder):
            continue

        style_file = os.path.join(folder, "style.txt")
        dc_files = glob.glob(os.path.join(folder, "*.dc.json"))

        if os.path.exists(style_file) and dc_files:
            # Load style prompt
            with open(style_file, "r", encoding="utf-8") as f:
                style_prompt = f.read().strip()

            # Load sidecar metadata
            dc_path = dc_files[0]
            with open(dc_path, "r", encoding="utf-8") as f:
                try:
                    metadata = json.load(f)
                except Exception as e:
                    print(f"  ⚠  Failed to parse JSON for {folder}: {e}")
                    continue

            # Ensure CLAP embedding and Qwen descriptions exist
            clap_emb = metadata.get("clap_embedding")
            qwen_desc = metadata.get("description")

            if clap_emb and qwen_desc:
                dataset.append({
                    "track_name": os.path.basename(folder),
                    "style_prompt": style_prompt,
                    "description": qwen_desc,
                    "instruments": metadata.get("ai_instruments", ""),
                    "mood": metadata.get("ai_mood", ""),
                    "genre": metadata.get("ai_genre", ""),
                    "clap_embedding": np.array(clap_emb, dtype=np.float32)
                })

    print(f"Found {len(dataset)} valid matching tracks in dataset.")
    return dataset


def format_instrument_prompt(inst_string):
    if not inst_string or inst_string.strip() == "":
        return ""
    return f"This music features the following instrumentation: {inst_string}."


def format_mood_prompt(mood_string):
    if not mood_string or mood_string.strip() == "":
        return ""
    return f"The mood of this music is {mood_string}."


def main():
    try:
        tokenizer, session = load_tokenizer_and_model()
    except Exception as e:
        print(f"Error initializing models: {e}")
        return

    dataset = scan_dataset()
    if not dataset:
        print("No valid track data found to evaluate. Exiting.")
        return

    results = []

    print("\nProcessing embeddings and similarities...")
    # Precompute text embeddings for all style prompts (for mismatched negative checks)
    all_style_embeddings = []
    for item in dataset:
        emb = get_text_embedding(item["style_prompt"], tokenizer, session)
        all_style_embeddings.append(emb)

    for idx, item in enumerate(dataset):
        audio_emb = item["clap_embedding"]

        # 1. Similarity with Ground Truth style
        gt_emb = all_style_embeddings[idx]
        sim_gt = float(np.dot(audio_emb, gt_emb))

        # 2. Similarity with Qwen Description
        desc_emb = get_text_embedding(item["description"], tokenizer, session)
        sim_desc = float(np.dot(audio_emb, desc_emb))

        # 3. Similarity with Qwen Keywords (if present)
        sim_inst = None
        if item["instruments"]:
            inst_p = format_instrument_prompt(item["instruments"])
            inst_emb = get_text_embedding(inst_p, tokenizer, session)
            sim_inst = float(np.dot(audio_emb, inst_emb))

        sim_mood = None
        if item["mood"]:
            mood_p = format_mood_prompt(item["mood"])
            mood_emb = get_text_embedding(mood_p, tokenizer, session)
            sim_mood = float(np.dot(audio_emb, mood_emb))

        # 4. Mismatched baseline similarity (compare with all OTHER style prompts to get a negative baseline)
        neg_similarities = []
        for j, other_style_emb in enumerate(all_style_embeddings):
            if idx != j:
                neg_similarities.append(float(np.dot(audio_emb, other_style_emb)))

        avg_mismatch = float(np.mean(neg_similarities))
        max_mismatch = float(np.max(neg_similarities))

        results.append({
            "track_name": item["track_name"],
            "ground_truth_style": item["style_prompt"],
            "qwen_description": item["description"],
            "sim_ground_truth": sim_gt,
            "sim_qwen_description": sim_desc,
            "sim_instruments": sim_inst,
            "sim_mood": sim_mood,
            "avg_mismatch": avg_mismatch,
            "max_mismatch": max_mismatch,
            "all_mismatches": neg_similarities
        })

        if (idx + 1) % 20 == 0 or (idx + 1) == len(dataset):
            print(f"  Processed {idx + 1}/{len(dataset)} tracks...")

    # Calculate global metrics
    sim_gt_vals = [r["sim_ground_truth"] for r in results]
    sim_desc_vals = [r["sim_qwen_description"] for r in results]
    avg_mismatch_vals = [r["avg_mismatch"] for r in results]
    max_mismatch_vals = [r["max_mismatch"] for r in results]

    # Mismatch distribution (all negative pairs pooled)
    all_negatives = []
    for r in results:
        all_negatives.extend(r["all_mismatches"])

    # Sweep thresholds to find the classification boundary
    best_t = 0.0
    best_acc = 0.0
    thresholds = np.linspace(0.0, 0.4, 41)
    
    # We want to separate Ground Truth similarity (positives) from all_negatives (negatives)
    pos = np.array(sim_gt_vals)
    neg = np.array(all_negatives)
    
    for t in thresholds:
        tp = np.sum(pos >= t)
        fp = np.sum(neg >= t)
        fn = np.sum(pos < t)
        tn = np.sum(neg < t)
        accuracy = (tp + tn) / (len(pos) + len(neg))
        if accuracy > best_acc:
            best_acc = accuracy
            best_t = t

    report = {
        "metrics": {
            "track_count": len(dataset),
            "sim_ground_truth": {
                "mean": float(np.mean(sim_gt_vals)),
                "std": float(np.std(sim_gt_vals)),
                "min": float(np.min(sim_gt_vals)),
                "max": float(np.max(sim_gt_vals)),
                "p5": float(np.percentile(sim_gt_vals, 5)),
                "p10": float(np.percentile(sim_gt_vals, 10)),
                "p25": float(np.percentile(sim_gt_vals, 25)),
                "median": float(np.percentile(sim_gt_vals, 50)),
            },
            "sim_qwen_description": {
                "mean": float(np.mean(sim_desc_vals)),
                "std": float(np.std(sim_desc_vals)),
                "min": float(np.min(sim_desc_vals)),
                "max": float(np.max(sim_desc_vals)),
                "p5": float(np.percentile(sim_desc_vals, 5)),
                "p10": float(np.percentile(sim_desc_vals, 10)),
                "p25": float(np.percentile(sim_desc_vals, 25)),
                "median": float(np.percentile(sim_desc_vals, 50)),
            },
            "negative_mismatches": {
                "mean": float(np.mean(all_negatives)),
                "std": float(np.std(all_negatives)),
                "max": float(np.max(all_negatives)),
                "p90": float(np.percentile(all_negatives, 90)),
                "p95": float(np.percentile(all_negatives, 95)),
            },
            "classification_calibration": {
                "optimal_threshold": float(best_t),
                "max_accuracy": float(best_acc)
            }
        },
        "tracks": [
            {
                "track_name": r["track_name"],
                "sim_ground_truth": r["sim_ground_truth"],
                "sim_qwen_description": r["sim_qwen_description"],
                "sim_instruments": r["sim_instruments"],
                "sim_mood": r["sim_mood"]
            }
            for r in results
        ]
    }

    # Save report
    os.makedirs(os.path.dirname(OUTPUT_REPORT_PATH), exist_ok=True)
    with open(OUTPUT_REPORT_PATH, "w", encoding="utf-8") as f:
        json.dump(report, f, indent=2)

    # Print summary
    print("\n" + "=" * 60)
    print("  QWEN & CLAP SYNERGY EVALUATION REPORT")
    print("=" * 60)
    print(f"Total evaluated tracks: {len(dataset)}")
    print("\n1. Ground-Truth Prompt vs. Audio Similarity:")
    print(f"   Mean similarity: {report['metrics']['sim_ground_truth']['mean']:.4f} (std={report['metrics']['sim_ground_truth']['std']:.4f})")
    print(f"   5th percentile (95% recall): {report['metrics']['sim_ground_truth']['p5']:.4f}")
    print(f"   10th percentile (90% recall): {report['metrics']['sim_ground_truth']['p10']:.4f}")
    print(f"   Median similarity: {report['metrics']['sim_ground_truth']['median']:.4f}")

    print("\n2. Qwen-Generated Description vs. Audio Similarity:")
    print(f"   Mean similarity: {report['metrics']['sim_qwen_description']['mean']:.4f} (std={report['metrics']['sim_qwen_description']['std']:.4f})")
    print(f"   Median similarity: {report['metrics']['sim_qwen_description']['median']:.4f}")

    print("\n3. Mismatched (Negative Pair) Similarity Baseline:")
    print(f"   Mean similarity: {report['metrics']['negative_mismatches']['mean']:.4f} (std={report['metrics']['negative_mismatches']['std']:.4f})")
    print(f"   95th percentile (5% false positive): {report['metrics']['negative_mismatches']['p95']:.4f}")

    print("\n4. Decision Boundary Calibration:")
    print(f"   Optimal classification threshold: {report['metrics']['classification_calibration']['optimal_threshold']:.2f}")
    print(f"   Max separation accuracy: {report['metrics']['classification_calibration']['max_accuracy'] * 100:.2f}%")
    print("=" * 60)
    print(f"Detailed JSON report saved to: {OUTPUT_REPORT_PATH}\n")


if __name__ == "__main__":
    main()
