# Third-Party Licenses & Notices

PARLO's original code is licensed under [GPL-3.0-only](LICENSE). Bundled runtimes, downloadable models, and external services keep their own licenses and terms. Preserve original copyright notices and attributions when redistributing.

## Fonts

| Component | License | Reference |
| --- | --- | --- |
| **Inter** | SIL OFL 1.1 | [public/fonts/LICENSE-inter.txt](public/fonts/LICENSE-inter.txt) |
| **Bricolage Grotesque** | SIL OFL 1.1 | [public/fonts/LICENSE-bricolage.txt](public/fonts/LICENSE-bricolage.txt) |

## Transcription, Media & Runtime

| Component | Version | License | Reference |
| --- | --- | --- | --- |
| **whisper.cpp** | 1.8.3 | MIT | [licenses/whisper-cpp.txt](licenses/whisper-cpp.txt) · [Source](https://github.com/ggml-org/whisper.cpp/tree/v1.8.3) |
| **faster-whisper** | 1.2.1 | MIT | [licenses/faster-whisper.txt](licenses/faster-whisper.txt) · [Source](https://github.com/SYSTRAN/faster-whisper/tree/v1.2.1) |
| **PyAV** | 16.0.1 (Win) / 15.1.0 (Mac) | BSD-3-Clause | [licenses/pyav.txt](licenses/pyav.txt) · [Source](https://github.com/PyAV-Org/PyAV) |
| **imageio-ffmpeg** | 0.6.0 | BSD-2-Clause | [licenses/imageio-ffmpeg.txt](licenses/imageio-ffmpeg.txt) · [Source](https://github.com/imageio/imageio-ffmpeg) |
| **FFmpeg** (via PyAV / imageio-ffmpeg) | Per wheel build | LGPL / GPL | [FFmpeg legal](https://ffmpeg.org/legal.html) |
| **CPython** (`python-build-standalone`) | Pinned in [engine/python-runtime.json](engine/python-runtime.json) | PSF License | [Python license](https://docs.python.org/3/license.html) · [Distribution](https://github.com/astral-sh/python-build-standalone) |

Full Python dependency lockfiles live in [engine/requirements-lock.txt](engine/requirements-lock.txt) (transcription), [engine/documents-requirements-lock.txt](engine/documents-requirements-lock.txt) (documents/media, including the Pillow 11.3.0 fix for [CVE-2026-112399](https://github.com/python-pillow/Pillow/security/advisories/GHSA-5xm2-6676-qcp9) applied in [scripts/prepare-runtime.py](scripts/prepare-runtime.py)), and [engine/qwen-tts-requirements.txt](engine/qwen-tts-requirements.txt). Keep package `dist-info` metadata and license files intact when preparing runtimes.

## Local Speech & Models

| Component | Version | License | Reference |
| --- | --- | --- | --- |
| **Piper (`piper-tts`)** | 1.8.0 | GPL-3.0-or-later | [OHF-Voice/piper1-gpl](https://github.com/OHF-Voice/piper1-gpl) · [engine/PIPER-LICENSES.md](engine/PIPER-LICENSES.md) |
| **Piper Serena voices** (downloaded on demand) | Pinned in `engine/piper-voices.json` | CC-BY-4.0 | [engine/PIPER-LICENSES.md](engine/PIPER-LICENSES.md) |
| **Kokoro ONNX** | 0.6.1 | MIT | [licenses/kokoro-onnx.txt](licenses/kokoro-onnx.txt) · [Source](https://github.com/thewh1teagle/kokoro-onnx) |
| **Phonemizer** (used by Kokoro) | 3.4.0 | GPL-3.0-or-later | [bootphon/phonemizer](https://github.com/bootphon/phonemizer) |
| **eSpeak NG** (via Piper & `espeakng-loader`) | Per bundled wheel | GPL-3.0-or-later | [espeak-ng/espeak-ng](https://github.com/espeak-ng/espeak-ng) |
| **espeakng-loader** | 0.2.4 | MIT | [licenses/espeakng-loader.txt](licenses/espeakng-loader.txt) · [Source](https://github.com/thewh1teagle/espeakng-loader) |
| **Qwen3-TTS** (optional download) | `qwen-tts` 0.1.1 / 12Hz 1.7B | Apache-2.0 | [engine/QWEN-LICENSES.md](engine/QWEN-LICENSES.md) · [engine/QWEN-MODEL-LICENSE.txt](engine/QWEN-MODEL-LICENSE.txt) |

Pinned PyPI versions, source URLs, and hashes for Python speech/media wheels are tracked in [licenses/python-sources.json](licenses/python-sources.json).

## Frontend & Backend Dependencies

Exact versions (including transitive dependencies) are pinned in [package-lock.json](package-lock.json) and [src-tauri/Cargo.lock](src-tauri/Cargo.lock). Core libraries include:

- **React, React DOM, react-markdown, remark-gfm** — MIT
- **Lucide** — ISC
- **Tauri & Tauri plugins** — MIT / Apache-2.0
- **SQLite** — Public Domain (Rust bindings keep their respective licenses)

## External CLIs & Online Services

- **Claude Code** is proprietary and governed by [Anthropic's terms](https://code.claude.com/docs/en/legal-and-compliance). PARLO invokes the official CLI binary (also used for tool execution with Ollama). `@anthropic-ai/claude-agent-sdk` is a dev-only verification dependency.
- **Codex**, **Ollama**, and their models keep their own licenses; pinned CLI manifests live in `engine/*-runtime.json`.
- **Groq**, **Microsoft Edge TTS**, **ElevenLabs**, **Telegram**, **Slack**, and **Teams** are external services governed by their own terms and pricing.

## Redistributing Installers

When distributing compiled installers or binaries:

1. Include [LICENSE](LICENSE), this notice file, and the license/NOTICE texts of all bundled dependencies (including transitive ones).
2. Provide the exact corresponding source code, any modifications, and build scripts alongside distributed GPL components — especially bundled native binaries such as **eSpeak NG**, **FFmpeg**, and **CPython** (note that PyPI does not publish an sdist for `espeakng-loader` 0.2.4, so match its wheel to its wrapper source and embedded eSpeak NG version).
3. Keep license files and attributions alongside any bundled models.
