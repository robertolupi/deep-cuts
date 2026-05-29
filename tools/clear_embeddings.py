#!/usr/bin/env python3
"""
Clears the audio_embeddings table and resets all clap track_passes to pending.

Run from the repo root:
  python tools/clear_embeddings.py [--db PATH]
"""

import argparse
import sqlite3
import sqlite_vec

DEFAULT_DB = (
    "/Users/rlupi/Library/Application Support/com.rlupi.deep-cuts/deep_cuts.db"
)


def main():
    parser = argparse.ArgumentParser(description="Clear Deep Cuts CLAP embeddings")
    parser.add_argument("--db", default=DEFAULT_DB, help="Path to deep_cuts.db")
    args = parser.parse_args()

    conn = sqlite3.connect(args.db)
    conn.enable_load_extension(True)
    sqlite_vec.load(conn)
    conn.enable_load_extension(False)
    cur = conn.cursor()

    cur.execute("SELECT COUNT(*) FROM audio_embeddings")
    emb_count = cur.fetchone()[0]

    cur.execute("DELETE FROM audio_embeddings")
    cur.execute(
        "UPDATE track_passes SET status = 0, log = NULL, result = NULL, "
        "last_run_at = NULL, duration_ms = NULL WHERE pass_name = 'clap'"
    )
    clap_reset = cur.rowcount

    conn.commit()
    conn.close()

    print(f"Deleted {emb_count} embedding(s).")
    print(f"Reset {clap_reset} clap pass row(s) to pending.")


if __name__ == "__main__":
    main()
