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
- Transcribe entirely on your computer or use a supported cloud service with
  your own API key.
- Add notes and screenshots to a call and export its transcript as Markdown.
- Choose a local or cloud AI assistant to ask questions about a call and
  create documents, presentations, or slideshows using the sources you choose.
- Generate speech from text with a local engine or an online service.

Optional online services have their own accounts, terms, and pricing.
[Supported services and setup (Italian)](USO.md)

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

## Build from source

On macOS 13 or later, run these commands from the project folder. If macOS asks
to install the Apple command-line tools, finish that installation and rerun
the first command.

```sh
bash scripts/mac-test.command --deps
bash scripts/mac-test.command --build
```

On Windows x64, install the Tauri prerequisites first, then run in PowerShell:

```powershell
npm.cmd run desktop:deps
npm.cmd run desktop:build
```

See the [development guide](SVILUPPO.md) for output paths, installers and tests.

## License

The project’s original code is licensed under [GPL-3.0-only](LICENSE). See the
[third-party notices](THIRD_PARTY_NOTICES.md) for bundled components.
