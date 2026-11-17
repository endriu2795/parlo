# Third-party licenses and components

PARLO's original code is licensed under **GPL-3.0-only**; the full text is in
[LICENSE](LICENSE). This choice does not change the licenses of external
components, downloadable models, or connected services. Original copyright
notices and attributions must be preserved.

- **Inter** and **Bricolage Grotesque** use the SIL Open Font License 1.1.
  [Inter license](public/fonts/LICENSE-inter.txt) and
  [Bricolage Grotesque license](public/fonts/LICENSE-bricolage.txt).

## Local speech

- **Piper (`piper-tts` 1.8.0)**: GPL-3.0-or-later, project
  [OHF-Voice/piper1-gpl](https://github.com/OHF-Voice/piper1-gpl).
  The GPLv3 text is also in [LICENSE](LICENSE). Piper includes eSpeak NG.
  The Serena voices are downloaded separately and declared CC-BY-4.0:
  authors, model cards, and revision are in [PIPER-LICENSES.md](engine/PIPER-LICENSES.md).
- **Kokoro ONNX 0.6.1**: MIT, copyright 2025 github.com/thewh1teagle;
  [license text](licenses/kokoro-onnx.txt) and
  [sources](https://github.com/thewh1teagle/kokoro-onnx).
  Its license does not automatically cover the models or dependencies.
- **Phonemizer 3.4.0**, used by Kokoro: GPL-3.0-or-later, copyright of the
  project authors, including Mathieu Bernard and Hadrien Titeux.
  [Sources and copyright](https://github.com/bootphon/phonemizer),
  [GPLv3 text](LICENSE).
- **eSpeak NG**, included through Piper and `espeakng-loader`:
  [GPLv3 license and sources](https://github.com/espeak-ng/espeak-ng).
  The `espeakng-loader` 0.2.4 wrapper is MIT-licensed:
  [text](licenses/espeakng-loader.txt),
  [sources](https://github.com/thewh1teagle/espeakng-loader).
  The wrapper's license does not replace that of the bundled library.
- **Qwen3-TTS**: optional environment, downloaded separately. Code, Apache-2.0
  model, and revision are described in [QWEN-LICENSES.md](engine/QWEN-LICENSES.md);
  [Apache-2.0 text](engine/QWEN-MODEL-LICENSE.txt).

Version metadata for the Python packages Piper, Phonemizer, Kokoro,
espeakng-loader, and imageio-ffmpeg, with the source URLs and hashes published
on PyPI, is collected in [licenses/python-sources.json](licenses/python-sources.json).
PyPI does not publish a source archive for espeakng-loader 0.2.4: the link
between the wheel, the wrapper sources, and the embedded eSpeak must be
completed before binary distribution. No models or user data were downloaded.

## Transcription, documents, and media

- **whisper.cpp 1.8.3**: MIT, copyright 2023-2024 The ggml authors;
  [license](licenses/whisper-cpp.txt),
  [sources of the version used](https://github.com/ggml-org/whisper.cpp/tree/v1.8.3).
  The native scripts also keep `LICENSE-whisper.txt` in the runtime.
- **faster-whisper 1.2.1**: MIT, copyright 2023 SYSTRAN;
  [license](licenses/faster-whisper.txt),
  [sources](https://github.com/SYSTRAN/faster-whisper/tree/v1.2.1).
- **PyAV 16.0.1 (Windows) / 15.1.0 (Mac)**: BSD-3-Clause, copyright of the
  original authors; [license](licenses/pyav.txt),
  [sources](https://github.com/PyAV-Org/PyAV).
  The FFmpeg libraries contained in the wheels keep their own terms.
- **FFmpeg**, distributed through `imageio-ffmpeg` and the PyAV libraries:
  LGPL or GPL depending on the actual configuration of the binaries.
  [Official terms](https://ffmpeg.org/legal.html).
  The `imageio-ffmpeg` 0.6.0 wrapper is BSD-2-Clause:
  [license](licenses/imageio-ffmpeg.txt),
  [sources](https://github.com/imageio/imageio-ffmpeg).
  The wrapper's license is not sufficient to describe the FFmpeg binary.
- **CPython**, with version and distribution pinned in
  [python-runtime.json](engine/python-runtime.json):
  [Python license](https://docs.python.org/3/license.html),
  [python-build-standalone distribution](https://github.com/astral-sh/python-build-standalone).
  The licenses of the runtime's native libraries must also be preserved.

The list of Python packages and versions is in the lockfiles for
[transcription](engine/requirements-lock.txt) and
[documents/media](engine/documents-requirements-lock.txt).
The documents/media runtime includes Pillow 11.3.0 with the fix for
[CVE-2026-112399](https://github.com/python-pillow/Pillow/security/advisories/GHSA-5xm2-6676-qcp9),
applied during [runtime preparation](scripts/prepare-runtime.py).
The optional Qwen environment uses [qwen-tts-requirements.txt](engine/qwen-tts-requirements.txt).
Packages keep their own license files and `dist-info` metadata; do not remove
them when preparing a runtime.

## Interface and backend

The complete dependencies and versions, including transitive ones, are pinned
in [package-lock.json](package-lock.json) and [Cargo.lock](src-tauri/Cargo.lock).
Main components include:

- React/React DOM, react-markdown, and remark-gfm: MIT.
- Lucide: ISC, with any attributions for derived icons.
- Tauri and its plugins: MIT or Apache-2.0, depending on the package.
- SQLite: public domain; the Rust binding and other crates keep their own
  licenses.

This document is an index of the main components, not a complete inventory of
the licenses of every binary. A distribution must also collect the license
texts and any NOTICE files of the transitive dependencies actually included;
a lockfile alone does not replace them.

## External programs and services

Claude Code remains proprietary and subject to
[Anthropic's terms](https://code.claude.com/docs/en/legal-and-compliance).
The project uses the official CLI, installed separately or downloaded from its
distributor; it does not relicense it under the GPL. Ollama also uses it for
tools. The Agent SDK npm package is a development dependency with its own terms.

Codex, Ollama, and their models keep their own licenses. The revisions of the
runtimes prepared by the app are in the `engine/*-runtime.json` manifests.
Groq, Edge, and ElevenLabs are external services: access follows the terms and
any costs of the user's account.

## When distributing an installer

The choice of public repository and update channel is separate from this
document. Before distributing a new package:

1. Ship it with `LICENSE`, this index, and the license/NOTICE texts of the
   included dependencies.
2. Make the exact sources of the distributed GPL code, any modifications, and
   the scripts needed to rebuild it available next to the download.
   A generic link to a project's main branch does not identify those sources.
3. Tie each embedded native component to its version and configuration:
   in particular eSpeak NG, FFmpeg, the Python runtime, and its libraries.
   The Python sources in `licenses/python-sources.json` do not replace those
   of the native libraries already compiled into the wheels.
4. Preserve the licenses and attributions of the models actually distributed;
   models downloaded separately keep their own terms.

These steps concern whoever distributes the packages. Users of the app do not
need to register to read the licenses or perform any other operation.
The final collection for the installers is to be completed during the release
phase; no binary packages were verified or generated with this change.
