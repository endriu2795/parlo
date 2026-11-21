# PARLO

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="src/assets/parlo-lockup-white.svg">
  <img src="src/assets/parlo-lockup-indigo.svg" width="200" alt="PARLO">
</picture>

[English](README.en.md)

PARLO è un’app desktop open source per registrare, trascrivere e consultare
le call. Salva le registrazioni sul computer, cerca nel testo e usa un
assistente AI per ricavare riepiloghi, risposte e documenti.

Acquisisce il microfono e l’audio del computer: puoi usarla con Teams, Zoom,
Meet e le altre app di chiamata, senza invitare un bot alla riunione. Puoi
trascrivere sul computer con Whisper oppure scegliere un servizio online.

Per lavorare in team basta una cartella condivisa, anche su Google Drive o
OneDrive: ciascuno sceglie quali trascrizioni pubblicare e consulta quelle
dei colleghi direttamente in PARLO.

L’app è disponibile per Windows e macOS, in italiano e inglese.

## Cosa fa

### Registrazione

- Registra il microfono, l’audio del computer o entrambi. Avvia e ferma la
  registrazione con una scorciatoia, anche mentre usi un’altra app.
- Aggiungi note, screenshot e annotazioni a schermo.

Microfono e audio del computer sono disponibili su Windows e Mac. Su
Windows puoi scegliere i dispositivi da PARLO o seguire quelli predefiniti;
su Mac il microfono segue il dispositivo predefinito del sistema, anche
quando lo cambi durante la call. Su Windows puoi anche registrare il video
dello schermo o di un’area selezionata. Vedi anche [PARLO su Mac](MACOS.md).

### Trascrizione

- Usa Whisper sul computer, con tre livelli di velocità e precisione. Dopo
  il download del modello puoi trascrivere offline; PARLO sceglie tra
  processore e scheda grafica in base alle prestazioni.
- Online puoi usare **Groq** con la tua chiave API.
- Con **Endpoint personalizzato** puoi collegare un altro servizio di
  trascrizione, anche un server Whisper sul tuo computer o in rete. Il server
  va configurato separatamente e deve supportare l’API di trascrizione di
  OpenAI; in PARLO inserisci indirizzo, ID del modello e chiave API se richiesta.
- Aggiungi nomi, sigle e prodotti al dizionario per aiutare il riconoscimento.
- Importa file audio e video, oppure ricevi vocali e file da un bot Telegram
  personale.
- Se una trascrizione non riesce, puoi riprovare dall’audio salvato, anche
  cambiando servizio.

### Archivio

- Aggiungi manualmente tag alle call, per esempio per cliente, progetto o
  argomento. Cerca nei titoli, nei tag, nelle note e nel testo trascritto;
  filtra per i tag che hai assegnato e per data.
- Se conservi l’audio, clicca un orario nella trascrizione per riascoltare
  quel passaggio.
- Esporta le trascrizioni in Markdown.
- Scegli cosa conservare: solo testo, testo e audio oppure testo e video
  (su Windows).
- In **Impostazioni → Archivio** scegli quando eliminare automaticamente i
  dati: dopo 1, 3, 6, 12 o 24 mesi, oppure **Mai** per conservarli finché non
  li elimini tu.

### Assistente AI

- Collega **Claude** tramite Claude Code o **OpenAI** tramite Codex, usando
  il tuo account. Puoi anche usare **Ollama** sul computer o su un tuo server,
  oppure un endpoint compatibile con l’API di OpenAI.
- Scegli le fonti della chat: trascrizione, note, file e cartelle. Con `@`
  richiami una fonte o un’altra call per confrontare più riunioni.
- Crea documenti Word, fogli Excel, presentazioni PowerPoint, PDF, quiz e
  slideshow. I file prodotti restano associati alla call.
- Aggiungi file e cartelle per dare contesto all’assistente o guidare stile
  e formato dei risultati: per esempio un brief di progetto, un documento
  di esempio, linee guida editoriali o un template `.pptx`.
- Con Claude e Codex puoi usare i connettori MCP configurati
  sul tuo account, per esempio Drive o un CRM.
- Autorizza dalla chat operazioni come l’esecuzione di comandi di shell.

### Voce

Ascolta testi e trascrizioni con motori locali (Piper, Kokoro, Qwen3-TTS) o
servizi online (Edge, ElevenLabs).

## Condividere le trascrizioni con il team

PARLO usa una cartella condivisa per scambiare le trascrizioni tra colleghi.
Può essere una cartella sincronizzata da Google Drive per computer o
OneDrive, oppure una cartella di rete. Ogni persona mantiene il proprio
archivio locale e pubblica le trascrizioni da condividere.

1. Condividi una cartella con il team e rendila accessibile dal computer di
   ciascun partecipante.
2. In **Impostazioni → Condivisione**, ciascuno seleziona la propria copia
   della stessa cartella e attiva la visualizzazione delle trascrizioni degli
   altri.
3. Apri una call e usa **Condividi** per pubblicarla nella cartella. Puoi
   anche attivare la pubblicazione automatica delle nuove trascrizioni.

Nella scheda **Condivise** trovi le trascrizioni dei colleghi, con ricerca
per titolo o autore. Puoi leggerle e aprire una chat con l’assistente.
I file sono in Markdown: puoi consultarli anche con un editor di testo.
La condivisione include il testo della trascrizione; audio, video, allegati
e chat restano nell’archivio personale. Drive o OneDrive gestiscono
sincronizzazione e accessi alla cartella.

Puoi anche inviare le trascrizioni a canali **Slack** e **Teams** tramite
webhook. Vedi la [guida alla condivisione](USO.md#condividere-con-il-team).

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

### Compilare dai sorgenti

Il codice è in questa repository. La [guida alla compilazione](DEVELOPMENT.md)
(in inglese) descrive i passaggi per Windows e Mac. I componenti esterni
vengono scaricati da fonti ufficiali a versioni fisse; runtime, modelli e CLI
degli assistenti sono verificati con hash SHA-256. L’elenco è in
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## Primi passi

1. In **Impostazioni → Registrazione** scegli le sorgenti audio e dove
   trascrivere. Con la trascrizione locale, il modello si scarica al primo
   utilizzo.
2. Premi **Avvia registrazione** o usa la scorciatoia.
3. Al termine apri la call dall’archivio. Per usare la chat, scegli un
   fornitore in **Impostazioni → Assistente**.

## Dati e servizi

L’archivio è salvato sul computer. Trascrizione con Whisper, assistente con
Ollama sullo stesso computer e voci locali possono funzionare offline dopo
la configurazione e il download dei modelli.

Se usi un servizio online, l’operazione invia i contenuti al servizio scelto:
audio per la trascrizione, messaggi e fonti per l’assistente, testo per la
voce. Condivisione, Telegram, ricerca web e connettori usano i rispettivi
servizi quando li attivi. Chiavi API, token e webhook sono conservati nel
portachiavi del sistema. I servizi esterni hanno condizioni e costi propri.

La [guida ai dati](USO.md#dati-e-servizi) descrive la conservazione locale e
l’uso dei servizi online.

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
[GPL-3.0-only](LICENSE), senza garanzia. Puoi usarlo, modificarlo e
ridistribuirlo secondo i termini della licenza.

Componenti, modelli e servizi di terze parti mantengono le proprie licenze e
condizioni: vedi [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
