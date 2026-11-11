# PARLO

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="src/assets/parlo-lockup-white.svg">
  <img src="src/assets/parlo-lockup-indigo.svg" width="200" alt="PARLO">
</picture>

[Italiano](README.md)

PARLO is a free desktop app for **recording calls and finding them later**.
Start recording your microphone and system audio with a configurable keyboard
shortcut, even while using another app. After the call, the transcript goes
into your local archive.
Search for words spoken on the call, add tags, and filter by tag or date.

## Other features

- Import audio and video files for transcription.
- Transcribe on your computer or use Groq with your own key.
- Add notes and screenshots to a call and export its transcript as Markdown.
- Use Claude Code, Codex, or Ollama to ask questions about a call and create
  documents, presentations, or slideshows using the sources you choose.
- Generate speech online with Edge or ElevenLabs, or locally with Piper,
  Kokoro, and Qwen3-TTS. Qwen3-TTS is optional and initially available on
  Windows x64.

Optional online services have their own accounts, terms, and pricing.

## Install

- **Windows x64.** Download the `.exe` installer. It currently has no
  Authenticode signature. Windows may warn or block it depending on your
  security settings. [Official releases](https://github.com/endriu2795/parlo/releases)
- **macOS 13+.** For now, build from source on Apple Silicon or Intel.
  [Build instructions](#build-your-own-installer)

The first local transcription requires a model download. Speech language
detection is automatic by default.

## Turn a call into slides in your style

In the chat, open **Sources → Add shared files** and select your `.pptx`
template. It will be available across chats. After a call, enable the template
for that chat, mention it with `@`, and ask for a new deck based on the
transcript using the template’s layout, colors, and typography.

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
