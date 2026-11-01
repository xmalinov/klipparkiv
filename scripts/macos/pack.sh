#!/usr/bin/env bash
# Builds, signs and packs the macOS artifacts.
# Usage: scripts/macos/pack.sh <version> x64|arm64
set -euo pipefail

VERSION="${1:?usage: pack.sh <version> x64|arm64}"
ARCH="${2:?usage: pack.sh <version> x64|arm64}"
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"

APP="$ROOT/src-tauri/target/release/bundle/macos/Klipparkiv.app"
OUT="$ROOT/release"
PREFIX="klipparkiv-$VERSION-mac-$ARCH"

if [[ ! -d "$APP" ]]; then
  echo "app bundle not found: $APP" >&2
  echo "run 'npm run build:macos' first" >&2
  exit 1
fi

bash "$HERE/sign-app.sh" "$APP"
bash "$HERE/verify-app.sh" "$APP" "$VERSION"
bash "$HERE/make-dmg.sh" "$APP" "$OUT/$PREFIX.dmg"

TMP=$(mktemp -d "${TMPDIR:-/tmp}/klipparkiv-update.XXXXXX")
trap 'rm -rf "$TMP"' EXIT
ditto "$APP" "$TMP/Klipparkiv.app"
bash "$HERE/make-update-archive.sh" "$TMP" "$OUT/$PREFIX.app.tar.gz"

echo "macOS artifacts:"
ls -lh "$OUT/$PREFIX.dmg" "$OUT/$PREFIX.app.tar.gz"
