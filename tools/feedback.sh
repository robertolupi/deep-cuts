#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MODELS_DIR="$SCRIPT_DIR/../models"
BINARIES_DIR="$SCRIPT_DIR/../src-tauri/binaries"
SERVER="$BINARIES_DIR/llama-server-aarch64-apple-darwin"

AUDIO="$1"
shift
QUESTION="$*"

# Pick a free port
PORT=$(python3 -c 'import socket; s=socket.socket(); s.bind(("",0)); print(s.getsockname()[1]); s.close()')

# Start the bundled llama-server and kill it on exit
SERVER_PID=""
cleanup() {
  [[ -n "$SERVER_PID" ]] && kill "$SERVER_PID" 2>/dev/null && wait "$SERVER_PID" 2>/dev/null || true
}
trap cleanup EXIT

DYLD_LIBRARY_PATH="$BINARIES_DIR" \
  "$SERVER" \
  -m "$MODELS_DIR/Qwen2-Audio-7B-Instruct.Q4_K_M.gguf" \
  --mmproj "$MODELS_DIR/Qwen2-Audio-7B-Instruct.mmproj-Q8_0.gguf" \
  --port "$PORT" \
  --host 127.0.0.1 \
  >/tmp/feedback_server.log 2>&1 &
SERVER_PID=$!

# Wait for the server to become healthy (up to 120 s)
echo "Waiting for llama-server (pid $SERVER_PID) on port $PORT..." >&2
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

# Decode audio to 16 kHz mono WAV (same path as the app) and build the payload
PAYLOAD_FILE=$(mktemp /tmp/feedback_payload_XXXXXX.json)
trap 'rm -f "$PAYLOAD_FILE"' EXIT

python3 - <<PYEOF
import base64, json, subprocess, tempfile, os

wav_tmp = tempfile.mktemp(suffix=".wav")
subprocess.run(
    ["ffmpeg", "-y", "-i", "$AUDIO", "-ar", "16000", "-ac", "1", "-f", "wav", wav_tmp],
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
    }]
}

with open("$PAYLOAD_FILE", "w") as f:
    json.dump(payload, f)
PYEOF

# Send the completions request and print the response
curl -sf "http://127.0.0.1:$PORT/v1/chat/completions" \
  -H "Content-Type: application/json" \
  -d "@$PAYLOAD_FILE" \
| jq -r '.choices[0].message.content'
