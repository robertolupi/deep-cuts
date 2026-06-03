#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MODELS_DIR="$SCRIPT_DIR/../models"
BINARIES_DIR="$SCRIPT_DIR/../src-tauri/binaries"
SERVER="$BINARIES_DIR/llama-server-aarch64-apple-darwin"

if [ "$#" -lt 3 ]; then
  echo "Usage: $0 <qwen|gemma4> <audio_path> <question>" >&2
  exit 1
fi

MODEL_TYPE="$1"
AUDIO="$2"
shift 2
QUESTION="$*"

MMPROJ_OPT=""
if [ "$MODEL_TYPE" = "qwen" ]; then
  MODEL_FILE="Qwen2-Audio-7B-Instruct.Q4_K_M.gguf"
  MMPROJ_FILE="Qwen2-Audio-7B-Instruct.mmproj-Q8_0.gguf"
  MMPROJ_OPT="--mmproj $MODELS_DIR/$MMPROJ_FILE"
elif [ "$MODEL_TYPE" = "gemma4" ]; then
  MODEL_FILE="gemma-4-12B-it-Q4_K_M.gguf"
  MMPROJ_FILE="mmproj-gemma-4-12B-it-Q8_0.gguf"
  MMPROJ_OPT="--mmproj $MODELS_DIR/$MMPROJ_FILE"
else
  echo "ERROR: Unknown model type '$MODEL_TYPE'. Choose 'qwen' or 'gemma4'." >&2
  exit 1
fi

# Pick a free port
PORT=$(python3 -c 'import socket; s=socket.socket(); s.bind(("",0)); print(s.getsockname()[1]); s.close()')

SERVER_PID=""
cleanup() {
  [[ -n "$SERVER_PID" ]] && kill "$SERVER_PID" 2>/dev/null && wait "$SERVER_PID" 2>/dev/null || true
}
trap cleanup EXIT

BOOT_START=$(date +%s)
DYLD_LIBRARY_PATH="$BINARIES_DIR" \
  "$SERVER" \
  -m "$MODELS_DIR/$MODEL_FILE" \
  $MMPROJ_OPT \
  --port "$PORT" \
  --host 127.0.0.1 \
  >/tmp/feedback_server.log 2>&1 &
SERVER_PID=$!

# Wait for the server to become healthy (up to 120 s)
echo "Waiting for llama-server ($MODEL_TYPE) on port $PORT..." >&2
for i in $(seq 1 120); do
  if curl -sf "http://127.0.0.1:$PORT/health" >/dev/null 2>&1; then
    break
  fi
  sleep 1
done

if ! curl -sf "http://127.0.0.1:$PORT/health" >/dev/null 2>&1; then
  echo "ERROR: llama-server did not become healthy. Check /tmp/feedback_server.log" >&2
  exit 1
fi
BOOT_END=$(date +%s)
echo "Server booted in $((BOOT_END - BOOT_START))s." >&2

# Decode audio to 16 kHz mono WAV (max 30s) and build payload
PAYLOAD_FILE=$(mktemp /tmp/feedback_payload_XXXXXX)
trap 'rm -f "$PAYLOAD_FILE"; cleanup' EXIT

python3 - <<PYEOF
import base64, json, subprocess, tempfile, os

wav_tmp = tempfile.mktemp(suffix=".wav")
# Cap the output at 30 seconds using -t 30 to stay within the context window
subprocess.run(
    ["ffmpeg", "-y", "-i", "$AUDIO", "-ar", "16000", "-ac", "1", "-t", "30", "-f", "wav", wav_tmp],
    check=True, capture_output=True,
)

with open(wav_tmp, "rb") as f:
    b64 = base64.b64encode(f.read()).decode()
os.unlink(wav_tmp)

payload = {
    "messages": [{
        "role": "user",
        "content": [
            {"type": "input_audio", "input_audio": {"data": b64, "format": "wav"}},
            {"type": "text", "text": "$QUESTION"}
        ]
    }],
    "temperature": 0.2
}

with open("$PAYLOAD_FILE", "w") as f:
    json.dump(payload, f)
PYEOF

echo "Sending completions request..." >&2
REQ_START=$(date +%s)
RESPONSE=$(curl -sf "http://127.0.0.1:$PORT/v1/chat/completions" \
  -H "Content-Type: application/json" \
  -d "@$PAYLOAD_FILE")
REQ_END=$(date +%s)

echo "--- Response ---"
echo "$RESPONSE" | jq -r '.choices[0].message.content'
echo "----------------"
echo "API Request took $((REQ_END - REQ_START))s." >&2
