"""Developer provisioning on macOS. Ship the resulting runtime to users."""
from pathlib import Path
import argparse
import hashlib
import json
import platform
import shutil
import subprocess
import tempfile
import urllib.request
import zipfile

root = Path(__file__).resolve().parents[1]
whisper_commit = '2eeeba56e9edd762b4b38467bab96c2517163158'  # v1.8.3
source_sha256 = '6a27142a484c66655fa619d211487cfa469e8344a8f159eaa1ae74cdc17d7eaa'
parser = argparse.ArgumentParser()
parser.add_argument('--engine', type=Path, default=root / '.tools/engine')
args = parser.parse_args()
if platform.system() != 'Darwin': raise SystemExit('Questo build richiede macOS e i Command Line Tools di Xcode.')
if not shutil.which('cmake'): raise SystemExit('Installa CMake nell’ambiente di sviluppo per compilare il runtime Metal.')
target = args.engine / 'whisper-cpp'
manifest = target / 'runtime.json'
if manifest.is_file() and json.loads(manifest.read_text()).get('source_sha256') == source_sha256:
    raise SystemExit(0)
with tempfile.TemporaryDirectory(prefix='whisper-metal-', dir=root / '.tools') as folder:
    directory = Path(folder)
    source = directory / 'source.zip'
    urllib.request.urlretrieve(f'https://codeload.github.com/ggml-org/whisper.cpp/zip/{whisper_commit}', source)
    with source.open('rb') as stream:
        if hashlib.file_digest(stream, 'sha256').hexdigest() != source_sha256:
            raise RuntimeError('Checksum dei sorgenti whisper.cpp non valido.')
    with zipfile.ZipFile(source) as archive:
        if any(Path(n).is_absolute() or '..' in Path(n).parts for n in archive.namelist()):
            raise RuntimeError('Percorso archivio non sicuro')
        archive.extractall(directory)
    build = directory / 'build'
    source_tree = directory / f'whisper.cpp-{whisper_commit}'
    subprocess.run(['cmake','-S',str(source_tree),'-B',str(build),'-DCMAKE_BUILD_TYPE=Release',
        '-DGGML_METAL=ON','-DGGML_METAL_EMBED_LIBRARY=ON','-DWHISPER_BUILD_TESTS=OFF',
        '-DCMAKE_BUILD_WITH_INSTALL_RPATH=ON','-DCMAKE_INSTALL_RPATH=@loader_path'],check=True)
    subprocess.run(['cmake','--build',str(build),'--target','whisper-cli','--parallel','4'],check=True)
    target.mkdir(parents=True,exist_ok=True)
    shutil.copy2(build / 'bin/whisper-cli',target / 'whisper-cli')
    for library in build.rglob('*.dylib'): shutil.copy2(library,target / library.name)
    shutil.copy2(source_tree / 'LICENSE',target / 'LICENSE-whisper.txt')
    manifest.write_text(json.dumps({'backend':'metal','version':'1.8.3','platform':'Darwin','architecture':platform.machine(),
                                    'source_sha256':source_sha256}))
