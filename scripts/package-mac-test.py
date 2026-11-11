"""Allowlist-only test kit: no calls, settings, account files or developer venv."""
from pathlib import Path
import argparse
import zipfile
import hashlib

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, default=root / 'deliverables/PARLO-Mac-test.zip')
args = parser.parse_args()
files = []
for directory in ('src', 'src-tauri/src', 'public/fonts'):
    for path in (root / directory).rglob('*'):
        relative = path.relative_to(root)
        if path.is_file() and path.suffix in ('.py', '.rs', '.m', '.ts', '.tsx', '.css', '.json', '.txt', '.png', '.svg', '.woff2') and not any(p in ('__pycache__', 'runtime', 'models') for p in relative.parts):
            files.append((path, relative.as_posix()))
for name in ('package.json', 'package-lock.json', 'index.html', 'overlay.html', 'tsconfig.json', 'vite.config.ts',
             'engine/worker.py', 'engine/video.py', 'engine/acceleration.py', 'engine/cpp_live.py', 'engine/transcript_quality.py', 'engine/calibrate.py', 'engine/model_download.py', 'engine/import_media.py',
             'engine/requirements-lock.txt', 'engine/python-runtime.json', 'engine/claude-runtime.json', 'engine/codex-runtime.json', 'engine/antigravity-runtime.json', 'scripts/extract-antigravity.py',
             'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock', 'src-tauri/build.rs', 'src-tauri/tauri.conf.json',
             'src-tauri/tauri.macos.conf.json', 'src-tauri/Info.plist', 'src-tauri/Entitlements.plist',
             'src-tauri/macos/it.lproj/InfoPlist.strings',
             'scripts/build-mac.py', 'scripts/build-whisper-metal.py', 'scripts/bundle_python.py', 'scripts/prepare-runtime.py',
             'scripts/mac-test.command', 'scripts/mac-launch.command', 'scripts/mac-test.mjs',
             'scripts/install-engine.py', 'scripts/setup-engine.py', 'scripts/setup-accelerator.py', 'MACOS.md'):
    if not (root / name).is_file():
        raise SystemExit(f'File necessario assente: {name}')
    files.append((root / name, name))
for path in (root / 'src-tauri/icons').glob('*'):
    if path.is_file() and path.suffix in ('.png', '.ico', '.icns', '.rgba'):
        files.append((path, path.relative_to(root).as_posix()))

instructions = '''PARLO — kit di prova Mac

Serve macOS 13 o successivo. Kit predisposto per Apple Silicon e Intel.
Questa è una prima build di prova: il codice Mac non è ancora collaudato su un Mac.

1. Estrai tutto lo ZIP in una cartella normale (per esempio sul Desktop).
2. Apri Prepara PARLO.command con doppio clic.
   Se macOS lo blocca prima di aprire Terminale, apri PRIMO AVVIO MAC.html.
   La guida permette di copiare il comando per avviare il kit da Terminale,
   senza cambiare le impostazioni di sicurezza del Mac. Riferisci il messaggio
   esatto: file danneggiato e sviluppatore non verificato sono casi distinti.
3. Se macOS chiede gli strumenti Apple, conferma l’installazione. Quando finisce,
   riapri lo stesso file. È l’unico componente di sistema necessario per compilare.
4. Attendi: il kit scarica e prepara Node, Python, Rust e CMake nella propria
   cartella .tools, poi compila l’app. Non serve Homebrew né scrivere comandi.
   Servono Internet e parecchi GB liberi; la prima compilazione può essere lunga.
5. L’app si apre al termine. In futuro usa Avvia PARLO.command.
6. Apri PARLO: il motore si prepara automaticamente e scarica il modello predefinito.
   Scarica solo il modello scelto e calibra il motore. Questo primo setup richiede Internet.
   Tiny/Small vengono scaricati alla selezione; l'anteprima solo se attivata.
7. Per la prima call consenti Microfono e Registrazione schermo in macOS.
   Il secondo permesso serve per l’audio PC; PARLO non salva video.
8. Per la chat scegli Claude Code o Codex nel selettore dell’assistente, poi
   apri il relativo Account. Se il programma manca, lo installa PARLO.
   Completa l’accesso al TUO account nel browser; non serve il terminale.

Il launcher non è firmato e notarizzato con Developer ID. Per lo sviluppatore
non verificato, Apple indica Privacy e sicurezza → Apri comunque, se disponibile.
Per file danneggiato: conserva il testo esatto, estrai una nuova copia e usa
PRIMO AVVIO MAC.html per la verifica. La preparazione controlla SHA-256 dei file;
se non corrispondono si ferma prima dei download. I checksum rilevano errori
di copia, non sostituiscono una firma Apple. L'app compilata ha firma ad hoc.

PROVE UTILI
- Call breve: microfono, audio PC, poi entrambi. Controlla Tu/Speaker ed eco.
- Minimizza durante registrazione ed elaborazione; controlla notifica e risultato.
- Avvia una seconda registrazione mentre la prima viene elaborata.
- Prova account Claude/Codex, cambio modello, chat e selezione/consenso MCP.
- Prova una shortcut di tre o quattro tasti; rilasciali tutti per confermare.
- Nei passaggi con ripetizioni anomale verifica l’avviso e Mostra testo originale.
  L’originale resta consultabile; la riduzione non ricostruisce parole mancanti.
- Segnala durata call, durata elaborazione, modello, motore e processore Mac.

SE LA PREPARAZIONE FALLISCE
Rimanda Diagnostica-PARLO-Mac.zip, creato accanto a questo file.
Contiene soltanto log della preparazione/compilazione e versioni degli strumenti.
Non raccoglie audio, trascrizioni, chat o credenziali. I log possono contenere
nomi di cartelle locali. Per un problema dell’app, descrivi cosa è successo
e copia il dettaglio dell’errore da PARLO; la diagnostica è solo della build.
'''
args.output.parent.mkdir(parents=True, exist_ok=True)
prefix = 'PARLO Mac test/'
contents = {}
with zipfile.ZipFile(args.output, 'w', zipfile.ZIP_DEFLATED) as archive:
    for source, name in sorted(set(files), key=lambda item: item[1]):
        info = zipfile.ZipInfo(prefix + name)
        info.create_system = 3
        info.external_attr = (0o100755 if name.endswith('.command') else 0o100644) << 16
        info.compress_type = zipfile.ZIP_DEFLATED
        data = source.read_bytes()
        if name.endswith('.command'):
            data = data.removeprefix(b'\xef\xbb\xbf').replace(b'\r\n', b'\n')
        contents[name] = data
        archive.writestr(info, data)
    for source, name in [('scripts/mac-test.command', 'Prepara PARLO.command'), ('scripts/mac-launch.command', 'Avvia PARLO.command')]:
        # Top-level launcher delegates to the same tested script inside scripts/.
        script = f'#!/bin/bash\nexec /bin/bash "$(cd "$(dirname "$0")" && pwd)/{source}"\n'
        info = zipfile.ZipInfo(prefix + name)
        info.create_system = 3
        info.external_attr = 0o100755 << 16
        info.compress_type = zipfile.ZIP_DEFLATED
        contents[name] = script.encode('utf-8')
        archive.writestr(info, contents[name])
    contents['LEGGIMI.txt'] = instructions.encode('utf-8')
    contents['PRIMO AVVIO MAC.html'] = (root / 'scripts/mac-test-guide.html').read_bytes()
    for name in ['LEGGIMI.txt', 'PRIMO AVVIO MAC.html']:
        info = zipfile.ZipInfo(prefix + name)
        info.create_system = 3
        info.external_attr = 0o100644 << 16
        info.compress_type = zipfile.ZIP_DEFLATED
        archive.writestr(info, contents[name])
    manifest = ''.join(f'{hashlib.sha256(data).hexdigest()}  {name}\n' for name, data in sorted(contents.items()))
    info = zipfile.ZipInfo(prefix + 'VERIFICA-SHA256.txt')
    info.create_system = 3
    info.external_attr = 0o100644 << 16
    info.compress_type = zipfile.ZIP_DEFLATED
    archive.writestr(info, manifest)
print(args.output)
