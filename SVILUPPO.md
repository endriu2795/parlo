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

Esegui soltanto i controlli pertinenti. Per modifiche TypeScript:

```sh
npx tsc --noEmit
```

Per modifiche Python esegui i test unitari del modulo interessato; per Rust
usa i test mirati da `src-tauri`. Compilare l'app, creare un installer e
pubblicare una release sono operazioni distinte.

### Collaudi eseguiti

Le prove usano dati di esempio e passaggi fissi. Le misure di durata dipendono
dal PC. Il profiling riguarda solo le prestazioni, come lo scroll dell’archivio.

- **App Windows — `scripts/desktop-qa.cjs`.** Apre PARLO in WebView2 con 480 call
  sintetiche. Carica e scorre 460 righe, cerca una call fuori dalle pagine già
  caricate e verifica che la sidebar monti solo le righe vicine a quelle visibili.
  Registra un campione pubblico dall’audio di sistema, annulla l’elaborazione e
  preme «Riprova» fino a ottenere la trascrizione. Verifica anche che un file
  condiviso cambi mentre è aperto e che la chat riceva una risposta in streaming.
  Il server HTTP di test è avviato dallo script e controlla se la trascrizione
  entra nella richiesta prima e dopo averla esclusa dalle fonti.
- **Audio Windows e Whisper.** `scripts/audio-input-check.ps1` controlla il
  microfono predefinito. Il test Rust `default_devices_deliver_wasapi_packets`
  conta i campioni del microfono e verifica un tono di 997 Hz nell’audio di
  sistema. Va avviato esplicitamente con `--ignored` mentre il tono è in
  riproduzione. `scripts/benchmark-engine.py --minutes 0.75` usa il runtime Python
  del progetto per trascrivere 45 secondi di parlato pubblico con eco sintetico.
  Richiede il modello locale e il campione `.tools/engine/benchmark.wav`.
- **Test di comportamento.** I test Rust in `db.rs`, `processing.rs`, `main.rs`,
  `sharing.rs`, `audio.rs`, `audio_windows.rs`, `engine_child.rs` e `video.rs`
  verificano ricerca e paginazione, coda e annullamento, ripristino e scadenza,
  condivisione, gestione delle interruzioni audio e selezione video.
  I test Python `test_worker`, `test_mixing`, `test_final` e
  `test_transcript_quality` verificano mix, eco, pipeline e qualità del testo.
  Con `node --test` si possono eseguire i file `scripts/test-archive-dates.mjs`,
  `test-transcript-navigation.mjs`, `test-annotations.mjs` e
  `test-source-mentions.mjs` per filtri, navigazione, disegno e fonti della chat.

È stato fatto anche un confronto in Edge headless con backend simulato su 500
call locali e 500 condivise. Il collaudo nel repository da usare per lo scroll
della finestra desktop è `scripts/desktop-qa.cjs`.

Per ripetere il collaudo desktop servono **Windows con WebView2, Node 22.13 o
successivo, dipendenze npm, runtime Whisper locale con modello e campione WAV,
e un’uscita audio predefinita**. Dalla radice del progetto, in PowerShell:

```powershell
$env:TAURI_CONFIG = '{"identifier":"app.parlo.desktop.qa","plugins":{"updater":null}}'
cargo build --manifest-path src-tauri/Cargo.toml --bin callnotes
Remove-Item Env:TAURI_CONFIG
node scripts/desktop-qa.cjs
```

Il campione `.tools/engine/benchmark.wav` è `samples/jfk.wav` di whisper.cpp
v1.8.3. Lo prepara la configurazione dell’accelerazione. Se manca, puoi
scaricarlo con lo stesso indirizzo usato da `scripts/setup-accelerator.py`:

```powershell
Invoke-WebRequest 'https://raw.githubusercontent.com/ggml-org/whisper.cpp/v1.8.3/samples/jfk.wav' -OutFile .tools/engine/benchmark.wav
```

Lo script avvia il server di sviluppo e usa un archivio temporaneo. Il server
HTTP di test risponde sullo stesso PC e non richiede account, credenziali o un
endpoint AI esterno. `--recording-only` esegue solo registrazione, annullamento
e riprova. `--chat-only` prova solo la cartella condivisa e la chat, senza
usare il modello Whisper o riprodurre audio.

Per ridistribuire il pacchetto consulta le
[licenze dei componenti](THIRD_PARTY_NOTICES.md).

I manutentori possono seguire la [guida alla release ufficiale](RELEASE.md).
