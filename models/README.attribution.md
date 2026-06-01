# Deep Cuts Models

This repository hosts pre-trained models for the open-source [Deep Cuts](https://github.com/robertolupi/deep-cuts) desktop audio management application.

These files are ONNX exports of official pre-trained models released by **LAION** and **sentence-transformers**, generated from the source checkpoints using the export scripts in `tools/` of the Deep Cuts repository. No modifications have been made to the underlying weights, parameters, or architectures.

---

## 📦 Model Inventory

| File | Purpose | Original Source | Size |
|---|---|---|---|
| `clap_audio_encoder.onnx` | CLAP audio encoder (structure) | `laion/clap-htsat-unfused` | ~2 MB |
| `clap_audio_encoder.onnx.data` | CLAP audio encoder (weights) | `laion/clap-htsat-unfused` | ~112 MB |
| `clap_text_encoder.onnx` | CLAP text encoder (structure) | `laion/clap-htsat-unfused` | ~1 MB |
| `clap_text_encoder.onnx.data` | CLAP text encoder (weights) | `laion/clap-htsat-unfused` | ~479 MB |
| `clap-tokenizer.json` | CLAP text tokenizer | `laion/clap-htsat-unfused` | ~2 MB |
| `all-minilm-l6-v2.onnx` | MiniLM sentence encoder (structure) | `sentence-transformers/all-MiniLM-L6-v2` | ~756 KB |
| `all-minilm-l6-v2.onnx.data` | MiniLM sentence encoder (weights) | `sentence-transformers/all-MiniLM-L6-v2` | ~86 MB |
| `all-minilm-l6-v2-tokenizer.json` | MiniLM tokenizer | `sentence-transformers/all-MiniLM-L6-v2` | ~456 KB |

---

## ⚖️ License

All models in this repository are distributed under the **Apache License 2.0**.

You are free to use, reproduce, distribute, and create derivative works, including for commercial purposes, subject to the terms of the Apache 2.0 license. A copy of the license is available at:

> https://www.apache.org/licenses/LICENSE-2.0

---

## ✍️ Attribution

### CLAP — Contrastive Language-Audio Pretraining

Original model weights are the property of **LAION**.

- *Original checkpoint:* [`laion/clap-htsat-unfused`](https://huggingface.co/laion/clap-htsat-unfused)
- *Paper:*
  > Wu, Y., Chen, K., Zhang, T., Hui, Y., Berg-Kirkpatrick, T., & Dubnov, S. (2023). Large-Scale Contrastive Language-Audio Pretraining with Feature Fusion and Keyword-to-Caption Augmentation. In *IEEE International Conference on Acoustics, Speech and Signal Processing (ICASSP 2023)*.

### all-MiniLM-L6-v2 — Sentence Embeddings

Original model weights are the property of **sentence-transformers / Microsoft**.

- *Original checkpoint:* [`sentence-transformers/all-MiniLM-L6-v2`](https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2)
- *Original base model:* [`microsoft/MiniLM-L6-H384-uncased`](https://huggingface.co/microsoft/MiniLM-L6-H384-uncased)
