#!/usr/bin/env bash
# Ad-hoc (or Developer ID) code signing for the macOS app bundle.
# Usage: scripts/macos/sign-app.sh path/to/Klipparkiv.app
set -euo pipefail

APP="${1:?usage: sign-app.sh path/to/Klipparkiv.app}"
IDENTITY="${IDENTITY:--}"
SIGN_ARGS=(--force --sign "$IDENTITY")
if [[ "$IDENTITY" == "-" ]]; then
  SIGN_ARGS+=(--timestamp=none)
else
  # Developer ID signatures used for notarization need a secure timestamp and
  # hardened runtime. Ad-hoc signatures cannot use them.
  SIGN_ARGS+=(--timestamp --options runtime)
fi

if [[ ! -d "$APP/Contents" ]]; then
  echo "not an app bundle: $APP" >&2
  exit 1
fi

# Tauri normally bundles the helper, but keep the build deterministic.
HELPER="$APP/Contents/MacOS/klipparkiv-updater"
if [[ ! -f "$HELPER" ]]; then
  for candidate in \
    "src-tauri/target/release/klipparkiv-updater" \
    "src-tauri/target/debug/klipparkiv-updater"; do
    if [[ -f "$candidate" ]]; then
      cp "$candidate" "$HELPER"
      chmod 755 "$HELPER"
      break
    fi
  done
fi
if [[ ! -f "$HELPER" ]]; then
  echo "klipparkiv-updater is missing from the bundle" >&2
  exit 1
fi

sign_macho() {
  local file="$1"
  if file "$file" | grep -q "Mach-O"; then
    codesign "${SIGN_ARGS[@]}" "$file"
  fi
}

# Sign nested code first, then the bundle. --deep is only used for verification.
if [[ -d "$APP/Contents/Resources/bin" ]]; then
  for file in "$APP/Contents/Resources/bin/"*; do
    [[ -f "$file" ]] && sign_macho "$file"
  done
fi

# The helper must be signed before the main executable: codesign validates
# sibling Mach-O files when signing a bundle's main binary.
sign_macho "$HELPER"
for file in "$APP/Contents/MacOS/"*; do
  [[ "$file" == "$HELPER" ]] && continue
  [[ -f "$file" ]] && sign_macho "$file"
done

codesign "${SIGN_ARGS[@]}" "$APP"
codesign --verify --deep --strict --verbose=2 "$APP"

echo "signed $APP with identity '$IDENTITY'"
