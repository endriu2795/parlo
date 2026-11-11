# PARLO su Mac

Il backend nativo è ora scritto: AVAudioEngine per il microfono, ScreenCaptureKit per l’audio del computer, WAV temporanei mono 16 kHz su un orologio comune, livelli audio, stop e notifiche. Non viene registrato o salvato video. macOS 13 o successivo. Nessun driver audio virtuale necessario.

**Stato:** codice predisposto da Windows; non ancora compilato con un SDK Apple né verificato su un Mac fisico. Non è una release Mac certificata. Sono ancora da verificare compilazione, permessi, cattura, calibrazione Metal, background e packaging.

## Build per chi sviluppa

La guida comune per preparare i prerequisiti e compilare senza credenziali di
pubblicazione è [SVILUPPO.md](SVILUPPO.md). Qui restano i dettagli specifici Mac.

Per un tester senza strumenti di sviluppo, inviare `deliverables/PARLO-Mac-test.zip`, generato con `python scripts/package-mac-test.py`. Estrae lo ZIP e apre **Prepara PARLO.command**. Il kit rileva Intel/Apple Silicon, richiede gli strumenti Apple se assenti e prepara Node 22.23.3, Python 3.12.15, Rust 1.99.0 e CMake nella propria `.tools`, senza Homebrew o configurare il PATH del sistema. Compila una `.app` debug con firma ad hoc, la mostra nel Finder e la avvia. **Avvia PARLO.command** la riapre successivamente. Servono Internet e spazio per dipendenze/compilazione; la prima preparazione può essere lunga. Se fallisce genera **Diagnostica-PARLO-Mac.zip**, con log della build e versioni, senza audio, trascrizioni, chat o credenziali. Questo kit rende possibile il primo collaudo sul Mac del tester; non conferma che la build Mac sia già funzionante.

Su un Mac: Xcode Command Line Tools (`xcode-select --install`), Rust, Node/npm, Python 3.12 e CMake. Dalla cartella del progetto:

```sh
npm ci
python3 scripts/build-mac.py --test
```

Con `--test` lo script compila whisper.cpp 1.8.3 con Metal e il bridge Objective-C,
include il runtime e genera una `.app` debug in `src-tauri/target/debug/bundle/macos`.
Non richiede certificati Apple o la chiave dell'updater. Assembla anche CPython
3.12.15 con le dipendenze pronte, verificando versione e SHA-256 fissati in
`engine/python-runtime.json`. Una build nativa per ciascuna architettura
(Apple Silicon o Intel); non copiare una venv Windows o fra architetture.
I modelli non sono inclusi e vengono preparati dall'app.

Per chi distribuisce: senza `--test` lo script genera `.app` e `.dmg` release
in `src-tauri/target/release/bundle` e richiede l'identità Apple oppure la scelta
esplicita `--adhoc`. Il canale aggiornamenti è facoltativo. Vedi
[creare un installer per il proprio fork](SVILUPPO.md#creare-un-installer-per-il-proprio-fork).

## Primo avvio

Il kit scaricato contiene **PRIMO AVVIO MAC.html** e **VERIFICA-SHA256.txt**. Se Finder blocca **Prepara PARLO.command** prima che Terminale si apra, conservare il testo esatto del messaggio: “danneggiato” non è una prova sufficiente per diagnosticare la quarantena. Il launcher del kit non ha firma Developer ID/notarizzazione. Per lo sviluppatore non verificato, il percorso documentato da Apple è Privacy e sicurezza → Apri comunque, se disponibile. La guida HTML offre anche il comando `/bin/bash` con il percorso della sola copia del kit, da copiare nel Terminale: non rimuove attributi di quarantena né cambia le impostazioni di sicurezza del sistema. Usarlo soltanto per il pacchetto ricevuto dallo sviluppatore. All’avvio la preparazione confronta i file con il manifest SHA-256 e si ferma prima dei download in caso di file mancanti o modificati. Il manifest verifica l’integrità della copia, non l’identità del distributore; non sostituisce firma e notarizzazione.

Se la verifica fallisce, estrarre lo ZIP aggiornato in una nuova cartella. Se lo script è partito, la diagnostica include l’esito di questo controllo; se è stato bloccato prima dell’avvio, nessun log dello script può essere creato. Il comportamento di Finder/Gatekeeper va ancora verificato sul Mac del tester. Riferimenti Apple: [apertura delle app scaricate](https://support.apple.com/102445), [software modificato o danneggiato](https://support.apple.com/guide/mac-help/mh40619/mac).

Aprire la `.app` compilata. PARLO prepara automaticamente il motore incluso nella propria cartella dati; nessun Python di sistema, terminale, pip o modifica al PATH richiesti. Internet serve per il modello selezionato. Tiny per l’anteprima viene richiesto soltanto attivandola. Una build priva del motore pronto deve essere ricompilata con `scripts/build-mac.py`. Thread, RAM e GPU vengono rilevati sul Mac. Metal viene confrontato con CPU sul campione pubblico; se fallisce, rimane disponibile CPU. Il flusso Mac resta da verificare sul dispositivo fisico.

Le dipendenze Mac usano PyAV 15.1.0 e ONNX Runtime 1.22.1, che hanno pacchetti Python 3.12 compatibili con macOS 13 sia Intel sia Apple Silicon. Le versioni Windows restano separate tramite i marker del file `engine/requirements-lock.txt`. La risoluzione e disponibilità dei pacchetti per entrambe le architetture sono state controllate da Windows; esecuzione e inferenza richiedono ancora il collaudo sul Mac.

Al primo avvio di una registrazione consentire Microfono e Registrazione schermo in Privacy e sicurezza. ScreenCaptureKit richiede quest’ultimo permesso anche per acquisire soltanto audio. Dopo un cambio del permesso schermo macOS può richiedere di riaprire l’app. I permessi sono attribuiti a PARLO, perché la cattura gira nel processo dell’app.

Le release precedenti erano firmate ad hoc: il passaggio 0.2.1 → 0.2.2 può quindi lasciare in Privacy e sicurezza autorizzazioni associate all’identità della vecchia versione. La chiusura della finestra nasconde l’app e non termina il processo. In Impostazioni → Registrazione, **Ripristina permessi e riavvia PARLO** azzera soltanto Microfono e ScreenCapture per `app.parlo.desktop`, riavvia realmente l’app e permette di concedere nuovamente l’accesso alla registrazione successiva. Non ripristina i permessi di altre app. Per la 0.2.2 già installata, uscire completamente da PARLO dal menu **Esci**, rimuovere e aggiungere nuovamente l’app nelle liste Privacy e sicurezza, quindi riaprire PARLO.

Per una release firmata configura `APPLE_SIGNING_IDENTITY` con il tuo Developer ID Application. Per una prova locale puoi usare `--adhoc`, con i limiti sui permessi descritti sopra. Firma dell’app, notarizzazione e firma degli aggiornamenti si configurano separatamente. Riferimenti: [identità del codice e permessi macOS](https://developer.apple.com/documentation/technotes/tn3127-inside-code-signing-requirements), [firma macOS con Tauri](https://v2.tauri.app/distribute/sign/macos/).

**Account Claude** installa automaticamente il binario nativo ufficiale quando manca e apre il login nel browser. Installazione privata nella cartella del motore, con versione e checksum fissati in `engine/claude-runtime.json`; nessun Node o terminale necessari per la chat dell’app. Il kit non include l’account di chi lo ha creato.

Il selettore dell’assistente permette anche **Codex**. Account Codex riutilizza il programma/account nativo se disponibile, oppure prepara il pacchetto ufficiale 0.162.1 nella cartella del motore e apre l’accesso ChatGPT nel browser. Il callback completa il login senza incollare codici in un terminale. Versione e checksum Intel/Apple Silicon sono in `engine/codex-runtime.json`. Modelli, MCP selezionati e autorizzazioni sono separati da Claude; istruzioni di progetto e cronologia della call sono condivise. Download, protocollo, streaming e test UI verificati su Windows; avvio e login su Mac richiedono ancora il collaudo fisico.

Archivio e motore: cartella dati dell’app per il singolo utente, senza scrivere nel bundle `/Applications`. Shortcut acquisita premendo i tasti, incluso Cmd; combinazioni occupate vengono rifiutate senza cambiare quella precedente. La barra del titolo segue il comportamento nativo macOS.

Il kit include i controlli sulle ripetizioni estreme e la visualizzazione con avviso e **Mostra testo originale**. La riduzione conserva l’originale e non ricostruisce contenuto mancante. Il retry limitato ai blocchi sospetti è specifico di OpenVINO su Windows; gli altri motori usano i rispettivi decoder e la verifica comune del testo. Questa funzione è verificata su Windows, ancora da collaudare sul Mac insieme all’inferenza Metal.

## Verifica prima della distribuzione

- Su Apple Silicon e Intel: avvio da DMG installato in Applications, download dipendenze e modelli, fallback CPU e inferenza offline.
- Microfono, audio PC e entrambi: cuffie/altoparlanti, silenzi lunghi, cambio dispositivo, permesso negato e revocato, stop e nuova call.
- Call lunga, timestamp Tu/Speaker, duplicazioni/eco e file audio temporanei eliminati dopo il salvataggio.
- Finestra chiusa, minimizzata e app in background: cattura e elaborazione proseguono; ritorno via tray/shortcut e notifica desktop. Evitare sospensione manuale del Mac durante una call.
- Login Claude e Codex, modelli dinamici, MCP e richiesta di autorizzazione dei tool dalla build installata.

Per distribuire normalmente fuori dal Mac di sviluppo restano firma Developer ID e notarizzazione Apple (account/certificati del distributore). Il runtime Python è incluso dalla procedura di build; avvio del runtime e permessi dopo installazione da DMG vanno ancora verificati su Mac. La build ad hoc serve alle prove; non è già firmata e notarizzata per la distribuzione.
