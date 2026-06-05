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
  "min_app_version": "X.Y.Z",
  ...
}
```

This tells older app installs that they must update before the new model manifest applies.

---

## 2. Update changelogs

Update both:
- `CHANGELOG.md` (root) — add bullet points for this release under the current version tag
- `docs/changelog.md` — mirror the same entries (this is the public-facing website changelog)

Use `git log <prev-tag>..HEAD --oneline` to enumerate commits since the last release.

---

## 3–5. Pre-build checks, build, and post-build verification

These steps are fully automated by `tools/release-build.sh`. Run it from the repo root:

```bash
tools/release-build.sh
```

The script:
- Extracts `ACOUSTID_CLIENT_KEY` automatically from `src-tauri/.cargo/config.toml`
- Verifies all sidecar binaries and dylibs are present
- Checks no `/opt/homebrew/` paths exist in the source binaries
- Verifies `@loader_path/../Frameworks` is in the rpath
- Validates the Developer ID signing cert
- Checks `tauri.conf.json` frameworks match `src-tauri/binaries/`
- Runs `cargo test` and `npm test`
- Runs `npm run tauri build`
- Verifies the built `.app` bundle layout, paths, signatures, and Gatekeeper status

Output lands in `src-tauri/target/release/bundle/`.

If you've run `brew upgrade llama.cpp` since the last release, re-run `tools/bundle-llama-server.sh` before running the build script.

To run the build manually instead:

```bash
ACOUSTID_CLIENT_KEY=$(grep ACOUSTID_CLIENT_KEY src-tauri/.cargo/config.toml | sed 's/.*= *"\(.*\)"/\1/') \
  npm run tauri build
```

---

## 6. Publish

Replace `X.Y.Z` with the current version:

```bash
VERSION=$(grep '^version' src-tauri/Cargo.toml | head -1 | sed 's/.*= *"\(.*\)"/\1/')

git add models/manifest.json CHANGELOG.md docs/changelog.md tools/ skills/
git commit -m "chore: release v$VERSION"
git tag "v$VERSION"
git push origin main --tags

gh release create "v$VERSION" \
  "src-tauri/target/release/bundle/dmg/Deep Cuts_${VERSION}_aarch64.dmg" \
  --title "v$VERSION" \
  --notes "$(sed -n "/^## \[${VERSION}\]/,/^---/p" CHANGELOG.md | head -n -1)"
```

---

## 7. Checklist

- [ ] `min_app_version` in `models/manifest.json` set to current version
- [ ] `CHANGELOG.md` (root) and `docs/changelog.md` updated
- [ ] `tools/release-build.sh` passes all checks (covers steps 3–5)
- [ ] Git tag pushed, DMG attached to GitHub release via `gh release create`
