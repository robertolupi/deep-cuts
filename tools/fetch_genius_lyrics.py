"""
Fetch lyrics with section labels from Genius for all library tracks.

Output: <audio_path>.lyrics.txt alongside each audio file.
Format matches the existing Downspiral lyrics.txt convention:
  [Intro]
  line one
  line two

  [Verse 1]
  ...

Usage:
  python fetch_genius_lyrics.py [--dry-run] [--limit N] [--force]

Requirements:
  pip install requests beautifulsoup4 lxml thefuzz

Reads GENIUS_ACCESS_TOKEN from the environment (set in src-tauri/.cargo/config.toml).
"""

import argparse
import os
import re
import sqlite3
import time
from pathlib import Path

import requests
from bs4 import BeautifulSoup
from thefuzz import fuzz

DB = Path.home() / "Library/Application Support/com.rlupi.deep-cuts/deep_cuts.db"

def update_db_lyrics(track_id: int, lyrics: str) -> None:
    con = sqlite3.connect(DB)
    con.execute("UPDATE tracks SET lyrics = ? WHERE id = ?", (lyrics, track_id))
    con.commit()
    con.close()
API_BASE = "https://api.genius.com"
RATE_LIMIT_SEC = 1.0   # max 1 req/sec to stay within Genius free tier


def genius_search(query: str, token: str) -> list[dict]:
    """Search Genius and return raw hit list."""
    resp = requests.get(
        f"{API_BASE}/search",
        headers={"Authorization": f"Bearer {token}"},
        params={"q": query},
        timeout=10,
    )
    resp.raise_for_status()
    return resp.json()["response"]["hits"]


def best_hit(hits: list[dict], artist: str, title: str) -> dict | None:
    """Return the hit whose artist+title best matches our metadata, or None."""
    best, best_score = None, 0
    for hit in hits:
        r = hit["result"]
        hit_artist = r.get("primary_artist", {}).get("name", "")
        hit_title  = r.get("title", "")
        score = (
            fuzz.token_set_ratio(artist.lower(), hit_artist.lower()) * 0.5 +
            fuzz.token_set_ratio(title.lower(),  hit_title.lower())  * 0.5
        )
        if score > best_score:
            best_score, best = score, r
    # Require a reasonable match — tune threshold if needed
    return best if best_score >= 60 else None


def fetch_lyrics_page(url: str) -> str | None:
    """Scrape lyrics text (with [Section] markers) from a Genius song page."""
    resp = requests.get(url, timeout=15,
                        headers={"User-Agent": "Mozilla/5.0 (compatible; deep-cuts/1.0)"})
    if resp.status_code != 200:
        return None
    soup = BeautifulSoup(resp.text, "lxml")

    # Genius renders lyrics in data-lyrics-container divs
    containers = soup.find_all("div", {"data-lyrics-container": "true"})
    if not containers:
        return None

    lines = []
    for container in containers:
        # Replace <br> with newlines before extracting text
        for br in container.find_all("br"):
            br.replace_with("\n")
        # Section headers are in <a> or plain text inside square brackets
        text = container.get_text(separator="\n")
        lines.append(text)

    raw = "\n".join(lines)

    # Strip everything before the first [Section] marker
    match = re.search(r"^\[.+\]", raw, re.MULTILINE)
    if match:
        raw = raw[match.start():]

    # Normalise: collapse 3+ blank lines to 2, strip trailing whitespace
    raw = re.sub(r"\n{3,}", "\n\n", raw).strip()
    return raw


def has_section_labels(text: str) -> bool:
    """Return True if the lyrics contain at least one [Section] marker."""
    return bool(re.search(r"^\[.+\]", text, re.MULTILINE))


def lyrics_path(audio_path: str) -> Path:
    return Path(audio_path + ".lyrics.txt")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--dry-run", action="store_true",
                        help="Fetch and match but don't write files")
    parser.add_argument("--limit", type=int, default=0,
                        help="Stop after N tracks (0 = all)")
    parser.add_argument("--force", action="store_true",
                        help="Re-fetch even if lyrics file already exists")
    parser.add_argument("--min-score", type=int, default=60,
                        help="Minimum fuzzy match score (default 60)")
    args = parser.parse_args()

    token = os.environ.get("GENIUS_ACCESS_TOKEN")
    if not token:
        raise SystemExit("GENIUS_ACCESS_TOKEN not set in environment")

    con = sqlite3.connect(DB)
    already = "AND (lyrics IS NULL OR lyrics = '')" if not args.force else ""
    rows = con.execute(f"""
        SELECT id, path, artist, title
        FROM tracks
        WHERE artist IS NOT NULL AND artist != ''
          AND title  IS NOT NULL AND title  != ''
          {already}
        ORDER BY artist, title
    """).fetchall()
    con.close()

    print(f"Tracks with artist+title: {len(rows)}")

    stats = {"skipped_exists": 0, "no_hit": 0, "no_labels": 0, "written": 0, "error": 0}
    processed = 0

    for track_id, path, artist, title in rows:
        if args.limit and processed >= args.limit:
            break

        lpath = lyrics_path(path)

        if lpath.exists() and not args.force:
            stats["skipped_exists"] += 1
            continue

        query = f"{title} {artist}"
        try:
            hits = genius_search(query, token)
            time.sleep(RATE_LIMIT_SEC)
        except Exception as e:
            print(f"  [ERROR] search failed for {artist} – {title}: {e}")
            stats["error"] += 1
            processed += 1
            continue

        hit = best_hit(hits, artist, title)
        if not hit:
            print(f"  [NO HIT]  {artist} – {title}")
            stats["no_hit"] += 1
            processed += 1
            continue

        hit_artist = hit.get("primary_artist", {}).get("name", "")
        hit_title  = hit.get("title", "")
        url        = hit.get("url", "")

        try:
            lyrics = fetch_lyrics_page(url)
            time.sleep(RATE_LIMIT_SEC)
        except Exception as e:
            print(f"  [ERROR] scrape failed for {url}: {e}")
            stats["error"] += 1
            processed += 1
            continue

        if not lyrics:
            print(f"  [EMPTY]   {artist} – {title} → {hit_artist} – {hit_title}")
            stats["no_hit"] += 1
            processed += 1
            continue

        if not has_section_labels(lyrics):
            print(f"  [NO SECT] {artist} – {title} → {hit_artist} – {hit_title}")
            stats["no_labels"] += 1
            processed += 1
            continue

        print(f"  [OK]      {artist} – {title} → {hit_artist} – {hit_title}")
        if not args.dry_run:
            lpath.write_text(lyrics, encoding="utf-8")
            update_db_lyrics(track_id, lyrics)
        stats["written"] += 1
        processed += 1

    print(f"\nDone. {stats}")


if __name__ == "__main__":
    main()
