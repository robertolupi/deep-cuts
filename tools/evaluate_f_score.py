#!/usr/bin/env python3
import json
import os
import re
import sqlite3
import fnmatch
from pathlib import Path
from collections import defaultdict

DB = os.path.expanduser("~/Library/Application Support/com.rlupi.deep-cuts/deep_cuts.db")

SAX_TO_LMH = {"a": "L", "b": "L", "c": "M", "d": "H", "e": "H"}

def sax_to_lmh(c: str) -> str:
    if c in ("a", "b"):
        return "L"
    if c == "c":
        return "M"
    return "H"

def troll(n: int) -> str:
    if n == 1:
        return ""
    if n == 2:
        return "2"
    if n == 3:
        return "3"
    return "*"

def compute_fingerprint(sax: str) -> str:
    if not sax:
        return ""
    lmh = [sax_to_lmh(c) for c in sax]
    
    lmh_runs = []
    for c in lmh:
        if lmh_runs and lmh_runs[-1]["char"] == c:
            lmh_runs[-1]["count"] += 1
        else:
            lmh_runs.append({"char": c, "count": 1})
            
    tokens = []
    i = 0
    while i < len(lmh_runs):
        if i + 2 < len(lmh_runs) and lmh_runs[i]["char"] == 'M' and lmh_runs[i+1]["char"] == 'H' and lmh_runs[i+2]["char"] == 'M':
            tokens.append({
                "token": "MHM",
                "count": lmh_runs[i]["count"] + lmh_runs[i+1]["count"] + lmh_runs[i+2]["count"]
            })
            i += 3
        elif i + 1 < len(lmh_runs) and lmh_runs[i]["char"] == 'M' and lmh_runs[i+1]["char"] == 'H':
            tokens.append({
                "token": "MH",
                "count": lmh_runs[i]["count"] + lmh_runs[i+1]["count"]
            })
            i += 2
        elif i + 1 < len(lmh_runs) and lmh_runs[i]["char"] == 'H' and lmh_runs[i+1]["char"] == 'M':
            tokens.append({
                "token": "HM",
                "count": lmh_runs[i]["count"] + lmh_runs[i+1]["count"]
            })
            i += 2
        else:
            token = "L" if lmh_runs[i]["char"] == 'L' else "M" if lmh_runs[i]["char"] == 'M' else "H"
            tokens.append({
                "token": token,
                "count": lmh_runs[i]["count"]
            })
            i += 1
            
    return "".join(t["token"] + troll(t["count"]) for t in tokens)

# --- QUERY STRATEGIES ---

# 1. Verbatim (no wildcards)
def strategy_verbatim(fp: str) -> str:
    return fp

# 2. Loose LIKE (wildcards between milestones)
def strategy_loose_like(fp: str) -> str:
    if not fp:
        return ""
    raw_tokens = re.findall(r"(MHM|MH|HM|L|M|H)([23*])?", fp)
    tokens = [tok for tok, cnt in raw_tokens]
    starts_with_l = tokens[0] == 'L'
    ends_with_l = tokens[-1] == 'L'
    milestones = [t for t in tokens if t in ('MHM', 'MH', 'HM', 'H')]
    
    deduped = []
    for m in milestones:
        if not deduped or deduped[-1] != m:
            deduped.append(m)
            
    if not ends_with_l and len(deduped) > 2:
        deduped.pop()
        
    pattern = "L%" if starts_with_l else "%"
    if deduped:
        pattern += "%".join(deduped) + "%"
    if ends_with_l:
        pattern += "L"
    return pattern

# 3. Bounded Regex (optional counts and max gap of 5 chars)
def strategy_bounded_regex(fp: str) -> str:
    if not fp:
        return ""
    raw_tokens = re.findall(r"(MHM|MH|HM|L|M|H)([23*])?", fp)
    tokens = [tok for tok, cnt in raw_tokens]
    starts_with_l = tokens[0] == 'L'
    ends_with_l = tokens[-1] == 'L'
    milestones = [t for t in tokens if t in ('MHM', 'MH', 'HM', 'H')]
    
    deduped = []
    for m in milestones:
        if not deduped or deduped[-1] != m:
            deduped.append(m)
            
    if not ends_with_l and len(deduped) > 2:
        deduped.pop()
        
    pattern = r"^"
    if starts_with_l:
        pattern += r"L[23*]?.{0,5}?"
    else:
        pattern += r".*?"
    if deduped:
        pattern += r".{0,5}?".join(rf"{m}[23*]?" for m in deduped) + r".{0,5}?"
    if ends_with_l:
        pattern += r"L[23*]?$"
    else:
        pattern += r"$"
    return pattern

# 4. Unbounded Regex (optional counts and lazy wildcard gaps)
def strategy_unbounded_regex(fp: str) -> str:
    if not fp:
        return ""
    raw_tokens = re.findall(r"(MHM|MH|HM|L|M|H)([23*])?", fp)
    tokens = [tok for tok, cnt in raw_tokens]
    starts_with_l = tokens[0] == 'L'
    ends_with_l = tokens[-1] == 'L'
    milestones = [t for t in tokens if t in ('MHM', 'MH', 'HM', 'H')]
    
    deduped = []
    for m in milestones:
        if not deduped or deduped[-1] != m:
            deduped.append(m)
            
    if not ends_with_l and len(deduped) > 2:
        deduped.pop()
        
    pattern = r"^"
    if starts_with_l:
        pattern += r"L[23*]?.*?"
    else:
        pattern += r".*?"
    if deduped:
        pattern += r".*?".join(rf"{m}[23*]?" for m in deduped) + r".*?"
    if ends_with_l:
        pattern += r"L[23*]?$"
    else:
        pattern += r"$"
    return pattern


# --- GROUND TRUTH PARSING ---

def find_lyrics(track_path: str):
    folder = Path(track_path).parent
    candidate = folder / "lyrics.txt"
    return str(candidate) if candidate.exists() else None

def parse_lyrics_ground_truth(lyrics_path: str):
    try:
        text = Path(lyrics_path).read_text(errors="replace")
    except Exception:
        return None
    lines = text.splitlines()
    labels = set()
    for line in lines:
        m = re.match(r"^\[(.+?)\]", line.strip())
        if m:
            raw = m.group(1).lower().strip()
            labels.add(raw)
            
    # Derive structural features
    has_intro = any("intro" in l for l in labels)
    has_outro = any("outro" in l or "end" in l or "fade" in l for l in labels)
    has_chorus = any("chorus" in l or "drop" in l for l in labels)
    has_bridge = any("bridge" in l for l in labels)
    
    return (has_intro, has_outro, has_chorus, has_bridge)


def evaluate():
    if not os.path.exists(DB):
        print(f"Error: DB not found at {DB}")
        return

    con = sqlite3.connect(DB)
    rows = con.execute("SELECT id, path, title, artist, waveform_sax FROM tracks WHERE waveform_sax IS NOT NULL").fetchall()
    con.close()

    # Load tracks that have lyrics ground truth
    tracks = []
    for id_, path, title, artist, sax in rows:
        lpath = find_lyrics(path)
        if not lpath:
            continue
        gt = parse_lyrics_ground_truth(lpath)
        if not gt:
            continue
        
        fp = compute_fingerprint(sax)
        tracks.append({
            "id": id_,
            "title": title or "Unknown",
            "artist": artist or "Unknown",
            "fingerprint": fp,
            "gt_intro": gt[0],
            "gt_outro": gt[1],
            "gt_chorus": gt[2],
            "gt_bridge": gt[3],
        })

    n_gt = len(tracks)
    print(f"Loaded {n_gt} tracks with ground truth structural tags from lyrics.txt.")
    if n_gt < 5:
        print("Warning: Too few ground-truth tracks to evaluate. Ensure MP3_ROOT is correct.")
        return

    strategies = [
        ("Verbatim (Exact Match)", strategy_verbatim, "exact"),
        ("Loose LIKE (% wildcard)", strategy_loose_like, "like"),
        ("Bounded Regex (gap <= 5)", strategy_bounded_regex, "regex"),
        ("Unbounded Regex (.*?)", strategy_unbounded_regex, "regex"),
    ]

    print("\n--- Structural Similarity Search Evaluation ---")
    print(f"{'Strategy':<30} | {'Precision':<10} | {'Recall':<10} | {'F1-Score':<10} | {'Avg Matches':<12}")
    print("-" * 85)

    for name, get_pattern, ptype in strategies:
        total_p, total_r, total_f1 = 0.0, 0.0, 0.0
        total_matches = 0
        
        for q in tracks:
            pattern = get_pattern(q["fingerprint"])
            
            # Find matches in ground truth set
            matched = []
            for ot in tracks:
                matched_flag = False
                if ptype == "exact":
                    matched_flag = (ot["fingerprint"].upper() == pattern.upper())
                elif ptype == "like":
                    fn_pat = pattern.replace("%", "*").replace("_", "?").upper()
                    matched_flag = fnmatch.fnmatch(ot["fingerprint"].upper(), fn_pat)
                elif ptype == "regex":
                    rx = re.compile(pattern, re.IGNORECASE)
                    matched_flag = bool(rx.search(ot["fingerprint"]))
                
                if matched_flag:
                    matched.append(ot)
            
            total_matches += len(matched)
            
            # Ground truth matches are other tracks that share the EXACT same structural profile (same intro, outro, chorus, bridge values)
            gt_siblings = [ot for ot in tracks if 
                           ot["gt_intro"] == q["gt_intro"] and 
                           ot["gt_outro"] == q["gt_outro"] and 
                           ot["gt_chorus"] == q["gt_chorus"] and 
                           ot["gt_bridge"] == q["gt_bridge"]]
            
            # Calculate TP, FP, FN
            tp = len([m for m in matched if m in gt_siblings])
            fp = len([m for m in matched if m not in gt_siblings])
            fn = len([g for g in gt_siblings if g not in matched])
            
            precision = tp / (tp + fp) if (tp + fp) > 0 else 0
            recall = tp / (tp + fn) if (tp + fn) > 0 else 0
            f1 = 2 * (precision * recall) / (precision + recall) if (precision + recall) > 0 else 0
            
            total_p += precision
            total_r += recall
            total_f1 += f1

        avg_p = (total_p / n_gt) * 100
        avg_r = (total_r / n_gt) * 100
        avg_f1 = (total_f1 / n_gt) * 100
        avg_m = total_matches / n_gt
        
        print(f"{name:<30} | {avg_p:>8.1f}% | {avg_r:>8.1f}% | {avg_f1:>8.1f}% | {avg_m:>12.1f}")

if __name__ == "__main__":
    evaluate()
