#!/usr/bin/env python3
"""
SAX structural search explorer — validate block→RLE mappings using Downspiral lyrics.

Run with:
    tools/.venv/bin/streamlit run tools/sax_structure_explorer.py
"""

import json
import os
import re
import sqlite3
from collections import defaultdict
from pathlib import Path

import plotly.graph_objects as go
import plotly.express as px
import streamlit as st

DB = os.path.expanduser(
    "~/Library/Application Support/com.rlupi.deep-cuts/deep_cuts.db"
)
MP3_ROOT = os.path.expanduser("~/Downloads/MP3 Songs")

# SAX alphabet → energy level
SAX_TO_LMH = {"a": "L", "b": "L", "c": "M", "d": "H", "e": "H"}
SAX_COLORS = {"a": "#4a7fa5", "b": "#5ba3c9", "c": "#00f0ff", "d": "#f0a030", "e": "#ff5533"}
LMH_COLORS = {"L": "#5ba3c9", "M": "#00f0ff", "H": "#ff5533"}

# Canonical section label → expected energy
LABEL_ENERGY = {
    "intro":        "L",
    "verse":        "L",
    "pre-chorus":   "M",
    "prechorus":    "M",
    "pre chorus":   "M",
    "chorus":       "H",
    "final chorus": "H",
    "post-chorus":  "H",
    "bridge":       "M",
    "break":        "L",
    "drop":         "H",
    "outro":        "L",
    "end":          "L",
    "fade out":     "L",
}

# Block composer → RLE regex
BLOCK_PATTERNS = {
    "Intro":      r"^L",
    "Verse":      r"[LM]",
    "Pre-Chorus": r"M",
    "Chorus":     r"H",
    "Drop":       r"HLH",
    "Bridge":     r"[LM]",
    "Break":      r"L",
    "Build":      r"L.*M.*H",
    "Outro":      r"L$",
}

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
    
    # RLE with counts
    lmh_runs = []
    for c in lmh:
        if lmh_runs and lmh_runs[-1]["char"] == c:
            lmh_runs[-1]["count"] += 1
        else:
            lmh_runs.append({"char": c, "count": 1})
            
    # Greedy tokenize runs (group MHM, MH, HM, sum counts)
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
            
    # Format with troll counting
    return "".join(t["token"] + troll(t["count"]) for t in tokens)

def fingerprint_to_like_pattern(fp: str) -> str:
    if not fp:
        return ""
    # Tokenize the fingerprint: matches tokens like MHM, MH, HM, L, M, H, optionally followed by 2, 3, *
    raw_tokens = re.findall(r"(MHM|MH|HM|L|M|H)([23*])?", fp)
    if not raw_tokens:
        return ""
    
    # Strip counts/stars to get base tokens
    tokens = [tok for tok, cnt in raw_tokens]
    
    starts_with_l = tokens[0] == 'L'
    ends_with_l = tokens[-1] == 'L'
    
    # Extract high-energy milestones
    milestones = [t for t in tokens if t in ('MHM', 'MH', 'HM', 'H')]
    
    # De-duplicate consecutive milestones
    deduped = []
    for m in milestones:
        if not deduped or deduped[-1] != m:
            deduped.append(m)
            
    # Penalize large sequences: drop trailing milestone if not ending in L
    if not ends_with_l and len(deduped) > 1:
        deduped.pop()
        
    # Construct LIKE pattern
    pattern = "L%" if starts_with_l else "%"
    if deduped:
        pattern += "%".join(deduped) + "%"
    if ends_with_l:
        pattern += "L"
    return pattern



# ── data loading ──────────────────────────────────────────────────────────────

@st.cache_data
def load_tracks():
    con = sqlite3.connect(DB)
    rows = con.execute(
        "SELECT id, path, title, artist, duration_seconds, waveform_data, waveform_sax, genre "
        "FROM tracks WHERE waveform_sax IS NOT NULL"
    ).fetchall()
    con.close()
    tracks = []
    for id_, path, title, artist, dur, wf_json, sax, genre in rows:
        if not wf_json or not sax:
            continue
        try:
            wf = json.loads(wf_json)
        except Exception:
            continue
        tracks.append({
            "id": id_, "path": path, "title": title or Path(path).stem,
            "artist": artist, "duration": dur,
            "waveform": wf, "sax": sax,
            "genre": genre or "Unknown",
            "rle": to_rle(sax),
        })
    return tracks


def to_rle(sax: str) -> str:
    """Collapse SAX string to L/M/H run-length encoded string (no counts)."""
    lmh = [SAX_TO_LMH.get(c, "M") for c in sax]
    result = []
    for ch in lmh:
        if not result or result[-1] != ch:
            result.append(ch)
    return "".join(result)


def parse_lyrics(lyrics_path: str):
    """Return list of (label_raw, label_canonical, line_start, line_end) tuples."""
    try:
        text = Path(lyrics_path).read_text(errors="replace")
    except FileNotFoundError:
        return []
    lines = text.splitlines()
    sections = []
    for i, line in enumerate(lines):
        m = re.match(r"^\[(.+?)\]", line.strip())
        if m:
            raw = m.group(1)
            canon = raw.lower().strip().rstrip("0123456789 ").strip()
            sections.append({"raw": raw, "canon": canon, "line": i, "total": len(lines)})
    return sections


def find_lyrics(track_path: str):
    """Find lyrics.txt for a track path."""
    folder = Path(track_path).parent
    candidate = folder / "lyrics.txt"
    return str(candidate) if candidate.exists() else None


def section_to_sax_letter(sax: str, position: float) -> str:
    """Map a fractional position [0,1] to the SAX letter at that position."""
    idx = min(int(position * len(sax)), len(sax) - 1)
    return sax[idx]


def compile_blocks_to_regex(blocks: list[str]) -> str:
    """Compile list of block names to RLE regex with .* glue."""
    parts = []
    for i, b in enumerate(blocks):
        pat = BLOCK_PATTERNS.get(b, ".*")
        if b == "Intro" and i == 0:
            pat = r"^L"
        elif b == "Outro" and i == len(blocks) - 1:
            pat = r"L$"
        elif b in ("Intro", "Verse", "Break"):
            pat = r"L"
        elif b in ("Pre-Chorus", "Bridge"):
            pat = r"M"
        elif b in ("Chorus", "Drop"):
            pat = r"H"
        parts.append(pat)
    if len(parts) == 0:
        return ".*"
    # Join with .* glue, but keep anchors
    result = r".*".join(parts)
    return result


# ── pages ─────────────────────────────────────────────────────────────────────

def page_overview(tracks):
    st.header("Library overview")
    total = len(tracks)

    # RLE fingerprint distribution
    fingerprints = defaultdict(int)
    for t in tracks:
        fingerprints[t["rle"]] += 1

    top = sorted(fingerprints.items(), key=lambda x: -x[1])[:20]
    labels, counts = zip(*top)

    fig = go.Figure(go.Bar(
        x=list(labels), y=list(counts),
        marker_color=[LMH_COLORS.get(l[0], "#aaa") for l in labels],
    ))
    fig.update_layout(title=f"Top 20 RLE fingerprints ({total} tracks with SAX)",
                      xaxis_title="RLE pattern", yaxis_title="Count",
                      template="plotly_dark", height=350)
    st.plotly_chart(fig, use_container_width=True)

    # SAX letter frequency
    all_sax = "".join(t["sax"] for t in tracks)
    letter_counts = {l: all_sax.count(l) for l in "abcde"}
    total_letters = sum(letter_counts.values())
    fig2 = go.Figure(go.Bar(
        x=list(letter_counts.keys()),
        y=[v / total_letters * 100 for v in letter_counts.values()],
        marker_color=[SAX_COLORS[l] for l in "abcde"],
    ))
    fig2.update_layout(title="SAX letter distribution (% of all segments)",
                       xaxis_title="SAX letter", yaxis_title="%",
                       template="plotly_dark", height=300)
    st.plotly_chart(fig2, use_container_width=True)


def page_lyrics_correlation(tracks):
    st.header("Section label → energy correlation")
    st.caption("Using Downspiral tracks with lyrics.txt ground truth")

    # Build dataset
    label_letters = defaultdict(list)  # canon label → list of SAX letters at that position
    matched_tracks = 0

    for t in tracks:
        lpath = find_lyrics(t["path"])
        if not lpath:
            continue
        sections = parse_lyrics(lpath)
        if not sections:
            continue
        matched_tracks += 1
        n_lines = sections[-1]["total"] if sections else 1
        for s in sections:
            pos = s["line"] / max(n_lines, 1)
            letter = section_to_sax_letter(t["sax"], pos)
            canon = s["canon"]
            # Normalize common variants
            for key in LABEL_ENERGY:
                if key in canon:
                    label_letters[key].append(letter)
                    break

    st.metric("Tracks with lyrics matched", matched_tracks)

    if not label_letters:
        st.warning("No matched tracks found. Check MP3_ROOT path.")
        return

    # For each label, show distribution of SAX letters
    focus_labels = ["intro", "verse", "pre-chorus", "chorus", "bridge", "outro"]
    for label in focus_labels:
        letters = label_letters.get(label, [])
        if not letters:
            continue
        lmh_counts = {"L": 0, "M": 0, "H": 0}
        for l in letters:
            lmh_counts[SAX_TO_LMH.get(l, "M")] += 1
        total = len(letters)
        expected = LABEL_ENERGY.get(label, "?")

        cols = st.columns([2, 5])
        cols[0].markdown(f"**[{label.title()}]**  \nExpected: `{expected}`  \nn={total}")

        fig = go.Figure(go.Bar(
            x=list(lmh_counts.keys()),
            y=[v / total * 100 for v in lmh_counts.values()],
            marker_color=[LMH_COLORS[l] for l in "LMH"],
        ))
        fig.update_layout(template="plotly_dark", height=180, margin=dict(t=10, b=30, l=10, r=10),
                          yaxis_title="%", showlegend=False)
        cols[1].plotly_chart(fig, use_container_width=True)

    # Confusion matrix: expected LMH vs actual LMH
    st.subheader("Expected vs. actual energy (L/M/H)")
    conf = defaultdict(int)
    for label, letters in label_letters.items():
        expected = LABEL_ENERGY.get(label)
        if not expected:
            continue
        for l in letters:
            actual = SAX_TO_LMH.get(l, "?")
            conf[(expected, actual)] += 1

    lmh = ["L", "M", "H"]
    matrix = [[conf.get((exp, act), 0) for act in lmh] for exp in lmh]
    fig3 = go.Figure(go.Heatmap(
        z=matrix, x=lmh, y=lmh,
        colorscale="Blues",
        text=[[str(v) for v in row] for row in matrix],
        texttemplate="%{text}",
    ))
    fig3.update_layout(template="plotly_dark", height=300,
                       xaxis_title="Actual (SAX)", yaxis_title="Expected (label)",
                       title="Confusion: expected label energy vs SAX energy")
    st.plotly_chart(fig3, use_container_width=True)


def page_pattern_recall(tracks):
    st.header("Pattern recall — how well do RLE patterns fire?")

    # Build ground-truth sets
    has_intro, has_chorus, has_outro, has_bridge = set(), set(), set(), set()
    for t in tracks:
        lpath = find_lyrics(t["path"])
        if not lpath:
            continue
        sections = parse_lyrics(lpath)
        labels = {s["canon"] for s in sections}
        if any("intro" in l for l in labels):
            has_intro.add(t["id"])
        if any("chorus" in l for l in labels):
            has_chorus.add(t["id"])
        if any("outro" in l or "end" in l or "fade" in l for l in labels):
            has_outro.add(t["id"])
        if any("bridge" in l for l in labels):
            has_bridge.add(t["id"])

    track_by_id = {t["id"]: t for t in tracks}

    checks = [
        ("^L", "Starts quietly", has_intro),
        ("H", "Has a loud section", has_chorus),
        ("L$", "Ends quietly", has_outro),
        ("^L.*H", "Intro + Chorus", has_intro & has_chorus),
        ("^L.*H.*L$", "Intro + Chorus + Outro", has_intro & has_chorus & has_outro),
        ("HLH", "Drop structure", set()),  # no ground truth, show match count only
    ]

    rows = []
    for pattern, desc, ground_truth in checks:
        matches = {t["id"] for t in tracks if re.search(pattern, t["rle"])}
        if ground_truth:
            tp = len(matches & ground_truth)
            fn = len(ground_truth - matches)
            recall = tp / len(ground_truth) * 100 if ground_truth else 0
            precision = tp / len(matches) * 100 if matches else 0
            rows.append({
                "Pattern": f"`{pattern}`", "Description": desc,
                "Ground truth": len(ground_truth), "Matches": len(matches),
                "TP": tp, "Recall %": f"{recall:.0f}%", "Precision %": f"{precision:.0f}%",
            })
        else:
            rows.append({
                "Pattern": f"`{pattern}`", "Description": desc,
                "Ground truth": "—", "Matches": len(matches),
                "TP": "—", "Recall %": "—", "Precision %": "—",
            })

    st.dataframe(rows, use_container_width=True)


def page_block_composer(tracks):
    st.header("Block composer prototype")

    BLOCKS = list(BLOCK_PATTERNS.keys()) + ["Any"]

    if "composer_blocks" not in st.session_state:
        st.session_state.composer_blocks = []

    cols = st.columns(len(BLOCKS) + 1)
    for i, b in enumerate(BLOCKS):
        if cols[i].button(f"+ {b}", key=f"add_{b}"):
            st.session_state.composer_blocks.append(b)
    if cols[-1].button("Clear"):
        st.session_state.composer_blocks = []

    blocks = st.session_state.composer_blocks
    if blocks:
        st.markdown("**Sequence:** " + " → ".join(f"`{b}`" for b in blocks))
        regex = compile_blocks_to_regex(blocks)
        st.caption(f"Compiled regex: `{regex}`")

        matched = [t for t in tracks if re.search(regex, t["rle"])]
        st.metric("Matching tracks", len(matched))

        if matched:
            rows = [{"Title": t["title"], "Artist": t["artist"],
                     "RLE": t["rle"], "SAX": t["sax"][:16] + "…"} for t in matched[:50]]
            st.dataframe(rows, use_container_width=True)
    else:
        st.info("Click blocks above to compose a structural query.")


def page_track_detail(tracks):
    st.header("Track detail — waveform + lyrics alignment")

    # Only show tracks with lyrics
    lyric_tracks = [t for t in tracks if find_lyrics(t["path"])]
    if not lyric_tracks:
        st.warning("No tracks with lyrics found.")
        return

    options = {f"{t['title']} ({t['artist'] or 'Unknown'})": t for t in lyric_tracks}
    choice = st.selectbox("Track", list(options.keys()))
    t = options[choice]

    lpath = find_lyrics(t["path"])
    sections = parse_lyrics(lpath) if lpath else []

    # Waveform with SAX coloring
    wf = t["waveform"]
    sax = t["sax"]
    peak = max(wf) if wf else 1
    n = len(wf)

    bar_colors = [SAX_COLORS.get(sax[min(int(i * len(sax) / n), len(sax) - 1)], "#aaa") for i in range(n)]

    fig = go.Figure()
    fig.add_trace(go.Bar(
        x=list(range(n)), y=[v / peak for v in wf],
        marker_color=bar_colors, name="Waveform",
    ))

    # Overlay section markers
    if sections:
        n_lines = sections[-1]["total"]
        for s in sections:
            pos = s["line"] / max(n_lines, 1)
            x_bin = pos * n
            energy = LABEL_ENERGY.get(s["canon"], "?")
            fig.add_vline(x=x_bin, line_dash="dot", line_color="white", line_width=1,
                          annotation_text=s["raw"], annotation_position="top",
                          annotation_font_size=9)

    fig.update_layout(template="plotly_dark", height=300, showlegend=False,
                      title=f"{t['title']} — SAX: {t['rle']}",
                      xaxis_title="Waveform bin (128 total)", yaxis_title="Normalized energy")
    st.plotly_chart(fig, use_container_width=True)

    # Audio player
    mp3_path = t["path"]
    if os.path.exists(mp3_path):
        with open(mp3_path, "rb") as f:
            st.audio(f.read(), format="audio/mpeg")

    # Lyrics text
    if lpath and Path(lpath).exists():
        with st.expander("Lyrics"):
            st.text(Path(lpath).read_text(errors="replace"))


def page_similarity_search(tracks):
    st.header("Duration-Aware Similarity Search")
    st.caption("Select a track to compute its fingerprint, see its SQL LIKE wildcard compression, and find matching tracks.")

    # Search and select track
    options = {f"[{t['id']}] {t['title']} — {t['artist'] or 'Unknown'} ({t['rle']})": t for t in tracks}
    choice = st.selectbox("Select query track", list(options.keys()))
    t = options[choice]

    # Compute duration-aware fingerprint
    fp = compute_fingerprint(t["sax"])
    like_pattern = fingerprint_to_like_pattern(fp)

    st.subheader("Query Details")
    c1, c2, c3, c4 = st.columns(4)
    c1.metric("Selected Track RLE", t["rle"])
    c2.metric("Raw SAX string", t["sax"][:16] + "...")
    c3.metric("waveform_fingerprint", fp)
    c4.metric("LIKE search pattern", like_pattern)

    # Perform LIKE search
    st.subheader("Matching Tracks")
    
    import fnmatch
    
    # Translate SQL LIKE pattern to fnmatch pattern (replace % with *, _ with ?)
    fn_pattern = like_pattern.replace("%", "*").replace("_", "?")
    
    matched = []
    for ot in tracks:
        ofp = compute_fingerprint(ot["sax"])
        if fnmatch.fnmatch(ofp.upper(), fn_pattern.upper()):
            matched.append((ot, ofp))
            
    st.write(f"Found **{len(matched)}** matching tracks using pattern `{like_pattern}`")
    
    if matched:
        # Display as a dataframe
        rows = []
        for ot, ofp in matched:
            olike = fingerprint_to_like_pattern(ofp)
            rows.append({
                "ID": ot["id"],
                "Title": ot["title"],
                "Artist": ot["artist"],
                "Genre": ot.get("genre") or "—",
                "waveform_sax": ot["sax"],
                "waveform_fingerprint": ofp,
                "compressed LIKE": olike,
            })
        st.dataframe(rows, use_container_width=True)

        # Plot waveforms side-by-side or stacked for selected matches
        st.subheader("Visual Waveform Comparison")
        num_to_show = min(5, len(matched))
        st.write(f"Showing waveforms for top {num_to_show} matches:")
        
        for idx in range(num_to_show):
            ot, ofp = matched[idx]
            wf = ot["waveform"]
            sax = ot["sax"]
            peak = max(wf) if wf else 1
            n = len(wf)
            bar_colors = [SAX_COLORS.get(sax[min(int(i * len(sax) / n), len(sax) - 1)], "#aaa") for i in range(n)]

            fig = go.Figure()
            fig.add_trace(go.Bar(
                x=list(range(n)), y=[v / peak for v in wf],
                marker_color=bar_colors, name="Waveform",
            ))
            fig.update_layout(
                template="plotly_dark", 
                height=150, 
                showlegend=False,
                margin=dict(t=20, b=20, l=10, r=10),
                title=f"[{ot['id']}] {ot['title']} — {ot['artist']} (fp: {ofp})"
            )
            st.plotly_chart(fig, use_container_width=True)


# ── main ──────────────────────────────────────────────────────────────────────

st.set_page_config(page_title="SAX Structure Explorer", layout="wide")
st.title("SAX Structure Explorer")

with st.spinner("Loading tracks…"):
    tracks = load_tracks()

st.sidebar.metric("Tracks with SAX", len(tracks))

page = st.sidebar.radio("Page", [
    "Overview",
    "Lyrics correlation",
    "Pattern recall",
    "Block composer",
    "Track detail",
    "Similarity search",
])

if page == "Overview":
    page_overview(tracks)
elif page == "Lyrics correlation":
    page_lyrics_correlation(tracks)
elif page == "Pattern recall":
    page_pattern_recall(tracks)
elif page == "Block composer":
    page_block_composer(tracks)
elif page == "Track detail":
    page_track_detail(tracks)
elif page == "Similarity search":
    page_similarity_search(tracks)
