"""Build on a Mac; users receive the app/DMG and never need these dev tools."""
import json
import os
import argparse
from pathlib import Path
import platform
import shutil
import subprocess
import sys
from bundle_python import bundle_python

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument('--test', action='store_true', help='Build debug e solo .app per il kit di prova')
parser.add_argument('--adhoc', action='store_true', help='Release locale gratuita con firma ad hoc; i permessi potrebbero dover essere ripristinati dopo un aggiornamento')
parser.add_argument('--updater-config', type=Path, help='File JSON con pubkey ed endpoints per firmare gli aggiornamenti')
args = parser.parse_args()
if args.updater_config and args.test:
    parser.error('--updater-config richiede una build release.')
updater = None
if args.updater_config:
    updater = json.loads(args.updater_config.read_text(encoding='utf-8'))
    if not isinstance(updater, dict) or not updater.get('pubkey') or not updater.get('endpoints'):
        parser.error('La configurazione updater richiede pubkey ed endpoints.')
    if not os.environ.get('TAURI_SIGNING_PRIVATE_KEY'):
        parser.error('Configura TAURI_SIGNING_PRIVATE_KEY per firmare gli aggiornamenti.')
if platform.system() != 'Darwin':
    raise SystemExit('La build Mac deve essere eseguita su macOS; Windows non include SDK e compilatore Apple.')
os.environ.setdefault('MACOSX_DEPLOYMENT_TARGET', '13.0')
signing_identity = os.environ.get('APPLE_SIGNING_IDENTITY', '').strip()
if args.adhoc and args.test:
    raise SystemExit('Usa --adhoc per una release locale oppure --test per la .app debug.')
if args.adhoc:
    signing_identity = '-'
    os.environ['APPLE_SIGNING_IDENTITY'] = '-'
    for name in ['APPLE_CERTIFICATE', 'APPLE_CERTIFICATE_PASSWORD', 'APPLE_ID', 'APPLE_PASSWORD',
                 'APPLE_TEAM_ID', 'APPLE_API_ISSUER', 'APPLE_API_KEY', 'APPLE_API_KEY_PATH']:
        os.environ.pop(name, None)
    print('Firma ad hoc richiesta: dopo l’aggiornamento potrebbe essere necessario ripristinare i permessi di PARLO.', flush=True)
if not args.test and (not signing_identity or signing_identity == '-') and not args.adhoc:
    raise SystemExit('Configura APPLE_SIGNING_IDENTITY con un certificato Developer ID Application oppure usa esplicitamente --adhoc per una release locale gratuita. La firma ad hoc non conserva l’identità tra versioni.')
for tool in ('xcrun', 'cmake', 'cargo', 'npm'):
    if not shutil.which(tool):
        raise SystemExit(f'Manca {tool} nell’ambiente di sviluppo. Vedi MACOS.md.')
subprocess.run(['xcrun', '--sdk', 'macosx', '--show-sdk-path'], check=True)
tools = root / '.tools'
tools.mkdir(exist_ok=True)
staging = tools / 'mac-engine'
subprocess.run([sys.executable, str(root / 'scripts/build-whisper-metal.py'), '--engine', str(staging)], check=True)
bundle_name = 'mac-arm64-metal' if platform.machine() == 'arm64' else 'mac-x64-metal'
bundle = root / 'engine/runtime' / bundle_name
shutil.copytree(staging / 'whisper-cpp', bundle, dirs_exist_ok=True)
# Keep unsigned development builds runnable. Distribution uses Developer ID
# signing and notarization through Tauri's standard Apple environment variables.
for binary in list(bundle.glob('*.dylib')) + [bundle / 'whisper-cli']:
    subprocess.run(['codesign', '--force', '--sign', '-', str(binary)], check=True)
config = json.loads((root / 'src-tauri/tauri.macos.conf.json').read_text())
python_archive = bundle_python('mac-arm64' if platform.machine() == 'arm64' else 'mac-x64')
target_platform = 'mac-arm64' if platform.machine() == 'arm64' else 'mac-x64'
subprocess.run([sys.executable, str(root / 'scripts/prepare-runtime.py'), '--platform', target_platform], check=True)
subprocess.run([sys.executable, str(root / 'scripts/prepare-runtime.py'), '--platform', target_platform, '--kind', 'documents'], check=True)
prepared = root / 'engine/runtime/prepared' / target_platform
config['bundle']['resources'].update({str(bundle): f'engine/runtime/{bundle_name}', str(prepared): f'engine/runtime/prepared/{target_platform}'})
config['bundle']['resources'][str(root / 'engine/runtime/documents' / target_platform)] = f'engine/runtime/documents/{target_platform}'
config['bundle']['macOS']['entitlements'] = str(root / 'src-tauri/Entitlements.plist')
config['bundle']['macOS']['infoPlist'] = str(root / 'src-tauri/Info.plist')
# Ad hoc is an explicit choice for local builds; it must not be a silent
# fallback in the public release workflow.
config['bundle']['macOS']['signingIdentity'] = signing_identity or '-'
if args.test:
    config['bundle']['targets'] = ['app']
if updater:
    config['bundle']['createUpdaterArtifacts'] = True
    config['plugins'] = {'updater': updater}
config_file = tools / 'tauri-macos-build.json'
config_file.write_text(json.dumps(config), encoding='utf-8')
command = ['npm', 'run', 'tauri', '--', 'build', '--config', str(config_file)]
if args.test:
    command.append('--debug')
subprocess.run(command, cwd=root, check=True)
if args.test:
    print('App di prova disponibile in src-tauri/target/debug/bundle/macos.')
else:
    print('App e DMG disponibili in src-tauri/target/release/bundle.')
