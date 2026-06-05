#!/usr/bin/env bash
# bundle-llama-server.sh
#
# Replaces the bundled llama-server binary and its dylibs with the version
# currently installed via Homebrew, then re-links everything so the binary
# is self-contained in src-tauri/binaries/ (no Homebrew paths at runtime).
#
# Run this after `brew upgrade llama.cpp` to keep the bundled binary in sync.
# Requires: Homebrew llama.cpp, ggml; Xcode command-line tools (install_name_tool, codesign).
#
# Usage: tools/bundle-llama-server.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARIES_DIR="$SCRIPT_DIR/../src-tauri/binaries"

LLAMA_BIN="/opt/homebrew/bin/llama-server"
LLAMA_LIB="/opt/homebrew/opt/llama.cpp/lib"
GGML_LIB="/opt/homebrew/opt/ggml/lib"

# ── Preflight ──────────────────────────────────────────────────────────────
for req in install_name_tool codesign otool; do
  command -v "$req" >/dev/null || { echo "ERROR: $req not found"; exit 1; }
done
[[ -x "$LLAMA_BIN" ]] || { echo "ERROR: $LLAMA_BIN not found — install with: brew install llama.cpp"; exit 1; }
[[ -d "$GGML_LIB"  ]] || { echo "ERROR: $GGML_LIB not found — install with: brew install ggml"; exit 1; }

VERSION=$("$LLAMA_BIN" --version 2>&1 | head -1)
echo "Bundling $VERSION"
echo "  from $LLAMA_BIN"
echo "  into $BINARIES_DIR"
echo ""

# ── Helper: patch one file ─────────────────────────────────────────────────
patch_file() {
  local file="$1"

  # 1. Replace the rpath that points at the Homebrew lib dir with @loader_path
  #    so dylibs resolve relative to wherever this file lives.
  if otool -l "$file" | grep -q "@loader_path/../lib"; then
    install_name_tool -rpath "@loader_path/../lib" "@loader_path" "$file"
  fi

  # Add @loader_path/../Frameworks to RPATH so it finds dylibs copied into Frameworks in bundled apps
  install_name_tool -add_rpath "@loader_path/../Frameworks" "$file" 2>/dev/null || true

  # 2. Patch hardcoded /opt/homebrew/opt/ggml/lib/* references → @rpath
  while IFS= read -r abs_path; do
    local base
    base=$(basename "$abs_path")
    echo "  patching $base in $(basename "$file")"
    install_name_tool -change "$abs_path" "@rpath/$base" "$file"
  done < <(otool -L "$file" | awk '{print $1}' | grep "^/opt/homebrew/opt/ggml/lib/")

  # 3. Re-sign with the Apple Developer ID certificate (required for Gatekeeper/distribution)
  # Fallback to ad-hoc "-" if not specified or available.
  local identity="${APPLE_SIGN_IDENTITY:-"Developer ID Application: Roberto Lupi (83BHH8484C)"}"
  echo "  signing $(basename "$file") with: $identity"
  
  if [ "$identity" = "-" ]; then
    codesign --force --sign - "$file" 2>/dev/null
  else
    codesign --force --options runtime --sign "$identity" "$file" 2>/dev/null
  fi
}

# ── Step 1: copy & patch the server binary ────────────────────────────────
TARGET_BIN="$BINARIES_DIR/llama-server-aarch64-apple-darwin"
echo "Copying binary..."
cp "$LLAMA_BIN" "$TARGET_BIN"
chmod +x "$TARGET_BIN"
patch_file "$TARGET_BIN"

# ── Step 2: copy & patch llama.cpp dylibs (@rpath ones) ──────────────────
echo "Copying llama.cpp dylibs..."
LLAMA_DYLIBS=(
  libllama-server-impl.dylib
  libllama-common.0.dylib
  libllama.0.dylib
  libmtmd.0.dylib
)
for name in "${LLAMA_DYLIBS[@]}"; do
  src="$LLAMA_LIB/$name"
  dst="$BINARIES_DIR/$name"
  [[ -f "$src" ]] || { echo "  WARNING: $src not found, skipping"; continue; }
  echo "  $name"
  cp "$src" "$dst"
  patch_file "$dst"
done

# ── Step 3: copy & patch ggml dylibs (hardcoded absolute paths) ──────────
echo "Copying ggml dylibs..."
GGML_DYLIBS=(
  libggml.0.dylib
  libggml-base.0.dylib
)
for name in "${GGML_DYLIBS[@]}"; do
  src="$GGML_LIB/$name"
  dst="$BINARIES_DIR/$name"
  [[ -f "$src" ]] || { echo "  WARNING: $src not found, skipping"; continue; }
  echo "  $name"
  cp "$src" "$dst"
  patch_file "$dst"
done

# ── Done ──────────────────────────────────────────────────────────────────
echo ""
echo "Done. Bundling complete!"
