# PARLO

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="src/assets/parlo-lockup-white.svg">
  <img src="src/assets/parlo-lockup-indigo.svg" width="200" alt="PARLO">
</picture>

[Italiano](README.md)

PARLO is an open-source desktop app for Windows and macOS that records calls,
transcribes them, and keeps them in a searchable archive on your computer.

It records your microphone and system audio, so it works with any platform
(Teams, Zoom, Meet, browser calls) without adding a bot to the meeting.
Transcription can run entirely on your machine. Once a call is archived, you
can work with an AI assistant to ask questions and produce documents.

The interface is available in Italian and English.

## What it does

### Recording

- Start and stop with a global keyboard shortcut, even while using another app.
- Capture the microphone, system audio, or both. On Windows, devices can
  follow system changes automatically, for example when you plug in headphones.
- See a live transcript preview while you talk; it is replaced by the final
  transcript when the call ends.
- During the call, add notes, capture an area of the screen, and draw
  annotations on top of other windows. On Windows you can also record the
  screen, or part of it, together with the annotations.

### Transcription

- **On your computer**, with Whisper at three levels (fast, balanced, highest
  accuracy). The model downloads once, then works offline. The app picks CPU
  or GPU automatically, depending on which is faster on your machine.
- **In the cloud**, with Groq and your own key, or with an endpoint compatible
  with OpenAI’s transcription API.
- A dictionary of recurring terms (names, acronyms, products) improves
  recognition.
- Import existing audio and video files, or send voice messages and files to
  a personal Telegram bot: PARLO receives and transcribes them.
- If transcription fails, the audio is kept and you can retry, also with a
  different service.

### Archive

- Search titles, tags, notes, and spoken text, with tag and date filters.
- Play back the call: click a timestamp in the transcript to jump there.
- Export to Markdown.
- Choose what to keep (text only, text and audio, text and video) and for how
  long.

### AI assistant

- Use a subscription you already have: **Claude** via Claude Code, **OpenAI**
  via Codex, **Google** via Antigravity. Alternatively, use **Ollama** on your
  computer or your own server, or any OpenAI-compatible endpoint.
- Ask questions about the call and choose which sources to include: transcript,
  notes, linked files and folders. Type `@` to mention a specific source or
  another call in the archive, to compare several meetings in one chat.
- Ask for Word documents, Excel sheets, PowerPoint decks, PDFs, quizzes, or
  slideshows. Generated files stay attached to the call. If you add your own
  `.pptx` template, slides follow its layout, colors, and typography.
- With Claude, Codex, and Antigravity you can use the MCP connectors already
  configured on your account (for example Drive or a CRM).
- Sensitive operations, such as shell commands, ask for confirmation in the chat.

### Sharing and speech

- A shared folder, for example on Google Drive, lets you publish transcripts
  and browse your colleagues’ ones in the **Shared** tab.
- Send a transcript to a Slack or Teams channel through a webhook.
- Read text and transcripts aloud with local engines (Piper, Kokoro,
  Qwen3-TTS) or online services (Edge, ElevenLabs).

## Privacy: what stays on your computer

The archive is stored on your computer, **unencrypted**. If you choose local
transcription, Ollama for the assistant, and a local voice, no data leaves
your computer.

When you choose an online service, PARLO sends it only the data the operation
needs: audio for transcription, messages and sources for chat, text for
speech. API keys and webhooks are stored in the system keychain. External
services have their own terms and pricing.

## Install

- **Windows x64.** Download the `.exe` installer from
  [Releases](https://github.com/endriu2795/parlo/releases). Python and the
  document and media tools are bundled. The installer does not have an
  Authenticode signature yet, so Windows may warn or block it depending on
  your security settings.
- **macOS 13 or later** (Apple Silicon or Intel). For now, build from source:
  see [Build from source](#build-from-source) and [PARLO on Mac](MACOS.en.md).

### Building it yourself instead of using the installer

All of the app's code is in this repository. If you would rather not run a
downloaded installer, you can read the code and build PARLO on your own
computer, on Windows or Mac, following [Build from source](#build-from-source).
External components are downloaded from official sources at pinned versions.
Runtimes, models, and assistant CLIs are also verified with SHA-256 hashes. The
full list is in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## Getting started

1. In **Settings → Recording**, choose the audio sources and where to
   transcribe. With local transcription, the model downloads on first use.
2. Start recording with **New recording** or the shortcut.
3. When you are done, open the call from the archive. To use the chat, pick a
   provider in **Settings → Assistant**.

## Build from source

On macOS 13 or later, run these commands from the project folder. If macOS asks
to install the Apple command-line tools, finish that installation and run the
first command again.

```sh
bash scripts/mac-test.command --deps
bash scripts/mac-test.command --build
```

On Windows x64, install the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)
first, then run in PowerShell:

```powershell
npm.cmd run desktop:deps
npm.cmd run desktop:build
```

The [development guide](DEVELOPMENT.md) covers output paths, installers for
forks, and tests.

## Documentation

- [User guide](USO.en.md): recording, transcription, assistant, speech,
  sharing, and data.
- [PARLO on Mac](MACOS.en.md): permissions and testing status.
- [Building and contributing](DEVELOPMENT.md): Windows and Mac builds, fork
  installers, tests.
- [Claude Code integration](CLAUDE-CODE-INTEGRATION.md): how the app uses the CLI.
- [Third-party notices](THIRD_PARTY_NOTICES.md).

## License

PARLO’s original code is licensed under [GPL-3.0-only](LICENSE) and comes with
no warranty. Third-party components, models, and services keep their own
licenses and terms; see the [third-party notices](THIRD_PARTY_NOTICES.md).
