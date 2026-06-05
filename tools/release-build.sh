#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

APP="src-tauri/target/release/bundle/macos/Deep Cuts.app"

# ── helpers ──────────────────────────────────────────────────────────────────
pass() { echo "  ✓ $*"; }
fail() { echo "  ✗ $*" >&2; exit 1; }
section() { echo; echo "▶ $*"; }

# ── 1. Extract compile-time secrets ──────────────────────────────────────────
section "Extracting secrets from .cargo/config.toml"
ACOUSTID_CLIENT_KEY=$(grep 'ACOUSTID_CLIENT_KEY' src-tauri/.cargo/config.toml | sed 's/.*= *"\(.*\)"/\1/')
[[ -n "$ACOUSTID_CLIENT_KEY" ]] && pass "ACOUSTID_CLIENT_KEY found" || fail "ACOUSTID_CLIENT_KEY not found"
export ACOUSTID_CLIENT_KEY

# ── 2. Pre-build checks ───────────────────────────────────────────────────────
section "Pre-build checks"

# Sidecar binaries
for f in "src-tauri/binaries/llama-server-aarch64-apple-darwin" \
          "src-tauri/binaries/fpcalc-aarch64-apple-darwin"; do
  [[ -f "$f" ]] && pass "$(basename $f) present" || fail "Missing: $f"
done

for dylib in libggml-base.0 libggml-blas.0 libggml-cpu.0 libggml-metal.0 \
             libggml-rpc.0 libggml.0 libllama-common.0 libllama.0 \
             libllama-server-impl libmtmd.0; do
  [[ -f "src-tauri/binaries/${dylib}.dylib" ]] && pass "${dylib}.dylib present" || fail "Missing dylib: ${dylib}.dylib"
done

# No hardcoded Homebrew paths
if otool -L src-tauri/binaries/llama-server-aarch64-apple-darwin | grep -q '/opt/homebrew/'; then
  fail "Homebrew paths found in llama-server binary"
else
  pass "No /opt/homebrew/ paths in source llama-server"
fi

# @loader_path/../Frameworks in rpath
if otool -l src-tauri/binaries/llama-server-aarch64-apple-darwin | grep -q '@loader_path/../Frameworks'; then
  pass "@loader_path/../Frameworks in rpath"
else
  fail "@loader_path/../Frameworks missing from rpath"
fi

# Signing identity
if security find-identity -v -p codesigning | grep -q "Developer ID Application: Roberto Lupi (83BHH8484C)"; then
  pass "Developer ID Application cert valid"
else
  fail "Developer ID Application cert not found"
fi

# frameworks in tauri.conf.json match binaries/
section "Checking tauri.conf.json frameworks vs binaries/"
CONF_DYLIBS=$(python3 -c "
import json, re, sys
d = json.load(open('src-tauri/tauri.conf.json'))
for f in d['bundle']['macOS']['frameworks']:
    print(re.sub(r'^binaries/', '', f))
" | sort)
BIN_DYLIBS=$(ls src-tauri/binaries/*.0.dylib src-tauri/binaries/libllama-server-impl.dylib 2>/dev/null \
  | xargs -I{} basename {} | sort)
MISSING=$(comm -23 <(echo "$CONF_DYLIBS") <(echo "$BIN_DYLIBS"))
[[ -z "$MISSING" ]] && pass "All framework entries present in binaries/" || fail "Missing in binaries/: $MISSING"

# ── 3. Tests ──────────────────────────────────────────────────────────────────
section "Running tests"
cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5
pass "cargo test passed"
npm test 2>&1 | tail -5
pass "npm test passed"

# ── 4. Build ──────────────────────────────────────────────────────────────────
section "Building release"
npm run tauri build

# ── 5. Post-build verification ────────────────────────────────────────────────
section "Post-build verification"

# Binaries in Contents/MacOS/
for bin in deep-cuts fpcalc llama-server; do
  [[ -f "$APP/Contents/MacOS/$bin" ]] && pass "Contents/MacOS/$bin present" || fail "Missing: Contents/MacOS/$bin"
done

# Dylibs in Contents/Frameworks/
for dylib in libggml-base.0.dylib libggml-blas.0.dylib libggml-cpu.0.dylib \
             libggml-metal.0.dylib libggml-rpc.0.dylib libggml.0.dylib \
             libllama-common.0.dylib libllama.0.dylib libllama-server-impl.dylib \
             libmtmd.0.dylib; do
  [[ -f "$APP/Contents/Frameworks/$dylib" ]] && pass "Frameworks/$dylib present" || fail "Missing: Frameworks/$dylib"
done

# No Homebrew paths in bundle
if otool -L "$APP/Contents/MacOS/llama-server" | grep -q '/opt/homebrew/'; then
  fail "Homebrew paths found in bundled llama-server"
else
  pass "No /opt/homebrew/ paths in bundled llama-server"
fi

# @loader_path/../Frameworks in bundle rpath
if otool -l "$APP/Contents/MacOS/llama-server" | grep -q '@loader_path/../Frameworks'; then
  pass "@loader_path/../Frameworks in bundled llama-server rpath"
else
  fail "@loader_path/../Frameworks missing from bundled llama-server rpath"
fi

# Code signature
codesign --verify --deep --strict "$APP" && pass "codesign --verify --deep --strict OK"

TEAM=$(codesign -dv "$APP/Contents/Frameworks/libllama.0.dylib" 2>&1 | grep TeamIdentifier | awk -F= '{print $2}')
[[ "$TEAM" == "83BHH8484C" ]] && pass "TeamIdentifier=$TEAM" || fail "Unexpected TeamIdentifier: $TEAM"

SPCTL=$(spctl --assess --type execute --verbose "$APP" 2>&1)
if echo "$SPCTL" | grep -q "accepted"; then
  pass "spctl: accepted (notarized)"
elif echo "$SPCTL" | grep -q "Unnotarized Developer ID"; then
  pass "spctl: Unnotarized Developer ID (expected for local build)"
else
  fail "spctl: unexpected result: $SPCTL"
fi

# ── Done ──────────────────────────────────────────────────────────────────────
echo
DMG=$(ls src-tauri/target/release/bundle/dmg/*.dmg 2>/dev/null | head -1)
echo "✅ Release build complete: $DMG"
