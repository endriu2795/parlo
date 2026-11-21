# Claude Code integration

PARLO runs the native Claude Code CLI for assistant conversations. This guide
describes the process, authentication, and checks for sign-in problems.

The React frontend calls Tauri commands. The Rust backend starts `claude` as
a child process and exchanges messages over stdin and stdout. The
`@anthropic-ai/claude-agent-sdk` dependency (0.3.296) is used by verification
scripts only; the app uses the CLI directly.

## Installation and sign-in

If `claude` is unavailable, PARLO downloads the official Claude Code 2.1.296
binary into its `engine` folder, verifies its size and SHA-256, and stores
the path. On Windows, it runs `claude.exe` rather than the npm `.cmd` launcher.

The sign-in flow uses the CLI's authentication commands:

1. Run `claude auth status` and read `loggedIn` from the JSON response.
2. Start `claude auth login` and keep the process open. The CLI handles OAuth
   and prints the sign-in link. Rust reads it from stdout and displays it in
   the app.
3. The “Apri la pagina di accesso” (open sign-in page) button opens the link
   in the system browser, using `explorer.exe` on Windows or `open` on macOS.
4. If Claude displays a code in the browser, write it to stdin of the same
   `claude auth login` process, followed by a newline and a flush.
5. When the process exits, run `claude auth status` again to check the result.

Claude Code stores and manages the authentication for the system user. PARLO
reuses it and does not set a separate `CLAUDE_CONFIG_DIR` or store its own
OAuth tokens.

## Chat

Each conversation starts `claude -p` with `--input-format stream-json`,
`--output-format stream-json`, `--permission-prompt-tool stdio`, `--model` and
`--system-prompt`. It sends JSON messages on stdin, receives replies and events
on stdout, and handles tool permission requests in Rust. It omits `--bare`
because that flag would disable subscription-based access in this flow.

## Diagnosing sign-in problems

Check `claude auth login` with the same executable and user environment as
the app. Keep stdin and stdout attached while the browser is open, so the
process can receive the returned code.

The link parser reads stdout line by line and accepts URLs on `claude.ai` or
`console.anthropic.com`. Links printed on stderr or in another format do not
appear in the app.

## Code references

- [auth.rs](src-tauri/src/auth.rs): sign-in process and returned code.
- [main.rs](src-tauri/src/main.rs): Tauri commands and opening the browser.
- [process.rs](src-tauri/src/process.rs): streaming chat and tool permissions.
- [claude_runtime.rs](src-tauri/src/claude_runtime.rs): CLI installation.
