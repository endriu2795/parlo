# Compilare PARLO

Per modificare il codice e provarlo sul proprio computer. Non servono account
GitHub di pubblicazione, certificati commerciali o la chiave degli aggiornamenti
del progetto. Ogni sistema compila la propria app: Windows x64, Mac Apple Silicon
oppure Mac Intel. Linux e Windows ARM non sono supportati da questi script.

L'interfaccia usa React/TypeScript e Vite; il backend usa Tauri 2/Rust, SQLite
e un runtime Python privato per trascrizione, documenti e media.

## Windows x64

Installa i [prerequisiti Tauri](https://v2.tauri.app/start/prerequisites/):

- Git e Node.js 22 con npm.
- Rust stable con toolchain MSVC, tramite [rustup](https://rustup.rs/).
- Visual Studio Build Tools 2022 con **Sviluppo desktop con C++** e Windows SDK.
- WebView2 Runtime, se non gia presente nel sistema.
- Python 3.12 a 64 bit, disponibile come `python` nel terminale.

Apri PowerShell nella copia del progetto. La prima volta prepara le dipendenze:

```powershell
npm.cmd ci
python scripts/bundle_python.py --platform win-x64
python scripts/prepare-runtime.py --platform win-x64
python scripts/prepare-runtime.py --platform win-x64 --kind documents
```

Questi comandi scaricano Python e i pacchetti nelle cartelle del progetto.
Non scaricano i modelli di trascrizione o voce. La prima preparazione richiede
Internet e spazio per i runtime; va ripetuta quando cambiano i rispettivi lockfile
o il manifest Python. Attendi che ogni comando termini senza errori.

Avvia l'app con ricaricamento dell'interfaccia:

```powershell
npm.cmd run desktop
```

Lo script usa Rust installato nel sistema, oppure quello privato in `.tools`
se disponibile. Impostazioni e dati di questa sessione restano in `.local-data`,
il motore in `.tools/engine`; queste cartelle sono escluse da Git.

Per compilare un eseguibile debug senza installer:

```powershell
npm.cmd run desktop:build
```

Output: `src-tauri/target/debug/callnotes.exe`. Per eseguirlo mantenendo separati
i dati di sviluppo, nella stessa PowerShell:

```powershell
$env:CALLNOTES_DATA_DIR = Join-Path $PWD '.local-data'
$env:CALLNOTES_ENGINE_DIR = Join-Path $PWD '.tools/engine'
& .\src-tauri\target\debug\callnotes.exe
```

Per una prima prova della trascrizione puoi scegliere **CPU** nelle impostazioni.
Il runtime GPU Vulkan richiede una preparazione aggiuntiva: Vulkan SDK, CMake
e i sorgenti [whisper.cpp v1.8.3](https://github.com/ggml-org/whisper.cpp/tree/v1.8.3)
in `.tools/whisper.cpp-1.8.3`, poi
`powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build-whisper-cpp.ps1 -VulkanSdk "PERCORSO_SDK"`.
Non serve il Vulkan SDK sui computer che eseguono un pacchetto gia completo.

## Mac Apple Silicon o Intel

Serve macOS 13 o successivo. Installa Git, Node.js 22 con npm, Rust stable,
Python 3.12 disponibile come `python3`, CMake e gli strumenti Apple:

```sh
xcode-select --install
```

Nel Terminale, dalla copia del progetto:

```sh
npm ci
python3 scripts/build-mac.py --test
```

Lo script prepara Python e i due runtime, compila whisper.cpp con Metal e crea
una `.app` debug per l'architettura del Mac in uso. Output:
`src-tauri/target/debug/bundle/macos/PARLO.app`.
Non genera un DMG, non pubblica nulla e non richiede la chiave dell'updater.
La firma ad hoc viene applicata localmente; non e una notarizzazione Apple.

Per aprirla con dati di sviluppo separati:

```sh
export CALLNOTES_DATA_DIR="$PWD/.local-data"
export CALLNOTES_ENGINE_DIR="$PWD/.tools/engine"
"src-tauri/target/debug/bundle/macos/PARLO.app/Contents/MacOS/callnotes"
```

Per modificare l'interfaccia con ricaricamento, dopo la preparazione precedente,
avvia invece dalla stessa shell:

```sh
npm run tauri -- dev --config src-tauri/tauri.macos.conf.json
```

`npm run desktop` e lo script Windows: su Mac usa il comando qui sopra.
Per la cattura concedi Microfono e Registrazione schermo nelle impostazioni
di privacy di macOS. Dettagli e limiti del collaudo in [MACOS.md](MACOS.md).
Le istruzioni non equivalgono a una verifica su hardware Mac fisico.

## Creare un installer per il proprio fork

Compila sulla piattaforma a cui è destinato l’installer, dopo aver installato i
prerequisiti sopra ed eseguito `npm ci` (`npm.cmd ci` su Windows). Per tenere il
fork separato dall’app ufficiale, scegli un tuo `productName` e un tuo `identifier`
in `src-tauri/tauri.conf.json` prima di compilare.

- **Windows x64** — `npm.cmd run desktop:installer` crea il setup `.exe` in
  `src-tauri/target/release/bundle/nsis`. Lo script prepara i runtime inclusi.
  L’installer risultante si apre come un normale setup Windows.
- **macOS** — `python3 scripts/build-mac.py --adhoc` crea `.app` e `.dmg` in
  `src-tauri/target/release/bundle`. Apri il DMG e copia l’app in Applicazioni.
  La firma ad hoc è adatta a prove locali. Per la distribuzione con Developer ID,
  configura la tua identità di firma Apple e usa `python3 scripts/build-mac.py`.

Il controllo degli aggiornamenti è spento nelle build locali. Per distribuire
aggiornamenti dal tuo fork, prepara un JSON con `pubkey` ed `endpoints`, imposta
`TAURI_SIGNING_PRIVATE_KEY` con la tua chiave e passa `-UpdaterConfig PERCORSO`
a `scripts/build.ps1 -Installer` su Windows o `--updater-config PERCORSO` a
`scripts/build-mac.py` su Mac. Il formato del feed e le firme sono descritti nella
[guida updater di Tauri](https://v2.tauri.app/plugin/updater/).

## Primo utilizzo e problemi comuni

- Trascrizione locale: l'app scarica il modello scelto quando lo prepari;
  il modello non fa parte del clone. Groq e una scelta cloud separata.
- Chat: Claude Code e Codex richiedono il proprio accesso quando usati con
  i servizi cloud. Ollama e opzionale e usa anche la CLI Claude Code per gli
  strumenti. Vedi [USO.md](USO.md).
- Se compare **il pacchetto non contiene il motore pronto**, completa la
  preparazione dei runtime indicata per il sistema; installare solo una venv
  Python non prepara i manifest richiesti dall'app.
- Se mancano `cargo`, `npm`, `python`/`python3`, o il linker MSVC, completa
  i prerequisiti e riapri il terminale.
- `npm run dev` da solo mostra l'interfaccia nel browser: non avvia cattura,
  database e altri comandi nativi. Usa l'avvio Tauri per provare il prodotto.
- Le build debug hanno il controllo degli aggiornamenti spento.

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

Per ridistribuire il pacchetto consulta le
[licenze dei componenti](THIRD_PARTY_NOTICES.md).

I manutentori possono seguire la [guida alla release ufficiale](RELEASE.md).
