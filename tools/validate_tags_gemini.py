#!/usr/bin/env python3
"""
validate_tags_gemini.py

For each song in ~/Downloads/MP3 Songs:
  1. Upload the audio to Gemini and ask for a full analysis (genre, mood, instruments, vibe, vocal, context tags).
  2. Show the existing qwen tags from the Deep Cuts DB and ask Gemini to split them into
     CORRECT and INCORRECT lists (JSON-parseable).
  3. Show the Suno style.txt prompt and ask if it correctly describes the song.

All songs are processed in parallel (one thread per song).
Results are written to validate_tags_results.json in the same directory as this script.

Usage:
    GOOGLE_API_KEY=<key> python validate_tags_gemini.py [--limit N] [--output path]
"""

import argparse
import json
import os
import sqlite3
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

try:
    from google import genai
    from google.genai import types
except ImportError:
    sys.exit("google-genai not installed. Run: pip install google-genai")

# ── Config ──────────────────────────────────────────────────────────────────

SONGS_DIR = Path.home() / "Downloads" / "MP3 Songs"
DB_PATH   = Path.home() / "Library" / "Application Support" / "com.rlupi.deep-cuts" / "deep_cuts.db"
MODEL     = "gemini-3.1-flash-lite"

ANALYSIS_PROMPT = """\
Listen carefully to this audio track.

Respond ONLY with the following 7 lines, nothing else. Use English. Be specific and concise.

GENRE: <genre and subgenre, comma-separated>
MOOD: <mood and emotional feel, comma-separated>
INSTRUMENTS: <main instruments, comma-separated>
DESCRIPTION: <2-3 sentences of plain prose describing the track>
VIBE_TAGS: <3 creative tags for atmosphere/style/vibe — NOT genres, moods, or instruments>
VOCAL_TAGS: <voice type (male/female/instrumental/ensemble/choir) and lyrics language (english/spanish/instrumental/etc)>
CONTEXT_TAGS: <2 suitable listening contexts (e.g. study, club, sleep, workout) and 1 estimated release decade (e.g. 1990s, 2000s)>

Example output format (do not copy these values):
GENRE: ambient techno, minimal techno
MOOD: hypnotic, meditative
INSTRUMENTS: synthesizer, drum machine, bass
DESCRIPTION: A sparse late-night electronic piece built around a cycling four-bar synth loop. Filtered hi-hats and a rolling kick push it forward while a distant pad adds warmth. The overall feel is cold and introspective.
VIBE_TAGS: ethereal, raw, nocturnal
VOCAL_TAGS: instrumental
CONTEXT_TAGS: study, late night, 2000s\
"""

TAG_VALIDATION_PROMPT = """\
Here are the AI-generated tags currently stored for this song in the music library.
Each tag is in the format "namespace:label".

{tags}

Based on the audio you just heard, split these tags into two groups.
Respond ONLY with valid JSON in exactly this format, no prose:

{{
  "correct": ["namespace:label", ...],
  "incorrect": ["namespace:label", ...]
}}\
"""

STYLE_VALIDATION_PROMPT = """\
Here is the Suno music generation prompt that was used to create this song:

{style}

Based on the audio you just heard, does this prompt correctly describe the song?
Respond ONLY with valid JSON in exactly this format, no prose:

{{
  "matches": true or false,
  "notes": "one sentence explaining why or why not"
}}\
"""

# ── Database helpers ──────────────────────────────────────────────────────────

def load_track_data(audio_path: Path) -> dict | None:
    """Return qwen tags and debug fields for the track at audio_path, or None if not found."""
    try:
        conn = sqlite3.connect(str(DB_PATH))
        conn.row_factory = sqlite3.Row

        row = conn.execute(
            """SELECT id, ai_genre, ai_mood, ai_instruments, description
               FROM tracks WHERE path = ?""",
            (str(audio_path),)
        ).fetchone()

        if row is None:
            conn.close()
            return None

        track_id = row["id"]
        tag_rows = conn.execute(
            """SELECT tg.name FROM track_tags tt
               JOIN tags tg ON tg.id = tt.tag_id
               WHERE tt.track_id = ? AND tt.source = 'qwen'
               ORDER BY tg.name""",
            (track_id,)
        ).fetchall()

        conn.close()

        return {
            "track_id": track_id,
            "ai_genre": row["ai_genre"],
            "ai_mood": row["ai_mood"],
            "ai_instruments": row["ai_instruments"],
            "description": row["description"],
            "qwen_tags": [r["name"] for r in tag_rows],
        }
    except Exception as e:
        return {"error": str(e), "qwen_tags": []}


# ── Per-song worker ───────────────────────────────────────────────────────────

def validate_song(song_dir: Path, client: genai.Client) -> dict:
    """Run the three-step Gemini validation for one song directory."""
    song_name = song_dir.name

    # Find the audio file (prefer .mp3, fall back to any audio)
    audio_file = None
    for ext in (".mp3", ".m4a", ".wav", ".aif", ".aiff", ".flac", ".ogg"):
        candidate = song_dir / f"{song_name}{ext}"
        if candidate.exists():
            audio_file = candidate
            break
    if audio_file is None:
        candidates = list(song_dir.glob("*.mp3")) + list(song_dir.glob("*.m4a")) + \
                     list(song_dir.glob("*.wav")) + list(song_dir.glob("*.aif")) + \
                     list(song_dir.glob("*.aiff"))
        audio_file = candidates[0] if candidates else None

    if audio_file is None:
        return {"song": song_name, "error": "No audio file found"}

    style_file = song_dir / "style.txt"
    style_text = style_file.read_text(encoding="utf-8").strip() if style_file.exists() else None

    db_data = load_track_data(audio_file)
    qwen_tags = db_data.get("qwen_tags", []) if db_data else []

    result = {
        "song": song_name,
        "audio_path": str(audio_file),
        "db": db_data,
        "gemini_analysis": None,
        "tag_validation": None,
        "style_validation": None,
        "error": None,
    }

    # Upload audio via Files API
    uploaded = None
    try:
        mime = "audio/mpeg" if audio_file.suffix.lower() in (".mp3",) else \
               "audio/wav" if audio_file.suffix.lower() in (".wav",) else \
               "audio/aiff" if audio_file.suffix.lower() in (".aif", ".aiff") else \
               "audio/mp4"
        safe_name = song_name.encode("ascii", errors="replace").decode("ascii")
        uploaded = client.files.upload(
            file=audio_file,
            config=types.UploadFileConfig(mime_type=mime, display_name=safe_name),
        )
        # Wait until the file is ACTIVE
        for _ in range(30):
            if uploaded.state and uploaded.state.name == "ACTIVE":
                break
            time.sleep(2)
            uploaded = client.files.get(name=uploaded.name)
    except Exception as e:
        result["error"] = f"File upload failed: {e}"
        return result

    audio_part = types.Part.from_uri(file_uri=uploaded.uri, mime_type=uploaded.mime_type)

    # ── Step 1: Analysis ─────────────────────────────────────────────────────
    try:
        resp1 = client.models.generate_content(
            model=MODEL,
            contents=[
                types.Content(role="user", parts=[
                    audio_part,
                    types.Part(text=ANALYSIS_PROMPT),
                ])
            ],
        )
        analysis_text = resp1.text.strip()
        result["gemini_analysis"] = analysis_text
    except Exception as e:
        result["error"] = f"Step 1 (analysis) failed: {e}"
        _cleanup(client, uploaded)
        return result

    # ── Step 2: Tag validation ────────────────────────────────────────────────
    if qwen_tags:
        tags_block = "\n".join(qwen_tags)
        try:
            # Continue the same conversation so Gemini still "has" the audio context
            resp2 = client.models.generate_content(
                model=MODEL,
                contents=[
                    types.Content(role="user", parts=[
                        audio_part,
                        types.Part(text=ANALYSIS_PROMPT),
                    ]),
                    types.Content(role="model", parts=[types.Part(text=analysis_text)]),
                    types.Content(role="user", parts=[
                        types.Part(text=TAG_VALIDATION_PROMPT.format(tags=tags_block))
                    ]),
                ],
            )
            raw2 = resp2.text.strip()
            # Strip optional markdown code fence
            json_str = raw2.removeprefix("```json").removeprefix("```").removesuffix("```").strip()
            result["tag_validation"] = json.loads(json_str)
        except json.JSONDecodeError:
            result["tag_validation"] = {"raw": raw2, "parse_error": True}
        except Exception as e:
            result["tag_validation"] = {"error": str(e)}
    else:
        result["tag_validation"] = {"skipped": "no qwen tags in DB"}

    # ── Step 3: Style prompt validation ──────────────────────────────────────
    if style_text:
        try:
            resp3 = client.models.generate_content(
                model=MODEL,
                contents=[
                    types.Content(role="user", parts=[
                        audio_part,
                        types.Part(text=ANALYSIS_PROMPT),
                    ]),
                    types.Content(role="model", parts=[types.Part(text=analysis_text)]),
                    types.Content(role="user", parts=[
                        types.Part(text=STYLE_VALIDATION_PROMPT.format(style=style_text))
                    ]),
                ],
            )
            raw3 = resp3.text.strip()
            json_str = raw3.removeprefix("```json").removeprefix("```").removesuffix("```").strip()
            result["style_validation"] = json.loads(json_str)
            result["style_text"] = style_text
        except json.JSONDecodeError:
            result["style_validation"] = {"raw": raw3, "parse_error": True}
        except Exception as e:
            result["style_validation"] = {"error": str(e)}
    else:
        result["style_validation"] = {"skipped": "no style.txt found"}

    _cleanup(client, uploaded)
    return result


def _cleanup(client: genai.Client, uploaded):
    try:
        if uploaded:
            client.files.delete(name=uploaded.name)
    except Exception:
        pass


# ── Main ──────────────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(description="Validate Deep Cuts qwen tags with Gemini")
    parser.add_argument("--limit", type=int, default=None, help="Process only N songs (for testing)")
    parser.add_argument("--workers", type=int, default=4, help="Parallel workers (default: 4)")
    parser.add_argument("--output", type=str, default=None, help="Output JSON path")
    args = parser.parse_args()

    api_key = os.environ.get("GOOGLE_API_KEY")
    if not api_key:
        sys.exit("GOOGLE_API_KEY environment variable is not set.")

    if not SONGS_DIR.exists():
        sys.exit(f"Songs directory not found: {SONGS_DIR}")

    song_dirs = sorted([d for d in SONGS_DIR.iterdir() if d.is_dir()])
    if args.limit:
        song_dirs = song_dirs[: args.limit]

    print(f"Found {len(song_dirs)} songs. Running with {args.workers} workers on {MODEL}.")

    # One client per thread (the SDK is not thread-safe across clients, but each thread
    # creates its own client instance using the same API key)
    results = []
    completed = 0

    def worker(song_dir: Path) -> dict:
        thread_client = genai.Client(api_key=api_key)
        return validate_song(song_dir, thread_client)

    with ThreadPoolExecutor(max_workers=args.workers) as executor:
        futures = {executor.submit(worker, d): d for d in song_dirs}
        for future in as_completed(futures):
            song_dir = futures[future]
            completed += 1
            try:
                res = future.result()
            except Exception as e:
                res = {"song": song_dir.name, "error": str(e)}
            results.append(res)
            status = "✓" if not res.get("error") else "✗"
            print(f"  [{completed}/{len(song_dirs)}] {status} {res['song']}")

    output_path = Path(args.output) if args.output else Path(__file__).parent / "validate_tags_results.json"
    output_path.write_text(json.dumps(results, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"\nResults written to {output_path}")

    # Quick summary
    errors   = sum(1 for r in results if r.get("error"))
    ok       = len(results) - errors
    tag_skipped = sum(1 for r in results if isinstance(r.get("tag_validation"), dict) and r["tag_validation"].get("skipped"))
    print(f"  {ok} succeeded, {errors} failed, {tag_skipped} had no qwen tags to validate.")


if __name__ == "__main__":
    main()
