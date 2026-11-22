<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="src/assets/parlo-lockup-white.svg">
    <img src="src/assets/parlo-lockup-indigo.svg" width="220" alt="PARLO">
  </picture>

  <p><strong>Open-source desktop app to record, transcribe, and chat with your calls — without bots.</strong></p>

  <p>
    <a href="https://github.com/endriu2795/parlo/releases">Download</a> ·
    <a href="docs/guide.md">User Guide</a> ·
    <a href="CONTRIBUTING.md">Build &amp; Contribute</a> ·
    <a href="README.it.md">Italiano</a>
  </p>
</div>

---

PARLO captures your microphone and system audio directly on your machine — working with Teams, Zoom, Google Meet, or any other app without inviting a bot — and also transcribes existing local audio/video files or Telegram voice notes.

Transcribe offline with Whisper or online via Groq / any OpenAI-compatible server, search your entire local archive, and use your preferred AI assistant to summarize calls, cross-reference past meetings or local files, and generate documents. When you work in a team, share transcripts through a plain Google Drive, OneDrive, or network folder — no central server required.

The interface automatically follows your system language (English or Italian).

## Highlights

- **Local recording & file import** — Capture mic, system audio, or both with a global shortcut, or import existing audio/video files without modifying the originals. Take timestamped notes, grab screenshots, and draw on-screen annotations (plus screen/region video recording on Windows).
- **Offline or cloud transcription** — Run Whisper locally (Tiny, Small, or Turbo with automatic CPU/GPU selection and optional live preview), use Groq, or point to your own OpenAI-compatible Whisper server. Includes a custom dictionary and a personal Telegram bot for voice notes on the go.
- **Fast local archive** — Everything is stored in a local SQLite database with full-text search across titles, tags, notes, and transcripts. Click any timestamp to replay that exact moment, export to Markdown, and set automatic retention rules.
- **Bring-your-own AI assistant** — Connect **Claude** (via Claude Code), **OpenAI** (via Codex), **Ollama** (local or remote), or any OpenAI-compatible endpoint.
  - Mention files, folders, or up to 8 other calls with `@` to compare meetings or match a style template.
  - Generate Word, Excel, PowerPoint, PDF, quizzes, and slideshows directly — no Office required.
  - Use your account's MCP connectors (Claude/Codex) and approve shell commands right in chat.
- **Built-in text-to-speech** — Listen to replies or scripts using local offline engines (Piper, Kokoro, optional Qwen3-TTS on Windows) or online services (Edge, ElevenLabs, custom endpoint).
- **Zero-server team sharing** — Pick a shared folder on Google Drive, OneDrive, or a LAN share. PARLO publishes clean `.md` transcripts so teammates can read and chat over shared calls right inside the app. Also supports Slack and Teams webhooks.

## Platform Support

| Feature | Windows x64 | macOS 13+ (Apple Silicon & Intel) |
| --- | :---: | :---: |
| Installer | Ready (`.exe` in Releases) | Build from source (testing in progress) |
| Mic & system audio | Yes (selectable devices or system default) | Yes (system default mic + ScreenCaptureKit) |
| Notes, screenshots & annotations | Yes | Yes |
| Screen / region video recording | Yes | Not yet |
| Local Whisper, AI assistant, sharing | Yes | Yes |
| Local Qwen3-TTS (optional) | Yes | Not yet |

## Installation

### Windows x64

Download the `.exe` installer from [GitHub Releases](https://github.com/endriu2795/parlo/releases). Python and all document/media tools are bundled.

> **Note:** The installer is not Authenticode-signed yet, so Windows SmartScreen may show a warning on first run.

### macOS 13+ (Apple Silicon or Intel)

An official signed DMG is not available yet. You can build `PARLO.app` locally in two commands from the cloned repository:

```sh
bash scripts/mac-test.command --deps
bash scripts/mac-test.command --build
```

The first command prepares Node, Python, Rust, and CMake inside the project folder (install Apple Command Line Tools if prompted, then rerun). Once built, open `src-tauri/target/debug/bundle/macos/PARLO.app`. See the [User Guide](docs/guide.md#macos-permissions) for macOS permissions and [CONTRIBUTING.md](CONTRIBUTING.md) for full build instructions.

## Quick Start

1. Open **Settings → Recording** to pick your audio sources and transcription engine (local Whisper downloads its model on first use).
2. Click **Start recording** or press your global shortcut.
3. Open the finished call from the archive — configure a provider in **Settings → Assistant** to start chatting with it.

## Privacy & Data

Your archive stays on your computer. Local Whisper, local Ollama, and local voices (Piper, Kokoro, Qwen3-TTS) work completely offline once downloaded.

Online services (Groq, Claude, Codex, ElevenLabs, Edge, Telegram, web search, MCP connectors, or remote servers) only receive the data needed for the actions you trigger. API keys, tokens, and webhooks are stored in your operating system's keychain. See [Data and Privacy](docs/guide.md#data-and-privacy) for details.

## Documentation

- **[User Guide](docs/guide.md)** ([Italiano](docs/guide.it.md)) — Recording, transcription, AI assistant workflows, voice, team sharing, and privacy.
- **[Contributing & Building](CONTRIBUTING.md)** — Architecture, Windows/macOS builds, tests, and forks.
- **[Third-Party Notices](THIRD_PARTY_NOTICES.md)** — Licenses for bundled runtimes, models, and dependencies.

## License

PARLO's original code is licensed under [GPL-3.0-only](LICENSE), with no warranty. Third-party components, models, and external services keep their own licenses and terms — see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
