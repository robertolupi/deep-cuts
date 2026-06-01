#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MODELS_DIR="$SCRIPT_DIR/../models"

MP3="$1"
shift
QUESTION="$*"

TMP=$(mktemp /tmp/feedback_XXXXXX.mp3)
trap 'rm -f "$TMP"' EXIT
cp "$MP3" "$TMP"

llama-mtmd-cli \
  -m "$MODELS_DIR/Qwen2-Audio-7B-Instruct.Q4_K_M.gguf" \
  --mmproj "$MODELS_DIR/Qwen2-Audio-7B-Instruct.mmproj-Q8_0.gguf" \
  --audio "$TMP" \
  -p "$QUESTION" 2>/dev/null
