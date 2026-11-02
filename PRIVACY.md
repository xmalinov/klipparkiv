# Privacy

Klipparkiv is a local desktop application. It has no accounts, no telemetry, no
analytics and no advertising.

## Network access

The app makes network requests only when they are needed:

- **Downloads and metadata** — yt-dlp contacts the site whose link you paste,
  using the options you choose.
- **yt-dlp updates** — when you press **Update yt-dlp**, the app downloads a
  release from the yt-dlp GitHub project.
- **App updates** — the app checks the GitHub Releases API for `update.json`,
  then downloads the signed update from GitHub if one is available.

There is no analytics endpoint and no background upload of your library, file
names, listening history or settings.

## Local data

On Windows, the portable build keeps settings, the library index, covers and
logs in `data\`, and media in `audio\` and `video\`. WebView2 may keep its own
cache in `%LOCALAPPDATA%`.

On macOS, the library and media live in `~/Movies/Klipparkiv`; settings, updates,
backups and logs live in `~/Library/Application Support/Klipparkiv`. WKWebView
may keep its own cache under `~/Library`.

Logs can contain URLs, titles, file paths and yt-dlp output. They stay on your
machine unless you choose to copy them into a bug report. Remove anything
private before sharing.

## This website

The GitHub Pages site uses no analytics and sets no cookies. GitHub may process
standard request logs under its own privacy policy.
