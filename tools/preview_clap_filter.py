#!/usr/bin/env python3
"""
preview_clap_filter.py

Previews option-3 filtering for CLAP tags: per-namespace score cutoff + hard cap.
Updates the `discard` column in track_tags so results are visible in the app
without changing any Rust code.

Only touches rows with source='clap'. Rows that fail either the score cutoff
or the per-namespace cap are marked discard=1; passing rows are set to discard=0.

Usage:
    python preview_clap_filter.py [--dry-run] [--show N]

Options:
    --dry-run   Print stats but do not write to the database.
    --show N    Print N sample tracks after applying filters (default: 10).
"""

import argparse
import sqlite3
from collections import defaultdict
from pathlib import Path

DB_PATH = Path.home() / "Library" / "Application Support" / "com.rlupi.deep-cuts" / "deep_cuts.db"

# Per-namespace: (max_tags, score_cutoff)
# score is L2 distance — lower means closer/better match.
NAMESPACE_RULES: dict[str, tuple[int, float]] = {
    "vocal": (3,  1.08),
    "inst":  (5,  1.10),
}
DEFAULT_RULE = (3, 1.10)  # fallback for unknown namespaces


def namespace_of(tag_name: str) -> str:
    return tag_name.split(":")[0] if ":" in tag_name else ""


def apply_filter(rows: list[tuple]) -> tuple[list[tuple], list[tuple]]:
    """
    rows: list of (track_tag_rowid, track_id, tag_name, score)
    Returns (keep, discard) lists of the same tuples.
    """
    # Group by (track_id, namespace), sorted by score asc within each group
    by_track_ns: dict[tuple, list[tuple]] = defaultdict(list)
    for row in rows:
        _, track_id, tag_name, _ = row
        ns = namespace_of(tag_name)
        by_track_ns[(track_id, ns)].append(row)

    keep, discard = [], []
    for (track_id, ns), group in by_track_ns.items():
        max_tags, cutoff = NAMESPACE_RULES.get(ns, DEFAULT_RULE)
        group.sort(key=lambda r: r[3])  # sort by score asc (best first)
        kept = 0
        for row in group:
            _, _, _, score = row
            if kept < max_tags and score <= cutoff:
                keep.append(row)
                kept += 1
            else:
                discard.append(row)

    return keep, discard


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--show", type=int, default=10)
    args = parser.parse_args()

    conn = sqlite3.connect(DB_PATH)
    conn.row_factory = sqlite3.Row

    # Load all non-discarded clap tags with scores
    rows = conn.execute("""
        SELECT tt.rowid, tt.track_id, t.name, tt.score
        FROM track_tags tt
        JOIN tags t ON t.id = tt.tag_id
        WHERE tt.source = 'clap' AND tt.score IS NOT NULL
        ORDER BY tt.track_id, tt.score
    """).fetchall()

    rows = [(r["rowid"], r["track_id"], r["name"], r["score"]) for r in rows]
    print(f"Total clap tags with score: {len(rows)}")

    keep, discard = apply_filter(rows)
    print(f"After filter — keep: {len(keep)}, discard: {len(discard)}")

    # Stats per namespace
    ns_keep = defaultdict(int)
    ns_discard = defaultdict(int)
    for _, _, name, _ in keep:
        ns_keep[namespace_of(name)] += 1
    for _, _, name, _ in discard:
        ns_discard[namespace_of(name)] += 1

    all_ns = sorted(set(ns_keep) | set(ns_discard))
    print(f"\n{'namespace':<10} {'keep':>6} {'discard':>8} {'cap':>5} {'cutoff':>8}")
    print("-" * 45)
    for ns in all_ns:
        cap, cutoff = NAMESPACE_RULES.get(ns, DEFAULT_RULE)
        print(f"{ns:<10} {ns_keep[ns]:>6} {ns_discard[ns]:>8} {cap:>5} {cutoff:>8.3f}")

    # Sample tracks
    if args.show > 0:
        track_keep = defaultdict(list)
        track_discard = defaultdict(list)
        for _, tid, name, score in keep:
            track_keep[tid].append((name, score))
        for _, tid, name, score in discard:
            track_discard[tid].append((name, score))

        sample_ids = list(track_keep.keys())[:args.show]
        titles = {
            r["id"]: (r["title"] or r["filename"], r["artist"])
            for r in conn.execute(
                f"SELECT id, title, filename, artist FROM tracks WHERE id IN ({','.join('?'*len(sample_ids))})",
                sample_ids,
            ).fetchall()
        }

        print(f"\n── Sample tracks ({'dry run' if args.dry_run else 'preview'}) ──")
        for tid in sample_ids:
            title, artist = titles.get(tid, ("?", "?"))
            print(f"\n  {title} — {artist}")
            for name, score in sorted(track_keep[tid], key=lambda x: x[1]):
                print(f"    ✓  {name:<35} {score:.4f}")
            for name, score in sorted(track_discard[tid], key=lambda x: x[1]):
                print(f"    ✗  {name:<35} {score:.4f}")

    if args.dry_run:
        print("\n[dry-run] No changes written.")
        conn.close()
        return

    # Write discard flags
    keep_rowids = [r[0] for r in keep]
    discard_rowids = [r[0] for r in discard]

    cur = conn.cursor()
    # Reset all clap tags to discard=0, then mark the discard set
    cur.execute("UPDATE track_tags SET discard = 0 WHERE source = 'clap'")
    if discard_rowids:
        cur.executemany(
            "UPDATE track_tags SET discard = 1 WHERE rowid = ?",
            [(rid,) for rid in discard_rowids],
        )
    conn.commit()
    conn.close()
    print(f"\nWrote discard flags: {len(keep_rowids)} kept, {len(discard_rowids)} discarded.")


if __name__ == "__main__":
    main()
