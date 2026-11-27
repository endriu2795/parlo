# Guida all’uso di PARLO

[English](guide.md) · [Torna al README](../README.it.md)

Tutto quello che serve per configurare registrazione, trascrizione, assistente AI, voce e condivisione in team.

---

## Registrazione e audio

Scegli le sorgenti audio in **Impostazioni → Registrazione** e imposta una scorciatoia globale in **Impostazioni → Scorciatoie** per avviare e fermare la registrazione da qualsiasi app. Durante la call puoi scrivere note, catturare aree dello schermo (aggiunte automaticamente alla trascrizione) e disegnare annotazioni sopra le altre finestre.

- **Windows:** Scegli dispositivi specifici oppure lascia quelli predefiniti di sistema per seguire automaticamente i cambi (es. quando colleghi le cuffie). Per registrare le voci degli altri partecipanti, assicurati che l’audio di sistema punti alla stessa uscita usata da Teams, Zoom o dal browser.
- **Video dello schermo (solo Windows):** In **Impostazioni → Archivio** seleziona **Testo e video** per registrare lo schermo o un’area selezionata insieme alle annotazioni.
- **macOS:** Il microfono segue automaticamente l’ingresso predefinito di macOS, anche se cambi dispositivo durante la call. L’audio di sistema viene acquisito tramite ScreenCaptureKit senza driver virtuali aggiuntivi.

### Permessi su macOS

Alla prima registrazione concedi **Microfono** e **Registrazione schermo** in **Impostazioni di Sistema → Privacy e sicurezza**. macOS richiede il permesso di Registrazione schermo per ScreenCaptureKit anche quando registri solo l’audio.

Se dopo una ricompilazione o un aggiornamento locale i permessi si bloccano, usa **Ripristina permessi e riavvia** in **Impostazioni → Registrazione** e concedili di nuovo.

---

## Trascrizione

Scegli il motore in **Impostazioni → Registrazione → Dove elaborare l’audio**:

| Opzione | Come funziona |
| --- | --- |
| **Su questo computer** | Esegue Whisper offline dopo il download del modello (**Veloce / Tiny**, **Bilanciata / Small**, **Massima precisione / Turbo**). PARLO testa e sceglie automaticamente GPU o CPU. Attiva **Mostra il testo mentre registri** per avere un’anteprima live durante la call. |
| **Online con Groq** | Usa la tua chiave API da [console.groq.com](https://console.groq.com). Se cade la connessione, ritenta solo i segmenti mancanti. |
| **Endpoint personalizzato** | Si collega a qualsiasi server compatibile OpenAI `POST audio/transcriptions` (es. server Whisper/Speaches locale o in LAN). Deve accettare WAV multipart e restituire `verbose_json` con timestamp. Usa **Prova endpoint** per verificare. |

- **Lingua e dizionario:** Puoi lasciare il rilevamento automatico o fissare la lingua della call. Aggiungi nomi, sigle e prodotti ricorrenti in **Impostazioni → Dizionario** per aiutare il riconoscimento (guida Whisper senza fare sostituzioni cieche).
- **Riprova in caso di errore:** Se la trascrizione fallisce, l’audio resta salvato — apri la call e premi **Riprova**, anche cambiando motore.
- **Importa audio o video:** Trascrive file già esistenti lasciando intatti gli originali.

### Bot Telegram personale

Invia vocali, audio o video (fino a 20 MB) dal telefono per trovarli trascritti in PARLO:

1. In **Impostazioni → Telegram**, apri `@BotFather`, invia `/newbot` e incolla il token in PARLO.
2. Premi **Continua** e **Copia comando**. Apri la chat privata del bot in Telegram, tocca **Avvia** se compare, poi incolla e invia il comando. Se il codice scade, premi **Genera un nuovo codice** in PARLO.
3. Da quel momento tutto ciò che invii al bot finisce nell’archivio di PARLO. Se l’app è chiusa, i messaggi in coda vengono recuperati alla riapertura (entro il limite di 24 ore di Telegram).

---

## Archivio e conservazione

- La ricerca copre titoli, tag manuali, note e trascrizioni, con filtri per tag e data.
- Clicca qualsiasi timestamp per riprodurre l’audio da quel punto esatto, cambia la velocità di riproduzione o esporta la trascrizione in Markdown.
- In **Impostazioni → Archivio** decidi cosa conservare per le nuove call (**Solo il testo**, **Testo e audio**, o **Testo e video** su Windows) e dopo quanto tempo eliminare automaticamente i dati (**1–24 mesi** oppure **Mai**). La scadenza elimina la call completa di trascrizione, note, screenshot, chat e media.

---

## Assistente AI

Configura il provider in **Impostazioni → Assistente**:

- **Claude** — Accedi con il tuo account Claude tramite la CLI nativa Claude Code (scaricata automaticamente se assente).
- **Codex** — Accedi con il tuo account OpenAI tramite la CLI Codex.
- **Ollama** — Si collega a un server Ollama locale o remoto. Usa un modello con supporto agli strumenti (tool calling) per generare documenti. Per call lunghe imposta almeno **32k token** di contesto (RAM permettendo). La ricerca web è disattivata di default: se attivata usa DuckDuckGo e invia il dominio delle pagine aperte ad Anthropic per un controllo di sicurezza.
- **Endpoint personalizzato** — Qualsiasi API chat compatibile OpenAI (URL base, ID modello, eventuale chiave API).

Usa **Istruzioni di progetto** per definire preferenze valide su tutte le call (lingua, tono, struttura delle risposte).

### Fonti, template e menzioni `@` tra call

Nella chat di una call, apri **File e fonti** per includere o escludere trascrizione, note e file/cartelle collegati (i file restano nei percorsi originali; usa **File/cartelle condivisi** per renderli disponibili in tutte le chat).

Digita `@` per richiamare una fonte specifica o citare **fino a 8 altre call** dell’archivio:

> Confronta le richieste del cliente con `@«Kickoff cliente · 12/11/2026»` e dimmi cosa è cambiato.

> Crea una presentazione dalla trascrizione usando `@Template.pptx` per lo stile, con decisioni e prossimi passi.

> Riassumi la call usando `@Brief.pdf` come contesto e seguendo struttura e tono di `@Esempio.docx`.

Se deselezioni la trascrizione, viene esclusa dai turni successivi; per azzerare la cronologia usa **Ricomincia chat**.

### Creazione documenti e strumenti

Con un provider che supporta gli strumenti puoi chiedere **Word (`.docx`), Excel (`.xlsx`), PowerPoint (`.pptx`), PDF, quiz o slideshow** — Python e le librerie sono inclusi nell’installer, non serve Office. I file generati restano associati alla call.

- **Comandi di shell:** Quando l’assistente chiede di eseguire Bash o PowerShell, puoi autorizzare il comando una volta sola o salvare la regola (revocabile dalle impostazioni).
- **Connettori MCP:** Con Claude e Codex puoi attivare i connettori MCP del tuo account (es. Drive, CRM) dal pulsante **Connettori** nella chat.

---

## Sintesi vocale (Voce)

Attiva la voce in **Impostazioni → Voce** per generare file MP3 da testi e copioni in chat:

- **Locale e offline:**
  - **Piper** — Leggero, veloce, solo CPU (voci italiane).
  - **Kokoro** — Solo CPU, timbro diverso (~354 MB di download, più lento di Piper).
  - **Qwen3-TTS** *(opzionale, solo Windows x64)* — Voci multilingue più espressive, richiede ~4,5 GB di download e hardware adeguato (consigliata GPU NVIDIA 8 GB+ VRAM; su CPU servono ~32 GB di RAM ed è molto lento).
- **Online:** **Edge** (gratuito, senza chiave, invia il testo a Microsoft), **ElevenLabs** (usa chiave API e crediti del tuo account) oppure un **Endpoint personalizzato**.

---

## Condivisione in team

### Cartella condivisa (Google Drive, OneDrive o rete locale)

PARLO permette di scambiare trascrizioni in team senza alcun server centrale:

1. Crea una cartella condivisa visibile in Esplora file o Finder su ogni computer (con Google Drive desktop, OneDrive o rete locale).
2. In **Impostazioni → Condivisione**, imposta il tuo nome, scegli quella cartella e attiva **Mostra le trascrizioni degli altri nella barra laterale**.
3. Apri una call e premi **Condividi** (oppure attiva **Pubblica automaticamente**).

I colleghi trovano le trascrizioni nella scheda **Condivise**, possono cercare per titolo o autore e aprire una chat AI sopra di esse. I file sono in semplice Markdown, leggibili anche fuori da PARLO.

Viene condiviso **solo il testo della trascrizione** — audio, video, allegati e chat personali restano privati nel tuo archivio locale. Attiva **Elimina le mie copie condivise alla scadenza della call** se vuoi che la scadenza dell’archivio rimuova anche i file pubblicati.

### Webhook Slack e Teams

In **Impostazioni → Condivisione**, attiva Slack o Teams e salva l’URL del webhook (per Teams usa un flusso Workflows del canale). Pubblica manualmente dal menu **Condividi** o attiva la pubblicazione automatica.

---

## Dati e privacy

- **Archivio locale:** Registrazioni, trascrizioni, note, screenshot e chat restano sul computer. PARLO non applica una cifratura aggiuntiva oltre a quella del disco (usa BitLocker o FileVault per proteggere l’archivio a riposo).
- **Credenziali:** Chiavi API, token Telegram e webhook sono salvati nel portachiavi del sistema operativo.
- **Offline vs online:** Whisper locale, Ollama locale e le voci locali non inviano dati fuori dal computer dopo il download dei modelli. Quando usi un servizio online (Groq, Claude, Codex, Ollama remoto, ElevenLabs, Edge, Telegram, Slack/Teams, ricerca web o connettori MCP), vengono inviati solo i dati necessari all’operazione richiesta, secondo i termini del rispettivo servizio.
