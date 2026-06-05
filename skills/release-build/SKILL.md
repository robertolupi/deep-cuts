---
name: release-build
description: End-to-end checklist for building a signed macOS release — bump manifest min_app_version, verify sidecars, build, and inspect the .app
---

# Release Build Checklist

The release process does **not** bump `src-tauri/Cargo.toml` — that version represents the current release and stays as-is. After shipping, bump it separately using the `bump-dev-version` skill to start the next dev cycle.

---

## 1. Bump `min_app_version` in `models/manifest.json`

Set `min_app_version` to the current version from `src-tauri/Cargo.toml`:

```json
{
  "manifest_version": 1,
  "min_app_version": "0.1.6",
  ...
}
```

This tells older app installs that they must update before the new model manifest applies.

---

## 2. Update `CHANGELOG.md`

Add bullet points for this release under the current version tag.

---

## 3. Pre-build verification

### 3a. Sidecar binaries present

```bash
ls -lh src-tauri/binaries/
```

Expected (at minimum):
- `llama-server-aarch64-apple-darwin`
- `fpcalc-aarch64-apple-darwin`
- All llama dylibs: `libggml*.dylib`, `libllama*.dylib`, `libmtmd*.dylib`

If you've run `brew upgrade llama.cpp` since the last release, re-run:
```bash
tools/bundle-llama-server.sh
```

### 3b. No hardcoded Homebrew paths in the source binaries

```bash
otool -L src-tauri/binaries/llama-server-aarch64-apple-darwin
```

Every dependency must be `@rpath/...` — no `/opt/homebrew/` lines.

### 3c. `@loader_path/../Frameworks` is in the RPATH

```bash
otool -l src-tauri/binaries/llama-server-aarch64-apple-darwin | grep -A2 "LC_RPATH"
```

Both `@loader_path` and `@loader_path/../Frameworks` must appear.

### 3d. Signing identity is valid

```bash
security find-identity -v -p codesigning | grep "Roberto Lupi"
```

Must show `Developer ID Application: Roberto Lupi (83BHH8484C)` with `1 valid identity`.

### 3e. dylibs in `tauri.conf.json` match `src-tauri/binaries/`

If you added new dylib dependencies, make sure they're listed under `bundle.macOS.frameworks` (not `bundle.resources`):

```bash
ls src-tauri/binaries/*.dylib
grep -A20 '"frameworks"' src-tauri/tauri.conf.json
```

---

## 4. Build

Steps 3–5 (pre-build checks, build, post-build verification) are automated by `tools/release-build.sh`. Run it from the repo root:

```bash
tools/release-build.sh
```

The script extracts `ACOUSTID_CLIENT_KEY` automatically from `src-tauri/.cargo/config.toml`, runs all pre-build checks, runs `cargo test` and `npm test`, calls `npm run tauri build`, then runs all post-build verification checks. Output lands in `src-tauri/target/release/bundle/`.

To run the build manually instead:

```bash
ACOUSTID_CLIENT_KEY=$(grep ACOUSTID_CLIENT_KEY src-tauri/.cargo/config.toml | sed 's/.*= *"\(.*\)"/\1/') \
  npm run tauri build
```

---

## 5. Verify the built `.app`

```bash
APP="src-tauri/target/release/bundle/macos/Deep Cuts.app"
```

### 5a. Binaries and dylibs are in the right locations

```bash
ls "$APP/Contents/MacOS/"
# Expected: deep-cuts  fpcalc  llama-server

ls "$APP/Contents/Frameworks/"
# Expected: libggml-base.0.dylib  libggml-blas.0.dylib  libggml-cpu.0.dylib
#           libggml-metal.0.dylib  libggml-rpc.0.dylib  libggml.0.dylib
#           libllama-common.0.dylib  libllama-server-impl.dylib
#           libllama.0.dylib  libmtmd.0.dylib
```

If `Contents/Frameworks/` is empty, dylibs were listed under `bundle.resources` instead of `bundle.macOS.frameworks` in `tauri.conf.json`.

### 5b. No hardcoded Homebrew paths inside the bundle

```bash
# Every dependency should be @rpath/... — no /opt/homebrew/ lines
otool -L "$APP/Contents/MacOS/llama-server"

# Both @loader_path and @loader_path/../Frameworks must appear
otool -l "$APP/Contents/MacOS/llama-server" | grep -A2 "LC_RPATH"
```

Expected `LC_RPATH` output:
```
path @loader_path (offset 12)
path @loader_path/../Frameworks (offset 12)
```

### 5c. Everything is signed

```bash
# Verify the whole bundle including all Frameworks
codesign --verify --deep --strict "$APP" && echo "OK"

# Spot-check a dylib is individually signed with your Team ID
codesign -dv "$APP/Contents/Frameworks/libllama.0.dylib" 2>&1 | grep -E "Authority|TeamIdentifier"
# Expected: TeamIdentifier=83BHH8484C

# Gatekeeper simulation
spctl --assess --type execute --verbose "$APP"
```

`spctl` will report `rejected: Unnotarized Developer ID` for a locally-built release — this is expected. It only becomes `accepted` after submitting to Apple's notarization service via `xcrun notarytool`.

If `codesign --verify --deep` fails on a dylib, `bundle-llama-server.sh` didn't re-sign it after patching. Re-run the script and rebuild.

---

## 6. Publish

```bash
git add models/manifest.json CHANGELOG.md
git commit -m "chore: release v0.1.6"
git tag v0.1.6
git push origin main --tags
```

Then attach `Deep Cuts_0.1.6_aarch64.dmg` from `src-tauri/target/release/bundle/dmg/` to the GitHub release.

---

## 7. Checklist

- [ ] `min_app_version` in `models/manifest.json` set to current version
- [ ] `CHANGELOG.md` (root) and `docs/changelog.md` updated
- [ ] `tools/release-build.sh` passes all checks (covers steps 3–5)
- [ ] Git tag pushed, DMG attached to GitHub release
