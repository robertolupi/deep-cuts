#!/usr/bin/env python3
import json
import os
import re
import sqlite3
import fnmatch
from collections import defaultdict
from pathlib import Path

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

def fingerprint_to_like_pattern(fp: str) -> str:
    if not fp:
        return ""
    raw_tokens = re.findall(r"(MHM|MH|HM|L|M|H)([23*])?", fp)
    if not raw_tokens:
        return ""
    
    tokens = [tok for tok, cnt in raw_tokens]
    starts_with_l = tokens[0] == 'L'
    ends_with_l = tokens[-1] == 'L'
    
    milestones = [t for t in tokens if t in ('MHM', 'MH', 'HM', 'H')]
    
    deduped = []
    for m in milestones:
        if not deduped or deduped[-1] != m:
            deduped.append(m)
            
    if not ends_with_l and len(deduped) > 1:
        deduped.pop()
        
    pattern = "L%" if starts_with_l else "%"
    if deduped:
        pattern += "%".join(deduped) + "%"
    if ends_with_l:
        pattern += "L"
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
        like_pat = fingerprint_to_like_pattern(fp)
        tracks.append({
            "id": id_,
            "title": title or "Unknown Title",
            "artist": artist or "Unknown Artist",
            "genre": genre or "Unknown Genre",
            "sax": sax,
            "fingerprint": fp,
            "like_pattern": like_pat
        })

    total_tracks = len(tracks)
    print(f"Loaded {total_tracks} tracks with SAX structures.")

    # 1. Uniqueness analysis
    fp_counts = defaultdict(int)
    for t in tracks:
        fp_counts[t["fingerprint"]] += 1
    
    unique_fps = len(fp_counts)
    uniqueness = (unique_fps / total_tracks) * 100 if total_tracks > 0 else 0
    print(f"\n--- Uniqueness ---")
    print(f"Unique fingerprints: {unique_fps} / {total_tracks} ({uniqueness:.2f}% uniqueness)")
    
    print("\nTop 10 most common fingerprints:")
    sorted_fps = sorted(fp_counts.items(), key=lambda x: -x[1])[:10]
    for fp, count in sorted_fps:
        pct = (count / total_tracks) * 100
        print(f"  {fp:<20} : {count:>4} tracks ({pct:.1f}%)")

    # 2. Selectivity (match counts) analysis
    match_distribution = defaultdict(int)
    total_matches = 0
    
    for t in tracks:
        fn_pattern = t["like_pattern"].replace("%", "*").replace("_", "?").upper()
        matches = 0
        for ot in tracks:
            ofp_upper = ot["fingerprint"].upper()
            if fnmatch.fnmatch(ofp_upper, fn_pattern):
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
    print(f"\n--- Search Selectivity (LIKE query matches) ---")
    print(f"Average matches per query: {avg_matches:.1f} tracks ({avg_matches/total_tracks*100:.1f}% of library)")
    
    print("\nMatch count distribution:")
    for group in sorted(match_distribution.keys()):
        count = match_distribution[group]
        pct = (count / total_tracks) * 100
        print(f"  {group:<20} : {count:>4} queries ({pct:.1f}%)")

    # 3. Detail Analysis of Specific query types
    print(f"\n--- Sample Queries ---")
    
    # Let's find some interesting queries
    sample_queries = []
    # 1. Classical / Ambient Crescendo: starts quiet, long buildup, ends loud
    crescendos = [t for t in tracks if t["like_pattern"].startswith("L%") and t["like_pattern"].endswith("%") and len(t["like_pattern"]) <= 10]
    if crescendos:
        sample_queries.append(("Crescendo/Build", crescendos[0]))
        
    # 2. Pop / Rock Alternating Verse-Chorus: starts quiet, alternates
    alternators = [t for t in tracks if len(t["like_pattern"]) > 12]
    if alternators:
        sample_queries.append(("Complex Alternating", alternators[0]))
        
    # 3. Simple A/B or Drop:
    simple_drops = [t for t in tracks if "MH" in t["like_pattern"] and "HM" in t["like_pattern"]]
    if simple_drops:
        sample_queries.append(("Build + Dissolve", simple_drops[0]))

    for label, query in sample_queries:
        print(f"\nQuery Type: {label}")
        print(f"  Track: '{query['title']}' by {query['artist']} [{query['genre']}]")
        print(f"  SAX  : {query['sax']}")
        print(f"  FP   : {query['fingerprint']}")
        print(f"  LIKE : {query['like_pattern']}")
        
        # Find matches
        fn_pattern = query["like_pattern"].replace("%", "*").replace("_", "?").upper()
        matches = []
        for ot in tracks:
            ofp_upper = ot["fingerprint"].upper()
            if fnmatch.fnmatch(ofp_upper, fn_pattern):
                matches.append(ot)
                
        print(f"  Top 5 / {len(matches)} matches:")
        for m in sorted(matches, key=lambda x: (x["genre"] != query["genre"], x["id"]))[:5]:
            print(f"    - [{m['id']}] '{m['title']}' by {m['artist']} [{m['genre']}] (FP: {m['fingerprint']})")

if __name__ == "__main__":
    run_evaluation()
