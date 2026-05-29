---
name: check-prototype
description: Guidelines for researching the music-intelligence prototype in ~/src/music-intelligence
---

# Check Prototype Skill

This skill provides instructions for researching the `music-intelligence` project (`~/src/music-intelligence`, fully resolved to `/Users/rlupi/src/music-intelligence`), which deep-cuts is a clean-room reimplementation of.

## Context & Purpose

`music-intelligence` is the prior Tauri/Rust/Svelte 5 desktop app for managing audio collections with ML enrichment. When the user asks to "check the prototype" or "check music-intelligence", explore its files and apply relevant patterns to deep-cuts.

## Instructions

1. **Locate the Prototype**:
   * Path: `/Users/rlupi/src/music-intelligence`

2. **Core Components to Analyze**:
   * **IPC Commands**: `src-tauri/src/lib.rs` — command handlers, managed state, `generate_handler![]`
   * **Database**: `src-tauri/src/database.rs` — schema migrations, `Track` struct, `setup_test_db`
   * **Scanner**: `src-tauri/src/scanner.rs` — recursive audio file scanner, sidecar `.mi.json` pattern
   * **ML Passes**: `src-tauri/src/` — `classifier.rs` (ONNX), `embeddings.rs`, `dsp.rs`, `spectrogram.rs`
   * **Frontend**: `frontend/src/` — Svelte 5 components, TypeScript track interfaces
   * **Skills**: `skills/` — task-specific SKILL.md playbooks

3. **Key differences from deep-cuts**:
   * music-intelligence has a `frontend/` subdirectory; deep-cuts has frontend at root
   * music-intelligence uses `python run_dev.py`; deep-cuts uses `npm run tauri`
   * music-intelligence bundle ID: `com.rlupi.music-intelligence`; deep-cuts: `com.rlupi.deep-cuts`
   * music-intelligence DB: `music_intelligence.db`; deep-cuts: `deep_cuts.db`
   * music-intelligence has full ML pipeline (CLAP, Qwen, embeddings, pass scheduler); deep-cuts is being built up

4. **Comparison & Porting**:
   * Port algorithms, patterns, and UI components as needed.
   * Adapt path references, struct names, and identifiers to deep-cuts conventions.
   * The original prototype (pre-music-intelligence) is also available at `/Users/rlupi/src/music-index` for older reference code.
