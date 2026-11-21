# Building and contributing to PARLO

Build PARLO from a terminal in the project folder. Prepare dependencies once;
after code changes, rerun the build command. This guide also covers fork
installers, contributions, and tests.

## macOS (Apple Silicon or Intel)

Use macOS 13 or later, on Apple Silicon or Intel, with a copy of the project
folder on the Mac.

1. Open **Terminal**.
2. Type `cd ` (with a space after `cd`), drag the project folder that contains
   `scripts` into the Terminal window, then press Enter.
3. Prepare the dependencies:

```sh
bash scripts/mac-test.command --deps
```

The command prepares Node, Python, Rust, and CMake inside the project folder.
If macOS asks to install the Apple command-line tools, complete that
installation, then rerun the command. Continue when it prints «Dipendenze
pronte» (dependencies ready).

4. Build the app. The first build downloads additional components and can
   take several minutes:

```sh
bash scripts/mac-test.command --build
```

5. After the build, open the project folder in Finder and go to
   `src-tauri → target → debug → bundle → macos`. Open **PARLO.app**.
   Repeat step 4 after code changes.

For audio permissions, see [PARLO on Mac](MACOS.en.md#first-launch).

## Windows x64

Install Git, Node 22, stable Rust with the MSVC toolchain, WebView2, and
Visual Studio Build Tools with **Desktop development with C++** and the
Windows SDK. See the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

In PowerShell:

```powershell
npm.cmd run desktop:deps
npm.cmd run desktop:build
```

`desktop:deps` installs the npm dependencies and the app's Python runtime.
`desktop:build` builds `src-tauri/target/debug/callnotes.exe`. To run the app
with development data:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run.ps1
```

For live reload, run `npm.cmd run desktop`.

To build the Vulkan GPU runtime, run
`scripts/build-whisper-cpp.ps1 -VulkanSdk SDK_PATH` with the Vulkan SDK, CMake
and the whisper.cpp 1.8.3 sources in `.tools/whisper.cpp-1.8.3`.

## Building an installer for your fork

- **Windows**: `npm.cmd run desktop:installer`. The setup file is written to
  `src-tauri/target/release/bundle/nsis`. Packaging also requires 64-bit
  Python 3.12.
- **macOS**: after preparing the dependencies, run
  `bash scripts/mac-test.command --installer`. The DMG is written to
  `src-tauri/target/release/bundle/dmg`. The ad hoc signature is for local
  use. For a Developer ID build, use `scripts/build-mac.py` with
  your signing identity and notarization settings.

To install a fork alongside PARLO, give it a distinct name and identifier in
`src-tauri/tauri.conf.json`. For fork updates, configure your own key and feed
and pass `-UpdaterConfig` to `scripts/build.ps1` or `--updater-config` to
`scripts/build-mac.py`. See the [Tauri updater guide](https://v2.tauri.app/plugin/updater/).

## First run

Choose the recording sources and assistant in the app settings. Follow
[Getting started](README.en.md#getting-started) for the first recording and
the [user guide](USO.en.md) for the other features.

## Contributing

Use synthetic test data. Keep commits limited to code and documentation;
exclude `.local-data`, `.tools`, prepared runtimes, recordings, transcripts,
credentials, and keys. Contributions to the project code use
[GPL-3.0-only](LICENSE).

## Test suite

Run the checks that cover the behavior you changed. For a regression, fix the
code; for an intentional behavior change, update the expected result and add
coverage where needed. The sections below list the CI suites and checks that
require additional tools or hardware.

### Tests run in CI

[CI](.github/workflows/ci.yml) runs on pushes to `main` and on pull requests
targeting `main`. Jobs run in parallel. Timings and results are in the
[GitHub Actions logs](https://github.com/endriu2795/parlo/actions/workflows/ci.yml).

- **TypeScript and Node on Linux.** TypeScript type-checks the frontend.
  In `scripts`, `test-archive-dates.mjs` checks date ranges and time zones,
  `test-transcript-navigation.mjs` passage selection,
  `test-annotations.mjs` drawing and resizing,
  `test-file-links.mjs` file references and
  `test-source-mentions.mjs` mentions and shared sources.
- **Rust on Windows x64.** The job builds the test binary. Tests cover search,
  pagination, SQLite, sharing, queueing and cancellation, crash recovery, tool
  permissions, streaming chat against a test server, audio and video logic,
  and updates.
- **Python engine on Linux.** `test_worker` checks word grouping,
  `test_mixing` mixing and echo, `test_final` final transcription and error
  recovery, `test_transcript_quality` repetitions, `test_dictionary` lexical
  hints, `test_acceleration` CPU/GPU selection and `test_web_tools` search
  with mocked responses.
- **Python scripts on Linux.** `test-download-network.py` tests download
  resumption, `test-install-engine.py` runtime installation and restore,
  `test-model-setup.py` model selection and download,
  `test-prepare-runtime.py` package integrity and `test-local-release.py` a
  simulated release preparation and publication. The three Windows dependency
  bundle cases run on Windows.

### Automated tests to run when needed

- **Documents, video and speech.** `test_document_tools` covers Word, Excel,
  PDF and slideshows. `test_video` covers annotations, recovery from capture
  errors and audio-video sync. `test_speech_tools` covers speech synthesis,
  decoding and alignment. `test_qwen_speech` covers text splitting, model reuse
  and CPU/CUDA handling with mocks. Run them from `engine` with
  `python -m unittest` followed by the module name, using a Python
  environment that has the component's dependencies. CI runs only the Python
  suites listed above. Documents and speech also require Office/PDF libraries,
  FFmpeg or Piper.
- **Browser UI.** `npm run test:ui` uses Playwright and a mocked backend to
  check the archive, filters, shortcuts, settings and the recording indicator.
  It requires a browser set up for Playwright.
- **Runtime, audio and performance.** The native Whisper runtime tests and
  `scripts/benchmark-engine.py` require the runtime and a local model. The
  benchmark uses a public sample. `scripts/audio-input-check.ps1` checks the
  default microphone. The Rust test `default_devices_deliver_wasapi_packets`
  runs with `--ignored` while a 997 Hz tone is playing. The keychain test uses
  the test machine's keychain.

#### Automated end-to-end check of the Windows app

`scripts/desktop-qa.cjs` runs fixed scenarios against a temporary archive of
480 synthetic calls with predefined titles and dates and random IDs. It
checks pagination, search beyond loaded pages, scrolling and frame times,
recording of a public sample, cancellation and retry, shared-file updates,
and streaming chat with the transcript included and excluded.

The full run requires **Windows with WebView2, Node 22.13 or later, npm
dependencies, the local Whisper runtime with a model and a WAV sample, and a
default audio output**. To run it from the project root in PowerShell:

```powershell
$env:TAURI_CONFIG = '{"identifier":"app.parlo.desktop.qa","plugins":{"updater":null}}'
cargo build --manifest-path src-tauri/Cargo.toml --bin callnotes
Remove-Item Env:TAURI_CONFIG
node scripts/desktop-qa.cjs
```

The sample `.tools/engine/benchmark.wav` is `samples/jfk.wav` from whisper.cpp
v1.8.3. The acceleration setup prepares it. If it is missing, download it with:

```powershell
Invoke-WebRequest 'https://raw.githubusercontent.com/ggml-org/whisper.cpp/v1.8.3/samples/jfk.wav' -OutFile .tools/engine/benchmark.wav
```

The chat uses a local HTTP test server. To run part of the scenarios, pass:

- `--recording-only` for recording, cancellation, and retry.
- `--chat-only` for the shared folder and chat; this mode does not require
  Whisper or an audio output.

### Manual checks

- **Install and update.** Install the package, start the app and, when
  available, test updating from a previous version.
- **Recording.** On Mac, check the system permissions on first launch. Test
  the microphone, system audio, device switching and screen capture with
  annotations. Check transcription quality and speed with the available model
  and GPU.
- **Sharing.** If you use Google Drive for the shared folder, publish a test
  transcript and check that it appears on the other computer.
- **Assistant.** Try Ollama on a long transcript and check response time and
  quality. If you use a custom endpoint, send a request to your server.

Before redistributing a package, review the
[third-party licenses](THIRD_PARTY_NOTICES.md).

Maintainers can follow the [official release guide](RELEASE.md).
