# Changelog

All notable changes to Deep Cuts will be documented here.

## [Unreleased] — 0.1.0

Initial public release.

### Features

- Recursive library scanner with sidecar (`.dc.json`) persistence
- Audio analysis pipeline: BPM (with correction and refinement), key, loudness (EBU R128), waveform, duration
- Essentia ONNX classifier: 400-class Discogs genre, vocal/instrumental detection, seven mood axes
- CLAP audio embeddings (512-dim) stored in `sqlite-vec` for KNN similarity search
- Qwen2-Audio-7B local descriptions via `llama-server`
- all-MiniLM-L6-v2 description embeddings for semantic text search
- Music Map: UMAP 2D projection of CLAP embeddings, D3 zoom/pan, filter-aware, KNN inspection pane
- Full-text and faceted filtering: genre, key, BPM range, vocals, folder, similarity
- WaveSurfer.js waveform and spectrogram player with track detail pane
- Duplicates view: CLAP-based similarity detection with clickable track rows
- Sonic Glitch design system with dark, light, and accessible high-contrast themes
- macOS sandbox-compatible, fully offline
