# Slide Up

A desktop audio/video player built with Tauri 2, Rust, Vue 3 and TypeScript. Playback is powered by a bundled **libmpv** engine; there is no simulated playback.

## Architecture

- **Frontend** (`src/`): Vue 3 + Pinia + Tailwind CSS v4. `src/services/tauri.ts` is the only bridge to native commands/events.
- **Backend** (`src-tauri/src/`): Rust/Tauri commands, SQLite (`rusqlite`, bundled) for the library/playlists/settings, `lofty` for audio tags, and a dynamic `libmpv` loader.
- **Playback**: `libmpv-2.dll` is loaded at runtime with `libloading`. On Windows the video output is embedded as a native child HWND of the app window via mpv's `wid` option; on other platforms mpv uses its own window.
- **Data**: stored in `slideup.db` under the OS app-data directory. Thumbnails and screenshots live alongside it.

## Requirements

- Windows (x64) for embedded video rendering; Rust and Node.js for development.
- `src-tauri/resources/libmpv-2.dll` must be present. It is bundled into production builds via `tauri.conf.json` → `bundle.resources`.
- `src-tauri/binaries/onnxruntime.dll` must be present. It is loaded at runtime through `ort` and bundled via `tauri.conf.json` → `bundle.resources`; no build-time linking, bindgen or libclang is required.

## Commands

```bash
npm install
npm run tauri dev     # development app
npm run build         # type-check + frontend bundle
cargo test --manifest-path src-tauri/Cargo.toml   # libmpv decode smoke test
npm run tauri build   # production installer
```

## Features

- Audio: MP3/WAV/FLAC/AAC/OGG/M4A, metadata, cover art, seek, volume, speed, shuffle, repeat, queue.
- Video: MP4/MKV/AVI/MOV/WebM, fullscreen, hardware acceleration, frame step, screenshots, aspect ratio, subtitles (SRT/ASS/SSA/VTT), audio/subtitle track selection.
- Library: file/folder import, drag & drop, thumbnails, favorites, recently played, playlists with M3U/M3U8 import/export, search and filters.
- Settings: light/dark theme, playback defaults, subtitle preferences, and configurable keyboard shortcuts.

## Default shortcuts

| Key | Action |
| --- | --- |
| `Space` | Play / Pause |
| `←` / `→` | Seek −5 s / +5 s |
| `↑` / `↓` | Volume up / down |
| `F` | Fullscreen |
| `Esc` | Exit fullscreen |
| `M` | Mute |
| `N` / `P` | Next / Previous |
| `Ctrl+O` | Open files |
| `Ctrl+Shift+O` | Open folder |

## ONNX Runtime on other platforms

The app loads ONNX Runtime dynamically (`ort`'s `load-dynamic` feature), so there is nothing to compile or link on any platform. Replace `src-tauri/binaries/onnxruntime.dll` with the matching library for your target and put it in the same folder:

- Windows: `onnxruntime.dll`
- macOS: `libonnxruntime.dylib`
- Linux: `libonnxruntime.so`

`ORT_DYLIB_PATH` can also point at a custom library. The provider candidates are chosen per platform (DirectML → CPU on Windows, CoreML → CPU on macOS, CUDA → CPU on Linux) and the first one that creates a session wins.

## Offline captions (SenseVoice)

Settings → Live captions manages a local SenseVoice model (`model.int8.onnx`, `tokens.txt` and `silero_vad.onnx`, ~230 MB) downloaded into `app_data_dir/models/sense-voice`. Downloads are streaming, resumable, cancellable and atomically published; progress is emitted as `model-download-progress`. `initialize_engine` initializes ONNX Runtime once per process, then builds `OnnxCaptionEngine` (src-tauri/src/onnx_caption_engine.rs) off the UI thread. The engine loads a CPU Silero VAD session plus a SenseVoice STT session, probes GPU providers and falls back to CPU with thread count `clamp(cores/2, 1, 4)`. It exposes `is_speech()` and `transcribe_chunk()` primitives; capturing player audio and drawing the on-screen overlay are not wired up yet. `src-tauri/src/model_commands.rs` is an alternate, unwired draft kept for reference — the active commands are the five in `model_manager.rs`.

**GPU limitation:** the bundled `onnxruntime.dll` is the stock CPU build of ORT 1.20.1, so the GPU attempt fails and the hardware badge shows **CPU**. To enable hardware acceleration, replace the DLL with an ORT build that includes DirectML (Windows), CoreML (macOS) or CUDA (Linux); the code will then select it automatically. The live-captions toggle only persists the user preference — audio capture, transcription and the on-screen overlay are a separate playback feature and are not implemented here.

## AI upscaler models

Settings → AI upscaling lists SPAN x2/x4, Real-ESRGAN Compact, ESPCN x2/x4 and the FSRCNNX x2 mpv shader. ONNX files go to `app_data_dir/models/upscalers`, GLSL shaders to `app_data_dir/shaders`, using the same streaming/resumable/cancellable downloader and `upscaler-download-progress` event as the caption model. Only sources verified with a HEAD request are enabled:

- Real-ESRGAN Compact — `realesr-general-x4v3.onnx`, 4,866,417 bytes, from `huggingface.co/CoderViking/realesr-general-x4v3-onnx`.
- FSRCNNX x2 — `FSRCNNX_x2_8-0-4-1.glsl`, 71,296 bytes, from the `igv/FSRCNN-TensorFlow` 1.1 GitHub release.

The SPAN and ESPCN entries are shown as a catalog with "Source URL needed" until verified URLs are supplied. Once the FSRCNNX shader is installed, the video player shows an **AI Enhance** button (top bar and right-click menu) that applies it at runtime via mpv's `glsl-shaders` property; toggling it off clears the property.

Once Real-ESRGAN Compact is downloaded, the video player also shows an **Upscale Video** button (top bar and right-click menu). It runs a fully offline batch upscale: ffmpeg decodes raw RGB frames, the ONNX model upscales each frame 4x with ONNX Runtime, and ffmpeg re-encodes to `app_data_dir/upscaled/<name>-realesrgan.mp4` with the original audio copied. Progress is emitted as `upscale-progress`, and the work runs off the UI thread. **ffmpeg and ffprobe must be installed and on PATH** — they are not bundled, and the command reports a clear error if missing. This is separate from the live FSRCNNX shader toggle; the other ONNX models are not applied to playback.

## Notes

Codec support is provided by the bundled libmpv build, not by OS codecs. Unsupported or corrupted files surface a user-facing error instead of failing silently. On Windows the video is a native Win32 child window. While the player is focused it sits below a transparent webview, so playback controls, popovers and the context menu render above the video. When the window loses focus the video moves above an opaque webview, avoiding a full WebView2 alpha-composite every frame while another app is active.
