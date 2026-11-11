"""Provision only hardware-compatible accelerators; inference stays offline."""
from pathlib import Path
import argparse
import json
import os
import platform
import shutil
import subprocess
import sys
import urllib.request
import hashlib
import zipfile

root = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(root / 'engine'))
import acceleration
from acceleration import ui_text

def download(url, target, size=None, sha256=None):
    def valid(path):
        if not path.is_file(): return False
        if size is not None and path.stat().st_size != size: return False
        if sha256:
            with path.open('rb') as stream:
                return hashlib.file_digest(stream, 'sha256').hexdigest() == sha256
        return True
    if valid(target): return
    target.parent.mkdir(parents=True, exist_ok=True)
    temporary = target.with_suffix(target.suffix + '.part')
    if (size is not None or sha256) and valid(temporary):
        temporary.replace(target)
        return
    offset = temporary.stat().st_size if temporary.is_file() else 0
    request = urllib.request.Request(url, headers={'Range': f'bytes={offset}-'} if offset else {})
    with urllib.request.urlopen(request, timeout=120) as response:
        resumed = offset > 0 and response.status == 206 and response.headers.get('Content-Range', '').startswith(f'bytes {offset}-')
        count = offset if resumed else 0
        total = size or int(response.headers.get('Content-Length', '0')) + count
        last = -1
        with temporary.open('ab' if resumed else 'wb') as output:
            while block := response.read(1024 * 1024):
                count += len(block)
                if size and count > size: raise RuntimeError('Download più grande del previsto.')
                output.write(block)
                percent = count * 100 // total if total else 0
                if percent != last:
                    print(ui_text(f'PARLO_STAGE:Download {target.name}… {percent}%', f'PARLO_STAGE:Downloading {target.name}… {percent}%'), flush=True)
                    last = percent
        if total and count != total: raise RuntimeError('Download incompleto: riprova per riprenderlo.')
    if not valid(temporary):
        temporary.unlink(missing_ok=True)
        raise RuntimeError('Verifica del download non riuscita. Riprova.')
    temporary.replace(target)

CUDA_PACKAGES = [
    ('cublas', 'nvidia_cublas_cu12-12.4.5.8-py3-none-win_amd64.whl',
     'https://files.pythonhosted.org/packages/e2/2a/4f27ca96232e8b5269074a72e03b4e0d43aa68c9b965058b1684d07c6ff8/nvidia_cublas_cu12-12.4.5.8-py3-none-win_amd64.whl',
     396895858, '5a796786da89203a0657eda402bcdcec6180254a8ac22d72213abc42069522dc'),
    ('cudnn', 'nvidia_cudnn_cu12-9.1.0.70-py3-none-win_amd64.whl',
     'https://files.pythonhosted.org/packages/3f/d0/f90ee6956a628f9f04bf467932c0a25e5a7e706a684b896593c06c82f460/nvidia_cudnn_cu12-9.1.0.70-py3-none-win_amd64.whl',
     679925892, '6278562929433d68365a07a4a1546c237ba2849852c0d4b2262a486e805b977a'),
]


def setup_cuda(engine):
    # Verified native DLLs in app data; never pip, global PATH or system drivers.
    for package, filename, url, size, checksum in CUDA_PACKAGES:
        destination = engine / 'cuda' / package
        marker = destination / '.presentio-cuda'
        if marker.is_file() and marker.read_text() == checksum and list(destination.rglob('*.dll')): continue
        wheel = engine / 'downloads' / filename
        print(ui_text('PARLO_STAGE:Preparazione accelerazione NVIDIA…', 'PARLO_STAGE:Preparing NVIDIA acceleration…'), flush=True)
        download(url, wheel, size, checksum)
        stage = engine / 'cuda' / f'.{package}-staging'
        stage.mkdir(parents=True, exist_ok=True)
        with zipfile.ZipFile(wheel) as archive:
            for info in archive.infolist():
                path = Path(info.filename)
                if path.is_absolute() or '..' in path.parts or '\\' in info.filename or ':' in info.filename:
                    raise RuntimeError('Percorso non valido nel pacchetto NVIDIA.')
                if not info.is_dir() and (info.filename.startswith(f'nvidia/{package}/') or 'LICENSE' in path.name.upper()):
                    target = stage / path
                    target.parent.mkdir(parents=True, exist_ok=True)
                    with archive.open(info) as source, target.open('wb') as output:
                        shutil.copyfileobj(source, output)
        if not list(stage.rglob('*.dll')): raise RuntimeError('Pacchetto NVIDIA incompleto.')
        (stage / '.presentio-cuda').write_text(checksum)
        if destination.exists(): destination.rename(engine / 'cuda' / f'.{package}-previous-{os.getpid()}')
        stage.rename(destination)


def preferred_backend(hw, runtime, compute_mode):
    if compute_mode == 'cpu': return 'faster'
    if any(g['backend'] == 'cuda' for g in hw['gpus']): return 'cuda'
    if any(g['backend'] == 'openvino' for g in hw['gpus']): return 'openvino'
    if runtime.is_file() and any(g['backend'] == 'cpp' for g in hw['gpus']): return 'cpp'
    return 'faster'


def setup(engine, mid, compute_mode='auto', preview=False):
    print(ui_text('PARLO_STAGE:Rilevamento delle risorse di questo computer…', 'PARLO_STAGE:Detecting hardware…'), flush=True)
    hw = acceleration.hardware(refresh=True)
    print('Hardware: ' + json.dumps(hw, ensure_ascii=False), flush=True)
    # A developer builds this once. Users receive only the portable runtime.
    bundle_name = 'mac-arm64-metal' if platform.machine() in ('arm64','aarch64') else 'mac-x64-metal'
    bundle = Path(os.environ.get('PARLO_RUNTIME_BUNDLE', str(root / 'engine/runtime' / ('win-x64-vulkan' if platform.system() == 'Windows' else bundle_name))))
    if bundle.is_dir():
        shutil.copytree(bundle, engine / 'whisper-cpp', dirs_exist_ok=True)
    if platform.system() == 'Darwin' and any(g['backend']=='cpp' for g in hw['gpus']) and not (engine / 'whisper-cpp/runtime.json').is_file() and os.environ.get('PARLO_DEVELOPER_BUILD') == '1':
        subprocess.run([sys.executable, str(root / 'scripts/build-whisper-metal.py'), '--engine', str(engine)], check=True)
    if platform.system() == 'Darwin' and not (engine / 'whisper-cpp/runtime.json').is_file():
        print('Runtime Metal non incluso: uso CPU. Installa una build Mac completa per accelerazione GPU.', flush=True)
    runtime = engine / 'whisper-cpp/runtime.json'
    backend = preferred_backend(hw, runtime, compute_mode)
    if preview and backend == 'faster':
        raise RuntimeError('GPU non disponibile. Puoi scegliere CPU esplicitamente nelle impostazioni dell’anteprima.')
    from model_download import ensure_faster_model
    from huggingface_hub import hf_hub_download, snapshot_download
    try:
      if backend == 'cpp':
        cpp_name = {'turbo':'large-v3-turbo','small':'small','tiny':'tiny'}[mid]
        name = f'ggml-{cpp_name}-q8_0.bin'
        target = engine / 'models' / f'ggml-{mid}.bin'
        if not target.is_file():
            print(ui_text(f'PARLO_STAGE:Download Whisper {mid} per la GPU…', f'PARLO_STAGE:Downloading Whisper {mid} for GPU…'), flush=True)
            cached = hf_hub_download('ggerganov/whisper.cpp', name, cache_dir=str(engine / 'downloads/huggingface'))
            partial = target.with_suffix('.bin.part')
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(cached, partial)
            partial.replace(target)
        if os.name == 'nt' and not preview:
            # The generic CPU library in the Windows Vulkan package exceeded
            # the public-fixture timeout. Keep the chosen model's verified INT8
            # CPU adapter available instead of relying on that library.
            print(ui_text('PARLO_STAGE:Preparazione del modello per il ripiego CPU…', 'PARLO_STAGE:Preparing the CPU fallback model…'), flush=True)
            ensure_faster_model(engine, mid)
      elif backend == 'openvino':
        name = {'turbo': 'large-v3-turbo', 'small': 'small', 'tiny': 'tiny'}[mid]
        repository = f'OpenVINO/whisper-{name}-int8-ov'
        target = engine / 'models' / f'whisper-{mid}-openvino'
        complete = (target / 'presentio-model.json').is_file() and all((target / name).is_file() for name in ['openvino_encoder_model.xml', 'openvino_encoder_model.bin', 'openvino_decoder_model.xml', 'openvino_decoder_model.bin'])
        if not complete:
            print(ui_text(f'PARLO_STAGE:Download Whisper {mid} ottimizzato per Intel…', f'PARLO_STAGE:Downloading Whisper {mid} optimized for Intel…'), flush=True)
            metadata = json.load(urllib.request.urlopen('https://huggingface.co/api/models/' + repository, timeout=30))
            revision = metadata['sha']
            snapshot_download(repository, revision=revision, local_dir=str(target),
                              allow_patterns=['*.xml', '*.bin', '*.json', '*.txt', '*.model'])
            (target / 'presentio-model.json').write_text(json.dumps({'model': mid, 'precision': 'int8',
                'revision': revision, 'repository': repository}), encoding='utf-8')
      else:
        ensure_faster_model(engine, mid)
        if backend == 'cuda' and os.name == 'nt': setup_cuda(engine)
    except Exception as error:
        if preview: raise
        print(f'Accelerazione non disponibile: {error}. Preparazione CPU dello stesso modello.', flush=True)
        ensure_faster_model(engine, mid)
    if preview: return  # Preview preparation never starts a full-call benchmark.
    # Benchmark only resources that are actually installed, never download extra
    # model formats just to compare them. Native formats also support CPU.
    try:
        settings = {'whisper_model': str(engine / 'models' / f'whisper-{mid}')}
        if acceleration.profile_for(settings): return
        download('https://raw.githubusercontent.com/ggml-org/whisper.cpp/v1.8.3/samples/jfk.wav', engine / 'benchmark.wav')
        print(ui_text('PARLO_STAGE:Ottimizzazione per questo computer…', 'PARLO_STAGE:Optimizing for this computer…'), flush=True)
        result = subprocess.run([sys.executable, str(root / 'engine' / 'calibrate.py'), '--engine', str(engine), '--model', mid], check=False)
        if result.returncode:
            ensure_faster_model(engine, mid)
    except (OSError, RuntimeError) as error:
        print(f'Ottimizzazione rinviata: {error}. Il modello rimane disponibile.', flush=True)

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--engine', type=Path, default=root / '.tools' / 'engine')
    parser.add_argument('--model', choices=['turbo', 'tiny', 'small'], default='turbo')
    args = parser.parse_args()
    setup(args.engine.resolve(), args.model)
