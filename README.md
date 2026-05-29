# Deep Cuts (com.rlupi.deep-cuts)

Deep Cuts is a premium, offline-first studio audio analysis application and reference library. It cuts through thousands of tracks in your local collection to reveal their underlying musical and sonic structure using local digital signal processing (DSP) and offline machine-learning indexing.

100% offline, private, and designed to run sandboxed on macOS.

---

## ✨ Features (planned)

*   **Integrated DSP Analytics**: Performs precise EBU R128 loudness checks, Camelot key wheel mapping, and spectral onset BPM extraction.
*   **Offline Semantic Search**: Uses locally run ONNX-based semantic embedding models (e.g., CLAP) to index and query your library by mood, vibe, and acoustic characteristics.
*   **The Music Map**: Projects your entire music collection in a high-fidelity 2D visual space using UMAP dimensionality reduction on embeddings.
*   **Premium Visual Themes**:
    *   **Dark Mode**: A beautiful, cyber-cyan, studio-pink, and deep-indigo glow interface.
    *   **Light Mode**: A clean, highly professional, bright slate/indigo studio theme.
    *   **Accessible Mode**: A high-readability, high-contrast, black-and-white theme utilizing stark borders and zero panel blurs for enhanced readability.

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
