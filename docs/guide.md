# PARLO User Guide

[Italiano](guide.it.md) · [Back to README](../README.md)

Everything you need to set up recording, transcription, the AI assistant, voice output, and team sharing.

---

## Recording & Audio

Configure audio sources in **Settings → Recording** and set a global shortcut in **Settings → Shortcuts** to start/stop recording from any app. During a call you can take notes, capture screen regions (automatically attached to the transcript), and draw annotations over any window.

- **Windows:** Choose specific mic/output devices or leave the system defaults so PARLO automatically follows device changes (e.g., plugging in a headset). To capture other participants, make sure system audio points to the same output used by Teams, Zoom, or your browser.
- **Screen video (Windows only):** Switch **Settings → Archive** to **Text and video** to record your screen or a selected region together with annotations.
- **macOS:** The microphone automatically follows macOS's default input, even if switched mid-call. System audio is captured via ScreenCaptureKit with no extra virtual audio driver.

### macOS permissions

On first recording, grant **Microphone** and **Screen Recording** in **System Settings → Privacy & Security**. macOS requires Screen Recording permission for ScreenCaptureKit even when capturing audio only.

If permissions break after rebuilding or updating an ad-hoc build, click **Reset permissions and restart** in **Settings → Recording** and re-approve when prompted.

---

## Transcription

Pick your engine under **Settings → Recording → Where to process audio**:

| Option | How it works |
| --- | --- |
| **On this computer** | Runs Whisper offline once downloaded (**Fast / Tiny**, **Balanced / Small**, **Highest accuracy / Turbo**). PARLO automatically benchmarks and picks GPU or CPU. Enable **Show text while recording** for a live preview during the call. |
| **Online with Groq** | Uses your API key from [console.groq.com](https://console.groq.com). Automatically retries failed chunks if the connection drops. |
| **Custom endpoint** | Connects to any OpenAI-compatible `POST audio/transcriptions` server (e.g., local or LAN Whisper/Speaches). Must accept multipart WAV and return `verbose_json` with timestamps. Use **Test endpoint** to verify. |

- **Language & Dictionary:** Keep auto-detect or lock a call language. Add recurring names, acronyms, and product terms in **Settings → Dictionary** to bias recognition (it guides Whisper rather than doing blind find-and-replace).
- **Retry on failure:** If transcription fails, the audio is preserved — open the call and click **Try again** (you can switch providers first).
- **Import audio/video:** Use **Import audio or video** to transcribe existing files without moving or modifying the originals.

### Personal Telegram bot

Send voice notes, audio, or video files (up to 20 MB) from your phone and have PARLO transcribe them automatically:

1. In **Settings → Telegram**, open `@BotFather`, run `/newbot`, and paste the token into PARLO.
2. Click **Continue** and **Copy command**. Open the bot’s private chat in Telegram, press **Start** if prompted, then paste and send the command. If the code expires, choose **Generate a new code** in PARLO.
3. Once connected, anything you send to the bot lands in your PARLO archive. If PARLO is closed, pending messages are picked up on next launch (within Telegram's 24-hour queue window).

---

## Archive & Retention

- Search covers titles, manual tags, notes, and full transcripts, with filters for tags and dates.
- Click any timestamp to jump audio playback to that exact moment, adjust playback speed, or export the transcript as Markdown.
- In **Settings → Archive**, choose what new calls keep (**Text only**, **Text and audio**, or **Text and video** on Windows) and set automatic cleanup (**1–24 months** or **Never**). Expiry removes the recording along with its transcript, notes, screenshots, chat, and media.

---

## AI Assistant

Configure a provider in **Settings → Assistant**:

- **Claude** — Signs in with your Claude account via the native Claude Code CLI (downloaded automatically if missing).
- **Codex** — Signs in with your OpenAI account via the Codex CLI.
- **Ollama** — Connects to a local or remote Ollama server. Choose a tool-capable model if you want document generation. For long meetings, set conversation memory to at least **32k tokens** (hardware permitting). Web search is off by default; enabling it uses DuckDuckGo and sends fetched page domains to Anthropic for a safety check.
- **Custom endpoint** — Any OpenAI-compatible chat API (base URL, model ID, optional API key).

Use **Project instructions** to set global preferences (language, tone, output structure) across all calls.

### Sources, templates & cross-call `@` mentions

In any call's chat, open **Files and sources** to toggle the transcript, notes, or linked files/folders (linked files stay in place; use **Shared files/folders** to make references available across every chat).

Type `@` to target specific sources — or mention **up to 8 other calls** from your archive:

> Compare the client's requirements with `@«Client kickoff · 12/11/2026»` and list what changed.

> Turn this transcript into a deck using `@Template.pptx` for style, highlighting decisions and next steps.

> Summarize the meeting using `@Brief.pdf` for context and following the structure of `@Sample.docx`.

Unchecking the transcript excludes it from future turns; use **Start over** to clear the existing chat history.

### Document generation & tools

With a tool-capable provider, ask the assistant to create **Word (`.docx`), Excel (`.xlsx`), PowerPoint (`.pptx`), PDF, quizzes, or slideshows** — Python and all required libraries ship with PARLO, so Microsoft Office is not needed. Generated files stay attached to the call.

- **Shell approvals:** When the assistant requests a Bash or PowerShell command, approve it once or remember the rule (revocable in settings).
- **MCP connectors:** With Claude and Codex, enable the MCP servers configured on your account (e.g., Google Drive, CRM) from the **Connectors** button in chat.

---

## Text-to-Speech (Voice)

Enable speech output in **Settings → Voice** to turn chat texts or scripts into MP3 audio:

- **Local & offline:**
  - **Piper** — Fast, lightweight, CPU-only (Italian voices).
  - **Kokoro** — CPU-only, different voice timbre (~354 MB download, slower than Piper).
  - **Qwen3-TTS** *(optional, Windows x64 only)* — Expressive multilingual voices, requires ~4.5 GB download and strong hardware (NVIDIA 8 GB+ VRAM recommended; CPU mode needs ~32 GB RAM and is slow).
- **Online:** **Edge** (free, no key, sends text to Microsoft), **ElevenLabs** (uses your API key and credits), or a **Custom endpoint**.

---

## Team Sharing

### Shared folder (Google Drive, OneDrive, or LAN)

PARLO lets teams share transcripts without hosting a server:

1. Create a shared folder accessible from each teammate's File Explorer or Finder (via Google Drive for Desktop, OneDrive, or a network drive).
2. In **Settings → Sharing**, set your display name, pick that folder, and turn on **Show other people's transcripts in the sidebar**.
3. Open any call and click **Share** (or enable **Publish automatically**).

Teammates see published calls in the **Shared** tab, can search by title or author, and can chat with the AI over them. Files are plain Markdown, readable even outside PARLO.

Only the **transcript text** is shared — audio, video, attachments, and personal chats stay private in your local archive. Enable **Delete my shared copies when the call expires** if you want archive retention to clean up your published files too.

### Slack & Teams webhooks

In **Settings → Sharing**, enable Slack or Teams and paste a channel webhook URL (for Teams, use a Workflows webhook). Publish manually from a call's **Share** menu or turn on automatic publishing.

---

## Data and Privacy

- **Local first:** Recordings, transcripts, notes, screenshots, and chats live in your local archive. PARLO does not add its own encryption layer on top of disk encryption (use BitLocker / FileVault for full-disk protection).
- **Credentials:** API keys, Telegram tokens, and webhook URLs are stored in the OS keychain.
- **Offline vs. online:** Local Whisper, local Ollama, and local voices never send data off your machine after model download. When you choose an online service (Groq, Claude, Codex, remote Ollama, ElevenLabs, Edge, Telegram, Slack/Teams, web search, or MCP connectors), only the content required for that action is sent under that provider's terms.
