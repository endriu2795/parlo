# PARLO

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="src/assets/parlo-lockup-white.svg">
  <img src="src/assets/parlo-lockup-indigo.svg" width="200" alt="PARLO">
</picture>

[English](README.en.md)

PARLO è un’app desktop open source per Windows e macOS che registra le call,
le trascrive e le conserva in un archivio consultabile sul tuo computer.

Registra il microfono e l’audio del computer, quindi funziona con qualsiasi
piattaforma (Teams, Zoom, Meet, telefonate da browser) senza aggiungere bot
alla riunione. La trascrizione può avvenire interamente in locale. Sulle call
archiviate puoi lavorare con un assistente AI per fare domande e produrre
documenti.

## Cosa fa

### Registrazione

- Avvio e arresto con una scorciatoia da tastiera globale, anche mentre usi
  un’altra app.
- Cattura del microfono, dell’audio del computer o di entrambi. Su Windows i
  dispositivi possono seguire automaticamente i cambi di sistema, per esempio
  quando colleghi le cuffie.
- Anteprima della trascrizione mentre parli, sostituita a fine call dal testo
  definitivo.
- Durante la call puoi aggiungere note, catturare un’area dello schermo e
  disegnare annotazioni sopra le altre finestre. Su Windows puoi registrare
  anche il video dello schermo, o di una sua area, insieme alle annotazioni.

### Trascrizione

- **Sul computer**, con Whisper in tre livelli (veloce, bilanciato, massima
  precisione). Il modello si scarica una volta e poi funziona offline. L’app
  sceglie automaticamente tra processore e scheda grafica, in base a cosa è
  più veloce sulla tua macchina.
- **In cloud**, con Groq e la tua chiave oppure con un endpoint compatibile
  con l’API di trascrizione di OpenAI.
- Un dizionario di termini ricorrenti (nomi, sigle, prodotti) aiuta il
  riconoscimento.
- Puoi importare file audio e video esistenti, oppure inviare vocali e file a
  un bot Telegram personale: PARLO li riceve e li trascrive.
- Se la trascrizione non riesce, l’audio resta salvato e puoi riprovare, anche
  con un altro servizio.

### Archivio

- Ricerca su titoli, tag, note e testo pronunciato, con filtri per tag e data.
- Riascolto della call: clicchi un orario nel testo e salti al punto
  corrispondente.
- Esportazione in Markdown.
- Scegli cosa conservare (solo testo, testo e audio, testo e video) e per
  quanto tempo.

### Assistente AI

- Puoi usare l’abbonamento che hai già: **Claude** tramite Claude Code,
  **OpenAI** tramite Codex, **Google** tramite Antigravity. In alternativa
  puoi usare **Ollama** sul tuo computer o su un tuo server, oppure un
  endpoint compatibile con l’API di OpenAI.
- Fai domande sulla call e scegli quali fonti usare: trascrizione, note, file
  e cartelle collegati. Con `@` citi una fonte specifica o un’altra call
  dell’archivio, per confrontare più riunioni nella stessa chat.
- Chiedi documenti Word, fogli Excel, presentazioni PowerPoint, PDF, quiz o
  slideshow. I file prodotti restano associati alla call. Se colleghi un tuo
  template `.pptx`, le slide ne seguono layout, colori e caratteri.
- Con Claude, Codex e Antigravity puoi usare i connettori MCP già configurati
  sul tuo account (per esempio Drive o un CRM).
- Le operazioni sensibili, come i comandi di shell, chiedono conferma nella
  chat.

### Condivisione e voce

- Una cartella condivisa, per esempio su Google Drive, permette di pubblicare
  le trascrizioni e consultare quelle dei colleghi dalla scheda **Condivise**.
- Invio della trascrizione a un canale Slack o Teams tramite webhook.
- Lettura ad alta voce di testi e trascrizioni con motori locali (Piper,
  Kokoro, Qwen3-TTS) o servizi online (Edge, ElevenLabs).

## Privacy: cosa resta sul computer

L’archivio è salvato sul tuo computer, **senza cifratura**. Se scegli la
trascrizione locale, Ollama per l’assistente e una voce locale, nessun dato
lascia il computer.

Quando scegli un servizio online, PARLO invia a quel servizio solo i dati
necessari all’operazione: l’audio per la trascrizione, i messaggi e le fonti
per la chat, il testo per la voce. Chiavi API e webhook sono conservati nel
portachiavi del sistema. I servizi esterni hanno condizioni e costi propri.

## Installazione

- **Windows x64.** Scarica l’installer `.exe` dalle
  [Releases](https://github.com/endriu2795/parlo/releases). Python e gli
  strumenti per documenti e media sono già inclusi. L’installer non ha ancora
  una firma Authenticode, quindi Windows può mostrare un avviso o bloccarlo,
  secondo le impostazioni di sicurezza.
- **macOS 13 o successivo** (Apple Silicon o Intel). Per ora l’app si compila
  dai sorgenti con due comandi: vedi la
  [guida alla compilazione](DEVELOPMENT.md#macos-apple-silicon-or-intel) (in inglese) e lo
  [stato della versione Mac](MACOS.md).

### Compilare da sé invece di usare l’installer

Tutto il codice dell’app è in questa repository. Se preferisci non eseguire
un installer scaricato, puoi leggere il codice e compilare PARLO sul tuo
computer, su Windows o su Mac, seguendo la
[guida alla compilazione](DEVELOPMENT.md) (in inglese). I componenti esterni
vengono scaricati da fonti ufficiali, a versioni fisse. Runtime, modelli e CLI
degli assistenti sono verificati anche con hash SHA-256. L’elenco completo è
in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## Primi passi

1. In **Impostazioni → Registrazione** scegli le sorgenti audio e dove
   trascrivere. Con la trascrizione locale, il modello si scarica al primo
   utilizzo.
2. Avvia una registrazione con **Nuova registrazione** o con la scorciatoia.
3. Al termine apri la call dall’archivio. Per usare la chat, scegli un
   fornitore in **Impostazioni → Assistente**.

## Documentazione

- [Guida all’uso](USO.md): registrazione, trascrizione, assistente, voce,
  condivisione e dati.
- [PARLO su Mac](MACOS.md): permessi e stato del collaudo.
- [Compilare e contribuire](DEVELOPMENT.md) (in inglese): build su Windows
  e Mac, installer per un fork, test.
- [Integrazione con Claude Code](CLAUDE-CODE-INTEGRATION.md) (in inglese):
  come l’app usa la CLI.
- [Licenze di terze parti](THIRD_PARTY_NOTICES.md) (in inglese).

## Licenza

Il codice originale di PARLO è distribuito con licenza
**GNU General Public License versione 3 (GPL-3.0-only)**: puoi usarlo,
modificarlo e ridistribuirlo nel rispetto delle sue condizioni. Il testo
integrale è in [LICENSE](LICENSE). Il software è fornito senza alcuna garanzia.

Componenti, modelli e servizi di terze parti mantengono le proprie licenze e
condizioni, che la GPL del progetto non modifica. Attribuzioni e riferimenti
sono in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
