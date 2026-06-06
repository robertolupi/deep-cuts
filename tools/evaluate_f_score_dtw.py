#!/usr/bin/env python3
import json
import os
import re
import sqlite3
import fnmatch
import random
from collections import defaultdict

DB = os.path.expanduser("~/Library/Application Support/com.rlupi.deep-cuts/deep_cuts.db")

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

# --- DTW Distance in Python ---
def dtw_distance(s1: str, s2: str) -> float:
    a1 = [ord(c) - ord('a') for c in s1]
    a2 = [ord(c) - ord('a') for c in s2]
    n, m = len(a1), len(a2)
    dtw = [[float('inf')] * (m + 1) for _ in range(n + 1)]
    dtw[0][0] = 0.0
    for i in range(1, n + 1):
        for j in range(1, m + 1):
            cost = abs(a1[i-1] - a2[j-1])
            dtw[i][j] = cost + min(dtw[i-1][j], dtw[i][j-1], dtw[i-1][j-1])
    return dtw[n][m] / max(n, m)

# --- QUERY STRATEGIES ---

def strategy_verbatim(fp: str) -> str:
    return fp

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


def evaluate():
    if not os.path.exists(DB):
        print(f"Error: DB not found at {DB}")
        return

    con = sqlite3.connect(DB)
    rows = con.execute("SELECT id, title, artist, genre, waveform_sax FROM tracks WHERE waveform_sax IS NOT NULL").fetchall()
    con.close()

    tracks = []
    for id_, title, artist, genre, sax in rows:
        fp = compute_fingerprint(sax)
        tracks.append({
            "id": id_,
            "title": title or "Unknown",
            "artist": artist or "Unknown",
            "sax": sax,
            "fingerprint": fp
        })

    n_total = len(tracks)
    print(f"Loaded {n_total} tracks from database.")

    # Select 200 random queries for evaluation to be fast and representative
    random.seed(42)
    queries = random.sample(tracks, min(200, n_total))

    strategies = [
        ("Verbatim (Exact Match)", strategy_verbatim, "exact"),
        ("Loose LIKE (% wildcard)", strategy_loose_like, "like"),
        ("Bounded Regex (gap <= 5)", strategy_bounded_regex, "regex"),
        ("Unbounded Regex (.*?)", strategy_unbounded_regex, "regex"),
    ]

    print("\n--- Structural Similarity Search Evaluation (Ground Truth: DTW <= 0.6) ---")
    print(f"{'Strategy':<30} | {'Precision':<10} | {'Recall':<10} | {'F1-Score':<10} | {'Avg Matches':<12}")
    print("-" * 85)

    # DTW threshold 0.6 defines "similar" shape
    DTW_THRESHOLD = 0.6

    for name, get_pattern, ptype in strategies:
        total_p, total_r, total_f1 = 0.0, 0.0, 0.0
        total_matches = 0
        
        for q in queries:
            pattern = get_pattern(q["fingerprint"])
            
            # Find matches in the full database
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
            
            # Ground truth: DTW <= DTW_THRESHOLD against query's raw SAX
            gt_siblings = [ot for ot in tracks if dtw_distance(q["sax"], ot["sax"]) <= DTW_THRESHOLD]
            
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

        avg_p = (total_p / len(queries)) * 100
        avg_r = (total_r / len(queries)) * 100
        avg_f1 = (total_f1 / len(queries)) * 100
        avg_m = total_matches / len(queries)
        
        print(f"{name:<30} | {avg_p:>8.1f}% | {avg_r:>8.1f}% | {avg_f1:>8.1f}% | {avg_m:>12.1f}")

if __name__ == "__main__":
    evaluate()
