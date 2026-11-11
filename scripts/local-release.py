"""Build natively and assemble a shared GitHub draft without running Actions."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tempfile

import release

ROOT = release.ROOT
REPOSITORY = os.environ.get('RELEASE_REPOSITORY') or release.DEFAULT_RELEASE_REPOSITORY


def run(*command, capture=False):
    return subprocess.run(command, cwd=ROOT, check=True, text=True, encoding='utf-8',
                          capture_output=capture)


def source_state():
    commit = run('git', 'rev-parse', 'HEAD', capture=True).stdout.strip()
    if not re.fullmatch(r'(?:[0-9a-f]{40}|[0-9a-f]{64})', commit):
        raise ValueError('Commit sorgente non valido.')
    dirty = bool(run('git', 'status', '--porcelain', '--untracked-files=normal', capture=True).stdout.strip())
    return commit, dirty


def native_platform():
    architecture = platform.machine().lower()
    if sys.platform == 'win32' and architecture in ('amd64', 'x86_64'):
        return 'windows-x86_64'
    if sys.platform == 'darwin':
        if architecture in ('arm64', 'aarch64'): return 'darwin-aarch64'
        if architecture == 'x86_64': return 'darwin-x86_64'
    raise ValueError('Compila su Windows x64, Mac Apple Silicon o Mac Intel. Ogni computer produce il proprio pacchetto nativo.')


def workspace_tools():
    folder = ROOT / '.tools'
    folder.mkdir(exist_ok=True)
    if not folder.resolve().is_relative_to(ROOT.resolve()):
        raise ValueError('La cartella di lavoro temporanea deve restare nel progetto.')
    return folder


def package_folder(commit, target):
    folder = workspace_tools() / 'local-releases' / release.version() / commit / target
    if not folder.resolve().is_relative_to(ROOT.resolve()):
        raise ValueError('Percorso pacchetti locali non valido.')
    return folder


def asset_names(target):
    prefix = f'PARLO_{release.version()}_{target}'
    package = prefix + ('-setup.exe' if target == 'windows-x86_64' else '.app.tar.gz')
    return [package, package + '.sig'] + ([prefix + '.dmg'] if target.startswith('darwin-') else [])


def digest(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def validate_package(folder, target, commit):
    metadata = json.loads((folder / f'update-{target}.json').read_text(encoding='utf-8'))
    names = asset_names(target)
    if (metadata.get('version') != release.version() or metadata.get('platform') != target
            or metadata.get('source_commit') != commit or metadata.get('source_dirty') is not False
            or metadata.get('file') != names[0]):
        raise ValueError(f'Pacchetto {target} di versione/commit diverso o compilato con modifiche non committate. Ricompila dallo stesso commit pulito.')
    if set(metadata.get('sha256', {})) != set(names):
        raise ValueError(f'Elenco file del pacchetto {target} non valido.')
    for name in names:
        file = folder / name
        if not file.is_file() or file.stat().st_size == 0 or digest(file) != metadata['sha256'][name]:
            raise ValueError(f'Pacchetto mancante o modificato: {name}')
    if not metadata.get('signature') or (folder / names[1]).read_text(encoding='utf-8').strip() != metadata['signature']:
        raise ValueError(f'Firma del pacchetto {target} non coerente.')
    return names


def github_ready(commit):
    if not shutil.which('gh'):
        raise ValueError('Installa GitHub CLI (gh), poi esegui gh auth login con un account che possa scrivere nella repository delle release.')
    release.gh('auth', 'status')
    metadata = json.loads(release.gh('repo', 'view', REPOSITORY, '--json', 'visibility,viewerPermission').stdout)
    if metadata['visibility'] != 'PUBLIC':
        raise ValueError('La repository di distribuzione deve essere pubblica.')
    if metadata['viewerPermission'] not in ('ADMIN', 'MAINTAIN', 'WRITE'):
        raise ValueError(f'L’account GitHub deve avere accesso in scrittura a {REPOSITORY}.')
    remote = release.gh('api', f'repos/{REPOSITORY}/git/commits/{commit}', '--jq', '.sha', check=False)
    if remote.returncode or remote.stdout.strip() != commit:
        raise ValueError(f'Fai push del commit {commit[:12]} su {REPOSITORY} prima di pubblicare.')


def draft(commit, create=False, allow_published=False):
    tag = 'v' + release.version()
    marker = f'<!-- parlo-source:{commit} -->'
    def read():
        return release.gh('release', 'view', tag, '--repo', REPOSITORY, '--json', 'isDraft,body,assets', check=False)
    result = read()
    if result.returncode != 0 and create:
        # Two computers may try to create the same draft; reload the winner.
        created = release.gh('release', 'create', tag, '--repo', REPOSITORY, '--target', commit, '--draft',
                             '--title', f'PARLO {release.version()}',
                             '--notes', f'Pacchetti locali di PARLO {release.version()}.\n\n{marker}', check=False)
        result = read()
        if result.returncode != 0:
            raise ValueError(created.stderr.strip() or result.stderr.strip() or 'Creazione bozza non riuscita.')
    elif result.returncode != 0:
        raise ValueError(result.stderr.strip() or 'Bozza non trovata: carica prima i pacchetti.')
    metadata = json.loads(result.stdout)
    if not metadata['isDraft'] and not allow_published:
        raise ValueError(f'La versione {tag} è già pubblicata: imposta un numero nuovo, committa e fai push senza tag nella repository sorgenti.')
    if marker not in metadata.get('body', ''):
        raise ValueError('Questa bozza appartiene a un altro commit o non contiene la provenienza dei sorgenti. Usa un numero di versione nuovo.')
    return metadata


def upload(commit, target, publish_now=False):
    folder = package_folder(commit, target)
    names = validate_package(folder, target, commit)
    github_ready(commit)
    if publish_now:
        metadata = draft(commit, create=True, allow_published=True)
        existing = {asset['name'] for asset in metadata['assets']}
        # A colleague may add a missing platform to an already public version.
        # Never overwrite a binary that users may already have downloaded.
        pending = [name for name in names + [f'update-{target}.json'] if name not in existing]
        if pending:
            release.gh('release', 'upload', 'v' + release.version(), '--repo', REPOSITORY,
                       *map(str, [folder / name for name in pending]))
        publish_ready(commit)
        return
    draft(commit, create=True)
    release.gh('release', 'upload', 'v' + release.version(), '--repo', REPOSITORY, '--clobber',
               *map(str, [folder / name for name in names] + [folder / f'update-{target}.json']))
    print(f'Pacchetto {target} caricato nella bozza: https://github.com/{REPOSITORY}/releases', flush=True)
    print('Quando tutti i pacchetti sono pronti: npm run release:publish (Windows + entrambi i Mac).', flush=True)
    print('Per Windows e solo questo Mac: npm run release:publish -- --platforms windows-x86_64 ' + (target if target.startswith('darwin-') else 'darwin-aarch64'), flush=True)


def build(args, commit, dirty):
    target = native_platform()
    if args.upload and dirty:
        raise ValueError('Prima del caricamento committa e fai push dei sorgenti senza tag. La build con modifiche locali è disponibile senza --upload.')
    if args.adhoc and not target.startswith('darwin-'):
        raise ValueError('--adhoc serve soltanto per le build Mac.')
    if target.startswith('darwin-') and not args.adhoc and os.environ.get('APPLE_SIGNING_IDENTITY', '').strip() in ('', '-'):
        raise ValueError('Sul Mac configura APPLE_SIGNING_IDENTITY oppure aggiungi --adhoc per la build gratuita con firma ad hoc.')
    if not os.environ.get('TAURI_SIGNING_PRIVATE_KEY'):
        key = ROOT / '.tools/updater/parlo.key'
        if not key.is_file():
            key = key.with_name('presentio.key')
        if not key.is_file():
            raise ValueError('Manca la chiave aggiornamenti. Usa la chiave esistente in .tools/updater oppure configura TAURI_SIGNING_PRIVATE_KEY.')
        os.environ['TAURI_SIGNING_PRIVATE_KEY'] = str(key)
    if args.upload:
        github_ready(commit)
        existing = release.gh('release', 'view', 'v' + release.version(), '--repo', REPOSITORY, '--json', 'isDraft', check=False)
        if existing.returncode == 0 and not json.loads(existing.stdout)['isDraft'] and not getattr(args, 'publish_now', False):
            raise ValueError('Versione già pubblicata. Imposta un numero nuovo prima di compilare.')
        if existing.returncode == 0:
            draft(commit, allow_published=True) if getattr(args, 'publish_now', False) else draft(commit)
    npm = 'npm.cmd' if sys.platform == 'win32' else 'npm'
    run(npm, 'ci', '--no-audit', '--no-fund')
    updater_config = str(release.configure_updater())
    if target == 'windows-x86_64':
        powershell = shutil.which('powershell.exe')
        if not powershell: raise ValueError('PowerShell Windows non disponibile.')
        gpu = ROOT / 'engine/runtime/win-x64-vulkan'
        if not all((gpu / name).is_file() for name in ['whisper-cli.exe', 'runtime.json']):
            run(powershell, '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(ROOT / 'scripts/build-whisper-cpp.ps1'))
        run(powershell, '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(ROOT / 'scripts/build.ps1'), '-Installer', '-BuildPython', sys.executable, '-UpdaterConfig', updater_config)
    else:
        run(sys.executable, str(ROOT / 'scripts/build-mac.py'), '--updater-config', updater_config, *(['--adhoc'] if args.adhoc else []))
    after_commit, after_dirty = source_state()
    if after_commit != commit or (not dirty and after_dirty):
        raise ValueError('I sorgenti sono cambiati durante la compilazione: ricompila prima di raccogliere o caricare i pacchetti.')
    release.ASSETS = package_folder(commit, target)
    release.collect(target)
    file = release.ASSETS / f'update-{target}.json'
    metadata = json.loads(file.read_text(encoding='utf-8'))
    metadata.update(source_commit=commit, source_dirty=dirty or after_dirty,
                    sha256={name: digest(release.ASSETS / name) for name in asset_names(target)})
    file.write_text(json.dumps(metadata, indent=2) + '\n', encoding='utf-8')
    print(f'Pacchetti locali: {release.ASSETS}', flush=True)
    if args.upload: upload(commit, target, publish_now=True) if getattr(args, 'publish_now', False) else upload(commit, target)


def publish(commit, targets, allow_published=False):
    if not targets or len(set(targets)) != len(targets) or any(target not in release.PLATFORMS for target in targets):
        raise ValueError('Selezione piattaforme non valida.')
    github_ready(commit)
    metadata = draft(commit, allow_published=True) if allow_published else draft(commit)
    names = [name for target in targets for name in asset_names(target) + [f'update-{target}.json']]
    available = {asset['name'] for asset in metadata['assets']}
    missing = sorted(set(names) - available)
    if missing:
        raise ValueError('Bozza incompleta. Carica i pacchetti mancanti: ' + ', '.join(missing))
    with tempfile.TemporaryDirectory(prefix='local-release-', dir=workspace_tools()) as temporary:
        stage = Path(temporary)
        if not stage.resolve().is_relative_to(ROOT.resolve()): raise ValueError('Percorso temporaneo non valido.')
        patterns = [part for name in names for part in ['--pattern', name]]
        release.gh('release', 'download', 'v' + release.version(), '--repo', REPOSITORY, '--dir', str(stage), *patterns)
        for target in targets: validate_package(stage, target, commit)
        # Assets already reside in the draft: publish only the complete updater
        # manifest, then make the release visible. Never re-upload binaries here.
        release.ASSETS = stage
        if allow_published:
            release.publish('v' + release.version(), platforms=targets, upload_packages=False, allow_existing=True)
        else:
            release.publish('v' + release.version(), platforms=targets, upload_packages=False)


def publish_ready(commit):
    metadata = draft(commit, allow_published=True)
    available = {asset['name'] for asset in metadata['assets']}
    targets = [target for target in release.PLATFORMS
               if set(asset_names(target) + [f'update-{target}.json']).issubset(available)]
    if not targets:
        raise ValueError('Nessun pacchetto completo pronto per la pubblicazione.')
    publish(commit, targets, allow_published=True)


def ship(commit, dirty):
    if dirty:
        raise ValueError(f'Committa i cambiamenti e fai push su {REPOSITORY} prima di pubblicare.')
    target = native_platform()
    github_ready(commit)
    latest = release.gh('release', 'view', '--repo', REPOSITORY, '--json', 'tagName', check=False)
    if latest.returncode == 0:
        tag = json.loads(latest.stdout)['tagName']
        if re.fullmatch(r'v\d+\.\d+\.\d+', tag) and tuple(map(int, tag[1:].split('.'))) > tuple(map(int, release.version().split('.'))):
            raise ValueError(f'È già uscita {tag}. Recupera il codice della versione corrente prima di pubblicare.')
    existing = release.gh('release', 'view', 'v' + release.version(), '--repo', REPOSITORY,
                          '--json', 'isDraft', check=False)
    if existing.returncode == 0:
        metadata = draft(commit, allow_published=True)
        available = {asset['name'] for asset in metadata['assets']}
        if set(asset_names(target) + [f'update-{target}.json']).issubset(available):
            publish_ready(commit)
            print(f'{target} {release.version()} è già disponibile: nessuna ricompilazione necessaria.', flush=True)
            return
    folder = package_folder(commit, target)
    if (folder / f'update-{target}.json').is_file():
        validate_package(folder, target, commit)
        upload(commit, target, publish_now=True)
        return
    args = argparse.Namespace(upload=True, publish_now=True,
                              adhoc=target.startswith('darwin-') and os.environ.get('APPLE_SIGNING_IDENTITY', '').strip() in ('', '-'))
    build(args, commit, dirty)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    commands.add_parser('version', help='Allinea la nuova versione prima del commit').add_argument('version')
    commands.add_parser('ship', help='Un comando: compila e pubblica questo sistema, oppure lo aggiunge alla stessa release già pubblicata')
    builder = commands.add_parser('build', help='Compila il pacchetto per questo computer')
    builder.add_argument('--upload', action='store_true', help='Carica il pacchetto nella bozza pubblica dopo la build')
    builder.add_argument('--adhoc', action='store_true', help='Mac: firma ad hoc gratuita, con possibile ripristino permessi dopo aggiornamento')
    commands.add_parser('upload', help='Carica il pacchetto locale già compilato dallo stesso commit')
    publisher = commands.add_parser('publish', help='Pubblica la bozza completa e abilita gli aggiornamenti')
    publisher.add_argument('--platforms', nargs='+', choices=release.PLATFORMS, default=list(release.PLATFORMS))
    args = parser.parse_args()
    if args.command == 'version':
        release.set_version(args.version)
        return
    release.release_repository()
    release.validate('v' + release.version())
    commit, dirty = source_state()
    if args.command == 'ship': ship(commit, dirty)
    elif args.command == 'build': build(args, commit, dirty)
    elif dirty: raise ValueError('Committa e fai push dei sorgenti senza tag prima di caricare o pubblicare.')
    elif args.command == 'upload': upload(commit, native_platform())
    else: publish(commit, args.platforms)


if __name__ == '__main__':
    try:
        main()
    except subprocess.CalledProcessError as error:
        raise SystemExit(error.stderr or f'Comando interrotto (codice {error.returncode}). Controlla lo stato della bozza prima di riprovare.') from error
    except (OSError, ValueError, KeyError) as error:
        raise SystemExit(str(error)) from error
