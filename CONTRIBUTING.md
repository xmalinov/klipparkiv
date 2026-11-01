# Contributing

Thanks for taking the time to help. This project is a small, opinionated
cross-platform app (portable on Windows, a DMG app on macOS), so please keep
changes focused.

## Before you start

- Open an issue for anything larger than a typo fix. It is cheaper to agree on the
  approach first.
- Do not commit binaries: `yt-dlp.exe`, `ffmpeg.exe`, `ffprobe.exe`, `bin/`, `data/`,
  `audio/`, `video/`, `dist/`, `dist-portable/` and `release/` are ignored on purpose.

## Setup

```bash
npm ci
npm run tauri dev
```

You need Node.js 24+ and Rust stable. Windows also needs MSVC Build Tools and
WebView2; macOS needs Xcode Command Line Tools.

For real downloads:

- Windows: put `yt-dlp.exe`, `ffmpeg.exe` and `ffprobe.exe` into `bin\`.
- macOS: run `node scripts/fetch-binaries.mjs mac-arm64` (or `mac-x64`). The
  binaries go to `src-tauri/resources/bin` and are copied into the app; dev
  builds also find them there.

## Checks

All of these must pass before a pull request is merged:

```bash
npm run typecheck
npm run test:run
npm run build
cargo fmt    --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test   --manifest-path src-tauri/Cargo.toml
node scripts/check-version.mjs
```

## Style

- TypeScript: strict, no `any` unless there is no reasonable alternative.
- Rust: `cargo fmt` is the source of truth; keep command handlers small.
- UI text is Russian, code and comments are English. Keep it that way.
- One logical change per commit. Describe *why* in the message, not *what*.

## Pull requests

- Reference the issue (`Closes #123`).
- Describe how you tested the change (OS, scenario, files produced).
- Screenshots are welcome for UI changes.
