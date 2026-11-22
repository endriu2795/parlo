# Building & Contributing to PARLO

Thanks for your interest in PARLO. This guide covers the codebase layout, local builds on Windows and macOS, tests, forks, and contribution rules.

---

## Architecture

PARLO is a desktop app built with **Tauri 2**, **React 19 + TypeScript**, **Rust**, and a bundled **Python** engine:

- **`src/`** — React + TypeScript frontend (Vite). UI strings live in Italian source keys with English translations in `src/messages.en.json`; locale auto-detects from the OS.
- **`src-tauri/`** — Rust backend: audio capture (WASAPI on Windows, ScreenCaptureKit/CoreAudio on macOS), screen recording, SQLite archive, OS keychain credentials, sharing, updater, and AI provider bridges.
  - Claude Code and Codex run as native child CLIs communicating over stdio (`src-tauri/src/auth.rs`, `process.rs`, `claude_runtime.rs`, `codex.rs`, `codex_runtime.rs`), reusing the user's own CLI login without storing separate OAuth tokens.
- **`engine/`** — Python engine for local Whisper (`whisper.cpp` + `faster-whisper`), CPU/GPU calibration, media import, document generation (`document_tools.py`), local TTS (`speech_tools.py`, `qwen_speech.py`), and optional web search.
- **`scripts/`** — Build, packaging, runtime preparation, release, and QA scripts.

---

## Building from Source

### Windows x64

**Prerequisites:** Git, Node 22, stable Rust (MSVC toolchain), WebView2, and Visual Studio Build Tools with **Desktop development with C++** and the Windows SDK. See the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

```powershell
# 1. Install npm dependencies and local Python runtime (once)
npm.cmd run desktop:deps

# 2. Build debug binary -> src-tauri/target/debug/callnotes.exe
npm.cmd run desktop:build
```

Run with an isolated development archive:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run.ps1
```

Or run with hot reload:

```powershell
npm.cmd run desktop
```

**Optional Vulkan GPU build (whisper.cpp 1.8.3):** place the `whisper.cpp` v1.8.3 sources in `.tools/whisper.cpp-1.8.3`, install CMake + the Vulkan SDK, and run:

```powershell
scripts/build-whisper-cpp.ps1 -VulkanSdk <SDK_PATH>
```

### macOS 13+ (Apple Silicon or Intel)

From the repository root in Terminal:

```sh
# 1. Prepare Node, Python, Rust, and CMake inside .tools/ (once)
#    If macOS prompts for Apple Command Line Tools, finish installing them and rerun.
bash scripts/mac-test.command --deps

# 2. Build PARLO.app -> src-tauri/target/debug/bundle/macos/PARLO.app
bash scripts/mac-test.command --build
```

Repeat step 2 after code changes. Note that physical Mac validation is still ongoing; see [macOS permissions](docs/guide.md#macos-permissions) for first-run setup.

---

## Installers and Forks

- **Windows installer (NSIS):** requires 64-bit Python 3.12. Run `npm.cmd run desktop:installer` → output in `src-tauri/target/release/bundle/nsis`.
- **macOS installer (DMG):** after `--deps`, run `bash scripts/mac-test.command --installer` → output in `src-tauri/target/release/bundle/dmg` (ad-hoc signed for local use; for Developer ID signing & notarization, run `scripts/build-mac.py` with Tauri's Apple signing environment variables).
- **Forks:** To install a fork side-by-side with PARLO, change `productName` and `identifier` in `src-tauri/tauri.conf.json`. For custom auto-updates, configure your own key/feed and pass `-UpdaterConfig` (`scripts/build.ps1`) or `--updater-config` (`scripts/build-mac.py`).

Before redistributing any package, review [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). Maintainers releasing official builds can follow [docs/release.md](docs/release.md).

---

## Running Tests

[CI](.github/workflows/ci.yml) runs on every push and pull request to `main` (Node/TS on Ubuntu, Python on Ubuntu, Rust on Windows). Run the suites relevant to your changes:

```sh
# TypeScript type-check & frontend unit tests
npx --no-install tsc --noEmit
node --test scripts/test-*.mjs

# Python engine & setup/release tests
python -m unittest discover -s engine -p "test_*.py"
# (CI runs: test_worker, test_mixing, test_final, test_transcript_quality, test_dictionary, test_acceleration, test_web_tools)
for f in scripts/test-*.py; do python "$f"; done

# Rust tests (Windows)
cargo test --locked --manifest-path src-tauri/Cargo.toml -- --skip tts::tests::os_vault_roundtrip_uses_an_isolated_test_entry
```

### Optional / environment-dependent checks

- **Documents, video & speech Python suites** (`test_document_tools`, `test_video`, `test_speech_tools`, `test_qwen_speech`): run inside `engine/` using an environment with the corresponding Office/FFmpeg/Piper dependencies installed.
- **Playwright UI smoke tests:** `npm run test:ui` (mocked backend; requires Playwright browser setup).
- **Windows desktop E2E (`scripts/desktop-qa.cjs`):** tests a 480-call synthetic archive, recording, sharing, and streaming chat against a local HTTP mock server. Needs Windows + WebView2, Node 22.13+, and `.tools/engine/benchmark.wav` (whisper.cpp v1.8.3 `samples/jfk.wav`):

```powershell
$env:TAURI_CONFIG = '{"identifier":"app.parlo.desktop.qa","plugins":{"updater":null}}'
cargo build --manifest-path src-tauri/Cargo.toml --bin callnotes
Remove-Item Env:TAURI_CONFIG
node scripts/desktop-qa.cjs          # or pass --recording-only / --chat-only
```

---

## Contributing Guidelines

- Keep PRs focused and include tests for behavioral changes or bug fixes.
- Use **synthetic test data only**. Never commit `.local-data`, `.tools`, built runtimes, recordings, transcripts, API keys, or personal data.
- All contributions to PARLO's original code are licensed under [GPL-3.0-only](LICENSE).
