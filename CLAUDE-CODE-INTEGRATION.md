# Claude Code integration

This document describes how PARLO uses Claude Code for the chat. It is meant
for developers and for diagnosing sign-in problems.

The app is **React + Tauri**. The frontend calls Tauri commands; the **Rust
backend starts the native `claude` CLI as a child process** and talks to it
over stdin/stdout. The `@anthropic-ai/claude-agent-sdk` dependency (0.3.296)
is used only by verification scripts: **the app flow does not use the SDK**.

## Installation and sign-in

- If `claude` is not available, the app downloads the official Claude Code
  **2.1.296** binary into its own `engine` folder, checks its size and SHA-256,
  and stores the path. On Windows it uses `claude.exe`, not the npm `.cmd`
  launcher.
- To check the account it runs `claude auth status` and reads `loggedIn` from
  the returned JSON.
- To sign in it starts `claude auth login` and **keeps that process open**.
  The CLI handles OAuth and prints the sign-in link; Rust reads it from stdout
  and shows it in the app. The “Apri la pagina di accesso” (open sign-in page)
  button opens that link in the system browser (`explorer.exe` on Windows,
  `open` on macOS).
- If Claude shows a code in the browser, the app writes it to the **stdin of
  the same `claude auth login` process**, followed by a newline and a flush.
  When the process exits, it runs `claude auth status` again to confirm the
  sign-in.

The app **does not implement OAuth or store its own tokens**: it reuses the
authentication saved by the CLI for the same system user. It does not set a
separate `CLAUDE_CONFIG_DIR`. To reproduce the sign-in you therefore need the
same binary, the same user environment, and a process with stdin/stdout
attached while the browser is open.

## Chat

The chat starts `claude -p` with `--input-format stream-json`,
`--output-format stream-json`, `--permission-prompt-tool stdio`, `--model` and
`--system-prompt`. It sends JSON messages on stdin, receives replies and events
on stdout, and handles tool permission requests in Rust. The code avoids
`--bare`, which here would disable subscription-based access.

**If the browser sign-in does not start:** first check that
`claude auth login` works with the same executable and user as the app. The
current parser reads the link **only from stdout, line by line**, and accepts
`claude.ai` or `console.anthropic.com` URLs: a link printed on stderr or in a
different format will not appear in the UI.

Code references: `src-tauri/src/auth.rs` (sign-in), `src-tauri/src/main.rs`
(Tauri commands and browser), `src-tauri/src/process.rs` (chat),
`src-tauri/src/claude_runtime.rs` (binary).
