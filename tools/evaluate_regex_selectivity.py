#!/usr/bin/env python3
import os
import re
import sqlite3
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

def fingerprint_to_regex_pattern(fp: str) -> str:
    if not fp:
        return ""
    raw_tokens = re.findall(r"(MHM|MH|HM|L|M|H)([23*])?", fp)
    if not raw_tokens:
        return ""
    
    tokens = [tok for tok, cnt in raw_tokens]
    
    # Compress sequence by stripping non-essential L/M tokens in large sequences
    starts_with_l = tokens[0] == 'L'
    ends_with_l = tokens[-1] == 'L'
    milestones = [t for t in tokens if t in ('MHM', 'MH', 'HM', 'H')]
    
    deduped = []
    for m in milestones:
        if not deduped or deduped[-1] != m:
            deduped.append(m)
            
    if not ends_with_l and len(deduped) > 2:
        deduped.pop()
        
    # Build regex: allow optional troll count [23*]? after each milestone token
    # and join milestones with a bounded lazy wildcard .{0,5}? to restrict gap distance
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

def run_evaluation():
    if not os.path.exists(DB):
        print(f"Error: Database not found at {DB}")
        return

    con = sqlite3.connect(DB)
    rows = con.execute(
        "SELECT id, title, artist, genre, waveform_sax FROM tracks WHERE waveform_sax IS NOT NULL"
    ).fetchall()
    con.close()

    tracks = []
    for id_, title, artist, genre, sax in rows:
        fp = compute_fingerprint(sax)
        regex_pat = fingerprint_to_regex_pattern(fp)
        tracks.append({
            "id": id_,
            "title": title or "Unknown Title",
            "artist": artist or "Unknown Artist",
            "genre": genre or "Unknown Genre",
            "sax": sax,
            "fingerprint": fp,
            "regex_pattern": regex_pat
        })

    total_tracks = len(tracks)
    print(f"Loaded {total_tracks} tracks with SAX structures.")

    # Selectivity (match counts) analysis
    match_distribution = defaultdict(int)
    total_matches = 0
    
    for t in tracks:
        rx = re.compile(t["regex_pattern"], re.IGNORECASE)
        matches = 0
        for ot in tracks:
            if rx.search(ot["fingerprint"]):
                matches += 1
        total_matches += matches
        
        # Categorize match count
        if matches == 1:
            match_distribution["1 (Only self)"] += 1
        elif matches <= 5:
            match_distribution["2 - 5 matches"] += 1
        elif matches <= 20:
            match_distribution["6 - 20 matches"] += 1
        elif matches <= 100:
            match_distribution["21 - 100 matches"] += 1
        else:
            match_distribution["> 100 matches"] += 1

    avg_matches = total_matches / total_tracks if total_tracks > 0 else 0
    print(f"\n--- Search Selectivity (REGEX query matches) ---")
    print(f"Average matches per query: {avg_matches:.1f} tracks ({avg_matches/total_tracks*100:.1f}% of library)")
    
    print("\nMatch count distribution:")
    for group in sorted(match_distribution.keys()):
        count = match_distribution[group]
        pct = (count / total_tracks) * 100
        print(f"  {group:<20} : {count:>4} queries ({pct:.1f}%)")

    # Show Strauss' Zarathoustra / build query matches
    print(f"\n--- Zarathoustra / build matches with regex ---")
    zarathoustra_sax = "abaaaaaababdeccbbbddeccbcceeeeee"
    zarathoustra_fp = compute_fingerprint(zarathoustra_sax)
    zarathoustra_rx_pat = fingerprint_to_regex_pattern(zarathoustra_fp)
    print(f"Zarathoustra FP: {zarathoustra_fp}")
    print(f"Zarathoustra Regex Pattern: {zarathoustra_rx_pat}")
    
    rx = re.compile(zarathoustra_rx_pat, re.IGNORECASE)
    matches = []
    for ot in tracks:
        if rx.search(ot["fingerprint"]):
            matches.append(ot)
            
    print(f"Found {len(matches)} matches (down from 842!):")
    for m in sorted(matches, key=lambda x: x["genre"])[:10]:
         print(f"  - [{m['id']}] '{m['title']}' by {m['artist']} [{m['genre']}] (FP: {m['fingerprint']})")

if __name__ == "__main__":
    run_evaluation()
