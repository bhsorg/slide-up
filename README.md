# Slide Up 🎬🎵

<div align="center">

<img src="public/slideup.png" width="96" height="96" alt="Slide Up Logo" />

<br />

**A high-performance, modern desktop media player built with Tauri 2, Rust, Vue 3, and bundled libmpv.**

[![Tauri 2](https://img.shields.io/badge/Tauri-v2.0-blue.svg?logo=tauri)](https://tauri.app/)
[![Rust](https://img.shields.io/badge/Rust-1.80+-orange.svg?logo=rust)](https://www.rust-lang.org/)
[![Vue 3](https://img.shields.io/badge/Vue-3.5-emerald.svg?logo=vuedotjs)](https://vuejs.org/)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.0-3178C6.svg?logo=typescript)](https://www.typescriptlang.org/)
[![libmpv](https://img.shields.io/badge/Engine-libmpv--2-red.svg)](https://mpv.io/)
[![Tailwind CSS](https://img.shields.io/badge/Tailwind-v4.0-38B2AC.svg?logo=tailwindcss)](https://tailwindcss.com/)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)
[![Donate](https://img.shields.io/badge/Donate-Buy%20Me%20A%20Coffee-yellow.svg?logo=buy-me-a-coffee)](https://buymeacoffee.com/hinditutorpoint)

[Download](#-download) • [Features](#key-features) • [Architecture](#architecture) • [Keyboard Shortcuts](#default-keyboard-shortcuts) • [Installation](#installation--development) • [Contributing](#-contributing) • [Donate](#-support--donations)

<br /><br />

![Slide Up Desktop](screenshots/slideup.png)

</div>

---

## 📥 Download

Get the latest stable release of **Slide Up** for your platform:

| Platform | Download Link | Notes |
| :--- | :--- | :--- |
| **Windows 10 / 11 (64-bit)** | [![Download Windows](https://img.shields.io/badge/Download-Latest%20Installer%20(.exe%20%2F%20.msi)-blue?style=for-the-badge&logo=windows&logoColor=white)](https://github.com/bhsorg/slide-up/releases/latest) | Automatic in-app updates, hardware acceleration & bundled libmpv |
| **Android Edition** | [![Download Android](https://img.shields.io/badge/Android-GitHub%20Edition-green?style=for-the-badge&logo=android&logoColor=white)](https://hinditutorpoint.github.io/slideup/) | Dedicated mobile edition for phones and tablets |

> 💡 **Tip**: All releases, changelogs, and installer assets are available on the [GitHub Releases Page](https://github.com/bhsorg/slide-up/releases).

---

## 🌟 Overview

**Slide Up** is a next-generation desktop media player engineered for maximum performance, broad format compatibility, and intelligent media handling. Unlike web-wrapped players that rely on browser HTML5 audio/video codecs, **Slide Up** embeds a native, hardware-accelerated **libmpv** engine directly within the operating system window.

Whether you are playing high-bitrate 4K HDR videos, lossless FLAC albums, online streaming playlists, or parsing torrent media packages, Slide Up delivers silky-smooth playback with real-time synchronized lyrics, companion artwork, and offline AI enhancement.

---

## 🚀 Key Features

### 🎧 Audio Playback
- **Universal Codec Support**: Full playback for MP3, FLAC, WAV, AAC, OGG, Opus, M4A, ALAC, AIFF, and more powered directly by libmpv.
- **Synchronized Lyrics (LRCLIB)**:
  - Real-time time-stamped LRC scrolling line-by-line display with millisecond precision.
  - Interactive lyrics navigation — click any line to seek directly to that exact moment.
  - Smart Bollywood / Transliteration query matching (e.g., automatically matches *Humari* $\leftrightarrow$ *Hamari*, *Adhoori* $\leftrightarrow$ *Adhuri*).
  - YouTube title junk cleaner (removes actor credits, promo tags, `- full song`, `| Emraan Hashmi`, etc. automatically).
  - Persistent disk cache for offline lyrics access across app restarts.
- **Audio Queue & Library**: Shuffle, repeat (one/all), dynamic queue reordering, artist bio popups, and gapless track playback.

### 🎥 Video Playback
- **Direct Native Rendering**: Video frames render into a native Win32 child HWND window with zero frame-dropping and minimal CPU/GPU overhead.
- **Hardware Acceleration**: Auto-probed hardware decoders (D3D11VA, DXVA2, NVDEC, VAAPI).
- **Subtitles Engine**: Full support for SRT, ASS, SSA, and WebVTT subtitles with styling, subtitle track switcher, and auto-detection.
- **Playback Controls**: Micro frame-stepping, aspect ratio switching (16:9, 4:3, 21:9, Original, Fill), native screenshot capture, and audio track selection.

### 🧲 BitTorrent Parser & File Browser
- **Pure TypeScript Bencode Decoder**: Decodes any `.torrent` file in real time with SHA-1 InfoHash calculation.
- **Interactive File Selection Browser**:
  - Filter chips to instantly isolate **All**, **Audio only**, **Video only**, or specific extensions (`.mp3`, `.wav`, `.flac`, `.mp4`, `.mkv`).
  - Selective import — choose only the exact tracks you want to add to your playlist.
  - Total selected size counter and file counter in real time.
- **Companion Artwork Matching**:
  - Automatically identifies companion image files (e.g. `song.mp3` $\leftrightarrow$ `song.png` / `song.jpg`) and folder cover art (`cover.jpg`, `folder.jpg`, `poster.png`).
  - Displays companion badges on each file and sets playlist cover art.
- **BEP 19 Web Seed & Archive.org Streaming**:
  - Automatically resolves `url-list`, `httpseeds`, and Archive.org download streams to direct, percent-encoded HTTPS URLs.
  - Enables direct, instant online streaming of torrent tracks without waiting for full P2P downloads.

### 🌐 Online M3U / M3U8 Playlist Streaming
- **Native CORS-Bypass Streaming**: Fetches external playlists via the native Rust backend, bypassing browser cross-origin restrictions entirely.
- **Auto-Categorization**: Automatically categorizes streams into **Audio (Music)** and **Video (Live TV)** with distinct badges.
- **Archive.org Link Sanitization**: Automatically normalizes unencoded whitespace (`%20`), ampersands (`%26`), and enforces HTTPS redirects.
- **Export & Import**: Export any playlist to clean `#EXTM3U` format or import remote URLs and local files with one click.

### 🤖 AI Capabilities
- **Live Captions (SenseVoice STT)**:
  - Local, offline SenseVoice Speech-to-Text model (`model.int8.onnx` + Silero VAD, ~230 MB).
  - Streaming, resumable, and cancellable downloader in Settings.
- **AI Video Super-Resolution & Upscaling**:
  - **FSRCNNX x2**: Real-time GLSL shader enhancement applied directly via mpv's pipeline.
  - **Real-ESRGAN Compact x4**: Offline batch video upscaler utilizing ONNX Runtime and ffmpeg frame extraction.

---

## 🏗️ Architecture

```
slide-up/
├── src/                          # Frontend (Vue 3 + TypeScript + Tailwind CSS)
│   ├── components/               # AudioPlayer, VideoPlayer, MediaGrid, PlaybackDock, etc.
│   ├── composables/              # useLyrics, useArtistBio, useToast, useSubtitleAvailability
│   ├── services/                 # tauri.ts (IPC bridge), lyrics.ts, artist.ts
│   ├── stores/                   # Pinia stores (player, playlist, library, settings)
│   ├── types/                    # TypeScript interfaces (media, lyrics, playlist)
│   ├── utils/                    # torrent.ts (bencode & web seed parser), m3u.ts
│   └── views/                    # Home, Playlists, Favorites, Recent, Settings, LiveTv
│
└── src-tauri/                    # Native Backend (Rust + Tauri 2)
    ├── src/
    │   ├── commands/             # IPC command handlers (playback, playlists, lyrics, library)
    │   ├── db/                   # SQLite database repository (rusqlite)
    │   ├── mpv/                  # Dynamic libmpv-2 loader & Win32 child window embedding
    │   ├── model_manager.rs      # Offline SenseVoice model downloader & manager
    │   ├── upscaler_manager.rs   # Real-ESRGAN and FSRCNNX upscaling pipeline
    │   └── lib.rs                # App entrypoint and Tauri plugin registrations
    ├── binaries/                 # onnxruntime.dll
    └── resources/                # libmpv-2.dll
```

---

## ⌨️ Default Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| `Space` | Play / Pause toggle |
| `←` / `→` | Seek backward / forward 5 seconds |
| `↑` / `↓` | Volume up / down (5% increments) |
| `M` | Mute / Unmute audio |
| `F` | Toggle fullscreen mode |
| `Esc` | Exit fullscreen mode |
| `N` / `P` | Next track / Previous track |
| `Ctrl + O` | Open media files from computer |
| `Ctrl + Shift + O` | Open entire folder into library |

---

## 🛠️ Installation & Development

### Prerequisites
- **Node.js**: v18.0 or newer
- **Rust**: v1.80 or newer (`rustup default stable`)
- **Windows (x64)**: Required for embedded Win32 video rendering (macOS/Linux support native windows)

### Setup Instructions

1. **Clone the Repository**:
   ```bash
   git clone https://github.com/bhsorg/slide-up.git
   cd slide-up
   ```

2. **Install Dependencies**:
   ```bash
   npm install
   ```

3. **Verify Native Binaries**:
   Ensure `src-tauri/resources/libmpv-2.dll` and `src-tauri/binaries/onnxruntime.dll` are in place.

4. **Run in Development Mode**:
   ```bash
   npm run tauri dev
   ```

5. **Build for Production**:
   ```bash
   npm run tauri build
   ```

---

## 🔒 Security, Privacy & License

- **Fully Offline Core**: All local media library scanning, audio tagging, and AI processing run 100% locally on your machine.
- **Zero Telemetry**: No user tracking or behavioral telemetry.
- **Secure Sandbox**: External web links and network streams strictly use secure HTTPS endpoints with verified fallbacks.
- **Desktop Repository**: [https://github.com/bhsorg/slide-up](https://github.com/bhsorg/slide-up)
- **Official Website / Blog**: [https://bloghindisupport.com](https://bloghindisupport.com)
- **Android Version Repo**: [https://github.com/hinditutorpoint/slideup](https://github.com/hinditutorpoint/slideup)
- **Android Version Website**: [https://hinditutorpoint.github.io/slideup/](https://hinditutorpoint.github.io/slideup/)
- **License**: Slide Up is licensed under the **GNU General Public License v3.0 (GPLv3)**. Any modified distribution or fork must remain open source under the same license terms.

---

## 🤝 Contributing

Contributions are warmly welcome! Whether it's reporting a bug, proposing a new feature, improving documentation, or submitting a pull request:

1. **Fork the Repository** on GitHub.
2. **Create your Feature Branch**:
   ```bash
   git checkout -b feature/AmazingFeature
   ```
3. **Commit your Changes**:
   ```bash
   git commit -m "feat: add some amazing feature"
   ```
4. **Push to the Branch**:
   ```bash
   git push origin feature/AmazingFeature
   ```
5. **Open a Pull Request**: Submit your PR with a clear description of the enhancements.

Feel free to open an issue in the [GitHub Issue Tracker](https://github.com/bhsorg/slide-up/issues) if you encounter any bugs or have feature suggestions.

---

## 💖 Support & Donations

**Slide Up** is a completely free, open-source project created with passion to provide an exceptional, ad-free desktop media experience.

If you enjoy using Slide Up and want to support its ongoing development, new AI capabilities, and open-source maintenance, please consider buying a coffee! Every contribution keeps the project thriving.

<div align="center">

<a href="https://buymeacoffee.com/hinditutorpoint" target="_blank">
  <img src="https://cdn.buymeacoffee.com/buttons/v2/default-yellow.png" alt="Buy Me A Coffee" height="50" style="height: 50px !important;width: 217px !important;" />
</a>

<br /><br />

[![Buy Me A Coffee](https://img.shields.io/badge/Buy%20Me%20A%20Coffee-hinditutorpoint-FFDD00?style=for-the-badge&logo=buy-me-a-coffee&logoColor=black)](https://buymeacoffee.com/hinditutorpoint)

</div>

---

<div align="center">
  <sub>Maintained by <strong><a href="https://bloghindisupport.com" target="_blank">bhsorg</a></strong> • <a href="https://github.com/bhsorg/slide-up" target="_blank">Desktop (GitHub)</a> • <a href="https://hinditutorpoint.github.io/slideup/" target="_blank">Android Edition</a></sub>
</div>
