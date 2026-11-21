# PARLO

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="src/assets/parlo-lockup-white.svg">
  <img src="src/assets/parlo-lockup-indigo.svg" width="200" alt="PARLO">
</picture>

[Italiano](README.md)

PARLO is an open-source desktop app for recording, transcribing, and
reviewing calls. Save recordings on your computer, search the text, and use
an AI assistant to produce summaries, answers, and documents.

It captures your microphone and system audio, so you can use it with Teams,
Zoom, Meet, and other calling apps without inviting a bot to the meeting.
Transcribe on your computer with Whisper or choose an online service.

To work as a team, use a shared folder on Google Drive, OneDrive, or a
network drive. Each person chooses which transcripts to publish and reads
their colleagues' transcripts directly in PARLO.

The app is available for Windows and macOS, in Italian and English.

## What it does

### Recording

- Record the microphone, system audio, or both. Start and stop with a
  keyboard shortcut, even while using another app.
- Add notes, screenshots, and on-screen annotations.

Microphone and system audio recording are available on Windows and Mac. On
Windows, choose devices in PARLO or follow the system defaults. On Mac, the
microphone follows the system's default input, including changes during a
call. On Windows, you can also record video of the screen or a selected area. See
[PARLO on Mac](MACOS.en.md).

### Transcription

- Run Whisper on your computer, with three speed and accuracy levels. After
  downloading the model, you can transcribe offline; PARLO picks CPU or GPU
  based on performance.
- For online transcription, use **Groq** with your API key.
- With **Custom endpoint**, connect another transcription service, including
  a Whisper server on your computer or network. Set up the server separately;
  it must support OpenAI's transcription API. In PARLO, enter its address,
  model ID, and API key if required.
- Add names, acronyms, and products to the dictionary to help recognition.
- Import audio and video files, or receive voice messages and files from a
  personal Telegram bot.
- If a transcript fails, retry from the saved audio, with a different service
  if needed.

### Archive

- Add tags to calls manually, for example by client, project, or topic.
  Search titles, tags, notes, and transcripts; filter by the tags you assigned
  and by date.
- If you keep the audio, click a timestamp in the transcript to play back
  that passage.
- Export transcripts as Markdown.
- Choose what to keep: text only, text and audio, or text and video (on
  Windows).
- In **Settings → Archive**, choose when to delete data automatically: after
  1, 3, 6, 12, or 24 months, or **Never** to keep it until you delete it.

### AI assistant

- Connect **Claude** via Claude Code or **OpenAI** via Codex using your
  account. You can also use **Ollama** on your computer or your own server,
  or an OpenAI-compatible endpoint.
- Choose the chat's sources: transcript, notes, files, and folders. Type `@`
  to mention a source or another call and compare several meetings.
- Create Word documents, Excel sheets, PowerPoint decks, PDFs, quizzes, and
  slideshows. Generated files stay attached to the call.
- Add files and folders to give the assistant context or guide the style and
  format of its output: for example a project brief, a sample document,
  editorial guidelines, or a `.pptx` template.
- With Claude and Codex, use the MCP connectors configured on
  your account, for example Drive or a CRM.
- Approve operations such as running shell commands from the chat.

### Speech

Listen to text and transcripts with local engines (Piper, Kokoro, Qwen3-TTS)
or online services (Edge, ElevenLabs).

## Share transcripts with your team

PARLO uses a shared folder to exchange transcripts between colleagues. Use a
folder synced by Google Drive for desktop or OneDrive, or a network folder.
Each person keeps their own local archive and publishes the transcripts they
want to share.

1. Share a folder with the team and make it accessible from each person's
   computer.
2. In **Settings → Sharing**, each person selects their local copy of the
   same folder and enables the option to show other people's transcripts.
3. Open a call and use **Share** to publish it to the folder. You can also
   enable automatic publishing for new transcripts.

The **Shared** tab lists your colleagues' transcripts, searchable by title or
author. Read them or open a chat with the assistant. Files are in Markdown,
so you can also read them in a text editor. Sharing includes the transcript
text; audio, video, attachments, and chats stay in each person's archive.
Drive or OneDrive handles folder sync and access.

You can also send transcripts to **Slack** and **Teams** channels through
webhooks. See the [sharing guide](USO.en.md#sharing-with-your-team).

## Install

- **Windows x64.** Download the `.exe` installer from
  [Releases](https://github.com/endriu2795/parlo/releases). Python and the
  document and media tools are bundled. The installer does not have an
  Authenticode signature yet, so Windows may warn or block it depending on
  your security settings.
- **macOS 13 or later** (Apple Silicon or Intel). For now, build from source:
  see [Build commands](#build-commands) and [PARLO on Mac](MACOS.en.md).

### Build from source

The code is in this repository. The [development guide](DEVELOPMENT.md)
describes the steps for Windows and Mac; the [commands below](#build-commands)
provide a quick start. External components are downloaded from official
sources at pinned versions. Runtimes, models, and assistant CLIs are verified
with SHA-256 hashes. See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for
the full list.

## Getting started

1. In **Settings → Recording**, choose the audio sources and where to
   transcribe. With local transcription, the model downloads on first use.
2. Click **Start recording** or use the shortcut.
3. When you are done, open the call from the archive. To use the chat, pick a
   provider in **Settings → Assistant**.

## Data and services

The archive is stored on your computer. Whisper transcription, Ollama on the
same computer, and local voices can work offline after setup and model
downloads.

When you use an online service, the operation sends content to that service:
audio for transcription, messages and sources for the assistant, text for
speech. Sharing, Telegram, web search, and connectors use their respective
services when you enable them. API keys, tokens, and webhooks are stored in
the system keychain. External services have their own terms and pricing.

The [data guide](USO.en.md#data-and-services) covers local storage and online
services.

## Build commands

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

PARLO's original code is licensed under [GPL-3.0-only](LICENSE), with no
warranty. You can use, modify, and redistribute it under the license terms.

Third-party components, models, and services keep their own licenses and
terms; see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
