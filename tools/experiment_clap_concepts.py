#!/usr/bin/env python3
"""
experiment_clap_concepts.py

Tests whether CLAP audio embeddings can detect concepts from Suno style prompts.

For each concept (instrument, texture, vocal style, etc.):
  - Embed the concept with CLAP text encoder using multiple phrase templates
  - Split tracks into "has concept" (concept appears in style.txt) vs "doesn't have it"
  - Compare similarity distributions to measure separation
  - Suggest a per-concept threshold where separation is good enough to be useful

Usage:
    python experiment_clap_concepts.py [--top N] [--min-positive N]
"""

import argparse
import re
import sqlite3
import struct
from collections import defaultdict
from pathlib import Path

import numpy as np

MODELS_DIR  = Path("/Volumes/Extreme Pro/deep-cuts-models")
DB_PATH     = Path.home() / "Library" / "Application Support" / "com.rlupi.deep-cuts" / "deep_cuts.db"
SONGS_DIR   = Path.home() / "Downloads" / "MP3 Songs"

# ── Phrase templates for CLAP text embedding ────────────────────────────────
# Each concept is embedded as the average of these templates.

TEMPLATES = [
    "a song featuring {concept}",
    "music with {concept}",
    "{concept} playing",
    "you can hear {concept} in this track",
]

# ── Concepts to test ─────────────────────────────────────────────────────────
# Grouped for readability. Add/remove freely.

VOCAL_CONCEPTS = [
    # From AudioSet (CLAP trained on these)
    "Male singing",
    "Female singing",
    "Child singing",
    "Choir",
    "Singing",
    "Vocal music",
    "Synthetic singing",
    "Beatboxing",
    # Vocal presence / absence
    "instrumental music with no vocals",
    "music with vocals",
    "a cappella singing",
    # Voice character
    "falsetto singing",
    "operatic singing",
    "whispering vocals",
    "spoken word over music",
    "rap vocals",
    "screaming vocals",
    "throat singing",
    "vocal harmonies",
    "duet singing",
    "group vocal chant",
    # Language/style hints
    "singing in Italian",
    "singing in Spanish",
    "singing in English",
    "singing in a foreign language",
    # Production
    "heavily processed vocals",
    "auto-tuned vocals",
    "reverb soaked vocals",
    "dry intimate vocals",
    "distorted vocals",
]

AUDIOSET_MUSIC_CONCEPTS = [
    # Instruments
    "Acoustic guitar", "Electric guitar", "Bass guitar", "Double bass", "Steel guitar, slide guitar",
    "Plucked string instrument", "Bowed string instrument", "Violin, fiddle", "Cello",
    "Piano", "Electric piano", "Keyboard (musical)", "Hammond organ", "Electronic organ", "Organ",
    "Synthesizer", "Drum kit", "Drum machine", "Bass drum", "Snare drum", "Hi-hat", "Cymbal",
    "Percussion", "Mallet percussion", "Trumpet", "Flute", "Saxophone", "Harmonica", "Harpsichord",
    "Brass instrument", "Wind instrument, woodwind instrument",
    # Voice
    "Male singing", "Female singing", "Child singing", "Choir", "Singing",
    "Beatboxing", "Synthetic singing", "Vocal music",
    # Genres
    "Rock music", "Rock and roll", "Progressive rock", "Psychedelic rock", "Punk rock",
    "Heavy metal", "Blues", "Jazz", "Swing music", "Soul music", "Funk", "Rhythm and blues",
    "Hip hop music", "Reggae", "Electronic music", "Electronic dance music", "Electronica",
    "House music", "Techno", "Trance music", "Drum and bass", "Dance music",
    "Classical music", "Folk music", "Ambient music", "New-age music", "Gospel music",
    "Christian music", "Carnatic music", "Afrobeat", "Salsa music", "Middle Eastern music",
    "Music of Latin America", "Music of Africa", "Music of Asia", "Music of Bollywood",
    "Pop music", "Independent music", "Soundtrack music", "Video game music",
    # Moods / feel
    "Angry music", "Happy music", "Sad music", "Scary music", "Tender music",
    "Exciting music", "Funny music",
    # Other
    "Background music", "Tapping (guitar technique)",
]

CONCEPTS = [
    # Production & texture
    "lo-fi recording quality",
    "overdriven distortion",
    "heavy compression",
    "clean polished production",
    "wall of sound",
    "sparse arrangement",
    "dense layered production",
    "dry sound with no reverb",
    "heavy reverb",
    "studio polished recording",
    "bedroom recording quality",
    "analog warmth",
    "digital crisp sound",

    # Dynamics & energy
    "builds gradually over time",
    "sudden drop in energy",
    "constant steady energy throughout",
    "explosive climax",
    "fades out slowly",
    "quiet and restrained",
    "very loud and intense",
    "wide dynamic range",
    "tension and release",
    "high energy",
    "low energy calm",

    # Rhythm & groove
    "driving beat",
    "syncopated rhythm",
    "straight rhythm no swing",
    "swing groove",
    "polyrhythmic",
    "no fixed tempo rubato",
    "danceable groove",
    "off-beat emphasis",
    "four on the floor kick",
    "half-time feel",
    "triplet feel",
    "waltz rhythm three four time",
    "fast tempo",
    "slow tempo",

    # Acoustic space
    "intimate close recording",
    "large concert hall reverb",
    "cavernous echo",
    "tight dry room",
    "wide stereo field",
    "mono narrow sound",

    # Complexity & structure
    "simple repetitive loop",
    "complex arrangement",
    "sudden tempo change",
    "key change modulation",
    "long instrumental intro",
    "abrupt ending",
    "call and response",

    # Cultural & regional
    "latin percussion rhythm",
    "african polyrhythm",
    "arabic maqam scale",
    "indian classical feel",
    "reggae offbeat rhythm",
    "flamenco feel",
    "bluegrass twang",
    "gospel feel",
    "bossa nova rhythm",

    # Sonic character
    "noisy and abrasive",
    "smooth and polished",
    "bright high frequencies",
    "dark muddy low end",
    "heavy bass",
    "glitchy stuttering",
    "pitch shifted vocals",
    "vocoder effect",
    "telephone filter effect",
    "pitched down voice",

    # Listener experience
    "trance inducing repetition",
    "jarring and dissonant",
    "soothing and calming",
    "anxious and tense",
    "euphoric",
    "melancholic and sad",
    "cinematic and epic",
    "lullaby feel",
    "party energy",
    "aggressive and angry",
    "peaceful and meditative",
    "nostalgic",
    "mysterious and eerie",
    "romantic",
    "playful and fun",
]

# ── CLAP inference ────────────────────────────────────────────────────────────

def load_clap_text_encoder():
    try:
        import onnxruntime as ort
        from tokenizers import Tokenizer
    except ImportError:
        raise ImportError("pip install onnxruntime tokenizers")

    tokenizer = Tokenizer.from_file(str(MODELS_DIR / "clap-tokenizer.json"))
    tokenizer.enable_truncation(max_length=512)
    tokenizer.enable_padding(pad_id=0, pad_token="[PAD]", length=None)

    model_path = str(MODELS_DIR / "clap_text_encoder.onnx")
    sess = ort.InferenceSession(model_path, providers=["CPUExecutionProvider"])

    def embed(text: str) -> np.ndarray:
        enc = tokenizer.encode(text)
        ids  = np.array([enc.ids],           dtype=np.int64)
        mask = np.array([enc.attention_mask], dtype=np.int64)
        out  = sess.run(["text_embedding"], {"input_ids": ids, "attention_mask": mask})
        vec = out[0][0].astype(np.float32)
        norm = np.linalg.norm(vec)
        return vec / norm if norm > 0 else vec

    return embed


def embed_concept(embed_fn, concept: str) -> np.ndarray:
    """Average embedding across all phrase templates."""
    vecs = [embed_fn(t.format(concept=concept)) for t in TEMPLATES]
    avg = np.mean(vecs, axis=0).astype(np.float32)
    norm = np.linalg.norm(avg)
    return avg / norm if norm > 0 else avg


# ── Load DB audio embeddings ──────────────────────────────────────────────────

def load_audio_embeddings() -> dict[int, np.ndarray]:
    import sqlite_vec
    conn = sqlite3.connect(str(DB_PATH))
    conn.enable_load_extension(True)
    sqlite_vec.load(conn)
    conn.enable_load_extension(False)
    rows = conn.execute(
        "SELECT track_id, embedding FROM audio_embeddings"
    ).fetchall()
    conn.close()

    result = {}
    for track_id, blob in rows:
        if len(blob) == 512 * 4:
            vec = np.frombuffer(blob, dtype=np.float32).copy()
            norm = np.linalg.norm(vec)
            result[track_id] = vec / norm if norm > 0 else vec
    return result


def load_track_paths() -> dict[int, Path]:
    conn = sqlite3.connect(str(DB_PATH))
    rows = conn.execute("SELECT id, path FROM tracks").fetchall()
    conn.close()
    return {tid: Path(p) for tid, p in rows}


# ── Style prompt loading ──────────────────────────────────────────────────────

def load_style_texts(track_paths: dict[int, Path]) -> dict[int, str]:
    """Map track_id → style.txt content for tracks that have one."""
    result = {}
    for track_id, audio_path in track_paths.items():
        style = audio_path.parent / "style.txt"
        if style.exists():
            result[track_id] = style.read_text(encoding="utf-8", errors="ignore").strip()
    return result


def embed_style_texts(embed_fn, style_texts: dict[int, str]) -> dict[int, np.ndarray]:
    """Embed every style.txt with CLAP text encoder."""
    print("  Embedding style prompts…")
    result = {}
    for track_id, text in style_texts.items():
        vec = embed_fn(text)
        result[track_id] = vec
    print(f"  {len(result)} style prompts embedded")
    return result


# ── Concept presence detection via text-text similarity ──────────────────────

def label_positives(concept_vec: np.ndarray,
                    style_vecs: dict[int, np.ndarray],
                    text_threshold: float = 0.30) -> set[int]:
    """
    A track is a 'positive' for a concept if its style.txt CLAP embedding
    is similar enough to the concept embedding (text-text cosine ≥ threshold).
    This avoids vocabulary mismatch from keyword matching.
    """
    return {
        tid for tid, svec in style_vecs.items()
        if float(np.dot(svec, concept_vec)) >= text_threshold
    }


# ── Analysis ──────────────────────────────────────────────────────────────────

def separation_score(pos: np.ndarray, neg: np.ndarray) -> float:
    """
    Fisher-like separation: (mean_pos - mean_neg) / sqrt((std_pos² + std_neg²) / 2)
    Higher = more separable. Rule of thumb: > 1.0 is worth using.
    """
    if len(pos) == 0 or len(neg) == 0:
        return 0.0
    mu_p, mu_n = pos.mean(), neg.mean()
    sp, sn = pos.std() + 1e-9, neg.std() + 1e-9
    return float((mu_p - mu_n) / np.sqrt((sp**2 + sn**2) / 2))


def suggest_threshold(pos: np.ndarray, neg: np.ndarray) -> float | None:
    """
    Midpoint between mean_pos and mean_neg, clamped to the range.
    Returns None if separation is too low to be meaningful.
    """
    if len(pos) == 0 or len(neg) == 0:
        return None
    mu_p, mu_n = pos.mean(), neg.mean()
    if mu_p <= mu_n:
        return None
    return float((mu_p + mu_n) / 2)


# ── Main ──────────────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--top", type=int, default=None,
                        help="Show only top N concepts by separation score")
    parser.add_argument("--min-positive", type=int, default=3,
                        help="Skip concepts with fewer than N positive examples (default: 3)")
    parser.add_argument("--text-threshold", type=float, default=0.45,
                        help="Text-text cosine threshold to label a track as positive (default: 0.45)")
    parser.add_argument("--audioset", action="store_true",
                        help="Use AudioSet music class labels instead of custom CONCEPTS list")
    parser.add_argument("--vocals", action="store_true",
                        help="Use vocal-focused concept list")
    args = parser.parse_args()

    print("Loading CLAP text encoder…")
    embed_fn = load_clap_text_encoder()

    print("Loading audio embeddings from DB…")
    audio_embs = load_audio_embeddings()
    print(f"  {len(audio_embs)} tracks with audio embeddings")

    track_paths = load_track_paths()
    style_texts = load_style_texts(track_paths)
    print(f"  {len(style_texts)} tracks with style.txt")

    # Only analyse tracks that have both audio embedding and style text
    common_ids = sorted(set(audio_embs) & set(style_texts))
    print(f"  {len(common_ids)} tracks usable for analysis\n")

    # Embed all style prompts once
    style_vecs = embed_style_texts(embed_fn, {tid: style_texts[tid] for tid in common_ids})

    if args.audioset:
        concept_list = AUDIOSET_MUSIC_CONCEPTS
    elif args.vocals:
        concept_list = VOCAL_CONCEPTS
    else:
        concept_list = CONCEPTS
    print("\nEmbedding concepts…")
    concept_vecs = {}
    for concept in concept_list:
        concept_vecs[concept] = embed_concept(embed_fn, concept)
    print(f"  {len(concept_vecs)} concepts embedded\n")

    # Compute similarities and collect results
    results = []
    for concept, cvec in concept_vecs.items():
        # Label positives via text-text similarity to style prompt
        pos_ids = label_positives(cvec, style_vecs, text_threshold=args.text_threshold)
        neg_ids = set(common_ids) - pos_ids

        sims_pos = [float(np.dot(audio_embs[tid], cvec)) for tid in pos_ids if tid in audio_embs]
        sims_neg = [float(np.dot(audio_embs[tid], cvec)) for tid in neg_ids if tid in audio_embs]

        pos = np.array(sims_pos)
        neg = np.array(sims_neg)
        sep = separation_score(pos, neg)
        threshold = suggest_threshold(pos, neg)

        results.append({
            "concept":    concept,
            "n_pos":      len(pos),
            "n_neg":      len(neg),
            "mean_pos":   float(pos.mean()) if len(pos) > 0 else float("nan"),
            "mean_neg":   float(neg.mean()) if len(neg) > 0 else float("nan"),
            "std_pos":    float(pos.std())  if len(pos) > 0 else float("nan"),
            "std_neg":    float(neg.std())  if len(neg) > 0 else float("nan"),
            "separation": sep,
            "threshold":  threshold,
        })

    # Filter by minimum positives
    results = [r for r in results if r["n_pos"] >= args.min_positive]

    # Sort by separation score
    results.sort(key=lambda r: r["separation"], reverse=True)

    if args.top:
        results = results[:args.top]

    # Print report
    print(f"{'CONCEPT':<30} {'N+':>4} {'N-':>4} {'μ+':>6} {'μ-':>6} {'SEP':>6} {'THRESH':>7}  VERDICT")
    print("─" * 85)
    for r in results:
        thresh_s = f"{r['threshold']:.3f}" if r["threshold"] is not None else "   n/a"
        mu_pos_s = f"{r['mean_pos']:.3f}"  if not np.isnan(r["mean_pos"]) else "   n/a"
        mu_neg_s = f"{r['mean_neg']:.3f}"  if not np.isnan(r["mean_neg"]) else "   n/a"
        sep_s    = f"{r['separation']:.2f}"

        if r["separation"] >= 1.5:
            verdict = "✓ STRONG"
        elif r["separation"] >= 0.8:
            verdict = "~ usable"
        elif r["separation"] >= 0.4:
            verdict = "? weak"
        else:
            verdict = "✗ noise"

        print(f"{r['concept']:<30} {r['n_pos']:>4} {r['n_neg']:>4} {mu_pos_s:>6} {mu_neg_s:>6} {sep_s:>6} {thresh_s:>7}  {verdict}")

    print()
    strong   = sum(1 for r in results if r["separation"] >= 1.5)
    usable   = sum(1 for r in results if 0.8 <= r["separation"] < 1.5)
    weak     = sum(1 for r in results if 0.4 <= r["separation"] < 0.8)
    noise    = sum(1 for r in results if r["separation"] < 0.4)
    print(f"Strong (≥1.5): {strong}  |  Usable (≥0.8): {usable}  |  Weak (≥0.4): {weak}  |  Noise (<0.4): {noise}")


def distribution_analysis():
    """
    Show raw CLAP similarity distribution across ALL tracks for every concept,
    without any style-prompt labeling. A bimodal distribution suggests CLAP
    can detect the concept; a unimodal/flat distribution means it can't (or
    the concept is ubiquitous). Prints a text histogram for each concept.
    """
    parser = argparse.ArgumentParser()
    parser.add_argument("--audioset", action="store_true")
    parser.add_argument("--vocals",   action="store_true")
    args = parser.parse_args()

    print("Loading CLAP text encoder…")
    embed_fn = load_clap_text_encoder()

    print("Loading audio embeddings…")
    audio_embs = load_audio_embeddings()
    audio_vecs = np.array(list(audio_embs.values()))  # (N, 512)
    print(f"  {len(audio_vecs)} tracks\n")

    if args.audioset:
        concept_list = AUDIOSET_MUSIC_CONCEPTS
    elif args.vocals:
        concept_list = VOCAL_CONCEPTS
    else:
        concept_list = CONCEPTS

    print("Embedding concepts and computing distributions…\n")

    BINS = 10
    rows = []
    for concept in concept_list:
        cvec = embed_concept(embed_fn, concept)
        sims = audio_vecs @ cvec  # (N,) cosine similarities

        lo, hi = sims.min(), sims.max()
        mean, std = sims.mean(), sims.std()
        p10, p25, p50, p75, p90 = np.percentile(sims, [10, 25, 50, 75, 90])

        # Text histogram: how many tracks fall in each decile bin
        counts, edges = np.histogram(sims, bins=BINS)
        bar = "".join(
            "█" * int(8 * c / counts.max()) if counts.max() > 0 else " "
            for c in counts
        )

        rows.append((concept, mean, std, lo, p25, p50, p75, p90, hi, bar))

    # Sort by std descending — high std = more spread = potentially detectable
    rows.sort(key=lambda r: r[2], reverse=True)

    print(f"{'CONCEPT':<38} {'MEAN':>6} {'STD':>5} {'P25':>6} {'P50':>6} {'P75':>6}  DISTRIBUTION (lo→hi)")
    print("─" * 100)
    for concept, mean, std, lo, p25, p50, p75, p90, hi, bar in rows:
        print(f"{concept:<38} {mean:>6.3f} {std:>5.3f} {p25:>6.3f} {p50:>6.3f} {p75:>6.3f}  [{lo:.2f}|{bar}|{hi:.2f}]")


if __name__ == "__main__":
    distribution_analysis()
