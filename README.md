# Deep Cuts (com.rlupi.deep-cuts)

Deep Cuts is an offline-first studio audio analysis application and reference library. It cuts through thousands of tracks in your local collection to reveal their underlying musical and sonic structure using local digital signal processing (DSP) and offline machine-learning indexing.

100% offline, private, and designed to run sandboxed on macOS.

---

## ✨ Features

### Implemented

*   **Library Indexing**: Recursively scans watched directories for audio files (MP3, FLAC, WAV, AIFF, M4A, and more). Reads embedded metadata tags (title, artist, album, BPM, key, year, lyrics, etc.) via `lofty`. Gracefully re-indexes changed files and skips unreadable ones.
*   **Sidecar Persistence** (`.dc.json`): Writes a JSON sidecar file next to each audio file containing all computed analysis results. Sidecars are restored automatically on re-index, so analysis work survives library moves and re-imports. The Export Sidecars command bulk-writes all tracks at once.
*   **Audio Analysis Pipeline**: A concurrent, spool-based analysis engine that processes the library in parallel using `num_cpus / 2` worker threads. A single Symphonia decode pass per file computes:
    *   **BPM** — spectral-flux onset envelope with autocorrelation and parabolic sub-sample refinement (40–210 BPM range, 80–160 BPM preference).
    *   **Key & Scale** — chromagram built via FFT, HPCP-style harmonic suppression, and Krumhansl-Schmuckler profile correlation.
    *   **Loudness** — integrated loudness (LUFS) and loudness range (LRA) via EBU R128 using `ebur128`.
    *   **Waveform** — 128-point RMS energy profile for fast visual rendering.
    *   **Duration** — derived from container metadata with a sample-count fallback.
*   **Analysis UI**: Dedicated Analysis tab with per-pass progress bars, average timing, failed-track error log, and per-pass / full-library reset controls.
*   **Dashboard**: Split-pane layout with a resizable divider. Top pane shows the audio player; bottom pane shows the searchable, filterable track list with BPM and key columns.
*   **Audio Player**: WaveSurfer.js waveform and spectrogram visualisation, play/pause/prev/next controls, and an expandable metadata details panel showing technical specs, key, loudness, lyrics, and comments.
*   **Search & Filter**: Real-time full-text search across title, artist, album, and filename. Features dynamically populated Genre and musical Key filters, and a popover-based BPM range selector (RangeSlider) with quick-presets and click-outside closure.
*   **The Music Map (UMAP Projection)**: 2D visual projection of the entire audio collection using CLAP embeddings and Rust-native UMAP dimensionality reduction. Rendered dynamically via a theme-adaptive Svelte 5 canvas element supporting D3 zoom/pan, hover metadata tooltips, and a dynamic top-10 primary genre scanning system.
*   **Acoustic Similarity Search (K-Nearest Neighbors)**: Index-based KNN similarity queries using virtual `audio_embeddings` tables via `sqlite-vec` to instantly find matching audio profiles on the Music Map's inspection pane, with native audio playback and progress scrubbing controls.
*   **Reveal in Finder / Explorer**: Opens the system file manager with the track's file selected. macOS, Windows, and Linux are all handled.
*   **Visual Themes**:
    *   **Dark Mode**: Cyber-cyan, studio-pink, and deep-indigo glow interface.
    *   **Light Mode**: Clean, professional bright slate/indigo studio theme.
    *   **Accessible Mode**: High-contrast black-and-white theme with stark borders and zero panel blurs.

### Planned

*   **Offline Semantic Text Search**: Locally run ONNX-based CLAP text query encoding (e.g. searching "ambient synths" or "heavy bassline") to retrieve matching audio files.
*   **Genre & Mood Classification**: Discogs-Effnet ONNX classifier for genre, vocal/instrumental detection, and seven mood axes.

---

## 🛠️ Technology Stack

*   **Frontend**: Svelte 5, static SvelteKit (SPA mode), Vite, vanilla CSS.
*   **Backend**: Tauri v2 (Rust).
*   **Storage & Vector Search**: SQLite (`rusqlite`) and `sqlite-vec` for local vector embeddings.
*   **Audio DSP & Tagging**: `lofty` and `symphonia` for audio decoding and tag parsing.
*   **Machine Learning**: `ort` (ONNX Runtime) for local, private model inference.

---

## 🚀 Development & Build

### Prerequisites

Ensure you have [Rust](https://www.rust-lang.org/) and [Node.js](https://nodejs.org/) installed.

### Installing Dependencies

Install both root monorepo and SvelteKit dependencies:

```bash
npm install
```

### Running in Development

Boot the Tauri dev shell and Svelte static server concurrently:

```bash
npm run tauri dev
```

### Building for Production

Compile the static SPA pages and bundle the sandboxed desktop application:

```bash
npm run tauri build
```

---

## ⚖️ Licensing

Deep Cuts is open-source software licensed under the **GNU Affero General Public License v3.0 (AGPLv3)**. See [LICENSE.md](LICENSE.md) for details.
