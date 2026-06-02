#!/usr/bin/env bash
set -euo pipefail

if [[ -z "${APPLE_ID:-}" || -z "${APPLE_PASSWORD:-}" || -z "${APPLE_TEAM_ID:-}" ]]; then
  echo "Error: APPLE_ID, APPLE_PASSWORD, and APPLE_TEAM_ID must be set." >&2
  exit 1
fi

DMG=$(find src-tauri/target/release/bundle/dmg -name "*.dmg" | head -1)
if [[ -z "$DMG" ]]; then
  echo "Error: No DMG found in src-tauri/target/release/bundle/dmg/" >&2
  exit 1
fi

echo "Submitting $DMG for notarization..."
xcrun notarytool submit "$DMG" \
  --apple-id "$APPLE_ID" \
  --password "$APPLE_PASSWORD" \
  --team-id "$APPLE_TEAM_ID" \
  --wait

echo "Stapling notarization ticket..."
xcrun stapler staple "$DMG"

echo "Verifying..."
spctl -a -vv --type open --context context:primary-signature "$DMG"
