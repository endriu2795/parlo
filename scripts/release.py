"""Release production only: collect explicit artifacts, then publish atomically."""
import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tomllib
from urllib.parse import quote

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / '.tools/release-assets'
PLATFORMS = ('windows-x86_64', 'darwin-aarch64', 'darwin-x86_64')
DEFAULT_RELEASE_REPOSITORY = 'endriu2795/parlo'
UPDATER_TEMPLATE = ROOT / 'scripts/official-updater.json'
UPDATER_CONFIG = ROOT / '.tools/updater/official.json'


def release_repository():
    repository = os.environ.get('RELEASE_REPOSITORY') or DEFAULT_RELEASE_REPOSITORY
    if not re.fullmatch(r'[A-Za-z0-9-]+/[A-Za-z0-9_.-]+', repository):
        raise ValueError('Imposta RELEASE_REPOSITORY con la repository GitHub pubblica di PARLO (owner/repo).')
    return repository


def configure_updater():
    repository = release_repository()
    config = json.loads(UPDATER_TEMPLATE.read_text(encoding='utf-8'))
    config['endpoints'] = [f'https://github.com/{repository}/releases/latest/download/latest.json']
    UPDATER_CONFIG.parent.mkdir(parents=True, exist_ok=True)
    UPDATER_CONFIG.write_text(json.dumps(config, indent=2) + '\n', encoding='utf-8')
    return UPDATER_CONFIG


def version():
    return json.loads((ROOT / 'package.json').read_text(encoding='utf-8'))['version']


def validate(tag):
    if not re.fullmatch(r'v\d+\.\d+\.\d+', tag):
        raise ValueError('Usa un tag stabile nel formato v1.2.3.')
    expected = tag[1:]
    paths = ('package.json', 'package-lock.json', 'src-tauri/tauri.conf.json')
    for name in paths:
        data = json.loads((ROOT / name).read_text(encoding='utf-8'))
        if data['version'] != expected:
            raise ValueError(f'Versione di {name} diversa dal tag {tag}.')
        if name == 'package-lock.json' and data['packages']['']['version'] != expected:
            raise ValueError('Aggiorna anche la versione radice di package-lock.json.')
    cargo = tomllib.loads((ROOT / 'src-tauri/Cargo.toml').read_text(encoding='utf-8'))
    if cargo['package']['version'] != expected:
        raise ValueError('Versione Cargo.toml diversa dal tag.')
    print(f'Versione {expected} pronta per la pubblicazione.')


def set_version(number):
    if not re.fullmatch(r'\d+\.\d+\.\d+', number):
        raise ValueError('Usa una versione nel formato 1.2.3.')
    for name in ('package.json', 'package-lock.json', 'src-tauri/tauri.conf.json'):
        path = ROOT / name
        data = json.loads(path.read_text(encoding='utf-8'))
        data['version'] = number
        if name == 'package-lock.json': data['packages']['']['version'] = number
        path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    path = ROOT / 'src-tauri/Cargo.toml'
    text, count = re.subn(r'(?m)(^\[package\][\s\S]*?^version\s*=\s*)"[^"]+"',
                          lambda match: match[1] + f'"{number}"', path.read_text(encoding='utf-8'), count=1)
    if count != 1: raise ValueError('Versione Cargo.toml non trovata.')
    path.write_text(text, encoding='utf-8')
    path = ROOT / 'src-tauri/Cargo.lock'
    text, count = re.subn(r'(?m)(^name = "callnotes"\nversion = )"[^"]+"',
                          lambda match: match[1] + f'"{number}"', path.read_text(encoding='utf-8'), count=1)
    if count != 1: raise ValueError('Versione Cargo.lock non trovata.')
    path.write_text(text, encoding='utf-8')
    validate('v' + number)


def only(folder, pattern):
    paths = list(folder.glob(pattern))
    if len(paths) != 1:
        raise ValueError(f'Atteso un solo {pattern} in {folder}; trovati {len(paths)}.')
    return paths[0]


def collect(platform):
    ASSETS.mkdir(parents=True, exist_ok=True)
    bundle = ROOT / 'src-tauri/target/release/bundle'
    prefix = f'PARLO_{version()}_{platform}'
    if platform == 'windows-x86_64':
        package = only(bundle / 'nsis', f'*_{version()}_*-setup.exe')
        name = f'{prefix}-setup.exe'
    else:
        package = only(bundle / 'macos', '*.app.tar.gz')
        name = f'{prefix}.app.tar.gz'
        shutil.copy2(only(bundle / 'dmg', f'*_{version()}_*.dmg'), ASSETS / f'{prefix}.dmg')
    signature_file = package.with_name(package.name + '.sig')
    signature = signature_file.read_text(encoding='utf-8').strip()
    if not signature:
        raise ValueError(f'Firma aggiornamento mancante per {platform}.')
    shutil.copy2(package, ASSETS / name)
    shutil.copy2(signature_file, ASSETS / f'{name}.sig')
    manifest = {'version': version(), 'platform': platform, 'file': name, 'signature': signature}
    (ASSETS / f'update-{platform}.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    print(f'Pacchetto {platform} e firma pronti.')


def gh(*args, check=True):
    return subprocess.run(['gh', *args], check=check, capture_output=True, text=True, encoding='utf-8')


def publish(tag, platforms=PLATFORMS, upload_packages=True, allow_existing=False):
    validate(tag)
    platforms = tuple(platforms)
    if not platforms or len(set(platforms)) != len(platforms) or any(p not in PLATFORMS for p in platforms):
        raise ValueError('Selezione piattaforme non valida.')
    if allow_existing and upload_packages:
        raise ValueError('Una release pubblicata può ricevere il manifest aggiornato, non la sostituzione dei pacchetti già pubblicati.')
    repository = release_repository()
    metadata = json.loads(gh('repo', 'view', repository, '--json', 'visibility').stdout)
    if metadata['visibility'] != 'PUBLIC':
        raise ValueError('La repository degli aggiornamenti deve essere pubblica.')
    manifest_platforms, files = {}, []
    for platform in platforms:
        fragment = json.loads((ASSETS / f'update-{platform}.json').read_text(encoding='utf-8'))
        if fragment['version'] != version() or fragment['platform'] != platform:
            raise ValueError(f'Pacchetto {platform} non coerente con la release.')
        name = fragment['file']
        if Path(name).name != name or not fragment['signature'].strip():
            raise ValueError('Nome pacchetto o firma non validi.')
        package = ASSETS / name
        if not package.is_file() or not package.stat().st_size:
            raise ValueError(f'Pacchetto {platform} mancante o vuoto.')
        manifest_platforms[platform] = {
            'url': f'https://github.com/{repository}/releases/download/{tag}/{quote(name)}',
            'signature': fragment['signature'],
        }
        files += [package, ASSETS / f'{name}.sig']
        if platform.startswith('darwin-'):
            dmg = ASSETS / f'PARLO_{version()}_{platform}.dmg'
            if not dmg.is_file() or not dmg.stat().st_size:
                raise ValueError(f'DMG {platform} mancante.')
            files.append(dmg)
    # A late-finishing older tag must never replace a newer release as latest.
    latest = gh('release', 'view', '--repo', repository, '--json', 'tagName', check=False)
    if latest.returncode == 0:
        latest_tag = json.loads(latest.stdout)['tagName']
        if re.fullmatch(r'v\d+\.\d+\.\d+', latest_tag) and tuple(map(int, latest_tag[1:].split('.'))) > tuple(map(int, tag[1:].split('.'))):
            raise ValueError(f'Esiste già una versione più recente: {latest_tag}.')
    existing = gh('release', 'view', tag, '--repo', repository, '--json', 'isDraft', check=False)
    if existing.returncode == 0:
        if not json.loads(existing.stdout)['isDraft'] and not allow_existing:
            raise ValueError('Versione già pubblicata: usa un nuovo numero di versione.')
    else:
        commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
        gh('release', 'create', tag, '--repo', repository, '--target', commit, '--draft',
           '--title', f'PARLO {version()}', '--notes', f'PARLO {version()} per Windows e macOS.')
    manifest = ASSETS / 'latest.json'
    manifest.write_text(json.dumps({
        'version': version(), 'notes': f'PARLO {version()}',
        'pub_date': datetime.now(timezone.utc).isoformat(), 'platforms': manifest_platforms,
    }, indent=2) + '\n', encoding='utf-8')
    # Upload only the release assets selected above.
    uploads = files if upload_packages else []
    gh('release', 'upload', tag, '--repo', repository, '--clobber', *map(str, uploads), str(manifest))
    gh('release', 'edit', tag, '--repo', repository, '--draft=false', '--latest')
    print(f'Pubblicata https://github.com/{repository}/releases/tag/{tag}')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest='command', required=True)
    commands.add_parser('validate').add_argument('tag')
    commands.add_parser('set-version').add_argument('version')
    commands.add_parser('configure-updater')
    commands.add_parser('collect').add_argument('--platform', required=True, choices=PLATFORMS)
    commands.add_parser('publish').add_argument('tag')
    args = parser.parse_args()
    try:
        if args.command == 'validate': validate(args.tag)
        elif args.command == 'set-version': set_version(args.version)
        elif args.command == 'configure-updater': print(configure_updater())
        elif args.command == 'collect': collect(args.platform)
        else: publish(args.tag)
    except subprocess.CalledProcessError as error:
        raise SystemExit(error.stderr or str(error)) from error
    except (OSError, ValueError, KeyError) as error:
        raise SystemExit(str(error)) from error
