"""Prepare verified Windows wheels at build time, not on the user's computer."""
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def main():
    if sys.platform != 'win32' or sys.version_info[:2] != (3, 12):
        raise RuntimeError('Prepare Windows engine dependencies using Python 3.12 on Windows.')
    root = Path(__file__).resolve().parents[1]
    requirements = root / 'engine/requirements-lock.txt'
    target = root / 'engine/runtime/wheels/win-x64'
    target.mkdir(parents=True, exist_ok=True)
    if not target.resolve().is_relative_to(root.resolve()) or target.is_symlink() or (hasattr(target, 'is_junction') and target.is_junction()):
        raise RuntimeError('Engine bundle must stay inside the project.')
    spec = importlib.util.spec_from_file_location('installer', root / 'scripts/install-engine.py')
    installer = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(installer)
    try:
        installer.validate_wheelhouse(target, requirements)
        print('Included engine dependencies already verified; no download needed.')
        return
    except (OSError, ValueError, RuntimeError):
        pass
    # pip downloads wheels only; no compiler/build scripts run during packaging.
    temporary_root = root / '.tools'
    temporary_root.mkdir(exist_ok=True)
    if not temporary_root.resolve().is_relative_to(root.resolve()):
        raise RuntimeError('Temporary build folder must stay inside the project.')
    with tempfile.TemporaryDirectory(prefix='wheel-build-', dir=temporary_root) as temp:
        stage = Path(temp).resolve()
        if not stage.is_relative_to(temporary_root.resolve()):
            raise RuntimeError('Temporary wheel folder outside build workspace.')
        subprocess.run([sys.executable, '-I', '-m', 'pip', 'download',
            '--disable-pip-version-check', '--only-binary=:all:',
            '-r', str(requirements), '--dest', str(stage)], check=True)
        names = {p.name for p in stage.glob('*.whl')}
        if not names:
            raise RuntimeError('No engine wheels downloaded.')
        for old in target.glob('*.whl'):
            if old.name not in names: old.unlink()
        for wheel in stage.glob('*.whl'): wheel.replace(target / wheel.name)
    wheels = sorted(target.glob('*.whl'))
    if not wheels:
        raise RuntimeError('No engine wheels downloaded.')
    manifest = {'version': 1, 'platform':'win-x64',
        'requirements_sha256':digest(requirements),
        'files':[{'name':p.name, 'size':p.stat().st_size, 'sha256':digest(p)} for p in wheels]}
    (target / 'manifest.json').write_text(json.dumps(manifest, indent=2)+'\n', encoding='utf-8')
    print(f'Engine dependencies bundled: {len(wheels)} wheels, {sum(p.stat().st_size for p in wheels)/1048576:.1f} MiB')


if __name__ == '__main__':
    main()
