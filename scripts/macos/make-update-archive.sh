#!/usr/bin/env bash
# Usage: scripts/macos/make-update-archive.sh bundle-dir output.app.tar.gz
set -euo pipefail

BUNDLE_DIR="${1:?usage: make-update-archive.sh bundle-dir output.tar.gz}"
TARBALL="${2:?usage: make-update-archive.sh bundle-dir output.tar.gz}"

if [[ ! -d "$BUNDLE_DIR/Klipparkiv.app" ]]; then
  echo "Klipparkiv.app not found in $BUNDLE_DIR" >&2
  exit 1
fi

mkdir -p "$(dirname "$TARBALL")"
rm -f "$TARBALL"
# COPYFILE_DISABLE avoids AppleDouble ._ files in the archive. Code signatures
# live inside the files, so a plain tar preserves them.
COPYFILE_DISABLE=1 tar -czf "$TARBALL" -C "$BUNDLE_DIR" Klipparkiv.app

if ! tar -tzf "$TARBALL" | grep -q '^Klipparkiv.app/Contents/Info.plist$'; then
  echo "update archive is missing the app bundle" >&2
  exit 1
fi

echo "created $TARBALL"
