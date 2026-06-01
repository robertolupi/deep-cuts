# Technical Evaluation: Sonic DNA & Multimodal QA

## 1. Feature Overview & User Experience
The Sonic DNA and Multimodal QA suite represents research-level, cutting-edge audio intelligence capabilities:
* **Sonic DNA & DTW (Structural Timbral Matching)**: Analyzes a song’s timbral evolution over its entire duration (not just static midpoints) using sliding-window CLAP extractions. Plots a continuous "Sonic DNA Timeline" showing when intros, drops, vocal entries, or transitions occur. Dynamic Time Warping (DTW) mathematically aligns timelines of different songs to spot cross-track similarities (e.g. *intro of Track A matches bridge of Track B*).
* **Acoustic EQ Prefiltering**: Isolates specific frequency bands (Low-Pass Filter <150Hz for sub-bass, High-Pass Filter >2kHz for percussion hats/snare, Band-Pass Filter 300Hz-3kHz for vocals) before generating spectrograms, letting users compare songs *strictly* by drum groove, percussive swing, or vocal texture.
* **Multimodal Chat QA ("Chat with Your Music")**: A native sidebar chatbot where the user selects tracks and asks natural language questions (e.g., *"What is the emotional delivery of the singer in this recording?"* or *"Compare the tension build of these two samples"*). Qwen2-Audio literally hears the music and streams conversational audio-text answers locally.

## 2. Technical Feasibility & Architecture

### A. Database Changes
* **Database Schema**: Massive changes. Storing a continuous sequence of 512-d vectors requires a new virtual table `sonic_dna_embeddings(track_id INTEGER, window_index INTEGER, embedding float[512], PRIMARY KEY(track_id, window_index))`.
* **Database Size Bloat**: A 5-minute song analyzed in 10-second windows with 5-second steps generates **60 embeddings** (instead of just 1 global embedding). For a library of 10,000 tracks, this multiplies database storage sizes by **20x to 50x**, adding gigabytes of database overhead.
* **int8 Quantization & Dimension Compression**: To prevent database bloat and keep sliding-window vector footprints compact, we implement int8 quantization (mapping floats to [-128, 127]) and principal component dimensionality reduction (compressing 512-d to 128-d). This reduces database overhead by up to 16x without significant loss in DTW alignment accuracy.

### B. Rust Backend Services
* **Sliding Window Extractor**: Extend [embeddings.rs](../../src-tauri/src/embeddings.rs) to split decodings into sequential 10-second buffers, feeding each to the CLAP encoder sequentially.
* **DSP EQ Filters**: Implement standard digital signal processing IIR/FIR filters (Butterworth low-pass, high-pass, and band-pass) in [dsp.rs](../../src-tauri/src/dsp.rs) to pre-filter audio buffers.
* **DTW Alignment Engine**: Write a Dynamic Time Warping matrix-search algorithm in Rust to align vector sequences of different lengths. We employ a **Coarse-to-Fine DTW Alignment** approach to speed up sequential sequence matching in Rust. By downsampling the timelines to perform a coarse matching pass first, we filter out poor alignment candidates and only run the full, fine-grained DTW matrix search on promising matches, reducing computational complexity from $O(N \cdot M)$ to $O(N + M)$ on average.
* **Multimodal QA Completions Handler**: Inside `llama.rs` and `analysis.rs`, implement a conversational completions coordinator. Since Qwen2-Audio has a specific multimodal prompt structure, we must compile chat history, slice target audio sections into WAV format, encode them as Base64, package them in the LLM completions payload, and stream the text response back.

### C. Svelte Frontend Controls
* **Multimodal Chat Sidebar**: A chat dialogue box in Svelte 5 with interactive voice-input and text capabilities.
* **Sonic DNA waveform visualizer**: Renders colored gradient lines on the timeline representing timbral changes.
* **Timeline Alignment Auditioning**: An interactive interface that overlays matched timelines, enabling users to click, drag, and audition aligned segments of different tracks (e.g. comparing the intro of Track A directly with the bridge of Track B) in real-time.
* **Token/VRAM Budget Indicators**: A dynamic visual budget indicator that estimates LLM context token usage and VRAM consumption prior to executing conversational Q&A queries, warning users when multi-audio QA files risk exhausting GPU capacity.

## 3. Implementation Roadmap & Sizing
* **Phase 1: Core Backend & Data Models**: 7.0 dev-days (sliding window CLAP database structure, DSP IIR filters, and Rust DTW sequence alignment).
* **Phase 2: Svelte Interface & Visual Layers**: 7.0 dev-days (Multimodal chat coordinator, Base64 audio slicers, conversational memory managers, and chat UI).
* **Phase 3: Polish, Edge Cases, & Tests**: 4.0 dev-days (optimizing heavy DB lookups, preventing GPU out-of-memory crashes on large audio buffers).
* **Total Estimated Dev-Time**: 18.0 dev-days

## 4. Performance & Resource Impact
* **CPU / GPU Overhead**: High. Running sliding window CLAP on a track multiplies analysis times by **20x**. Running Qwen2-Audio chat completions on multiple audio files causes **massive GPU memory spikes** and can lock up lower-end MacBooks.
* **Memory Footprint**: High. Loading multiple large audio chunks and caching intermediate LLM context windows can exceed 4GB of active RAM.
* **Database Size Impact**: Extreme. Increases library SQLite databases by **20x to 50x** due to sequential vector storage.

## 5. Technical Uncertainty & Risk Analysis
* **Risk Level**: High.
* **Llama-Server Memory Failures**: Feeding multiple 30s audio blocks into local Qwen2-Audio (`llama-server`) context windows often causes context-length overflow or outright server crashes if GPU VRAM is exhausted.
* **DTW Performance Bottleneck**: Comparing one track's Sonic DNA against thousands of others via DTW ($O(N \cdot M)$ time complexity) represents a severe CPU bottleneck. We must run pre-filtering heuristics (e.g. standard CLAP first) to reduce the candidate list to 10 tracks before running DTW.

## 6. Scoring Matrix & Priority
* **Effort Score**: 9 / 10 (18.0 dev-days of complex, research-level engineering)
* **Uncertainty Score**: 8 / 10 (high risk of VRAM exhaustion, complex sequence math, and untested DTW constraints)
* **Performance Impact Score**: 8 / 10 (sustained GPU/CPU blockage, massive database bloat)
* **Wow Factor Score**: 10 / 10 (absolute "science fiction" capability that completely wows the user)
* **Priority Score**: 5.5 / 10 (blended rating)

### Scoring Rationale
While this is a jaw-dropping, futuristic capability, the technical complexity, massive database bloat, and high risk of GPU crashes on M-series chips make it a research-level project. It should be deferred to a later phase once all Category A and B features are fully polished.
