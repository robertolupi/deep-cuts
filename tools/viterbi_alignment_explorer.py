#!/usr/bin/env python3
"""
Streamlit interactive tool for experimenting with ONNX Sequence Classifier + Viterbi Alignment.
Allows real-time tweaking of Viterbi transition priors, duration rules, and visual comparison
of argmax vs smoothed path.
"""

import os
import sys
import json
import numpy as np
import plotly.graph_objects as go
import streamlit as st
import onnxruntime as ort

st.set_page_config(page_title="Viterbi Alignment Explorer", layout="wide")

st.title("🎵 Viterbi Alignment Explorer")
st.markdown("""
This tool lets you experiment in real-time with the **Neural Sequence Tagger (ONNX)** and the **Viterbi Decoder** algorithm. 
Adjust transition priors, smoothing, and state settings to see how the alignment output changes.
""")

# Constants
CLASSES = ["unknown", "intro", "verse", "pre-chorus", "chorus", "bridge", "outro", "end"]
CHAR2IDX = {'a': 1, 'b': 2, 'c': 3, 'd': 4, 'e': 5}
COLORS = {
    "unknown": "#808080",
    "intro": "#4a7fa5",
    "verse": "#5ba3c9",
    "pre-chorus": "#c5a3c9",
    "chorus": "#ff5533",
    "bridge": "#f0a030",
    "outro": "#8f6fc0",
    "end": "#444444"
}

@st.cache_resource
def load_onnx_model():
    onnx_path = "models/sax_sequence_tagger.onnx"
    if not os.path.exists(onnx_path):
        return None
    return ort.InferenceSession(onnx_path)

@st.cache_data
def load_sample_tracks():
    tracks_path = "doc/collab/sessions/2026-06-06-sax-transformer/sample_tracks.json"
    if not os.path.exists(tracks_path):
        return []
    with open(tracks_path, "r") as f:
        return json.load(f)

session = load_onnx_model()
tracks = load_sample_tracks()

if not session:
    st.error("❌ `models/sax_sequence_tagger.onnx` not found. Please train/export the model first!")
    st.stop()

if not tracks:
    st.error("❌ `sample_tracks.json` not found in the collaborative session directory.")
    st.stop()

# Sidebar controls
st.sidebar.header("🕹️ Viterbi Decoder Configuration")

st.sidebar.subheader("1. Transition Priors (Base Coefficients)")
priors = {}
# Default logical music flows
priors[("intro", "verse")] = st.sidebar.slider("Intro ➔ Verse", 0.0, 1.0, 0.8, 0.05)
priors[("intro", "chorus")] = st.sidebar.slider("Intro ➔ Chorus", 0.0, 1.0, 0.1, 0.05)
priors[("verse", "pre-chorus")] = st.sidebar.slider("Verse ➔ Pre-Chorus", 0.0, 1.0, 0.7, 0.05)
priors[("verse", "chorus")] = st.sidebar.slider("Verse ➔ Chorus", 0.0, 1.0, 0.4, 0.05)
priors[("pre-chorus", "chorus")] = st.sidebar.slider("Pre-Chorus ➔ Chorus", 0.0, 1.0, 0.9, 0.05)
priors[("chorus", "verse")] = st.sidebar.slider("Chorus ➔ Verse", 0.0, 1.0, 0.3, 0.05)
priors[("chorus", "bridge")] = st.sidebar.slider("Chorus ➔ Bridge", 0.0, 1.0, 0.2, 0.05)
priors[("chorus", "outro")] = st.sidebar.slider("Chorus ➔ Outro", 0.0, 1.0, 0.3, 0.05)
priors[("bridge", "chorus")] = st.sidebar.slider("Bridge ➔ Chorus", 0.0, 1.0, 0.7, 0.05)
priors[("outro", "end")] = st.sidebar.slider("Outro ➔ End", 0.0, 1.0, 0.8, 0.05)

st.sidebar.subheader("2. Self-Transition / Duration Modeling")
self_loop_bonus = st.sidebar.slider("Self-Loop Base Prior (Log scale add)", 0.0, 5.0, 1.5, 0.1)

st.sidebar.subheader("3. Penalty Rules")
smoothing = st.sidebar.slider("Smoothing Parameter (Non-prior baseline)", 1e-4, 0.1, 0.01, 0.001)
unknown_prior = st.sidebar.slider("Filler/Unknown prior strength", 0.0, 1.0, 0.1, 0.05)

# Selection of Track
st.subheader("Select Track to Inspect")
track_names = [f"{t['title']} — {t['artist']}" for t in tracks]
selected_idx = st.selectbox("Track Selection", range(len(tracks)), format_func=lambda i: track_names[i])
track = tracks[selected_idx]

# Run model prediction
sax = track["waveform_sax"]
sax_ids = [CHAR2IDX.get(c, 0) for c in sax]

wf_raw = track["waveform_data"]
chunk_size = len(wf_raw) // len(sax)
waveform = [sum(wf_raw[i * chunk_size : (i + 1) * chunk_size]) / chunk_size for i in range(len(sax))]

onnx_inputs = {
    'sax_ids': [sax_ids],
    'waveform': [waveform]
}
outputs = session.run(['logits'], onnx_inputs)
logits = outputs[0][0] # [seq_len, num_classes]
# Softmax
exp_logits = np.exp(logits - np.max(logits, axis=-1, keepdims=True))
probs = exp_logits / np.sum(exp_logits, axis=-1, keepdims=True)

# Build transition matrix
n_states = len(CLASSES)
trans = np.full((n_states, n_states), smoothing)

# Inject priors
for (f_state, t_state), p_val in priors.items():
    i = CLASSES.index(f_state)
    j = CLASSES.index(t_state)
    trans[i, j] = p_val

# Unknown handling
u_idx = CLASSES.index("unknown")
for i in range(n_states):
    trans[i, u_idx] = unknown_prior
    trans[u_idx, i] = unknown_prior

# Inject self-loops
for i in range(n_states):
    trans[i, i] = 1.0

# Normalize rows
trans = trans / np.sum(trans, axis=-1, keepdims=True)

# Viterbi Algorithm with log probs
def viterbi(probs, transition_matrix, init_probs, self_loop_bonus):
    t_len = len(probs)
    n_states = len(init_probs)
    
    dp = np.full((t_len, n_states), -np.inf)
    back = np.zeros((t_len, n_states), dtype=int)
    
    # Init
    dp[0] = np.log(init_probs) + np.log(np.maximum(probs[0], 1e-9))
    
    # Recurse
    for t in range(1, t_len):
        for s in range(n_states):
            # transition value
            trans_vals = np.log(transition_matrix[:, s])
            # Apply self-loop log bonus if s == sp
            for sp in range(n_states):
                val = dp[t-1, sp] + trans_vals[sp]
                if sp == s:
                    val += self_loop_bonus
                if val > dp[t, s]:
                    dp[t, s] = val
                    back[t, s] = sp
            dp[t, s] += np.log(np.maximum(probs[t, s], 1e-9))
            
    # Terminate
    best_last = np.argmax(dp[-1])
    best_score = dp[-1, best_last]
    
    # Backtrack
    path = [0] * t_len
    path[-1] = best_last
    for t in range(t_len - 1, 0, -1):
        path[t-1] = back[t, path[t]]
        
    return path, best_score

# Uniform initial distribution
init = np.full(n_states, 1.0 / n_states)
viterbi_path_idx, score = viterbi(probs, trans, init, self_loop_bonus)

# Argmax (raw prediction per step)
argmax_path_idx = np.argmax(probs, axis=-1)

# Display Comparison
col1, col2 = st.columns(2)

with col1:
    st.subheader("Raw Model Probabilities")
    # Stacked bar plot or heat map of probabilities per segment
    fig_probs = go.Figure()
    for c_idx, cls in enumerate(CLASSES):
        fig_probs.add_trace(go.Bar(
            name=cls,
            x=list(range(len(sax))),
            y=probs[:, c_idx],
            marker_color=COLORS[cls]
        ))
    fig_probs.update_layout(
        barmode='stack',
        template="plotly_dark",
        height=300,
        margin=dict(t=20, b=20, l=10, r=10),
        xaxis_title="Audio Segment (1/32th of Track)",
        yaxis_title="Probability"
    )
    st.plotly_chart(fig_probs, use_container_width=True)

with col2:
    st.subheader("Alignment Path Comparison")
    
    fig_paths = go.Figure()
    # Argmax path representation
    fig_paths.add_trace(go.Scatter(
        x=list(range(len(sax))),
        y=[CLASSES[idx] for idx in argmax_path_idx],
        mode="lines+markers",
        name="Raw Model argmax",
        line=dict(color="#ff5533", width=2, dash="dash"),
        marker=dict(size=8)
    ))
    
    # Viterbi aligned path
    fig_paths.add_trace(go.Scatter(
        x=list(range(len(sax))),
        y=[CLASSES[idx] for idx in viterbi_path_idx],
        mode="lines+markers",
        name="Viterbi Path (Aligned)",
        line=dict(color="#00f0ff", width=4),
        marker=dict(size=10)
    ))
    
    fig_paths.update_layout(
        template="plotly_dark",
        height=300,
        margin=dict(t=20, b=20, l=10, r=10),
        yaxis=dict(categoryorder="array", categoryarray=CLASSES),
        xaxis_title="Audio Segment"
    )
    st.plotly_chart(fig_paths, use_container_width=True)

# Post-processing: Map unknown at start to intro (up to 4 segments max), and unknown at end to outro
viterbi_labels = [CLASSES[idx] for idx in viterbi_path_idx]

# Map unknowns at start to "intro" (max 4 segments)
idx = 0
while idx < len(viterbi_labels) and viterbi_labels[idx] == "unknown" and idx < 4:
    viterbi_labels[idx] = "intro"
    idx += 1

# Map unknowns at end to "outro"
idx = len(viterbi_labels) - 1
while idx >= 0 and viterbi_labels[idx] == "unknown":
    viterbi_labels[idx] = "outro"
    idx -= 1

# Generate compacted run-length summary
compacted_runs = []
for label in viterbi_labels:
    if not compacted_runs or compacted_runs[-1]["label"] != label:
        compacted_runs.append({"label": label, "count": 1})
    else:
        compacted_runs[-1]["count"] += 1

def troll_count(n):
    if n == 1:
        return ""
    elif n == 2:
        return "2"
    elif n == 3:
        return "3"
    else:
        return "*"

summary_elements = []
for run in compacted_runs:
    count_troll = troll_count(run["count"])
    if count_troll:
        summary_elements.append(f"{run['label']}{count_troll}")
    else:
        summary_elements.append(run['label'])

summary_str = " ➔ ".join(summary_elements)

# Display Summary
st.markdown("### 📋 Compacted Alignment Summary")
st.code(summary_str, language="text")


# Path outputs table
st.subheader("Aligned Section Labels List")
rows = []
for idx in range(len(sax)):
    rows.append({
        "Segment": idx + 1,
        "SAX Character": sax[idx],
        "Envelope Height": round(waveform[idx], 4),
        "Raw Argmax Predict": CLASSES[argmax_path_idx[idx]],
        "Viterbi Predict (Calibrated)": viterbi_labels[idx]
    })
st.table(rows)

st.info(f"✨ Decoder log likelihood score: {score:.4f}")

