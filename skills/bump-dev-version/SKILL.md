---
name: bump-dev-version
description: Bump the app version in Cargo.toml after a release to start the next dev cycle
---

# Bumping the Dev Version

Run this **after** a release has shipped, to advance the version for the next development cycle.

---

## Steps

1. Edit `src-tauri/Cargo.toml` — this is the single source of truth:
   ```toml
   version = "0.1.7"   # ← next dev version
   ```

2. Sync it to `package.json` immediately:
   ```bash
   node scripts/sync-version.js
   ```

3. Commit:
   ```bash
   git add src-tauri/Cargo.toml package.json
   git commit -m "chore: bump dev version to v0.1.7"
   ```

---

## Notes

- `src-tauri/tauri.conf.json` has no `version` field — it reads from `package.json` at build time. No edit needed there.
- `models/manifest.json` `min_app_version` is **not** changed here — that only moves at release time (see `release-build` skill).
- The sync script also runs automatically on every `npm run tauri dev` and `npm run tauri build`, so the manual sync in step 2 is just for keeping the working tree clean before committing.
