<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="src/assets/parlo-lockup-white.svg">
    <img src="src/assets/parlo-lockup-indigo.svg" width="220" alt="PARLO">
  </picture>

  <p><strong>App desktop open source per registrare, trascrivere e interrogare le tue call con l’AI — senza bot.</strong></p>

  <p>
    <a href="https://github.com/endriu2795/parlo/releases">Download</a> ·
    <a href="docs/guide.it.md">Guida all’uso</a> ·
    <a href="CONTRIBUTING.md">Compila e contribuisci</a> ·
    <a href="README.md">English</a>
  </p>
</div>

---

PARLO cattura microfono e audio del computer direttamente in locale — funziona con Teams, Zoom, Google Meet o qualsiasi altra app senza invitare bot — e trascrive anche file audio/video locali già esistenti o vocali da Telegram.

Trascrivi offline sul tuo PC con Whisper oppure online (Groq o un server compatibile OpenAI), cerca istantaneamente nell’archivio locale e usa il tuo assistente AI per riassumere call, incrociare riunioni precedenti o file locali e generare documenti. Per lavorare in team basta una normale cartella condivisa su Google Drive, OneDrive o rete locale — nessun server da mantenere.

L’interfaccia segue automaticamente la lingua del sistema (italiano o inglese).

## Punti chiave

- **Registrazione locale e import file** — Cattura microfono, audio di sistema o entrambi con scorciatoia globale, oppure importa file audio/video locali lasciando intatti gli originali. Prendi note con timestamp, cattura screenshot e disegna annotazioni sullo schermo (su Windows registra anche il video dello schermo o di una sua area).
- **Trascrizione offline o cloud** — Whisper in locale (Tiny, Small o Turbo con scelta automatica CPU/GPU e anteprima live opzionale), Groq oppure un tuo server compatibile OpenAI. Include dizionario personalizzato e bot Telegram personale per trascrivere vocali dal telefono.
- **Archivio locale veloce** — Tutto resta in un database SQLite sul computer, con ricerca full-text su titoli, tag, note e trascrizioni. Clicca qualsiasi timestamp per riascoltare quel punto, esporta in Markdown e imposta la cancellazione automatica.
- **Assistente AI a scelta** — Collega **Claude** (via Claude Code), **OpenAI** (via Codex), **Ollama** (locale o remoto) o un endpoint compatibile OpenAI.
  - Usa `@` per citare file, cartelle o fino a 8 altre call dell’archivio, confrontare riunioni o seguire un template di stile.
  - Genera documenti Word, Excel, PowerPoint, PDF, quiz e slideshow direttamente — senza avere Office installato.
  - Sfrutta i connettori MCP del tuo account (Claude/Codex) e autorizza i comandi di shell dalla chat.
- **Sintesi vocale integrata** — Ascolta testi e copioni con motori locali offline (Piper, Kokoro, Qwen3-TTS opzionale su Windows) o online (Edge, ElevenLabs, endpoint custom).
- **Condivisione team senza server** — Scegli una cartella condivisa su Google Drive, OneDrive o rete locale: PARLO pubblica file `.md` puliti che i colleghi possono leggere e interrogare con l’AI direttamente dall’app. Supporta anche webhook Slack e Teams.

## Supporto piattaforme

| Funzionalità | Windows x64 | macOS 13+ (Apple Silicon & Intel) |
| --- | :---: | :---: |
| Installer | Pronto (`.exe` nelle Releases) | Compilazione da sorgenti (collaudo in corso) |
| Microfono e audio di sistema | Sì (dispositivi selezionabili o predefiniti) | Sì (microfono predefinito + ScreenCaptureKit) |
| Note, screenshot e annotazioni | Sì | Sì |
| Registrazione video schermo / area | Sì | Non ancora |
| Whisper locale, assistente AI, condivisione | Sì | Sì |
| Qwen3-TTS locale (opzionale) | Sì | Non ancora |

## Installazione

### Windows x64

Scarica l’installer `.exe` dalle [GitHub Releases](https://github.com/endriu2795/parlo/releases). Python e tutti gli strumenti per documenti e media sono già inclusi.

> **Nota:** l’installer non ha ancora una firma Authenticode, quindi Windows SmartScreen può mostrare un avviso al primo avvio.

### macOS 13+ (Apple Silicon o Intel)

Non è ancora disponibile un DMG ufficiale firmato. Puoi compilare `PARLO.app` in locale con due comandi dalla cartella del repository:

```sh
bash scripts/mac-test.command --deps
bash scripts/mac-test.command --build
```

Il primo comando prepara Node, Python, Rust e CMake dentro la cartella del progetto (se macOS richiede gli Apple Command Line Tools, completa l’installazione e rilancia il comando). Al termine apri `src-tauri/target/debug/bundle/macos/PARLO.app`. Vedi la [Guida all’uso](docs/guide.it.md#permessi-su-macos) per i permessi macOS e [CONTRIBUTING.md](CONTRIBUTING.md) per i dettagli di build.

## Primi passi

1. Apri **Impostazioni → Registrazione** per scegliere sorgenti audio e motore di trascrizione (con Whisper locale il modello si scarica al primo uso).
2. Premi **Avvia registrazione** o usa la scorciatoia globale.
3. Apri la call completata dall’archivio — scegli un provider in **Impostazioni → Assistente** per iniziare a chattare.

## Privacy e dati

L’archivio resta sul tuo computer. Whisper locale, Ollama locale e le voci locali (Piper, Kokoro, Qwen3-TTS) funzionano interamente offline una volta scaricati i modelli.

I servizi online (Groq, Claude, Codex, ElevenLabs, Edge, Telegram, ricerca web, connettori MCP o server remoti) ricevono solo i dati necessari alle operazioni che avvii. Chiavi API, token e webhook sono salvati nel portachiavi di sistema. Dettagli in [Dati e privacy](docs/guide.it.md#dati-e-privacy).

## Documentazione

- **[Guida all’uso](docs/guide.it.md)** ([English](docs/guide.md)) — Registrazione, trascrizione, assistente AI, voce, condivisione in team e privacy.
- **[Compilare e contribuire](CONTRIBUTING.md)** (EN) — Architettura, build Windows/macOS, test e fork.
- **[Licenze di terze parti](THIRD_PARTY_NOTICES.md)** (EN) — Licenze di runtime, modelli e dipendenze incluse.

## Licenza

Il codice originale di PARLO è distribuito con licenza [GPL-3.0-only](LICENSE), senza garanzia. Componenti, modelli e servizi di terze parti mantengono le proprie licenze e condizioni — vedi [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
