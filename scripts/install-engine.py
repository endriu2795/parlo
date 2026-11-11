"""UI provisioning entry point; never requires a shell or administrator rights."""
import argparse
import hashlib
from contextlib import contextmanager
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import uuid


def validate_wheelhouse(folder, requirements):
    """Reject damaged bundled dependencies before creating/modifying the venv."""
    def digest(path):
        with path.open('rb') as stream:
            return hashlib.file_digest(stream, 'sha256').hexdigest()
    manifest = json.loads((folder / 'manifest.json').read_text(encoding='utf-8'))
    if manifest.get('version') != 1 or manifest.get('platform') != 'win-x64' or os.name != 'nt':
        raise RuntimeError('Pacchetto dipendenze non compatibile con questo computer.')
    if manifest.get('requirements_sha256') != digest(requirements):
        raise RuntimeError('Le dipendenze incluse non corrispondono a questa versione di PARLO.')
    names = set()
    for item in manifest.get('files', []):
        name = item.get('name', '')
        if not isinstance(name, str) or Path(name).name != name or not name.endswith('.whl') or name in names:
            raise RuntimeError('Manifest delle dipendenze non valido.')
        names.add(name)
        file = folder / name
        if file.is_symlink() or not file.is_file() or file.stat().st_size != item.get('size') or digest(file) != item.get('sha256'):
            raise RuntimeError('Dipendenza inclusa danneggiata. Scarica di nuovo il pacchetto PARLO.')
    if not names or names != {p.name for p in folder.glob('*.whl')}:
        raise RuntimeError('Pacchetto dipendenze incompleto o modificato.')


def executable(environment):
    return environment / ('Scripts/python.exe' if os.name == 'nt' else 'bin/python')


def uses_current_runtime(python):
    if not python.is_file():
        return False
    try:
        output = subprocess.run(
            [str(python), '-I', '-c', 'import json,sys,pip; print(json.dumps([sys.base_prefix, list(sys.version_info[:2])]))'],
            check=True, capture_output=True, text=True, timeout=20,
        )
        base, version = json.loads(output.stdout)
        return Path(base).resolve() == Path(sys.base_prefix).resolve() and version == list(sys.version_info[:2])
    except (OSError, subprocess.SubprocessError, ValueError):
        return False


def recover_interrupted_install(root, environment, journal):
    if not journal.exists():
        return
    info = json.loads(journal.read_text(encoding='utf-8'))
    name = info.get('backup')
    if name is not None:
        # A journal is local data, but must never authorize deleting another path.
        if not isinstance(name, str) or not name.startswith('.venv-previous-') or Path(name).name != name:
            raise RuntimeError('Stato di installazione non valido; nessun file è stato eliminato.')
        backup = root / name
        if backup.is_symlink() or (hasattr(backup, 'is_junction') and backup.is_junction()):
            raise RuntimeError('Backup dell’ambiente non valido.')
        if backup.is_dir():
            if environment.exists():
                shutil.rmtree(environment)
            backup.rename(environment)
    elif environment.exists():
        shutil.rmtree(environment)
    journal.unlink()


@contextmanager
def engine_environment(root):
    """Rebuild a legacy/moved venv at its final path; restore it if setup fails."""
    root = root.resolve()
    root.mkdir(parents=True, exist_ok=True)
    environment = root / 'venv'
    # Never traverse an externally supplied link during rollback/cleanup.
    if environment.is_symlink() or (hasattr(environment, 'is_junction') and environment.is_junction()):
        raise RuntimeError('La cartella venv deve essere locale a PARLO, senza collegamenti esterni.')
    if environment.exists() and not environment.is_dir():
        raise RuntimeError('La cartella venv non è valida; nessun file è stato eliminato.')
    journal = root / 'venv-installing.json'
    recover_interrupted_install(root, environment, journal)
    python = executable(environment)
    replacement = not uses_current_runtime(python)
    backup = None
    try:
        if replacement:
            if environment.exists():
                backup = root / f'.venv-previous-{uuid.uuid4()}'
            partial = root / 'venv-installing.json.tmp'
            partial.write_text(json.dumps({'backup': backup.name if backup else None}), encoding='utf-8')
            partial.replace(journal)
            if backup is not None:
                environment.rename(backup)
            print('PARLO_STAGE:Preparazione dell’ambiente con il Python incluso…', flush=True)
            subprocess.run([sys.executable, '-I', '-m', 'venv', str(environment)], check=True)
        yield python
    except BaseException:
        if replacement and (backup is None or backup.exists()):
            if environment.exists():
                shutil.rmtree(environment)
            if backup is not None:
                backup.rename(environment)
        if replacement:
            journal.unlink(missing_ok=True)
        raise
    else:
        journal.unlink(missing_ok=True)
        if backup is not None:
            # A cleanup error must not roll back an otherwise working new engine.
            shutil.rmtree(backup, ignore_errors=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--engine', type=Path, required=True)
    parser.add_argument('--model', choices=['turbo', 'tiny', 'small'], default='turbo')
    parser.add_argument('--wheelhouse', type=Path)
    args = parser.parse_args()
    source = Path(__file__).resolve().parents[1]
    requirements = source / 'engine/requirements-lock.txt'
    if args.wheelhouse:
        validate_wheelhouse(args.wheelhouse, requirements)
    with engine_environment(args.engine) as python:
        print('PARLO_STAGE:Preparazione delle dipendenze incluse…' if args.wheelhouse else 'PARLO_STAGE:Installazione delle dipendenze…', flush=True)
        install = [str(python), '-I', '-m', 'pip', 'install', '--disable-pip-version-check', '--only-binary=:all:', '-r', str(requirements)]
        if args.wheelhouse:
            install.extend(['--no-index', '--find-links', str(args.wheelhouse)])
        subprocess.run(install, check=True)
        print('PARLO_STAGE:Download modelli e ottimizzazione per questo computer…', flush=True)
        subprocess.run([str(python), '-u', str(source / 'scripts/setup-engine.py'), '--engine', str(args.engine.resolve()), '--model', args.model], check=True)
    print('Motore pronto.', flush=True)


if __name__ == '__main__':
    main()
