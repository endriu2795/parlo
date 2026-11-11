"""Explicit optional setup in PARLO's private directory, never during synthesis."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys


def emit(**value):
    print(json.dumps(value), flush=True)


def run(command):
    environment = {key: value for key, value in os.environ.items()
                   if not key.upper().startswith(('PYTHON', 'PIP_'))}
    result = subprocess.run(command, env=environment, capture_output=True,
                            creationflags=0x08000000 if os.name == 'nt' else 0)
    if result.returncode:
        raise RuntimeError('dependencies')
    return result.stdout


def prepare(root, device, repair=False):
    if os.name != 'nt' or sys.maxsize < 2**32:
        raise RuntimeError('platform')
    if device not in ('cpu', 'cuda'):
        raise RuntimeError('device')
    requirements = root / 'requirements.txt'
    identity = hashlib.sha256(requirements.read_bytes()).hexdigest()
    environment = root / ('venv-' + device)
    python = environment / 'Scripts/python.exe'
    if repair or not python.is_file():
        emit(stage='Preparazione dell’ambiente Qwen3-TTS separato…')
        # Repair only this private environment. Recreate it before installing
        # CUDA; force-reinstalling dependencies from PyPI could replace CUDA
        # PyTorch with the CPU build on the second pip invocation.
        if environment.resolve().parent != root.resolve():
            raise RuntimeError('device')
        run([sys.executable, '-I', '-m', 'venv'] + (['--clear'] if repair else []) + [str(environment)])
    emit(stage='Download e installazione PyTorch ' + ('CUDA per GPU NVIDIA…' if device == 'cuda' else 'per CPU…'))
    pip = [str(python), '-I', '-m', 'pip', '--isolated', 'install', '--disable-pip-version-check',
           '--only-binary=:all:', '--timeout', '30', '--retries', '2']
    run(pip + ['torch==2.10.0', 'torchaudio==2.10.0', '--index-url',
               'https://download.pytorch.org/whl/' + ('cu128' if device == 'cuda' else 'cpu')])
    emit(stage='Installazione dei componenti voce Qwen3-TTS…')
    # sox is a small pure-Python source package, so allow its official sdist here.
    common = [item for item in pip if item != '--only-binary=:all:']
    run(common + ['-r', str(requirements), '--index-url', 'https://pypi.org/simple'])
    emit(stage='Verifica del motore voce e del dispositivo scelto…')
    probe = """import json, torch, qwen_tts, soundfile, imageio_ffmpeg
device = %r
if device == 'cuda':
    if not torch.cuda.is_available(): raise RuntimeError('cuda_unavailable')
    if torch.cuda.get_device_properties(0).total_memory < 7.5 * 1024**3: raise RuntimeError('vram')
print(json.dumps({'device':device,'torch':torch.__version__}))
""" % device
    result = subprocess.run([str(python), '-I', '-c', probe], capture_output=True,
                            creationflags=0x08000000 if os.name == 'nt' else 0)
    if result.returncode:
        error = result.stderr.decode('utf-8', errors='replace')
        raise RuntimeError('vram' if 'vram' in error else 'cuda_unavailable' if 'cuda_unavailable' in error else 'probe')
    marker = root / ('runtime-' + device + '.json')
    partial = marker.with_suffix('.part')
    partial.write_text(json.dumps({'version': 1, 'requirements': identity, 'device': device}), encoding='utf-8')
    partial.replace(marker)
    emit(ready=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--root', type=Path, required=True)
    parser.add_argument('--device', choices=['cpu', 'cuda'], required=True)
    parser.add_argument('--repair', action='store_true')
    arguments = parser.parse_args()
    try:
        if shutil.disk_usage(arguments.root).free < 15 * 1024**3:
            raise RuntimeError('disk')
        prepare(arguments.root.resolve(), arguments.device, arguments.repair)
    except Exception as error:
        reason = str(error) if isinstance(error, RuntimeError) else type(error).__name__
        emit(error=reason if reason in ('platform', 'device', 'disk', 'dependencies', 'vram', 'cuda_unavailable', 'probe') else 'setup')
        raise SystemExit(1)
