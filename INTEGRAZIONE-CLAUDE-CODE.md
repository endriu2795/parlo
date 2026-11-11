# Come integriamo Claude Code nell’app

L’app è **React + Tauri**. Il frontend chiama comandi Tauri; il backend **Rust avvia la CLI nativa `claude` come processo figlio** e comunica con lei tramite stdin/stdout. La dipendenza `@anthropic-ai/claude-agent-sdk` (0.3.296) è usata solo da script di verifica: **il flusso dell’app non usa l’SDK**.

## Installazione e login

- Se `claude` non è disponibile, l’app scarica il binario ufficiale Claude Code **2.1.296** nella propria cartella `engine`, ne verifica dimensione e SHA-256 e salva il percorso. Su Windows usa `claude.exe`, non il launcher npm `.cmd`.
- Per verificare l’account esegue `claude auth status` e legge `loggedIn` dal JSON restituito.
- Per collegarlo avvia `claude auth login` e **tiene aperto quel processo**. La CLI gestisce OAuth e produce il link di accesso; Rust lo legge da stdout e lo mostra nell’app. Il pulsante “Apri la pagina di accesso” apre quel link nel browser di sistema (`explorer.exe` su Windows, `open` su macOS).
- Se Claude mostra un codice nel browser, l’app lo scrive nella **stdin dello stesso processo `claude auth login`**, seguito da newline e flush. Quando il processo termina, riesegue `claude auth status` per confermare l’accesso.

L’app **non implementa OAuth né conserva token propri**: riusa l’autenticazione salvata dalla CLI per lo stesso utente del sistema. Non configura un `CLAUDE_CONFIG_DIR` separato. Per replicare il login servono quindi lo stesso binario, lo stesso ambiente utente e un processo con stdin/stdout collegati durante l’apertura del browser.

## Chat

La chat avvia `claude -p` con `--input-format stream-json`, `--output-format stream-json`, `--permission-prompt-tool stdio`, `--model` e `--system-prompt`. Invia messaggi JSON su stdin, riceve risposte ed eventi su stdout e gestisce in Rust le richieste di permesso degli strumenti. Il codice evita `--bare`, che qui disabiliterebbe l’accesso tramite abbonamento.

**Se il browser login non parte:** controllare prima che `claude auth login` funzioni con lo stesso eseguibile e utente usati dall’app. Il parser attuale legge il link **solo da stdout, per righe**, e accetta URL `claude.ai` o `console.anthropic.com`: un link emesso su stderr o in formato diverso non apparirà nella UI.

Riferimenti nel codice: `src-tauri/src/auth.rs` (login), `src-tauri/src/main.rs` (comandi Tauri e browser), `src-tauri/src/process.rs` (chat), `src-tauri/src/claude_runtime.rs` (binario).
