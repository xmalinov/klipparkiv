# Third-party notices

Klipparkiv itself is licensed under the MIT License (see [LICENSE](LICENSE)).
The application also runs or bundles separate open-source components. Those
components keep their own licenses; the MIT license does not replace them.

This file is a practical notice, not legal advice. If you redistribute
Klipparkiv, keep this file, `LICENSE`, and the files in `LICENSES/` with the
build, and check the upstream projects for the exact terms.

## Bundled executables

### yt-dlp

- Project: <https://github.com/yt-dlp/yt-dlp>
- License: Unlicense (public domain dedication)
- License text: [LICENSES/Unlicense.txt](LICENSES/Unlicense.txt)
- Windows builds: the release workflow downloads the latest stable
  `yt-dlp.exe` from the yt-dlp GitHub releases.
- macOS builds: pinned in [`scripts/binaries.json`](scripts/binaries.json)
  (currently 2026.10.26).
- yt-dlp is a separate project and is not affiliated with Klipparkiv.

### FFmpeg and FFprobe

- Project: <https://ffmpeg.org/>
- Legal information: <https://ffmpeg.org/legal.html>
- Source code: <https://ffmpeg.org/download.html>
- Windows: the [gyan.dev essentials build](https://www.gyan.dev/ffmpeg/builds/)
  downloaded at release time (the current bundle reports 2026-10-29). That
  build is configured with `--enable-gpl --enable-version3`, so it is
  distributed under **GPLv3**. The full license text is in
  [LICENSES/GPL-3.0.txt](LICENSES/GPL-3.0.txt). The build page and the FFmpeg
  source repository provide the corresponding source and build configuration.
- macOS: [`ffmpeg-static` b6.1.1](https://github.com/eugeneware/ffmpeg-static)
  for the Intel and Apple Silicon builds. The applicable license depends on
  that build's configuration; see the project and `ffmpeg -L` for the exact
  terms. LGPL and GPL texts are included in `LICENSES/`.
- FFmpeg is a separate project and is not affiliated with Klipparkiv.

The bundled FFmpeg build is used only as a separate executable process for
muxing, transcoding and clipping. Klipparkiv does not link FFmpeg into its own
binary.

## Application dependencies

The desktop shell, frontend and Rust dependencies are built from the versions
recorded in [`package-lock.json`](package-lock.json) and
[`src-tauri/Cargo.lock`](src-tauri/Cargo.lock). Their licenses remain in effect.
The main frameworks are:

- [Tauri](https://tauri.app/) — MIT or Apache-2.0;
- [React](https://react.dev/) — MIT;
- [Rust crates](https://crates.io/) — per-crate licenses in `Cargo.lock`;
- [npm packages](https://www.npmjs.com/) — per-package licenses in
  `package-lock.json`.

## Content and trademarks

Klipparkiv does not host, index or distribute media. It is a local front end
that runs yt-dlp on links supplied by the user.

YouTube, Google, yt-dlp, FFmpeg and other names are trademarks of their
respective owners. Klipparkiv is not affiliated with or endorsed by any of them.

Use Klipparkiv only for media you own, have permission to save, or that is
offered under a license that allows downloading. You are responsible for
complying with the source site's terms of service and with copyright law.
