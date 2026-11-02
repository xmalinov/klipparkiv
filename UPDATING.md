# Klipparkiv updates

The app checks a signed `update.json` from GitHub Releases. Windows replaces
`klipparkiv.exe`; macOS replaces the whole `Klipparkiv.app` bundle. Settings,
the library, `audio/` and `video/` are never touched.

The first release with the updater is **1.2.0** on Windows. macOS starts at
**1.3.0**; there is no older macOS version to migrate.

## Artifacts

| Platform | Installer | Update asset | Manifest key |
|---|---|---|---|
| Windows x64 | `klipparkiv-<ver>-win-x64.zip` | `...-win-x64.exe` | `windows-x86_64` |
| macOS Intel | `klipparkiv-<ver>-mac-x64.dmg` | `...-mac-x64.app.tar.gz` | `darwin-x86_64` |
| macOS ARM | `klipparkiv-<ver>-mac-arm64.dmg` | `...-mac-arm64.app.tar.gz` | `darwin-aarch64` |

Each update asset has a minisign `.sig` file. macOS artifacts also include a
DMG for the assisted update path.

## Release flow

Pushing a `v*` tag runs `.github/workflows/release.yml`:

1. `build-windows` builds the portable zip and update exe.
2. `build-macos` (matrix: `macos-15-intel`, `macos-15`) builds, ad-hoc signs
   and packs the `.app`, DMG and update archive.
3. `publish` builds the release body from the `### [<version>]` section of
   `CHANGELOG.md` (`scripts/release-notes.mjs`), merges the platform fragments
   into `update.json`, and creates a **draft** release named
   `Klipparkiv <version>`. GitHub's auto-generated “What's Changed” notes are
   not used. Publish the draft with:
   `gh release edit v<version> --draft=false`.

The release workflow does **not** need a Developer ID or notarization. It needs
only the existing updater signing key.

## Release checklist

1. Bump the version (see below) and move the `Unreleased` changelog entries into
   a `### [<version>] — YYYY-MM-DD` section. This section is the release body:
   keep it user-facing and bilingual, and remove dependency-only noise.
2. Update the README, `README.txt` and the landing page on the `gh-pages` branch
   when the supported platforms, install steps or feature highlights change.
3. Confirm that `LICENSE`, `THIRD_PARTY_NOTICES.md`, `PRIVACY.md` and
   `LICENSES/` are present in the Windows zip and the macOS DMG.
4. Run the checks and the version check for the new tag:

   ```bash
   npm run check:version
   node scripts/check-version.mjs --tag v1.6.1
   ```

5. Commit, then push `main` and the tag:

   ```bash
   git tag -a v1.6.1 -m "Klipparkiv 1.6.1"
   git push origin main v1.6.1
   ```

6. The Release workflow builds Windows and both macOS bundles, then creates a
   **draft** release titled `Klipparkiv 1.6.1` with the changelog body. Check
   the title, notes and artifacts, then publish it:

   ```bash
   gh release edit v1.6.1 --draft=false
   ```

7. `releases/latest` and the landing page start pointing at the new version only
   after the release is published.

## Version bump

Bump the version in all six files:

- `package.json`
- `package-lock.json` (the root and `packages[""]` versions)
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.lock` (the `klipparkiv` package entry)
- `README.txt` (the `Klipparkiv <version>` line)

Refresh the lockfile after the `Cargo.toml` bump, then check:

```bash
cargo check --manifest-path src-tauri/Cargo.toml
node scripts/check-version.mjs --tag v1.6.1
```

The check refuses a mismatch between any of the version files and the tag.
The release workflow checks the tag against `package.json`.

## Manual release

### Windows

```powershell
npm run build:portable
npm run pack:portable

$version = (Get-Content package.json -Raw | ConvertFrom-Json).version
Copy-Item dist-portable\klipparkiv.exe "release\klipparkiv-$version-win-x64.exe"
npx tauri signer sign -f .secrets\klipparkiv-updater.key `
  --app-version $version "release\klipparkiv-$version-win-x64.exe"

node scripts/make-update-fragment.mjs `
  --platform windows-x86_64 `
  --version $version `
  --file "release\klipparkiv-$version-win-x64.exe" `
  --sig "release\klipparkiv-$version-win-x64.exe.sig" `
  --url "https://github.com/xmalinov/klipparkiv/releases/download/v$version/klipparkiv-$version-win-x64.exe" `
  --out "release\update-windows.json"
```

### macOS

```bash
npm ci
node scripts/fetch-binaries.mjs mac-arm64   # or mac-x64
npm run build:macos
VERSION=$(node -p "require('./package.json').version")
bash scripts/macos/pack.sh "$VERSION" arm64

npx tauri signer sign --app-version "$VERSION" "release/klipparkiv-$VERSION-mac-arm64.dmg"
npx tauri signer sign --app-version "$VERSION" "release/klipparkiv-$VERSION-mac-arm64.app.tar.gz"

node scripts/make-update-fragment.mjs \
  --platform darwin-aarch64 \
  --version "$VERSION" \
  --file "release/klipparkiv-$VERSION-mac-arm64.app.tar.gz" \
  --sig "release/klipparkiv-$VERSION-mac-arm64.app.tar.gz.sig" \
  --url "https://github.com/xmalinov/klipparkiv/releases/download/v$VERSION/klipparkiv-$VERSION-mac-arm64.app.tar.gz" \
  --manual-file "release/klipparkiv-$VERSION-mac-arm64.dmg" \
  --manual-sig "release/klipparkiv-$VERSION-mac-arm64.dmg.sig" \
  --manual-url "https://github.com/xmalinov/klipparkiv/releases/download/v$VERSION/klipparkiv-$VERSION-mac-arm64.dmg" \
  --out "release/update-macos-arm64.json"
```

### Merge

```bash
node scripts/merge-update-manifest.mjs \
  --out release/update.json \
  --version "$VERSION" \
  --require windows-x86_64,darwin-x86_64,darwin-aarch64 \
  release/update-windows.json \
  release/update-macos-x64.json \
  release/update-macos-arm64.json
```

Upload the artifacts and `update.json` to the draft release. Until the release
is published, `releases/latest/download/update.json` points to the previous
version.

## macOS signing without a Developer ID

Developer ID and notarization are not required. The build is ad-hoc signed:

```bash
bash scripts/macos/sign-app.sh src-tauri/target/release/bundle/macos/Klipparkiv.app
```

The script signs nested binaries inside-out, then the bundle, then verifies with
`codesign --verify --deep --strict`. Do not use `spctl` as a success criterion:
it rejects ad-hoc builds by design.

Consequences:

- On first launch, macOS asks the user to approve the app:
  - macOS 15+: System Settings → Privacy & Security → Open Anyway;
  - macOS 14 and older: right-click → Open;
  - Terminal: `xattr -dr com.apple.quarantine /Applications/Klipparkiv.app`.
- On Apple Silicon, every Mach-O must be signed. The script signs `yt-dlp`,
  `ffmpeg`, `ffprobe` and `klipparkiv-updater`.
- In-app updates are downloaded by the app, so they do not get a quarantine
  flag and do not need approval again.
- The ad-hoc CDHash changes on every build. macOS may reset TCC permissions
  (for example notifications) after an update.

The ad-hoc signature is an integrity measure, not proof of authorship.
Developer ID signing and notarization are out of scope for the current release
line.

## macOS update installation

Automatic path:

1. Download `Klipparkiv.app.tar.gz`.
2. Verify size, SHA-256 and minisign signature.
3. Extract into `~/Library/Application Support/Klipparkiv/update/<version>`.
4. Verify `CFBundleIdentifier`, version and `codesign --verify --deep --strict`.
5. Copy `klipparkiv-updater` out of the new bundle.
6. The helper waits for the app to exit, then atomically swaps the current
   `.app` with the new one (`renamex_np(RENAME_SWAP)`), removes quarantine and
   relaunches. File systems without swap support fall back to two renames.
7. On failure the helper swaps the previous bundle back.

Assisted path (used when the app cannot replace itself):

- App Translocation (`/AppTranslocation/` in the path);
- `/Applications` is not writable;
- macOS App Management (TCC) blocks bundle replacement;
- the helper failed to replace the bundle.

The app then downloads the signed DMG, verifies it, opens it and asks the user
to drag Klipparkiv into Applications. Data is preserved.

## Signing key

`src-tauri/update.pub` is the embedded public key. The matching private key was
generated into `.secrets/klipparkiv-updater.key` (gitignored). **Back it up.**
If it is lost, already installed clients cannot verify future updates.

GitHub Actions needs it as repository secrets:

- `TAURI_SIGNING_PRIVATE_KEY` — literal contents of
  `.secrets/klipparkiv-updater.key`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — empty for the generated key

macOS builds are ad-hoc signed. Developer ID signing and notarization are not
used, so a downloaded app still needs the usual Gatekeeper confirmation on first
launch.

## Security properties

- Update assets are verified with Ed25519/minisign before staging.
- The signature binds the artifact to the version in its trusted comment, so a
  tampered manifest cannot pair a newer version with an older signed archive.
- SHA-256 is checked as an additional integrity guard.
- Downgrades and versions not newer than the running app are ignored.
- macOS archives are validated for path traversal and must contain exactly
  `Klipparkiv.app` with the expected bundle id and version.
- The macOS helper swaps the app bundle atomically (`renamex_np(RENAME_SWAP)`)
  and keeps the previous bundle for rollback; the install path is never missing
  during replacement.
- Windows helper still validates that it only replaces files inside the
  portable root.
- Updating from a version older than 1.2.0 on Windows requires one manual
  download. The first macOS release is 1.3.0, so no migration is needed.

## Testing an update locally

1. Build version N and install it.
2. Build version N+1 into `release/` and sign the artifacts.
3. Put a test `update.json` on an HTTPS URL (or use a debug build with
   `KLIPPEARKIV_UPDATE_URL`).
4. Check the banner, download, install and rollback paths.
5. Verify that `data/`, `audio/` and `video/` are untouched.
