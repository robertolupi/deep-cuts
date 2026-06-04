#!/usr/bin/env python3
"""
run_clap_tagging.py

Tags every track in the library using CLAP audio embeddings vs all
music-relevant AudioSet class labels.

For each concept, we compute the cosine similarity to every track's audio
embedding. Then we z-score the similarities across all tracks (per concept)
and tag a track if its z-score exceeds a threshold. This means ubiquitous
concepts (e.g. "Drum") only tag tracks that are *particularly* drum-heavy,
not every song.

Results are written to the DB as source='clap', tag name '<namespace>:<label>'
where namespace is one of: inst, vocal, genre, feel — matching the existing tag taxonomy.

Usage:
    python run_clap_tagging.py [--zscore 1.5] [--dry-run]
"""

import argparse
import json
import sqlite3
from pathlib import Path

import numpy as np

MODELS_DIR = Path("/Volumes/Extreme Pro/deep-cuts-models")
DB_PATH    = Path.home() / "Library" / "Application Support" / "com.rlupi.deep-cuts" / "deep_cuts.db"
LABELS_PATH = Path.home() / "src/gh/CLAP/class_labels/audioset_class_labels_indices.json"

TEMPLATES = [
    "a song featuring {concept}",
    "music with {concept}",
    "{concept}",
]

# Maps AudioSet label → (namespace, tag_label) in the existing taxonomy.
# Unmapped concepts fall back to namespace 'clap' with a lowercased label.
CONCEPT_MAP: dict[str, tuple[str, str]] = {
    # ── Instruments ─────────────────────────────────────────────────────────
    "Acoustic guitar":                    ("inst", "acoustic guitar"),
    "Electric guitar":                    ("inst", "electric guitar"),
    "Bass guitar":                        ("inst", "bass guitar"),
    "Double bass":                        ("inst", "double bass"),
    "Steel guitar, slide guitar":         ("inst", "slide guitar"),
    "Plucked string instrument":          ("inst", "plucked string"),
    "Bowed string instrument":            ("inst", "bowed string"),
    "String section":                     ("inst", "strings"),
    "Violin, fiddle":                     ("inst", "violin"),
    "Cello":                              ("inst", "cello"),
    "Piano":                              ("inst", "piano"),
    "Electric piano":                     ("inst", "electric piano"),
    "Keyboard (musical)":                 ("inst", "keyboard"),
    "Hammond organ":                      ("inst", "hammond organ"),
    "Electronic organ":                   ("inst", "electronic organ"),
    "Organ":                              ("inst", "organ"),
    "Synthesizer":                        ("inst", "synthesizer"),
    "Drum kit":                           ("inst", "drums"),
    "Drum machine":                       ("inst", "drum machine"),
    "Bass drum":                          ("inst", "bass drum"),
    "Snare drum":                         ("inst", "snare"),
    "Hi-hat":                             ("inst", "hi-hat"),
    "Cymbal":                             ("inst", "cymbal"),
    "Percussion":                         ("inst", "percussion"),
    "Mallet percussion":                  ("inst", "mallet percussion"),
    "Vibraphone":                         ("inst", "vibraphone"),
    "Trumpet":                            ("inst", "trumpet"),
    "Brass instrument":                   ("inst", "brass"),
    "Wind instrument, woodwind instrument": ("inst", "woodwind"),
    "Flute":                              ("inst", "flute"),
    "Saxophone":                          ("inst", "saxophone"),
    "Harmonica":                          ("inst", "harmonica"),
    "Harpsichord":                        ("inst", "harpsichord"),
    "Tapping (guitar technique)":         ("inst", "guitar tapping"),
    "Singing bowl":                       ("inst", "singing bowl"),
    # ── Vocals ──────────────────────────────────────────────────────────────
    "Male singing":                       ("vocal", "male"),
    "Female singing":                     ("vocal", "female"),
    "Child singing":                      ("vocal", "child"),
    "Choir":                              ("vocal", "choir"),
    "Singing":                            ("vocal", "singing"),
    "Vocal music":                        ("vocal", "vocals"),
    "Synthetic singing":                  ("vocal", "synthetic"),
    "Beatboxing":                         ("vocal", "beatbox"),
    "Opera":                              ("vocal", "opera"),
    "Chant":                              ("vocal", "chant"),
    "Humming":                            ("vocal", "humming"),
    "Whistling":                          ("vocal", "whistling"),
    "Yodeling":                           ("vocal", "yodeling"),
    "Rapping":                            ("vocal", "rap"),
    # ── Feel ────────────────────────────────────────────────────────────────
    "Angry music":                        ("feel", "angry"),
    "Happy music":                        ("feel", "happy"),
    "Sad music":                          ("feel", "sad"),
    "Scary music":                        ("feel", "scary"),
    "Tender music":                       ("feel", "tender"),
    "Exciting music":                     ("feel", "exciting"),
    "Funny music":                        ("feel", "funny"),
}

# Labels caught by the keyword filter that are clearly not music concepts
EXCLUDE = {
    "Bird vocalization, bird call, bird song",
    "Bow-wow",
    "Burst, pop",
    "Car passing by",
    "Computer keyboard",
    "Fly, housefly",
    "Heart sounds, heartbeat",
    "Power windows, electric windows",
    "Reversing beeps",
    "Sailboat, sailing ship",
    "Ship",
    "Single-lens reflex camera",
    "Whale vocalization",
    "Whip",
    "Wind",
    "Wind chime",
    "Wind noise (microphone)",
    "Scrape",
    "Jingle bell",
    "Jingle, tinkle",
    "Rattle (instrument)",
    "Electronic tuner",
    "Distortion",        # production effect, not a music concept
    "Reverberation",     # same
    "Harmonic",          # too abstract
    "Music",             # too generic
    "Musical instrument",# too generic
    "Song",              # too generic
    "Background music",  # meta-label
    "Music for children",# too niche
    "Wedding music",     # too niche
    "Christmas music",   # too niche
    "Theme music",       # too vague
    "Jingle (music)",    # too vague
    "Independent music", # meta-label
}

# ── CLAP inference ────────────────────────────────────────────────────────────

def load_clap_text_encoder():
    import onnxruntime as ort
    from tokenizers import Tokenizer

    tokenizer = Tokenizer.from_file(str(MODELS_DIR / "clap-tokenizer.json"))
    tokenizer.enable_truncation(max_length=512)

    sess = ort.InferenceSession(
        str(MODELS_DIR / "clap_text_encoder.onnx"),
        providers=["CPUExecutionProvider"],
    )

    def embed(text: str) -> np.ndarray:
        enc  = tokenizer.encode(text)
        ids  = np.array([enc.ids],            dtype=np.int64)
        mask = np.array([enc.attention_mask],  dtype=np.int64)
        out  = sess.run(["text_embedding"], {"input_ids": ids, "attention_mask": mask})
        vec  = out[0][0].astype(np.float32)
        norm = np.linalg.norm(vec)
        return vec / norm if norm > 0 else vec

    return embed


def embed_concept(embed_fn, concept: str) -> np.ndarray:
    vecs = [embed_fn(t.format(concept=concept)) for t in TEMPLATES]
    avg  = np.mean(vecs, axis=0).astype(np.float32)
    norm = np.linalg.norm(avg)
    return avg / norm if norm > 0 else avg


# ── DB helpers ────────────────────────────────────────────────────────────────

def open_db() -> sqlite3.Connection:
    import sqlite_vec
    conn = sqlite3.connect(str(DB_PATH))
    conn.enable_load_extension(True)
    sqlite_vec.load(conn)
    conn.enable_load_extension(False)
    return conn


def load_audio_embeddings(conn) -> dict[int, np.ndarray]:
    rows = conn.execute("SELECT track_id, embedding FROM audio_embeddings").fetchall()
    result = {}
    for track_id, blob in rows:
        if len(blob) == 512 * 4:
            vec  = np.frombuffer(blob, dtype=np.float32).copy()
            norm = np.linalg.norm(vec)
            result[track_id] = vec / norm if norm > 0 else vec
    return result


def upsert_tag(conn, track_id: int, tag_name: str):
    normalized = tag_name.lower().strip()
    conn.execute(
        "INSERT OR IGNORE INTO tags (name, normalized_name) VALUES (?, ?)",
        (tag_name, normalized),
    )
    tag_id = conn.execute("SELECT id FROM tags WHERE name = ?", (tag_name,)).fetchone()[0]
    conn.execute(
        """INSERT OR REPLACE INTO track_tags (track_id, tag_id, source)
           VALUES (?, ?, 'clap')""",
        (track_id, tag_id),
    )


# ── Main ──────────────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--zscore", type=float, default=1.5,
                        help="Z-score threshold above which a track gets tagged (default: 1.5)")
    parser.add_argument("--max-tags", type=int, default=15,
                        help="Max tags per track — keeps top-N by z-score (default: 15, covers p90)")
    parser.add_argument("--dry-run", action="store_true",
                        help="Print tags without writing to DB")
    args = parser.parse_args()

    # Load AudioSet music labels
    all_labels = json.load(open(LABELS_PATH))
    music_kw = [
        'music', 'song', 'sing', 'vocal', 'voice', 'choir', 'beat', 'rhythm',
        'jazz', 'rock', 'pop', 'electronic', 'hip', 'reggae', 'blues', 'classical',
        'ambient', 'techno', 'house', 'metal', 'punk', 'folk', 'soul', 'funk',
        'trumpet', 'violin', 'cello', 'flute', 'saxophone', 'organ', 'keyboard',
        'piano', 'guitar', 'drum', 'bass', 'synth', 'percussion', 'cymbal',
        'snare', 'hi-hat', 'opera', 'chant', 'rap', 'disco', 'trance', 'salsa',
        'afro', 'latin', 'carnatic', 'bollywood', 'flamenco', 'swing', 'gospel',
        'christian', 'soundtrack', 'dance', 'instrument', 'string', 'brass',
        'wind', 'woodwind', 'mallet', 'pluck', 'bow', 'beatbox', 'falsetto',
        'humming', 'whistling', 'yodel', 'harmony', 'lullaby', 'vibraphone',
    ]
    concepts = [
        label for label in all_labels
        if any(kw in label.lower() for kw in music_kw)
        and label not in EXCLUDE
    ]
    print(f"Using {len(concepts)} music concepts from AudioSet\n")

    print("Loading CLAP text encoder…")
    embed_fn = load_clap_text_encoder()

    print("Loading audio embeddings…")
    conn = open_db()
    audio_embs = load_audio_embeddings(conn)
    track_ids  = list(audio_embs.keys())
    audio_mat  = np.array([audio_embs[tid] for tid in track_ids])  # (N, 512)
    print(f"  {len(track_ids)} tracks\n")

    print("Embedding concepts and scoring…")
    # sim_matrix[i, j] = similarity of track i to concept j
    sim_matrix = np.zeros((len(track_ids), len(concepts)), dtype=np.float32)
    for j, concept in enumerate(concepts):
        cvec = embed_concept(embed_fn, concept)
        sim_matrix[:, j] = audio_mat @ cvec

    # Z-score per concept (across all tracks)
    means = sim_matrix.mean(axis=0)
    stds  = sim_matrix.std(axis=0) + 1e-9
    z_matrix = (sim_matrix - means) / stds

    print(f"Applying z-score threshold of {args.zscore}…\n")

    # Collect tags per track: above z-score threshold, capped at max_tags by z-score rank
    tags_per_track: dict[int, list[str]] = {tid: [] for tid in track_ids}
    total_tags = 0
    for i, tid in enumerate(track_ids):
        # Indices of concepts above threshold, sorted by z-score descending
        above = [(j, float(z_matrix[i, j])) for j in range(len(concepts)) if z_matrix[i, j] >= args.zscore]
        above.sort(key=lambda x: -x[1])
        for j, _ in above[:args.max_tags]:
            concept = concepts[j]
            if concept in CONCEPT_MAP:
                ns, label = CONCEPT_MAP[concept]
            else:
                ns, label = "clap", concept.lower()
            tag = f"{ns}:{label}"
            tags_per_track[tid].append(tag)
            total_tags += 1

    # Stats
    tag_counts = [len(v) for v in tags_per_track.values()]
    print(f"  Total tags to write : {total_tags}")
    print(f"  Tags per track      : min={min(tag_counts)} mean={sum(tag_counts)/len(tag_counts):.1f} max={max(tag_counts)}")
    print(f"  Tracks with 0 tags  : {sum(1 for c in tag_counts if c == 0)}")

    # Most common concepts
    from collections import Counter
    concept_freq = Counter()
    for tags in tags_per_track.values():
        for t in tags:
            concept_freq[t] += 1
    print(f"\n  Top 20 most-tagged concepts:")
    for tag, count in concept_freq.most_common(20):
        print(f"    {tag:<45} {count:>4} tracks ({100*count/len(track_ids):.0f}%)")

    if args.dry_run:
        print("\n[dry-run] No changes written to DB.")
        return

    # Write to DB
    print("\nWriting tags to DB…")
    conn.execute("DELETE FROM track_tags WHERE source = 'clap'")
    for tid, tags in tags_per_track.items():
        for tag in tags:
            upsert_tag(conn, tid, tag)
    conn.commit()
    conn.close()
    print(f"Done. {total_tags} tags written.")


if __name__ == "__main__":
    main()
