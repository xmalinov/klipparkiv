#!/usr/bin/env bash
# Usage: scripts/macos/verify-app.sh path/to/Klipparkiv.app [version]
set -euo pipefail

APP="${1:?usage: verify-app.sh path/to/Klipparkiv.app [version]}"
VERSION="${2:-}"

if [[ ! -d "$APP/Contents" ]]; then
  echo "not an app bundle: $APP" >&2
  exit 1
fi

codesign --verify --deep --strict --verbose=2 "$APP"

IDENTIFIER=$(plutil -extract CFBundleIdentifier raw -o - "$APP/Contents/Info.plist")
if [[ "$IDENTIFIER" != "com.klipparkiv.app" ]]; then
  echo "unexpected bundle id: $IDENTIFIER" >&2
  exit 1
fi

if [[ -n "$VERSION" ]]; then
  BUNDLE_VERSION=$(plutil -extract CFBundleShortVersionString raw -o - "$APP/Contents/Info.plist")
  if [[ "$BUNDLE_VERSION" != "$VERSION" ]]; then
    echo "bundle version $BUNDLE_VERSION, expected $VERSION" >&2
    exit 1
  fi
fi

file "$APP/Contents/MacOS/klipparkiv"
codesign --display --verbose=4 "$APP" 2>&1 | sed -n '1,12p'
echo "verified $APP"
