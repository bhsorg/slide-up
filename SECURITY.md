# Security Policy

## Supported versions

| Version | Supported |
| ------- | --------- |
| 0.1.x   | Yes       |

## Reporting a vulnerability

Please **do not** open a public GitHub issue for security vulnerabilities.

Use GitHub's private vulnerability reporting for this repository:

<https://github.com/bhsorg/slide-up/security/advisories/new>

Include:

- A clear description of the issue and its impact.
- Steps to reproduce, including the app version and operating system.
- Any relevant logs, **with personal paths, workspace recovery keys and passwords removed**.

You should receive an acknowledgement within 7 days. We will keep you informed while we investigate and will credit you in the fix notes unless you prefer to remain anonymous.

## Scope

Slide Up stores media libraries, playlists and settings in per-workspace SQLCipher databases and keeps video covers/storyboards in encrypted `.hlv` files. Security issues in the following areas are especially relevant:

- Workspace isolation: access to another workspace's media, history, playlists or settings.
- Lock and password handling: bypassing the workspace lock or password prompt.
- Key management: exposure of workspace master keys, recovery keys or database keys.
- Cache encryption: plaintext media artwork/storyboards written to disk.
- Unsafe file handling from imported media, M3U playlists or external URLs.

## Out of scope

- Vulnerabilities in third-party media files (malicious media content is still untrusted input, but please report decoder issues as regular bugs unless they compromise the app).
- Attacks that require full administrator or physical access to the user's machine. A desktop application cannot protect data from the owner of the device.
- Reports without a reproducible path or impact statement.

## Encryption notes

Workspace databases are encrypted at rest with SQLCipher using a key derived from a workspace master key. That master key is either protected by Windows account protection (DPAPI) or by an optional per-workspace password. Recovery keys are shown only once at workspace creation and are not stored by the app; anyone with a recovery key can access that workspace, so treat it like a password.
