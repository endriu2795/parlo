# Compilare PARLO

Apri il terminale nella cartella del progetto. Prepara le dipendenze una volta,
poi rilancia solo il comando di build quando cambi il codice.

## Mac Apple Silicon o Intel

Serve macOS 13 o successivo e una copia della cartella del progetto sul Mac.

1. Apri **Terminale**.
2. Scrivi `cd `, con uno spazio dopo `cd`. Trascina nel Terminale la cartella
   del progetto che contiene `scripts`, poi premi Invio.
3. Copia questo comando, premi Invio e attendi che finisca:

```sh
bash scripts/mac-test.command --deps
```

Prepara Node, Python, Rust e CMake nella cartella del progetto. Se macOS chiede
di installare gli strumenti Apple, conferma. Al termine dell’installazione
Apple, ripeti il comando e attendi il messaggio «Dipendenze pronte».

4. Copia questo comando e premi Invio. La prima compilazione scarica altri
   componenti e può richiedere diversi minuti:

```sh
bash scripts/mac-test.command --build
```

5. Al termine, nel Finder apri la cartella del progetto e vai in
   `src-tauri → target → debug → bundle → macos`. Apri **PARLO.app**.
   Dopo modifiche al codice, ripeti il comando del punto 4.

Per i permessi audio, vedi [PARLO su Mac](MACOS.md#primo-avvio).

## Windows x64

Servono Git, Node 22, Rust stable con toolchain MSVC, WebView2 e
Visual Studio Build Tools con **Sviluppo desktop con C++** e Windows SDK.
[Prerequisiti Tauri](https://v2.tauri.app/start/prerequisites/)

In PowerShell:

```powershell
npm.cmd run desktop:deps
npm.cmd run desktop:build
```

Il primo comando prepara le dipendenze npm e il runtime Python privato.
Il secondo crea `src-tauri/target/debug/callnotes.exe`. Per avviarlo con i
dati di sviluppo, usa `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run.ps1`.
Per lavorare con ricaricamento automatico usa `npm.cmd run desktop`.

Se ti serve compilare anche il runtime GPU Vulkan, usa
`scripts/build-whisper-cpp.ps1 -VulkanSdk PERCORSO_SDK` con Vulkan SDK, CMake
e i sorgenti whisper.cpp 1.8.3 in `.tools/whisper.cpp-1.8.3`.

## Creare un installer per il proprio fork

- **Windows** — `npm.cmd run desktop:installer`. Il setup è in
  `src-tauri/target/release/bundle/nsis`. Per preparare i pacchetti serve
  anche Python 3.12 a 64 bit.
- **Mac** — dopo la preparazione delle dipendenze, usa
  `bash scripts/mac-test.command --installer`. Il DMG è in
  `src-tauri/target/release/bundle/dmg`. La firma ad hoc è per uso locale.
  Per distribuire con Developer ID usa `scripts/build-mac.py` con la tua
  identità di firma e la configurazione di notarizzazione.

Per installare un fork accanto a PARLO, assegna un nome e un identificatore
distinti in `src-tauri/tauri.conf.json`. Per gli aggiornamenti del fork configura
la tua chiave e il tuo feed e passa `-UpdaterConfig` a `scripts/build.ps1` o
`--updater-config` a `scripts/build-mac.py`. [Guida updater di Tauri](https://v2.tauri.app/plugin/updater/)

## Primo utilizzo

Prepara il modello dal pannello **Impostazioni → Registrazione**. Puoi iniziare
con CPU oppure configurare un servizio di trascrizione. Per usare l’assistente
scegli un fornitore in **Impostazioni → Assistente**. [Guida all’uso](USO.md)

## Contribuire e controllare le modifiche

Usa dati sintetici per le prove e committa solo codice e documentazione.
Non aggiungere `.local-data`, `.tools`, runtime preparati, registrazioni,
trascrizioni, credenziali o chiavi. Le modifiche contribuite al codice del
progetto seguono [GPL-3.0-only](LICENSE).

## Suite di test

I test funzionali verificano il comportamento atteso. Se un test fallisce,
controlla se la modifica ha introdotto una regressione o se ha cambiato
intenzionalmente il risultato atteso. Nel primo caso correggi il codice.
Nel secondo aggiorna il test e aggiungi un caso se la modifica introduce
un comportamento nuovo.

### Test eseguiti in CI

La [CI](.github/workflows/ci.yml) gira sui push a `main` e sulle pull request
verso `main`. I job partono in parallelo. Tempi e risultati sono nei
[log di GitHub Actions](https://github.com/endriu2795/parlo/actions/workflows/ci.yml).

- **TypeScript e Node su Linux.** TypeScript controlla i tipi del frontend.
  In `scripts`, `test-archive-dates.mjs` verifica intervalli e fusi orari,
  `test-transcript-navigation.mjs` la selezione dei passaggi,
  `test-annotations.mjs` il disegno e il ridimensionamento,
  `test-file-links.mjs` i riferimenti ai file e
  `test-source-mentions.mjs` le menzioni e le fonti condivise.
- **Rust su Windows x64.** Il job compila il binario dei test. I test
  verificano ricerca, paginazione, SQLite, condivisione, coda e annullamento,
  recupero dopo un arresto, permessi degli strumenti, chat in streaming con
  un server di prova, logica audio e video e aggiornamenti.
- **Motore Python su Linux.** `test_worker` verifica il raggruppamento delle
  parole, `test_mixing` il mix e l’eco, `test_final` la trascrizione finale
  e il recupero dagli errori, `test_transcript_quality` le ripetizioni,
  `test_dictionary` i suggerimenti lessicali, `test_acceleration` la scelta
  CPU/GPU e `test_web_tools` la ricerca con risposte simulate.
- **Script Python su Linux.** `test-download-network.py` prova la ripresa
  di un download, `test-install-engine.py` l’installazione e il ripristino
  del runtime, `test-model-setup.py` la scelta e il download dei modelli,
  `test-prepare-runtime.py` l’integrità dei pacchetti e
  `test-local-release.py` la preparazione e pubblicazione simulata della
  release. I tre casi del bundle di dipendenze Windows si eseguono su Windows.

### Test automatici da avviare quando servono

- **Documenti, video e voce.** `test_document_tools` prova Word, Excel,
  PDF e slideshow. `test_video` prova annotazioni, recupero dagli errori di
  cattura e sincronizzazione audio-video. `test_speech_tools` prova sintesi
  vocale, decodifica e allineamento. `test_qwen_speech` prova suddivisione
  del testo, riuso del modello e gestione CPU/CUDA con simulazioni.
  Si eseguono da `engine` con `python -m unittest` seguito dal nome del
  modulo, usando un Python con le dipendenze del componente.
  La CI esegue solo le suite Python elencate sopra. Documenti e voce
  richiedono anche librerie Office/PDF, FFmpeg o Piper.
- **Interfaccia nel browser.** `npm run test:ui` usa Playwright e un backend
  simulato per verificare archivio, filtri, scorciatoie, impostazioni e
  indicatore di registrazione. Richiede un browser predisposto per Playwright.
- **Runtime, audio e prestazioni.** I test nativi del runtime Whisper e
  `scripts/benchmark-engine.py` richiedono il runtime e un modello locale.
  Il benchmark usa un campione pubblico. `scripts/audio-input-check.ps1`
  controlla il microfono predefinito. Il test Rust
  `default_devices_deliver_wasapi_packets` si avvia con `--ignored` mentre
  è in riproduzione un tono di 997 Hz. Il test del portachiavi usa quello
  della macchina di prova.

#### Collaudo automatico dell’app Windows

`scripts/desktop-qa.cjs` ripete scenari fissi. Genera 480 call sintetiche
con titoli e date prestabiliti e assegna a ogni call un identificativo
casuale. Lo script definisce le azioni e i risultati attesi. Verifica paginazione
e ricerca oltre le pagine caricate, scorre l’archivio e misura i tempi dei
fotogrammi. Controlla anche la registrazione di un campione pubblico,
l’annullamento e «Riprova», l’aggiornamento di un file condiviso e la chat
in streaming con la trascrizione prima inclusa e poi esclusa.

La prova completa richiede **Windows con WebView2, Node 22.13 o successivo,
dipendenze npm, runtime Whisper locale con modello e campione WAV e
un’uscita audio predefinita**. Dalla radice del progetto, in PowerShell,
compila il binario di prova e avvia lo script.

```powershell
$env:TAURI_CONFIG = '{"identifier":"app.parlo.desktop.qa","plugins":{"updater":null}}'
cargo build --manifest-path src-tauri/Cargo.toml --bin callnotes
Remove-Item Env:TAURI_CONFIG
node scripts/desktop-qa.cjs
```

Il campione `.tools/engine/benchmark.wav` è `samples/jfk.wav` di whisper.cpp
v1.8.3. Lo prepara la configurazione dell’accelerazione. Se manca, scaricalo
con il comando seguente.

```powershell
Invoke-WebRequest 'https://raw.githubusercontent.com/ggml-org/whisper.cpp/v1.8.3/samples/jfk.wav' -OutFile .tools/engine/benchmark.wav
```

Lo script usa un archivio temporaneo e un server HTTP locale per la chat.
`--recording-only` esegue registrazione, annullamento e riprova.
`--chat-only` prova la cartella condivisa e la chat e si può eseguire senza
Whisper e senza uscita audio.

### Collaudi manuali

- **Installazione e aggiornamento.** Installa il pacchetto, avvia l’app e
  verifica l’aggiornamento da una versione precedente quando disponibile.
- **Registrazione.** Sul Mac verifica i permessi di sistema al primo avvio.
  Prova microfono, audio di sistema, cambio del dispositivo e cattura dello
  schermo con annotazioni. Verifica qualità e tempi della trascrizione con il
  modello e la GPU disponibili.
- **Condivisione.** Se usi Google Drive per la cartella condivisa, pubblica
  una trascrizione di prova e verifica che compaia sull’altro computer.
- **Assistente.** Prova Ollama su una trascrizione lunga e verifica tempi e
  qualità della risposta. Se usi un endpoint personalizzato, prova una
  richiesta sul tuo server.

Per ridistribuire il pacchetto consulta le
[licenze dei componenti](THIRD_PARTY_NOTICES.md).

I manutentori possono seguire la [guida alla release ufficiale](RELEASE.md).
