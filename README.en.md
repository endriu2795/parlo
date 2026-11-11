# PARLO

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="src/assets/parlo-lockup-white.svg">
  <img src="src/assets/parlo-lockup-indigo.svg" width="200" alt="PARLO">
</picture>

[Italiano](README.md)

Free desktop app for recording calls, transcribing audio, and working with the
results through an AI assistant. Record your microphone and system audio or
import an audio or video file. Transcribe locally or use Groq with your own key.
The app keeps its archive on your computer. Optional online services have their
own accounts, terms, and pricing.

## Install

Download the installer for your platform when available. [Official releases](https://github.com/endriu2795/parlo/releases)

Build targets are Windows x64 and macOS 13+
(Apple Silicon or Intel). The first local transcription requires a model
download. Speech language detection is automatic by default.

## Build your own installer

Build on the platform you want to package. Install Node.js 22, Rust stable and
Python 3.12 first. On Windows, use the MSVC Rust toolchain, Visual Studio Build
Tools with Desktop development with C++, and WebView2. On Mac, install CMake
and the Apple command-line tools with `xcode-select --install`.

Windows (PowerShell):

```powershell
npm.cmd ci
npm.cmd run desktop:installer
```

Run the setup `.exe` from `src-tauri/target/release/bundle/nsis`.

Mac (Terminal):

```sh
npm ci
python3 scripts/build-mac.py --adhoc
```

Open the `.dmg` in `src-tauri/target/release/bundle` and copy the app to
Applications. Use your own Developer ID if you plan to distribute a signed
Mac build. If you want your fork and the official app side by side, set a
distinct `productName` and `identifier` in `src-tauri/tauri.conf.json` before
building. Update checks are off by default for these builds.

See the [development guide in Italian](SVILUPPO.md) for development builds,
optional GPU support, and how to configure an update channel for your fork.

## License

The project’s original code is licensed under [GPL-3.0-only](LICENSE). See the
[third-party notices](THIRD_PARTY_NOTICES.md) for bundled components.
