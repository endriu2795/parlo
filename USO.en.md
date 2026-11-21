# PARLO user guide

[Italiano](USO.md)

Learn how to record a call, work with the transcript, and share it with your
team. For installation, see the [README](README.en.md).

## Recording a call

1. In **Settings → Recording**, choose the microphone, system audio, or both.
   To record the people you are talking to, pick the same audio output used
   by Teams, Zoom, or your browser.
2. Click **Start recording** or use the shortcut set in **Settings → Shortcuts**.
   The shortcut also works while PARLO is in the background.
3. During the call you can write notes, capture an area of the screen, and
   draw annotations on top of other windows. Screenshots are added to the
   transcript.
4. Click **Stop recording**. PARLO processes the audio and saves the
   transcript in the archive.

### Audio devices on Windows and Mac

On Windows, choose a microphone and audio output in PARLO or use the system
defaults. With the defaults selected, recording follows changes in Windows
audio settings, for example when switching to a headset. If you select a
specific device, PARLO keeps using that device.

On Mac, the microphone follows the default macOS input, including changes
during a call. System audio is captured through ScreenCaptureKit, without
selecting an output in PARLO.

### Recording screen video

On Windows you can also record the screen, or part of it, with the
annotations: in **Settings → Archive**, choose **Text and video**. Video takes
more disk space and resources. This feature is not yet available in the Mac
version.

## Transcription

**Settings → Recording → Where to process audio** offers three options.

- **On this computer.** Whisper processes audio on your computer and works
  offline after the model download. Choose **Fast**, **Balanced**, or
  **Highest accuracy**. PARLO picks CPU or GPU based on performance. You can
  enable **Show text while recording** to read a preview during the call.
- **Online with Groq.** Enter your key from
  [console.groq.com](https://console.groq.com). If the
  connection drops, the app retries only the affected part.
- **Custom endpoint.** Connect a service other than Groq, including a Whisper
  server on your computer or network. Install or prepare the server
  separately; it must accept `POST audio/transcriptions` requests with
  multipart WAV audio and return `verbose_json` with timestamps. Enter the
  base URL, the exact ID of a model available on the server, and an API key if
  required. **Test endpoint** sends a short audio sample to check the setup.

In **Call language** you can keep automatic detection or set a fixed language.

In **Settings → Dictionary**, add recurring names of people, companies,
products, or acronyms. The dictionary helps recognize these terms; it does
not replace words in the transcript automatically.

If processing fails, the audio is kept. Open the call and click
**Try again**, optionally after switching service.

### Importing audio and video

With **Import audio or video**, PARLO extracts the audio from the file and
transcribes it. The original file stays where it is.

### Receiving voice messages from Telegram

In **Settings → Telegram**, create a bot with `/newbot` in @BotFather and
paste its token into PARLO. Select **Continue**, then **Open bot in Telegram**
and **Start** in the chat. Once PARLO shows the bot as connected, you can send
voice messages, audio, or video files (up to 20 MB) to the bot, even from your
phone, and PARLO transcribes them with the current settings. If the app is
closed, messages are fetched when it reopens, within Telegram's 24-hour limit.

## Finding and replaying calls

Assign tags manually: open a call, click **Add tag**, type a label, and press
Enter. Use them for clients, projects, or topics.

Archive search covers titles, tags, notes, and transcribed text. Narrow the
results by date and by the tags you assigned. In an open call, search for a
passage, play back saved audio, change the playback speed, and click a
timestamp to jump to that point. **Export** saves the transcript as Markdown.

### Data retention

In **Settings → Archive**, choose what to keep (**Text only**,
**Text and audio**, or **Text and video** on Windows). This applies to new
recordings; with **Text only**, audio is deleted when the transcript is ready.

Under **Automatically delete after**, choose 1, 3, 6, 12, or 24 months. Expiry
applies to the recording with its transcript, chat, notes, screenshots, and
saved audio or video. Choose **Never** to keep data until you delete it.
**Delete expired recordings now** runs cleanup using the selected expiry.

## Using the assistant

Choose the provider in **Settings → Assistant**.

- **Claude:** sign in with your Claude account. PARLO uses Claude Code and
  downloads it if it is not installed.
- **Codex:** sign in with your OpenAI account. PARLO uses the Codex CLI.
- **Ollama:** choose a model on your computer or your own server. Creating
  documents and running operations requires a model that supports tools.
- **Custom endpoint:** enter the base URL, model ID, and API key if required
  for a server compatible with OpenAI's chat API.

With Ollama, requests go to the server you configured, on the same computer
or another machine. If the server does not respond, PARLO stops the request.
You can adjust the conversation memory; for long calls, you can use a context
of at least 32k tokens if the model and your memory allow it.

Web search is off by default for Ollama. If you enable **Search and open web
pages**, searches go through DuckDuckGo and each page's domain is sent to
Anthropic for a security check before the page is opened.

In **Project instructions** you can set preferences for the language, tone,
and format of answers. They apply to all calls.

### Asking questions and choosing sources

Open a call's chat. In **Files and sources**, choose which materials to use:
transcript, notes, linked files and folders. Type `@` to mention one in your
request. Linked files stay in their original locations.

Add context such as a project brief or documentation, and references for the
style and format of the output, such as a sample document, editorial
guidelines, or a presentation template.

You can also use `@` to mention **other calls in the archive**: type part of the
title or a spoken word and pick the call under **Other calls in the archive**.
Its transcript and notes become available to the assistant alongside the
open call, including for follow-up questions. For example:

> Compare the client's requests with @«Client kickoff · 12/11/2026» and tell
> me what changed.

You can mention up to 8 calls per chat. With Ollama, the context bar shows
how much conversation memory the sources and messages use.

If you uncheck the transcript, it is excluded from subsequent requests, but the
chat history may still contain passages already discussed. To start fresh,
use **Start over**.

### Creating documents

With a provider that supports tools, you can ask for Word documents, Excel
sheets, PowerPoint decks, PDFs, quizzes, or slideshows. Python and the
required libraries are bundled with the installer: Office is not needed.
Generated files appear in the call.

When the assistant asks to run Bash or PowerShell commands, you can approve
them in the chat once or remember the choice. You can revoke permissions in
the settings.

### Using files for context or as references

1. In **Files and sources**, use **Add files** or **Add folder** to link
   materials to the call. Use **Add shared files** or **Add shared folder**
   to make them available in all your chats.
2. Select the sources to use and mention them with `@`, explaining how they
   should guide the response.

For example, use a `.pptx` template as a reference for a presentation:

> Create a presentation from the transcript. Use @Template.pptx as the style
> reference and include decisions and next steps.

Or combine background information and writing instructions:

> Summarize the call. Use @Brief.pdf for context and follow the tone and
> structure of @Example.docx.

### MCP connectors

With Claude and Codex, the chat can use the MCP connectors
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

## Sharing with your team

### Shared folder on Drive, OneDrive, or a local network

1. Create a team folder and give colleagues permission to read and publish
   files. With Google Drive or OneDrive, use the desktop client to make the
   folder accessible from File Explorer or Finder.
2. In **Settings → Sharing**, enter your display name and choose the folder.
   Each colleague selects the same shared folder on their own computer; the
   local paths can differ.
3. Enable **Show other people’s transcripts in the sidebar** to browse the team's
   publications in the **Shared** tab.
4. To publish, open a call and select the folder from the **Share** menu.
   Enable **Publish automatically** to publish each new completed transcript.

PARLO writes a Markdown file for each published transcript. Drive or OneDrive
syncs it with colleagues; files on a network folder are accessible directly.
In the **Shared** tab, search by title or author, read the text, and open a
chat with the assistant.

Sharing includes the transcript text. Audio, video, attachments, and chats
stay in each person's archive. Markdown files can also be read without PARLO.

The **Delete my shared copies when the call expires** option applies archive
expiry to your publications, including those already in the folder.
Deletions are synced with the team.

### Slack and Teams channels

In **Settings → Sharing**, enable the channel and save its webhook. For
Teams, use the webhook of a Workflows flow configured for the channel.
Publish from the call's **Share** menu or enable **Publish automatically**
for that destination.

## Data and services

### Local archive

Recordings, transcripts, and chats are stored on your computer. PARLO does
not encrypt archive files itself; disk protection is configured through your
operating system's security settings. In **Settings → Archive**, choose what
to keep and for how long.

Linked source files stay in their original locations. API keys, tokens, and
webhooks are stored in the system keychain.

### Processing and sharing

Whisper, Ollama on the same computer, and local voices can process content
offline after setup and model downloads. If you configure Ollama on a server,
requests are sent to that server.

Online services receive the content of the operation: audio for
transcription, chat messages and sources for the assistant, text for speech.
Web search and connectors use the services described in their sections.
Voice messages and files sent to the bot go through Telegram; published
transcripts follow the sharing destinations and permissions you choose.
Each external service has its own terms and pricing.

[Building and contributing](DEVELOPMENT.md) · [Third-party notices](THIRD_PARTY_NOTICES.md)
