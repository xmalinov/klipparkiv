#!/usr/bin/env bash
# Usage: scripts/macos/make-dmg.sh path/to/Klipparkiv.app path/to/output.dmg
set -euo pipefail

APP="${1:?usage: make-dmg.sh path/to/Klipparkiv.app output.dmg}"
DMG="${2:?usage: make-dmg.sh path/to/Klipparkiv.app output.dmg}"
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"

STAGING=$(mktemp -d "${TMPDIR:-/tmp}/klipparkiv-dmg.XXXXXX")
trap 'rm -rf "$STAGING"' EXIT

ditto "$APP" "$STAGING/Klipparkiv.app"
ln -s /Applications "$STAGING/Applications"
cp "$HERE/FIRST-LAUNCH.txt" "$STAGING/README.txt"
cp "$ROOT/LICENSE" "$STAGING/LICENSE.txt"
cp "$ROOT/THIRD_PARTY_NOTICES.md" "$STAGING/THIRD_PARTY_NOTICES.md"
cp "$ROOT/PRIVACY.md" "$STAGING/PRIVACY.md"
cp "$ROOT/SECURITY.md" "$STAGING/SECURITY.md"
cp -R "$ROOT/LICENSES" "$STAGING/LICENSES"

mkdir -p "$(dirname "$DMG")"
rm -f "$DMG"
hdiutil create \
  -volname "Klipparkiv" \
  -srcfolder "$STAGING" \
  -ov -format UDZO \
  "$DMG"

echo "created $DMG"
