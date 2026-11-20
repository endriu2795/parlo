# Guida all’uso di PARLO

[English](USO.en.md)

Questa guida segue l’ordine delle impostazioni dell’app. Per installare PARLO
vedi il [README](README.md).

## Registrare una call

1. In **Impostazioni → Registrazione** scegli microfono, audio del computer o
   entrambi. Per registrare chi ti parla, scegli la stessa uscita audio usata
   da Teams, Zoom o dal browser.
2. Premi **Nuova registrazione** oppure usa la scorciatoia definita in
   **Impostazioni → Scorciatoie**. La scorciatoia funziona anche quando PARLO è
   in secondo piano.
3. Durante la call puoi scrivere note, catturare un’area dello schermo e
   disegnare annotazioni sopra le altre finestre. Gli screenshot entrano nella
   trascrizione.
4. Premi **Termina**. La trascrizione definitiva sostituisce l’anteprima e la
   call compare nell’archivio.

Su Windows puoi registrare anche il video dello schermo, o di una sua area,
con le annotazioni: in **Impostazioni → Archivio** scegli **Testo e video**.
Il video richiede più spazio e risorse.

## Trascrivere

In **Impostazioni → Registrazione → Dove trascrivere** hai tre opzioni.

- **Su questo computer.** Usa Whisper in locale: gratuito, privato, offline
  dopo il primo download. Scegli tra **Veloce**, **Bilanciata** e **Massima
  precisione** in base alla potenza del computer. L’app sceglie da sola tra
  processore e scheda grafica; con una GPU NVIDIA la trascrizione è molto più
  rapida. Durante la call mostra un’anteprima provvisoria.
- **Online con Groq.** Inserisci la tua chiave da console.groq.com. Se la
  connessione si interrompe, l’app ritenta solo la parte interessata.
- **Endpoint personalizzato.** Qualsiasi server compatibile con l’API di
  trascrizione di OpenAI (`audio/transcriptions` con risposta `verbose_json`).

In **Lingua della call** puoi lasciare il rilevamento automatico o fissare una
lingua.

In **Impostazioni → Dizionario** aggiungi nomi di persone, aziende, prodotti o
sigle che ricorrono spesso: la trascrizione li scrive correttamente più spesso.
Il dizionario non sostituisce parole in automatico; conviene inserire solo i
termini davvero frequenti.

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

| Fornitore | Cosa serve | Note |
|---|---|---|
| **Claude** | Account Claude | Usa Claude Code. Se non è installato, PARLO lo scarica. |
| **Codex** | Account OpenAI | Usa la CLI Codex. |
| **Antigravity** | Account Google | Richiede un accesso dedicato a PARLO. |
| **Ollama** | Ollama sul computer o su un tuo server | Nessun account cloud. Serve un modello che supporti gli strumenti. |
| **Endpoint personalizzato** | URL base, ID del modello, chiave facoltativa | Qualsiasi server compatibile con l’API chat di OpenAI. |

Con Ollama la chat resta locale: se il server non risponde, la richiesta si
ferma senza passare al cloud. La ricerca web è disattivata di default; se
attivi **Ricerca e apertura di pagine web**, le ricerche passano da DuckDuckGo
e, prima di aprire una pagina, il solo dominio viene inviato ad Anthropic per
un controllo di sicurezza. Puoi regolare
la memoria della conversazione; per call lunghe conviene un contesto di almeno
32k token, se il modello e la memoria lo consentono.

In **Istruzioni di progetto** puoi indicare preferenze su lingua, tono e
formato delle risposte. Valgono per tutte le call.

### Fare domande e scegliere le fonti

Apri la chat di una call. In **File e fonti** scegli quali materiali usare:
trascrizione, note, file e cartelle collegati. Digita `@` per citarne uno
nella richiesta. I file collegati restano nei loro percorsi originali.

Con `@` puoi citare anche **altre call dell’archivio**: scrivi parte del titolo
o una parola pronunciata e scegli la call in **Altre call dell’archivio**. La
sua trascrizione, con le note, entra nel contesto insieme a quella della call
aperta, e resta disponibile nelle domande successive. Per esempio:

> Confronta le richieste del cliente con @«Kickoff cliente · 12/11/2026» e
> dimmi cosa è cambiato.

Si possono citare fino a 8 call per chat. Con Ollama tieni d’occhio la barra
del contesto: più call occupano più spazio.

Se togli la spunta alla trascrizione, viene esclusa dalle richieste
successive, ma la cronologia della chat può ancora contenere passaggi già
discussi. Per ripartire da zero usa **Ricomincia chat**.

### Creare documenti

Con un fornitore che supporta gli strumenti puoi chiedere documenti Word,
fogli Excel, presentazioni PowerPoint, PDF, quiz o slideshow. Python e le
librerie necessarie sono inclusi nell’installer: non serve Office. I file
prodotti compaiono nella call.

Le operazioni sensibili, come i comandi Bash o PowerShell, chiedono conferma
nella chat. Puoi consentirle una volta o ricordare la scelta, e revocarla in
seguito dalle impostazioni.

### Slide con il tuo template

1. In chat apri **File e fonti → Aggiungi file condivisi** e scegli il tuo
   template `.pptx`. Resta disponibile in tutte le chat.
2. Nella call, verifica che sia spuntato e citalo con `@` nella richiesta.

> Crea una presentazione dalla trascrizione. Usa @Template.pptx come
> riferimento per lo stile e includi decisioni e prossimi passi.

### Connettori MCP

Con Claude, Codex e Antigravity la chat può usare i connettori MCP configurati
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

## Condividere

In **Impostazioni → Condivisione** puoi:

- impostare una **cartella condivisa**, per esempio su Google Drive o su un
  disco di rete. Le trascrizioni che pubblichi compaiono ai colleghi che usano
  la stessa cartella, e la scheda **Condivise** mostra le loro, con ricerca per
  titolo o autore. Se lo attivi, le tue copie condivise vengono eliminate alla
  scadenza della call;
- inviare la trascrizione a un canale **Slack** o **Teams** tramite webhook.
  Per Teams usa un flusso Workflows configurato per il canale.

## Dati e privacy

- L’archivio è salvato sul computer, **senza cifratura**.
- Trascrizione locale, Ollama e voce locale non inviano dati all’esterno.
- I servizi online ricevono solo i dati necessari: audio per la trascrizione,
  messaggi e fonti per la chat, testo per la voce.
- Chiavi API, token e webhook sono conservati nel portachiavi del sistema.
- I file collegati come fonti non vengono copiati: restano dove sono.

[Compilare e contribuire](DEVELOPMENT.md) (in inglese) · [Licenze di terze parti](THIRD_PARTY_NOTICES.md)
