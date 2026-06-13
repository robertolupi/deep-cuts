#!/usr/bin/env python3
import json
import os
import sqlite3
import random
from pathlib import Path

DB_PATH = os.path.expanduser("~/Library/Application Support/com.rlupi.deep-cuts/deep_cuts.db")
REPO_ROOT = Path(__file__).resolve().parent.parent
SESSION_DIR = REPO_ROOT / "doc/collab/sessions/2026-06-07-salami-eval-design"

def main():
    if not os.path.exists(DB_PATH):
        print(f"Error: Database not found at {DB_PATH}")
        return

    # 1. Query SALAMI tracks from database
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    
    # We look for tracks containing 'Salami/audio' in their path
    cursor.execute("SELECT id, path FROM tracks WHERE path LIKE '%Salami/audio/%'")
    rows = cursor.fetchall()
    conn.close()

    if not rows:
        print("No SALAMI tracks found in the database. Ensure the scan is completed and paths match '%Salami/audio/%'")
        return

    print(f"Found {len(rows)} SALAMI tracks in the database.")

    # 2. Extract track info and salami ID
    tracks = []
    for db_id, path in rows:
        filename = os.path.basename(path)
        salami_id_str = os.path.splitext(filename)[0]
        try:
            salami_id = int(salami_id_str)
            tracks.append({
                "db_id": db_id,
                "salami_id": salami_id,
                "path": path
            })
        except ValueError:
            print(f"Skipping track with non-integer filename: {path}")

    # 3. Sort to guarantee ordering before shuffle (ensures deterministic splits)
    tracks.sort(key=lambda t: t["salami_id"])

    # 4. Shuffle with a fixed seed
    random.seed(42)
    random.shuffle(tracks)

    # 5. Split (80% Validation, 20% Holdout)
    split_idx = int(len(tracks) * 0.8)
    val_tracks = tracks[:split_idx]
    holdout_tracks = tracks[split_idx:]

    print(f"Split completed: {len(val_tracks)} validation tracks, {len(holdout_tracks)} holdout tracks.")

    # 6. Save JSON files
    SESSION_DIR.mkdir(parents=True, exist_ok=True)
    val_path = SESSION_DIR / "validation_tracks.json"
    holdout_path = SESSION_DIR / "holdout_tracks.json"

    val_path.write_text(json.dumps(val_tracks, indent=2))
    holdout_path.write_text(json.dumps(holdout_tracks, indent=2))

    print(f"Saved splits to:")
    print(f"  - {val_path}")
    print(f"  - {holdout_path}")

if __name__ == "__main__":
    main()
