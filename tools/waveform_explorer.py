#!/usr/bin/env python3
"""
Waveform structure explorer — visualise envelope shapes and cluster classifications.

Run with:
    tools/.venv/bin/streamlit run tools/waveform_explorer.py
"""

import json
import math
import os
import sqlite3
import statistics
import subprocess

import streamlit as st
import plotly.graph_objects as go

DB = os.path.expanduser(
    "~/Library/Application Support/com.rlupi.deep-cuts/deep_cuts.db"
)
WINDOW_SEC = 10.0
FLAT_CV_THRESHOLD = 0.25
BIMODAL_THRESHOLD = 0.5
MONO_THRESHOLD = 0.4


# ── analysis ──────────────────────────────────────────────────────────────────

def analyze(wf):
    vals = [v for v in wf if math.isfinite(v) and v > 0]
    if len(vals) < 8:
        return None
    mean = statistics.mean(vals)
    stdev = statistics.stdev(vals)
    cv = stdev / mean if mean > 0 else 0

    lo, hi = min(vals), max(vals)
    span = hi - lo
    if span < 1e-6:
        return None
    normed = [(v - lo) / span for v in vals]

    buckets = [0] * 10
    for v in normed:
        buckets[min(int(v * 10), 9)] += 1
    total = len(normed)
    hist = [b / total for b in buckets]

    peak1_idx = max(range(10), key=lambda i: hist[i])
    peak2_idx = max((i for i in range(10) if abs(i - peak1_idx) >= 2), key=lambda i: hist[i])
    lo_idx, hi_idx = sorted([peak1_idx, peak2_idx])
    valley = min(hist[lo_idx:hi_idx + 1]) if hi_idx > lo_idx else hist[peak1_idx]
    peak_avg = (hist[peak1_idx] + hist[peak2_idx]) / 2
    bimodality_score = (peak_avg - valley) / (peak_avg + 1e-6)

    n = len(vals)
    xs = list(range(n))
    xmean = (n - 1) / 2
    ymean = mean
    num = sum((xs[i] - xmean) * (vals[i] - ymean) for i in range(n))
    den = math.sqrt(
        sum((x - xmean) ** 2 for x in xs) * sum((v - ymean) ** 2 for v in vals)
    )
    monotonicity = num / den if den > 1e-8 else 0.0

    threshold = sorted(vals)[int(len(vals) * 0.6)]
    peaks = []
    for i in range(1, len(vals) - 1):
        if vals[i] > threshold and vals[i] >= vals[i - 1] and vals[i] >= vals[i + 1]:
            if not peaks or i - peaks[-1] >= 5:
                peaks.append(i)

    return {
        "cv": cv,
        "bimodality": bimodality_score,
        "monotonicity": monotonicity,
        "peak_count": len(peaks),
        "hist": hist,
        "vals": vals,
    }


def classify(a):
    if a["cv"] < FLAT_CV_THRESHOLD:
        return "flat"
    if a["bimodality"] > BIMODAL_THRESHOLD:
        return "bimodal"
    if abs(a["monotonicity"]) > MONO_THRESHOLD:
        return "ramp-up" if a["monotonicity"] > 0 else "ramp-down"
    return "distributed"


def window_pcts_current(wf, duration_seconds):
    vals = [v for v in wf if math.isfinite(v) and v > 0]
    if not vals:
        return [0.2, 0.5, 0.8]
    bin_count = len(wf)
    min_sep = math.ceil((10.0 / duration_seconds) * bin_count) if duration_seconds > 0 else max(bin_count // 12, 1)
    min_sep = max(min_sep, 1)
    ranked = sorted([(i, v) for i, v in enumerate(wf) if math.isfinite(v)], key=lambda x: -x[1])
    selected = []
    for idx, _ in ranked:
        if all(abs(idx - p) >= min_sep for p in selected):
            selected.append(idx)
        if len(selected) == 3:
            break
    for idx, _ in ranked:
        if idx not in selected:
            selected.append(idx)
        if len(selected) == 3:
            break
    if len(selected) < 3:
        return [0.2, 0.5, 0.8]
    selected.sort()
    return [(s + 0.5) / bin_count for s in selected]


def window_pcts_adaptive(wf, duration_seconds, a):
    cluster = classify(a)
    bin_count = len(wf)
    min_sep = math.ceil((10.0 / duration_seconds) * bin_count) if duration_seconds > 0 else max(bin_count // 12, 1)
    min_sep = max(min_sep, 1)

    if cluster == "flat":
        return [0.15, 0.50, 0.85]

    # tercile
    finite_vals = sorted(v for v in wf if math.isfinite(v) and v > 0)
    t1 = finite_vals[len(finite_vals) // 3]
    t2 = finite_vals[2 * len(finite_vals) // 3]
    bins = [(i, v) for i, v in enumerate(wf) if math.isfinite(v)]
    low  = sorted([(i, v) for i, v in bins if v <= t1],      key=lambda x: -x[1])
    mid  = sorted([(i, v) for i, v in bins if t1 < v <= t2], key=lambda x: -x[1])
    high = sorted([(i, v) for i, v in bins if v > t2],       key=lambda x: -x[1])
    picks = []
    for tercile in [low, mid, high]:
        for idx, _ in tercile:
            if all(abs(idx - p) >= min_sep for p in picks):
                picks.append(idx)
                break
    if len(picks) < 3:
        return [0.2, 0.5, 0.8]
    picks.sort()
    return [(p + 0.5) / bin_count for p in picks]


# ── data loading ──────────────────────────────────────────────────────────────

@st.cache_data
def load_tracks():
    conn = sqlite3.connect(DB)
    conn.row_factory = sqlite3.Row
    rows = conn.execute(
        "SELECT id, filename, artist, title, duration_seconds, waveform_data "
        "FROM tracks WHERE waveform_data IS NOT NULL"
    ).fetchall()
    conn.close()

    tracks = []
    for row in rows:
        wf = json.loads(row["waveform_data"])
        a = analyze(wf)
        if not a:
            continue
        tracks.append({
            "id": row["id"],
            "filename": row["filename"],
            "artist": row["artist"] or "",
            "title": row["title"] or "",
            "duration": row["duration_seconds"] or 0,
            "waveform": wf,
            "cluster": classify(a),
            **a,
        })
    return tracks


# ── UI ────────────────────────────────────────────────────────────────────────

st.set_page_config(page_title="Waveform Explorer", layout="wide")
st.title("Waveform Structure Explorer")

tracks = load_tracks()
cluster_counts = {}
for t in tracks:
    cluster_counts[t["cluster"]] = cluster_counts.get(t["cluster"], 0) + 1

# Summary bar
cols = st.columns(len(cluster_counts) + 1)
cols[0].metric("Total tracks", len(tracks))
for col, (name, count) in zip(cols[1:], sorted(cluster_counts.items())):
    cols[list(cluster_counts.keys()).index(name) + 1].metric(
        name.capitalize(), f"{count} ({100*count//len(tracks)}%)"
    )

st.divider()

# Filters
col1, col2, col3 = st.columns([2, 2, 3])
with col1:
    cluster_filter = st.selectbox(
        "Cluster", ["all"] + sorted(cluster_counts.keys())
    )
with col2:
    sort_by = st.selectbox(
        "Sort by",
        ["cv", "bimodality", "monotonicity", "peak_count", "filename"],
    )
with col3:
    search = st.text_input("Search filename / artist / title")

filtered = tracks
if cluster_filter != "all":
    filtered = [t for t in filtered if t["cluster"] == cluster_filter]
if search:
    q = search.lower()
    filtered = [t for t in filtered if q in t["filename"].lower()
                or q in t["artist"].lower() or q in t["title"].lower()]
filtered.sort(key=lambda t: t[sort_by] if sort_by != "filename" else t["filename"])

st.write(f"Showing **{len(filtered)}** tracks")

# Track list + detail
selected_id = st.selectbox(
    "Select track",
    options=[t["id"] for t in filtered],
    format_func=lambda tid: next(
        f"[{t['cluster']}] {t['filename']}  —  cv={t['cv']:.2f}  bim={t['bimodality']:.2f}  mono={t['monotonicity']:+.2f}  peaks={t['peak_count']}"
        for t in filtered if t["id"] == tid
    ),
)

track = next(t for t in tracks if t["id"] == selected_id)
wf = track["waveform"]
dur = track["duration"]
a = {k: track[k] for k in ("cv", "bimodality", "monotonicity", "peak_count", "hist", "vals")}

cur_pcts = window_pcts_current(wf, dur)
adp_pcts = window_pcts_adaptive(wf, dur, a)

st.subheader(f"{track['filename']}")

conn = sqlite3.connect(DB)
track_path = conn.execute("SELECT path FROM tracks WHERE id = ?", (selected_id,)).fetchone()[0]
conn.close()
if os.path.exists(track_path):
    st.audio(track_path)

c1, c2, c3, c4, c5 = st.columns(5)
c1.metric("Cluster", track["cluster"])
c2.metric("CV", f"{track['cv']:.3f}")
c3.metric("Bimodality", f"{track['bimodality']:.3f}")
c4.metric("Monotonicity", f"{track['monotonicity']:+.3f}")
c5.metric("Peaks", track["peak_count"])

# Envelope plot
bins = list(range(len(wf)))
times = [b / len(wf) * dur for b in bins]

fig = go.Figure()
fig.add_trace(go.Scatter(x=times, y=wf, mode="lines", name="envelope",
                         line=dict(color="#4C9BE8", width=1.5)))

colors_cur = ["#E8844C", "#E8844C", "#E8844C"]
colors_adp = ["#4CE87A", "#4CE87A", "#4CE87A"]

for i, pct in enumerate(cur_pcts):
    t = pct * dur
    fig.add_vline(x=t, line=dict(color="#E8844C", width=2, dash="dash"),
                  annotation_text=f"C{i+1}" if i == 0 else f"C{i+1}",
                  annotation_position="top")

for i, pct in enumerate(adp_pcts):
    t = pct * dur
    fig.add_vline(x=t, line=dict(color="#4CE87A", width=2, dash="dot"),
                  annotation_text=f"A{i+1}",
                  annotation_position="bottom")

fig.update_layout(
    xaxis_title="Time (s)", yaxis_title="RMS energy",
    legend=dict(orientation="h"),
    margin=dict(t=40, b=40),
    height=300,
)
st.plotly_chart(fig, use_container_width=True)
st.caption("🟠 dashed = current windows   🟢 dotted = adaptive windows")

# Histogram
fig2 = go.Figure()
bucket_labels = [f"{i*10}–{i*10+10}%" for i in range(10)]
fig2.add_trace(go.Bar(x=bucket_labels, y=track["hist"], marker_color="#4C9BE8"))
fig2.update_layout(
    title="Energy histogram (normalised)",
    xaxis_title="Energy percentile bucket",
    yaxis_title="Fraction of bins",
    height=250,
    margin=dict(t=40, b=40),
)
st.plotly_chart(fig2, use_container_width=True)
