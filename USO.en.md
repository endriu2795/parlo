# PARLO user guide

[Italiano](USO.md)

This guide follows the order of the app's settings. To install PARLO, see the
[README](README.en.md).

## Recording a call

1. In **Settings → Recording**, choose the microphone, system audio, or both.
   To record the people you are talking to, pick the same audio output used
   by Teams, Zoom, or your browser.
2. Click **New recording** or use the shortcut set in **Settings → Shortcuts**.
   The shortcut also works while PARLO is in the background.
3. During the call you can write notes, capture an area of the screen, and
   draw annotations on top of other windows. Screenshots are added to the
   transcript.
4. Click **Stop**. The final transcript replaces the preview and the call
   appears in the archive.

On Windows you can also record the screen, or part of it, with the
annotations: in **Settings → Archive**, choose **Text and video**. Video takes
more disk space and resources.

## Transcription

**Settings → Recording → Where to transcribe** offers three options.

- **On this computer.** Runs Whisper locally: free, private, and offline after
  the first download. Choose **Fast**, **Balanced**, or **Highest accuracy**
  depending on your computer. The app picks CPU or GPU automatically; with an
  NVIDIA GPU, transcription is much faster. A provisional preview is shown
  during the call.
- **Online with Groq.** Enter your key from console.groq.com. If the
  connection drops, the app retries only the affected part.
- **Custom endpoint.** Any server compatible with OpenAI's transcription API
  (`audio/transcriptions` with a `verbose_json` response).

In **Call language** you can keep automatic detection or set a fixed language.

In **Settings → Dictionary**, add names of people, companies, products, or
acronyms you use often: the transcript will spell them correctly more often.
The dictionary does not replace words automatically; add only terms that come
up frequently.

If processing fails, the audio is kept. Open the call and click
**Try again**, optionally after switching service.

### Importing audio and video

With **Import audio or video**, PARLO extracts the audio from the file and
transcribes it. The original file stays where it is.

### Receiving voice messages from Telegram

In **Settings → Telegram** you can connect a personal bot: create it with
@BotFather using `/newbot` and paste the token into PARLO. You can then send
voice messages, audio, or video files (up to 20 MB) to the bot, even from your
phone, and PARLO transcribes them with the current settings. If the app is
closed, messages are fetched when it reopens, within Telegram's 24-hour limit.

## Finding and replaying calls

Archive search covers titles, tags, notes, and transcribed text. You can
narrow results by date and tag. In an open call you can search for a passage,
play back the audio, change the playback speed, and click a timestamp to jump
to that point. **Export** saves the transcript as Markdown.

In **Settings → Archive**, choose what to keep (**Text only**,
**Text and audio**, **Text and video**) and for how long. Audio and video
follow the same expiry as the recording.

## Using the assistant

Choose the provider in **Settings → Assistant**.

| Provider | What you need | Notes |
|---|---|---|
| **Claude** | Claude account | Uses Claude Code. If it is not installed, PARLO downloads it. |
| **Codex** | OpenAI account | Uses the Codex CLI. |
| **Antigravity** | Google account | Requires a sign-in dedicated to PARLO. |
| **Ollama** | Ollama on your computer or your own server | No cloud account. Requires a model that supports tools. |
| **Custom endpoint** | Base URL, model ID, optional key | Any server compatible with OpenAI's chat API. |

With Ollama the chat stays local: if the server does not respond, the request
stops instead of falling back to the cloud. Web search is off by default; if
you enable **Search and open web pages**, searches go through DuckDuckGo and,
before a page is opened, only its domain is sent to Anthropic for a security
check. You can adjust the conversation memory; for long calls, a context of at
least 32k tokens is recommended if the model and your memory allow it.

In **Project instructions** you can set preferences for the language, tone,
and format of answers. They apply to all calls.

### Asking questions and choosing sources

Open a call's chat. In **Files and sources**, choose which materials to use:
transcript, notes, linked files and folders. Type `@` to mention one in your
request. Linked files stay in their original locations.

You can also use `@` to mention **other calls in the archive**: type part of the
title or a spoken word and pick the call under **Other calls in the archive**.
Its transcript and notes are added to the context next to the open call, and
stay available for follow-up questions. For example:

> Compare the client's requests with @«Client kickoff · 12/11/2026» and tell
> me what changed.

You can mention up to 8 calls per chat. With Ollama, keep an eye on the
context bar: more calls take more space.

If you uncheck the transcript, it is excluded from subsequent requests, but the
chat history may still contain passages already discussed. To start fresh,
use **Start over**.

### Creating documents

With a provider that supports tools, you can ask for Word documents, Excel
sheets, PowerPoint decks, PDFs, quizzes, or slideshows. Python and the
required libraries are bundled with the installer: Office is not needed.
Generated files appear in the call.

Sensitive operations, such as Bash or PowerShell commands, ask for
confirmation in the chat. You can allow them once or remember the choice, and
revoke it later in the settings.

### Slides with your own template

1. In the chat, open **Files and sources → Add shared files** and select your
   `.pptx` template. It stays available in all chats.
2. In the call, make sure it is checked and mention it with `@` in your request.

> Create a presentation from the transcript. Use @Template.pptx as the style
> reference and include decisions and next steps.

### MCP connectors

With Claude, Codex, and Antigravity, the chat can use the MCP connectors
configured on your account, for example Drive or a CRM. Choose which ones to
enable from the **Connectors** button in the chat. Connectors receive only the
data for operations you authorize.

## Listening to text and transcripts

In **Settings → Voice**, choose the speech engine.

- **Online:** Edge, or ElevenLabs with your own key.
- **Local:** Piper and Kokoro, lightweight and without a GPU; Qwen3-TTS, higher
  quality, optional, Windows x64 only, with high memory requirements (about
  4.5 GB download).
- **Compatible endpoint**, if you run your own server.

Local voices require a model download, then work offline. Audio formats and
word-level timings depend on the engine.

## Sharing

In **Settings → Sharing** you can:

- set a **Shared folder**, for example on Google Drive or a network drive.
  Transcripts you publish appear to colleagues using the same folder, and the
  **Shared** tab shows theirs, searchable by title or author. If you enable
  it, your shared copies are deleted when the call expires;
- send the transcript to a **Slack** or **Teams** channel through a webhook.
  For Teams, use a Workflows flow configured for the channel.

## Data and privacy

- The archive is stored on your computer, **unencrypted**.
- Local transcription, Ollama, and local voices do not send data out.
- Online services receive only the data they need: audio for transcription,
  messages and sources for chat, text for speech.
- API keys, tokens, and webhooks are stored in the system keychain.
- Files linked as sources are not copied: they stay where they are.

[Building and contributing](DEVELOPMENT.md) · [Third-party notices](THIRD_PARTY_NOTICES.md)
