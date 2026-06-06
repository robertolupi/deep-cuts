# Session: Gemma 4 Audio Evaluation
**Date:** 2026-06-06  
**Participants:** Roberto, Gemini, Claude, Meta  
**Goal:** Evaluate Google's Gemma 4 E2B-it model for local audio chat, using Python-based testing rather than our pinned `llama-server`.

---

## [Roberto, ~09:33]
Google released a smaller version of gemma that can handle audio. We already tested with Claude before and gemma 12B couldn't handle song well, but maybe we can use this one for chat functionality. I think it's worth trying to evaluate it.

Also, we want to test this from python, without using `llama-server` because we are keeping `llama-server-aarch64-apple-darwin` pinned due to a bug with Qwen.

---

## [Gemini, ~09:40]
1. **Model Analysis & Evaluation Doc**: I have researched Gemma 4 E2B-it and created `doc/gemma4_evaluation.md`. The model features a unified, encoder-free architecture (only a single GGUF needed), a 128K context window, and uses ~3.5 GB VRAM (active parameter size is 2.3B, compared to Qwen2-Audio's 7B parameter / ~7.5 GB VRAM footprint).
2. **Python Testing Script**: I wrote `tools/test_gemma4_audio.py` which provides two ways to run audio evaluation from Python without relying on the compiled `llama-server`:
   - **LiteRT-LM (`litert-lm` TFLite)**: Highly optimized for edge/mobile devices, uses Mac GPU for text/CPU for audio, and can be run with `uvx litert-lm`.
   - **Transformers (PyTorch pipeline)**: Uses the `any-to-any` Hugging Face pipeline on MPS/GPU.

**→ Handoff:** Meta, please review `doc/gemma4_evaluation.md` and `tools/test_gemma4_audio.py` in the repo. What are your thoughts on evaluating this model locally using Python? Do you foresee any issues with LiteRT-LM / TFLite performance on Apple Silicon, and are there specific audio samples we should test to check if E2B handles music better than 12B did?
