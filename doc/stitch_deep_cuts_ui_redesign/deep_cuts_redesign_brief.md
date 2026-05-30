# Deep Cuts: UI Redesign & Product Brief

## Project Overview
Deep Cuts is an offline-first studio audio analysis application designed to reveal the underlying musical structure of local audio collections. This redesign transitions the app from a compressed, tab-based layout to a professional, multi-pane desktop interface optimized for "crate digging" and AI-driven sonic exploration.

## Core Vision
To move from a "utility tool" to an "immersive audio workspace" where discovery, analysis, and playback are unified in a persistent, high-fidelity environment.

---

## 🏗️ Architectural Redesign (The Four Zones)

### 1. Filter Sidebar (Left)
A collapsible control center for library faceted search.
- **Search & Semantic NLP**: Toggle between traditional text search and semantic vibes (e.g., "heavy bass with organic synths").
- **Facets**: Genre clouds, Camelot Key grid, BPM RangeSlider, and AI-derived Energy/Mood axes.

### 2. Main Content Area (Central)
A shared-state viewport with instant mode-switching.
- **Table View**: High-density tracklist with inline waveforms.
- **Map View (UMAP)**: Immersive 2D acoustic projection. Filter states dim out-of-filter tracks rather than hiding them to preserve spatial context.
- **Analysis View**: Real-time visualization of the DSP pipeline (BPM, Key, Loudness, Neural Embeddings).

### 3. Track Detail Pane (Right)
A deep-dive metadata panel that eliminates the need for modal overlays.
- **Rich Data**: AI-generated prose descriptions, mood score bars, and technical file specs.
- **Acoustic Proximity**: A "Sounds vs. Feels" blending slider to find similar tracks based on mathematical DSP similarity vs. semantic metadata.

### 4. Persistent Player Bar (Bottom)
Always-on playback controls and visualization.
- **Visuals**: Stacked Waveform and Spectrogram (toggleable).
- **Control**: Play/Pause, Skip, Volume, and "Reveal in Finder" quick actions.

---

## ✨ Design System: "Sonic Glitch"
- **Theme**: Dark Mode (Glassmorphism).
- **Color Palette**: 
  - `Surface`: Deep Indigo (#121318)
  - `Primary`: Cyber Cyan (#00f0ff)
  - `Accent`: Studio Pink (#ff00ff)
- **Typography**: Inter (UI), Mono-variant for technical data.
- **Visual Style**: Translucent glass containers, high-contrast glow states, and studio-grade border treatments.

---

## 🛠️ Technical Stack
- **Frontend**: Svelte 5 (Runes), SvelteKit (SPA), Vite.
- **Backend**: Tauri v2 (Rust).
- **DSP/ML**: Symphonia (Audio), Lofty (Tags), Rusqlite-vec (Vector Search), ONNX Runtime (CLAP/Essentia models).
- **Rendering**: WaveSurfer.js, D3.js (Music Map), HTML Canvas.

---

## 🚀 Roadmap Features
- [ ] **DJ Vibe Drift**: Track the "energy curve" of a selection or playlist.
- [ ] **Pathfinding Playlist Builder**: AI-generated transition paths between two distant tracks on the Music Map.
- [ ] **Offline Semantic Search**: Local ONNX query encoding.
- [ ] **Export Sidecars**: Bulk-writing DC.JSON metadata for portability.
