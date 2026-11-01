#!/usr/bin/env bash
# Usage: scripts/macos/make-dmg.sh path/to/Klipparkiv.app path/to/output.dmg
set -euo pipefail

APP="${1:?usage: make-dmg.sh path/to/Klipparkiv.app output.dmg}"
DMG="${2:?usage: make-dmg.sh path/to/Klipparkiv.app output.dmg}"
HERE="$(cd "$(dirname "$0")" && pwd)"

STAGING=$(mktemp -d "${TMPDIR:-/tmp}/klipparkiv-dmg.XXXXXX")
trap 'rm -rf "$STAGING"' EXIT

ditto "$APP" "$STAGING/Klipparkiv.app"
ln -s /Applications "$STAGING/Applications"
cp "$HERE/FIRST-LAUNCH.txt" "$STAGING/README.txt"

mkdir -p "$(dirname "$DMG")"
rm -f "$DMG"
hdiutil create \
  -volname "Klipparkiv" \
  -srcfolder "$STAGING" \
  -ov -format UDZO \
  "$DMG"

echo "created $DMG"
