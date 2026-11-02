# Security policy

## Supported versions

Security fixes are prepared for the latest published release. Older releases may
not receive fixes, especially when the issue is in a bundled yt-dlp or FFmpeg
build.

## Reporting a vulnerability

Please **do not open a public issue** for a security problem.

Use GitHub's private vulnerability reporting:

1. Open the repository's **Security** tab.
2. Choose **Report a vulnerability**.
3. Describe the issue, the affected version, the operating system and the
   smallest reproduction you can share.

If private reporting is unavailable, email the maintainer at
`xmalinov@gmail.com` with the subject `Klipparkiv security`.

Never include cookies, tokens, passwords, private URLs or downloaded media in a
report. A stack trace, version string and a minimal description are enough.

## Scope

Reports are especially useful for:

- update manifest or signature verification (Minisign);
- the Windows updater helper and the macOS bundle replacement;
- command injection or argument injection into yt-dlp/FFmpeg;
- path traversal in the library, thumbnails, clips or destination folders;
- opening files outside the configured library root;
- unsafe handling of cookies or browser profiles;
- a bundled dependency with a known exploitable vulnerability.

Out of scope:

- yt-dlp extraction failures or site changes (report those to the
  [yt-dlp project](https://github.com/yt-dlp/yt-dlp/issues));
- copyright, terms-of-service or content-availability questions;
- issues that require an already compromised machine or a malicious local
  administrator.

## What to expect

This is a small volunteer project. I aim to acknowledge a report within a few
days, agree on a disclosure date, and credit the reporter if they want credit.
There is no bug bounty.

## Safe harbour

Good-faith research that avoids privacy violations, service disruption and data
loss is welcome. Please give the project a reasonable chance to fix the issue
before publishing details.
