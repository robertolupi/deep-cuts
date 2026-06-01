# In-App Model Download Flow

## Background

Deep Cuts relies on four neural network model groups (~6.3 GB total) that are not
bundled with the application binary. Currently the only way to get them is to run
`python3 tools/download_models.py` manually from the terminal. The app detects
missing models and shows a warning banner, but offers no in-app remedy.

This document describes a design for fetching, verifying, and managing model files
entirely from within the app.

---

## Goals

- Users can download all required models without leaving the app or touching a terminal.
- The app remains usable (for non-ML passes) while models are absent or downloading.
- Model metadata (URLs, checksums, sizes) lives in the repository and is fetched at
  runtime — no server costs, no hardcoded URLs in the binary.
- A compiled-in fallback ensures the app works offline or when GitHub is unreachable.

---

## UX: Where and How Downloads Are Triggered

### No blocking onboarding wizard

A wizard is heavy for a power-user tool and unnecessary because the app already works
partially without models. Instead, downloads are triggered on demand from two surfaces:

**AnalysisPanel** — discovery surface  
The existing "Missing neural network model files" warning gains a "Download Models"
button alongside the current "Re-check" and "Proceed anyway" actions. This is where
most users will first notice the gap.

**LibrarySettings** — configuration surface  
The existing "Model Folder" card gains a "Download Models" button. The user picks
where models are stored here, so it is the logical place to also initiate the fetch.

Both surfaces invoke the same Tauri commands and share a `ModelDownloader.svelte`
component that renders per-model progress bars.

### Download selection

A simple checklist lets the user choose which model groups to download (default: all
missing). Groups are labelled with their disk footprint:

| Group | Size |
|---|---|
| Qwen Audio LLM | ~5.0 GB |
| CLAP Embedder | ~0.4 GB |
| MiniLM Text Embedder | ~0.1 GB |
| Essentia Classifier | ~0.8 GB |

---

## Graceful Degradation

Passes are divided by their model dependency. Data dependencies between passes (from
`PassSpec::dependencies`) also determine what can run usefully when a model group
is absent:

| Pass | Model files required | Data depends on |
|---|---|---|
| audio_analysis | none | — |
| bpm_correction | none | audio_analysis |
| clap | CLAP files | audio_analysis |
| qwen | Qwen GGUF + mmproj | audio_analysis, bpm_correction |
| description_embed | MiniLM ONNX + tokenizer | qwen |
| essentia | Essentia head files | audio_analysis |
| bpm_refinement | none | essentia |

Key implications for skipping:
- If **CLAP** models are absent, the clap pass is skipped. No downstream data
  dependency is affected (no other pass reads CLAP output).
- If **Qwen** models are absent, qwen is skipped — and `description_embed` must also
  be skipped because it has no text to embed (its data dependency on qwen is unmet).
- If **Essentia** models are absent, essentia is skipped — and `bpm_refinement` must
  also be skipped because it depends on Essentia output.
- If **MiniLM** models are absent but Qwen ran, `description_embed` is skipped.
  (Rare in practice since Qwen is far larger and more likely to be missing too.)
- `audio_analysis`, `bpm_correction`, and `bpm_refinement` (when essentia is present)
  require no model files and always run.

**Pipeline behaviour**: `run_analysis_pipeline` checks model presence at startup,
resolves the skip set (including transitive data-dependency skips), and omits those
passes from the run queue. It emits an `analysis-skipped-passes` event listing the
skipped pass names so the frontend can surface them.

**Pass card UI**: passes whose models are missing show an amber `⚠ models missing`
chip (mirroring the existing cyan `processing` tag). The chip links to the download
flow. Passes without model requirements are unaffected and run normally.

---

## Manifest File

### Location and distribution

A manifest file lives in the repository at `models/manifest.json`. It is fetched at
runtime from GitHub raw content:

```
https://raw.githubusercontent.com/<owner>/deep-cuts/main/models/manifest.json
```

No server is needed. GitHub's CDN serves the file; the only cost is egress from the
model hosting source (e.g. HuggingFace, direct URLs).

A copy of the manifest is compiled into the binary with Rust's `include_str!` macro as
a fallback for offline use or when GitHub is unreachable.

### Schema

Model groups map to an array of `files`. Each file is a single URL → filename
mapping with its own checksum and size. This handles models that are distributed
as multiple separate files (e.g. Essentia's per-task head files, or Qwen's split
GGUF shards if applicable) without assuming anything about bundling.

```json
{
  "manifest_version": 1,
  "min_app_version": "0.1.0",
  "update_notice": null,
  "models": {
    "qwen": {
      "label": "Qwen Audio LLM",
      "files": [
        { "filename": "qwen2-audio-7b-instruct-q4_k_m.gguf", "url": "https://...", "sha256": "...", "size_bytes": 4700000000 },
        { "filename": "qwen2-audio-mmproj.gguf",              "url": "https://...", "sha256": "...", "size_bytes": 300000000  }
      ]
    },
    "clap": {
      "label": "CLAP Embedder",
      "files": [
        { "filename": "clap_audio_encoder.onnx", "url": "...", "sha256": "...", "size_bytes": 0 },
        { "filename": "clap_mel_weights.bin",     "url": "...", "sha256": "...", "size_bytes": 0 }
      ]
    },
    "sentence": {
      "label": "MiniLM Text Embedder",
      "files": [
        { "filename": "all-minilm-l6-v2.onnx", "url": "...", "sha256": "...", "size_bytes": 0 },
        { "filename": "tokenizer.json",         "url": "...", "sha256": "...", "size_bytes": 0 }
      ]
    },
    "essentia": {
      "label": "Essentia Classifier",
      "files": [
        { "filename": "discogs-effnet-bs64-1.pb",   "url": "...", "sha256": "...", "size_bytes": 0 },
        { "filename": "discogs-effnet-bs64-1.json",  "url": "...", "sha256": "...", "size_bytes": 0 },
        { "filename": "...",                         "url": "...", "sha256": "...", "size_bytes": 0 }
      ]
    }
  }
}
```

The model group keys (`qwen`, `clap`, `sentence`, `essentia`) align with the
`ModelExistence` fields already used by `check_models_exist` on the Rust side
(`qwen_exists`, `clap_exists`, etc.), making the mapping straightforward.

A group is considered fully present when every file in its `files` array passes
the SHA256 check. The download command iterates files within a group sequentially;
groups can be downloaded in parallel if the user selects multiple.

`update_notice` is a nullable string. When non-null the frontend shows a dismissible
chip: "A new version of Deep Cuts is available." This requires no server — just a
commit to the manifest.

---

## Rust Backend

### New commands

**`fetch_model_manifest`**  
Called once on app launch. Checks the database for a `manifest_last_fetched`
timestamp; if it is less than 24 hours old, returns the cached manifest JSON stored
in the `app_settings` table without making a network request. Otherwise attempts an
HTTP GET of the GitHub raw URL (5 s timeout), stores the result and updates the
timestamp on success, and falls back to the compiled-in `include_str!` constant on
any network or parse error. The result is cached in a Svelte store for the lifetime
of the session so all components share the same manifest without redundant IPC calls.

**`preflight_model_download(models: Vec<String>)`**  
Issues a `HEAD` request (with automatic redirect tracking enabled) for every file in the selected groups before any data is transferred. For each file, validates:
- HTTP 2xx status (URL reachable, not 404/403). If the `HEAD` request fails with a `405 Method Not Allowed` or similar CDN-specific rejection on presigned redirect URLs, the system falls back to a short `GET` request (e.g. using `Range: bytes=0-0`) to confirm reachability and metadata headers.
- `Content-Length`, if present, matches `size_bytes` in the manifest. A missing `Content-Length` (common with chunked CDNs such as HuggingFace) is not an error. A present but mismatched value means the manifest entry is stale or the CDN is serving the wrong file — blocks the automatic download.
- `Accept-Ranges: bytes` is present (confirms resume will work; warns but does not block if absent).
- **Disk Space Verification:** Performs a target volume check using system APIs to ensure the destination volume has enough free space to accommodate the combined size of the selected models, plus a 500 MB safety buffer.

Returns a preflight report: a list of files with `ok: bool`, an optional `error` string, disk space check status, and a `manual_url` for any file that fails. The frontend shows this as a confirmation step — "4 files reachable, 6.3 GB total, sufficient disk space. Proceed?" — and for any blocked file renders the `manual_url` as a clickable link with a "Download manually" label so the user can fetch it from a browser and place it in the model folder.

**`download_models(models: Vec<String>)`**  
Accepts a list of model group keys (e.g. `["qwen", "clap"]`). First checks standard Tauri managed `DownloadState` (or active flag) to guarantee that no other download thread is already active, rejecting duplicate triggers. For each file across the selected groups, sequentially:
1. Resolves the destination path using the configured model folder (or default app data directory).
2. Issues a `Range: bytes=<offset>-` request if a `.part` file already exists, otherwise a plain GET.
3. Streams the response to `<filename>.part` in the destination directory.
4. Emits `model-download-progress { model: String, file: String, bytes_done: u64, bytes_total: u64 }` at regular intervals (~100 ms or every 1 MB, whichever comes first).
5. On completion, verifies SHA256. On match, renames `.part` → final filename.
6. Emits `model-download-complete { model: String, file: String }` or `model-download-error { model: String, file: String, message: String }`.

Downloads run sequentially within the command (one file at a time) to avoid saturating the user's connection. The command itself is spawned async so the UI stays responsive.

**`cancel_model_download`**  
Sets a cancellation flag that the download loop checks between chunks. Partially downloaded `.part` files are left on disk intentionally — they are the resume anchor for the next download attempt (see Resume support in Notes).

**`check_pending_resume`**  
Called on app launch. Scans the model folder for any `.part` files and returns their names and byte offsets. If any are found the frontend prompts the user: "A previous download was interrupted. Resume?" — Yes resumes from the existing offset, No deletes the `.part` files and starts fresh.

### Modifications to existing commands

**`run_analysis_pipeline`**: before queuing passes, call `check_models_exist` and dynamically build the skip set by traversing the directed acyclic graph (DAG) of pipeline passes. Any pass whose required models are missing is added to the skipped set, and this propagates transitively to any downstream passes that rely on their data outputs (e.g., skipping `qwen` dynamically skips `description_embed`). Omit all skipped passes from the run queue and emit `analysis-skipped-passes { passes: Vec<String> }` so the UI can flag them.

---

## Frontend

### `ModelDownloader.svelte` component

Shared between AnalysisPanel and LibrarySettings. States:

1. **Idle / manifest not loaded** — "Download Models" button. Clicking fetches the
   manifest and transitions to the selection state.
2. **Selecting** — checklist of model groups with size labels and current status
   (present / missing). "Verify & Download" primary button.
3. **Preflight** — runs `preflight_model_download`; shows a per-file reachability
   check with a spinner. On success displays total size and "Proceed" / "Cancel".
   On any failure, shows the offending file and error without proceeding.
4. **Downloading** — per-file progress bars using the same visual language as the
   analysis pass cards (colored accent bar, byte count). "Cancel" button.
5. **Complete** — brief success state, then auto-calls `check_models_exist` to dismiss
   the warning banner if all models are now present.
6. **Error** — per-file error row with retry affordance.

The component is embedded inline (not a modal) in both panels so the user can see
download progress while still reading other UI.

### AnalysisPanel changes

- The "Download Models" button replaces the current `python3 tools/download_models.py`
  command-copy widget inside the warning banner.
- The warning banner expands vertically to accommodate the `ModelDownloader` component
  when a download is in progress.
- Pass cards for ML-dependent passes show an amber `⚠ models missing` chip when the
  corresponding model group is absent.

### LibrarySettings changes

- The "Model Folder" card gains a "Download Models" button that opens the
  `ModelDownloader` component below the folder path row.
- The model folder must be set before downloading (the button is disabled with a
  tooltip if no folder is configured).

---

## Out of scope for v1

- **Model updates**: the manifest `sha256` field enables detecting when a hosted
  model has been replaced. The UI could show an "update available" chip on affected
  pass cards. Deferred to v2.

---

## Integration & Isolated Testing Strategy

To verify all network operations (resumability, preflight checks, cancellation, and corruption handling) without downloading gigabytes of data or hitting Hugging Face servers, we implement an offline mock testing framework.

### 1. Standalone Mock HTTP Server
Tests spin up a lightweight, local mock HTTP server (e.g., using `wiremock` or a custom TCP listener thread). The base URL of the downloader is parameterized so that tests can route calls locally instead of to Hugging Face.

### 2. Test Fixtures (Low Footprint)
- **Mock Model File:** A small 10 KB file filled with static, predictable bytes is used in place of the massive actual model files.
- **Precomputed Hash:** The SHA256 checksum and exact byte size of the 10 KB file are hardcoded into a test manifest that replicates the official schema.

### 3. Key Test Scenarios
- **Scenario A: Preflight Reachability & Fallback:**
  - *Standard:* Server returns `200 OK` on `HEAD` request.
  - *Fallback:* Server returns `405 Method Not Allowed` on `HEAD`. Client falls back to `GET` with `Range: bytes=0-0` and successfully validates reachability.
  - *Disk Space Check:* Mock the disk utility to report lower than required free space. Verify preflight fails gracefully.
- **Scenario B: Resumability (Interrupted Stream):**
  1. *First Run:* Client downloads first 4 KB of the mock file, then triggers cancellation. We assert `.part` file exists and is exactly 4,096 bytes.
  2. *Second Run:* Client resumes with a `Range: bytes=4096-` header. Server responds with `206 Partial Content` and streams remaining 6 KB. Client appends, calculates SHA256, matches the precomputed checksum, and renames `.part` -> final.
- **Scenario C: No-Range Server support:**
  - Client attempts to resume by sending a `Range` header, but mock server returns `200 OK` (unsupported). Client truncates `.part` and successfully downloads the full 10 KB from scratch.
- **Scenario D: Checksum Corruption:**
  - Mock server streams modified bytes (corrupted download). Client downloads full size, fails the SHA256 check, deletes the corrupted file, and emits `model-download-error`.

---

## Notes

**Resume support** is required given the 4.7 GB Qwen file. The download command
issues a `Range: bytes=<offset>-` request when a `.part` file already exists on
disk, and the server either honours it (HTTP 206) or restarts from zero (HTTP 200).
SHA256 is verified only once, over the complete file, after the final byte lands.
This means the full `.part` file must be read end-to-end for hashing even on a
resumed download — acceptable given that verification happens once and the
alternative (incremental across sessions) would require storing intermediate hash
state. The `.part` → final rename only happens after SHA256 passes.

**`download_models` concurrency model**: implemented as a `tauri::async_runtime::spawn` call (not a blocking `#[tauri::command]`) so that `cancel_model_download` can set the cancellation flag from a separate IPC call while the download loop is running. To coordinate this safely and prevent duplicate parallel download triggers, standard Tauri managed state `DownloadState` (containing `is_running` and `cancel_flag` wrapped in `Arc<AtomicBool>`) is registered. The IPC command returns immediately; progress is communicated entirely via events.

