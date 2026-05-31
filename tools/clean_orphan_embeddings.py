#!/usr/bin/env python3
"""Remove orphaned rows from vec0 virtual tables that have no matching track."""

import sqlite3
import sqlite_vec
import sys
from pathlib import Path

DB_PATH = Path.home() / "Library/Application Support/com.rlupi.deep-cuts/deep_cuts.db"


def clean(db_path: Path) -> None:
    conn = sqlite3.connect(db_path)
    conn.enable_load_extension(True)
    sqlite_vec.load(conn)
    conn.enable_load_extension(False)
    try:
        for table in ("audio_embeddings", "description_embeddings"):
            cur = conn.execute(
                f"DELETE FROM {table} WHERE track_id NOT IN (SELECT id FROM tracks)"
            )
            print(f"{table}: deleted {cur.rowcount} orphan row(s)")
        conn.commit()
    finally:
        conn.close()


if __name__ == "__main__":
    path = Path(sys.argv[1]) if len(sys.argv) > 1 else DB_PATH
    if not path.exists():
        print(f"Database not found: {path}", file=sys.stderr)
        sys.exit(1)
    clean(path)
    print("Done.")
