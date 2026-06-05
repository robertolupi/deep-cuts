---
layout: default
title: Technology
permalink: /tech/
---

<div class="content-wrapper" markdown="1">

# Under the Hood: Deep Cuts Architecture

Deep Cuts is built with a highly optimized, fully local native desktop stack. By eliminating external cloud processes and background services, it executes digital signal processing (DSP) algorithms and neural network models entirely in-process on consumer CPUs and Apple Silicon hardware.

## Core Tech Stack

* **Frontend**: Svelte 5 (in static SPA mode), SvelteKit, and Vite. Utilizes D3.js for canvas visualizations and WaveSurfer.js for audio waveforms and spectrogram elements.
* **Backend Core**: Tauri v2 and native Rust.
* **Local Database & Vector Index**: SQLite (`rusqlite`) compiled with `sqlite-vec` virtual table configurations to enable fast, local K-Nearest Neighbors (KNN) vector calculations.
* **Local Neural Inference**: `ort` (native ONNX Runtime bindings) runs CLAP and Essentia classifiers.
* **Local Multimodal LLM**: Bundled `llama-server` (llama.cpp) binary resolves local GGUF models (Qwen2-Audio) for offline, sandboxed audio Q&A sessions.

---

## Audio Analysis Pipeline

Deep Cuts processes watched folders recursively using a concurrent, spool-based analysis engine operating in dependency order across worker threads (`num_cpus / 2`):

1. **BPM & Temporal Analysis**: Spectral-flux onset envelope tracking with autocorrelation and parabolic sub-sample refinement (40–210 BPM range).
2. **Double-Pass Correction**:
   * Genre metadata coarse pass to resolve double-time or half-time errors.
   * Secondary sweep matching against Essentia classifier outputs for alignment.
3. **Key & Scale Detection**: Chromagram constructed via FFT, HPCP harmonic suppression, and Krumhansl-Schmuckler profile correlation.
4. **Loudness (EBU R128)**: Analyzes integrated loudness (LUFS) and loudness range (LRA) using the native `ebur128` library.
5. **Essentia Genre & Mood Classifiers**: Discogs-Effnet ONNX models evaluate 400 genre classes, vocal vs. instrumental probabilities, and seven mood axes (happy, sad, aggressive, relaxed, party, acoustic, electronic).
6. **LAION CLAP Embeddings**: Extracts 512-dimensional audio embeddings from high-energy (loudest) 10-second audio windows to ensure silent intros do not pollute vector indices.
7. **Description Embeddings**: Encodes local description tags using the `all-MiniLM-L6-v2` ONNX model for natural language semantic search.

---

## Non-Destructive Caching (.dc.json Sidecars)

To protect pristine audio files from metadata modification, Deep Cuts writes computed analysis results into lightweight `.dc.json` sidecar files stored next to each track (optional, toggled via Settings).
* **Relocation Safety**: If files are moved or directories renamed, re-indexing is near-instantaneous as the application reads existing sidecars instead of re-running the CPU-intensive analysis pipeline.

</div>
