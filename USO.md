# Guida all’uso di PARLO

[English](USO.en.md)

Qui trovi come registrare una call, lavorare sulla trascrizione e
condividerla con il team. Per l’installazione vedi il [README](README.md).

## Registrare una call

1. In **Impostazioni → Registrazione** scegli microfono, audio del computer o
   entrambi. Per registrare chi ti parla, scegli la stessa uscita audio usata
   da Teams, Zoom o dal browser.
2. Premi **Avvia registrazione** oppure usa la scorciatoia definita in
   **Impostazioni → Scorciatoie**. La scorciatoia funziona anche quando PARLO è
   in secondo piano.
3. Durante la call puoi scrivere note, catturare un’area dello schermo e
   disegnare annotazioni sopra le altre finestre. Gli screenshot entrano nella
   trascrizione.
4. Premi **Termina registrazione**. Al termine dell’elaborazione, la
   trascrizione definitiva sostituisce l’anteprima.

Su Windows puoi registrare anche il video dello schermo, o di una sua area,
con le annotazioni: in **Impostazioni → Archivio** scegli **Testo e video**.
Il video richiede più spazio e risorse.

## Trascrivere

In **Impostazioni → Registrazione → Dove trascrivere** hai tre opzioni.

- **Su questo computer.** Whisper elabora l’audio sul computer e funziona
  offline dopo il download del modello. Scegli tra **Veloce**, **Bilanciata**
  e **Massima precisione**. PARLO sceglie tra processore e scheda grafica in
  base alle prestazioni e mostra un’anteprima durante la call.
- **Online con Groq.** Inserisci la tua chiave da
  [console.groq.com](https://console.groq.com). Se la
  connessione si interrompe, l’app ritenta solo la parte interessata.
- **Endpoint personalizzato.** Qualsiasi server compatibile con l’API di
  trascrizione di OpenAI (`audio/transcriptions` con risposta `verbose_json`).

In **Lingua della call** puoi lasciare il rilevamento automatico o scegliere una
lingua.

In **Impostazioni → Dizionario** aggiungi nomi di persone, aziende, prodotti o
sigle ricorrenti. Il dizionario aiuta il riconoscimento di questi termini;
non esegue sostituzioni automatiche nel testo.

Se l’elaborazione non va a buon fine, l’audio resta salvato. Apri la call e
premi **Riprova**, eventualmente dopo aver cambiato servizio.

### Importare audio e video

Con **Importa audio o video** PARLO estrae l’audio dal file e lo trascrive.
Il file originale resta dov’era.

### Ricevere vocali da Telegram

In **Impostazioni → Telegram**, crea un bot con `/newbot` in @BotFather e
incolla il token in PARLO. Premi **Continua**, poi **Apri il bot in Telegram**
e **Avvia** nella chat. Quando PARLO mostra il bot collegato, puoi
inviare al bot vocali, file audio o video (fino a 20 MB) anche dal telefono, e
PARLO li trascrive con le impostazioni correnti. Se l’app è chiusa, i messaggi
vengono recuperati alla riapertura, entro le 24 ore previste da Telegram.

## Ritrovare e riascoltare

La ricerca dell’archivio copre titoli, tag, note e testo trascritto. Puoi
restringere i risultati per data e per tag. Nella call aperta puoi cercare un
passaggio, riascoltare l’audio, cambiare la velocità di riproduzione e cliccare
un orario per saltare al punto corrispondente. **Esporta** salva la
trascrizione in Markdown.

In **Impostazioni → Archivio** scegli cosa conservare (**Solo il testo**,
**Testo e audio**, **Testo e video**) e per quanto tempo. Audio e video seguono
la stessa scadenza della registrazione.

## Usare l’assistente

Scegli il fornitore in **Impostazioni → Assistente**.

- **Claude:** accedi con il tuo account Claude. PARLO usa Claude Code e lo
  scarica se non è installato.
- **Codex:** accedi con il tuo account OpenAI. PARLO usa la CLI Codex.
- **Ollama:** scegli un modello sul computer o su un tuo server. Per creare
  documenti ed eseguire operazioni serve un modello che supporti gli strumenti.
- **Endpoint personalizzato:** inserisci URL base, ID del modello e, se
  richiesta, la chiave API di un server compatibile con l’API chat di OpenAI.

Con Ollama le richieste vanno al server che hai configurato: sullo stesso
computer oppure su un’altra macchina. Se il server non risponde, PARLO ferma
la richiesta. Puoi regolare la memoria della conversazione; per call lunghe
puoi usare un contesto di almeno 32k token, se il modello e la memoria lo
consentono.

La ricerca web è disattivata di default per Ollama. Se attivi **Ricerca e
apertura di pagine web**, le ricerche passano da DuckDuckGo e il dominio di
ogni pagina da aprire viene inviato ad Anthropic per un controllo di sicurezza.

In **Istruzioni di progetto** puoi indicare preferenze su lingua, tono e
formato delle risposte. Valgono per tutte le call.

### Fare domande e scegliere le fonti

Apri la chat di una call. In **File e fonti** scegli quali materiali usare:
trascrizione, note, file e cartelle collegati. Digita `@` per citarne uno
nella richiesta. I file collegati restano nei loro percorsi originali.

Con `@` puoi citare anche **altre call dell’archivio**: scrivi parte del titolo
o una parola pronunciata e scegli la call in **Altre call dell’archivio**. La
sua trascrizione e le note diventano disponibili all’assistente insieme alla
call aperta, anche nelle domande successive. Per esempio:

> Confronta le richieste del cliente con @«Kickoff cliente · 12/11/2026» e
> dimmi cosa è cambiato.

Puoi citare fino a 8 call per chat. Con Ollama la barra del contesto indica
quanta memoria della conversazione è occupata dalle fonti e dai messaggi.

Se togli la spunta alla trascrizione, viene esclusa dalle richieste
successive, ma la cronologia della chat può ancora contenere passaggi già
discussi. Per ripartire da zero usa **Ricomincia chat**.

### Creare documenti

Con un fornitore che supporta gli strumenti puoi chiedere documenti Word,
fogli Excel, presentazioni PowerPoint, PDF, quiz o slideshow. Python e le
librerie necessarie sono inclusi nell’installer: non serve Office. I file
prodotti compaiono nella call.

Quando l’assistente chiede di eseguire comandi Bash o PowerShell, puoi
autorizzarli dalla chat una volta o ricordare la scelta. Puoi revocare le
autorizzazioni dalle impostazioni.

### Slide con il tuo template

1. In chat apri **File e fonti → Aggiungi file condivisi** e scegli il tuo
   template `.pptx`. Resta disponibile in tutte le chat.
2. Nella call, verifica che sia spuntato e citalo con `@` nella richiesta.

> Crea una presentazione dalla trascrizione. Usa @Template.pptx come
> riferimento per lo stile e includi decisioni e prossimi passi.

### Connettori MCP

Con Claude e Codex la chat può usare i connettori MCP configurati
sul tuo account, per esempio Drive o un CRM. Scegli quali attivare dal
pulsante **Connettori** nella chat. I connettori ricevono solo i dati delle
operazioni che autorizzi.

## Ascoltare testi e trascrizioni

In **Impostazioni → Voce** scegli il motore di sintesi vocale.

- **Online:** Edge oppure ElevenLabs con la tua chiave.
- **Locale:** Piper e Kokoro, leggeri e senza GPU; Qwen3-TTS, di qualità più
  alta, facoltativo, solo su Windows x64 e con requisiti di memoria elevati
  (download di circa 4,5 GB).
- **Endpoint compatibile**, se hai un tuo server.

Le voci locali richiedono il download del modello, poi funzionano offline.
Formati audio e disponibilità dei tempi delle singole parole dipendono dal
motore.

## Condividere con il team

### Cartella condivisa su Drive, OneDrive o rete locale

1. Crea una cartella per il team e assegna ai colleghi i permessi per leggere
   e pubblicare file. Con Google Drive o OneDrive, usa il client desktop per
   rendere la cartella accessibile da Esplora file o dal Finder.
2. In **Impostazioni → Condivisione**, inserisci il nome da mostrare e scegli
   la cartella. Ogni collega deve selezionare sul proprio computer la stessa
   cartella condivisa; il percorso locale può essere diverso.
3. Attiva **Mostra le trascrizioni degli altri nella barra laterale** per
   consultare le pubblicazioni del team nella scheda **Condivise**.
4. Per pubblicare, apri una call e scegli la cartella dal menu **Condividi**.
   Con **Pubblica automaticamente**, PARLO pubblica ogni nuova trascrizione
   completata.

PARLO scrive un file Markdown per ogni trascrizione pubblicata. Drive o
OneDrive lo sincronizzano con i colleghi; su una cartella di rete il file è
accessibile direttamente. Nella scheda **Condivise** puoi cercare per titolo
o autore, leggere il testo e aprire una chat con l’assistente.

Si condivide il testo della trascrizione. Audio, video, allegati e chat
restano nell’archivio personale. I file Markdown possono essere letti anche
senza PARLO.

L’opzione **Elimina le mie copie condivise alla scadenza della call** applica
la scadenza dell’archivio anche alle proprie pubblicazioni, comprese quelle
già presenti nella cartella. L’eliminazione viene sincronizzata con il team.

### Canali Slack e Teams

In **Impostazioni → Condivisione**, attiva il canale e salva il suo webhook.
Per Teams usa il webhook di un flusso Workflows configurato per il canale.
Pubblica dal menu **Condividi** della call oppure attiva **Pubblica
automaticamente** per quella destinazione.

## Dati e servizi

### Archivio locale

Registrazioni, trascrizioni e chat sono conservate sul computer. PARLO non
applica una cifratura propria all’archivio; la protezione del disco si
configura nelle impostazioni di sicurezza del sistema operativo. In
**Impostazioni → Archivio** scegli cosa conservare e per quanto tempo.

I file collegati come fonti restano nei percorsi originali. Chiavi API,
token e webhook sono conservati nel portachiavi del sistema.

### Elaborazione e condivisione

Whisper, Ollama sullo stesso computer e le voci locali possono elaborare i
contenuti offline dopo la configurazione e il download dei modelli.
Se configuri Ollama su un server, le richieste vengono inviate a quel server.

I servizi online ricevono i contenuti dell’operazione: audio per la
trascrizione, messaggi e fonti della chat per l’assistente, testo per la voce.
Ricerca web e connettori usano i servizi descritti nelle rispettive sezioni.
I vocali e i file inviati al bot passano da Telegram; le trascrizioni
pubblicate seguono le destinazioni e i permessi di condivisione scelti.
Ogni servizio esterno ha condizioni e costi propri.

[Compilare e contribuire](DEVELOPMENT.md) (in inglese) · [Licenze di terze parti](THIRD_PARTY_NOTICES.md)
