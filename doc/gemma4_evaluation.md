# Evaluation: Gemma 4 E2B-it for Local Chat & Audio

This document evaluates the feasibility and architectural advantages of replacing our current **Qwen2-Audio-7B-Instruct** chat model with Google's newly released **Gemma 4 E2B-it** (Effective 2B Instruction Tuned) model.

---

## Model Comparison

| Dimension | Current: Qwen2-Audio-7B-Instruct | Proposed: Gemma 4 E2B-it |
| :--- | :--- | :--- |
| **Active Parameters** | 7 Billion | **2.3 Billion** (5.1B total, PLE architecture) |
| **Model Size (Q4)** | ~4.7 GB GGUF + ~2.2 GB MMProj | **~3.2 GB** (QAT optimized GGUF) |
| **Min. RAM/VRAM** | ~7.5 GB | **~3.5 GB** |
| **Context Window** | 32K | **128K** |
| **Architecture** | Dual-file: Encoder-based (Requires GGUF + MMProj projector) | **Unified, Encoder-free** (Single GGUF with PLE projections) |
| **Target Hardware** | Consumer GPUs / Apple M-series (16GB+ RAM) | Edge devices / Laptops / Mobile (8GB+ RAM) |

---

## Architectural Advantages of Gemma 4 E2B-it

### 1. Significant Memory Reduction
For a desktop application like `deep-cuts` running alongside standard user tasks, the memory footprint is critical. 
* **Qwen2-Audio** consumes ~7.5 GB of VRAM, frequently causing out-of-memory failures or extreme swap lag on 8GB Macs and budget Windows laptops.
* **Gemma 4 E2B** fits comfortably inside ~3.5 GB of VRAM, making local background execution much safer and leaving room for the Tauri/Webview process and system resources.

### 2. Unified Single-File Deployment
* Currently, our `llama.rs` launcher must resolve, verify, and load two distinct files: `Qwen2-Audio-7B-Instruct.Q4_K_M.gguf` and `Qwen2-Audio-7B-Instruct.mmproj-Q8_0.gguf`.
* Gemma 4's unified architecture embeds visual/audio tokens directly into the LLM projection space. This means we only need to manage, download, and load a **single `.gguf` file**, simplifying our Rust backend launch sequence.

### 3. Expanded Context Window
* Qwen2-Audio has a relatively small context budget when long audio inputs are attached.
* Gemma 4's **128K context window** allows us to pass much longer chat histories alongside the 10-second (or longer) track WAV attachments without running out of tokens.

---

## Multimodal Compatibility in `llama.cpp`

Under the hood, `deep-cuts` uses a bundled `llama-server` sidecar to handle chat requests. We must verify how `llama-server` handles Gemma 4's unified audio format:

1. **Native Waveform Projection**:
   Unlike standard visual models that use `clip-style` projectors (`--mmproj`), Gemma 4 E2B projects raw audio waveforms directly.
2. **Endpoint Compatibility**:
   We need to verify if `llama-server`'s `/v1/chat/completions` API accepts the same `input_audio` payload format:
   ```json
   { "type": "input_audio", "input_audio": { "data": "<base64>", "format": "wav" } }
   ```
   If it does, the migration requires zero changes to our Rust message construction logic.

---

## Evaluation & Testing Plan

To evaluate Gemma 4 E2B-it locally without modifying the production code, we will use the following protocol:

### Step 1: Download the QAT GGUF
We will download the community-quantized or official Gemma 4 E2B GGUF to a temporary directory in the workspace.

### Step 2: Spawn a Standalone `llama-server`
Run the bundled sidecar server manually on port `8080` targeting the Gemma 4 model:
```bash
./src-tauri/binaries/llama-server-aarch64-apple-darwin -m models/gemma-4-E2B-it.Q4_0.gguf
```
*(Verify if llama-server requires any specific multimodal parameters or flags for Gemma 4).*

### Step 3: Test Audio Transcription & Reasoning
Send a test cURL request attaching a 10-second WAV sample from the library to `/v1/chat/completions` and verify:
1. **Transcription Accuracy**: How well it transcribes spoken vocals or lyrics.
2. **Musical / Acoustic Reasoning**: Test its ability to identify genre, instruments, and mood from the audio compared to Qwen's baseline outputs.
3. **Latency / Speed**: Measure Time-to-First-Token (TTFT) and tokens-per-second.
