"""Assemble the relocatable engine during the build. Never runs on customer PCs."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import runpy
import subprocess
import tarfile
import tempfile


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def remove_build_tools(site):
    # "pip*" also matches Piper and its distribution metadata.
    import shutil
    directories = [site / 'pip', site / 'setuptools',
                   *site.glob('pip-*.dist-info'), *site.glob('setuptools-*.dist-info')]
    for directory in directories:
        if directory.is_dir():
            if directory.is_symlink() or directory.resolve().parent != site.resolve():
                raise RuntimeError('Build tools must stay inside the staged site-packages directory.')
            shutil.rmtree(directory)


def patch_pillow_apng(site):
    # CVE-2026-112399. MoviePy 2.2.1 requires Pillow <12, so backport
    # the delayed allocation from Pillow 13 to the locked wheel.
    plugin = site / 'PIL/PngImagePlugin.py'
    source = plugin.read_text(encoding='utf-8')
    early_fill = (
        '        elif self.dispose_op == Disposal.OP_BACKGROUND:\n'
        '            self.dispose = Image.core.fill(self.mode, self.size)\n'
        '            self.dispose = self._crop(self.dispose, self.dispose_extent)\n'
    )
    paste = (
        '            if self.dispose:\n'
        '                self.im.paste(self.dispose, self.dispose_extent)\n'
    )
    delayed_fill = (
        '            if self.dispose_op == Disposal.OP_BACKGROUND:\n'
        '                x0, y0, x1, y1 = self.dispose_extent\n'
        '                dispose = Image.core.fill(self.mode, (x1 - x0, y1 - y0))\n'
        '                self.im.paste(dispose, self.dispose_extent)\n'
        '            elif self.dispose:\n'
        '                self.im.paste(self.dispose, self.dispose_extent)\n'
    )
    if source.count(early_fill) != 1 or source.count(paste) != 1:
        raise RuntimeError('Pillow changed. Review the CVE-2026-112399 backport.')
    plugin.write_text(source.replace(early_fill, '').replace(paste, delayed_fill), encoding='utf-8')


def cached_runtime_matches(manifest_file, packed, identity, modules=()):
    if not manifest_file.is_file() or not packed.is_file():
        return False
    try:
        saved = json.loads(manifest_file.read_text())
        if not all(saved.get(k) == v for k, v in identity.items()) or saved.get('sha256') != digest(packed):
            return False
        if modules:
            site = 'python/Lib/site-packages' if identity['platform'] == 'win-x64' else 'python/lib/python3.12/site-packages'
            required = {f'{site}/{name}/__init__.py' for name in modules}
            with tarfile.open(packed) as archive:
                for entry in archive:
                    if entry.isfile():
                        required.discard(entry.name.removeprefix('./'))
            if required:
                return False
        return True
    except (OSError, ValueError, tarfile.TarError):
        return False


def prepare(root, target_platform, wheelhouse=None, kind='engine'):
    runtime = json.loads((root / 'engine/python-runtime.json').read_text())
    artifact = next(a for a in runtime['artifacts'] if a['platform'] == target_platform)
    archive = root / 'engine/runtime/python' / artifact['file']
    if not archive.is_file() or archive.stat().st_size != artifact['size'] or digest(archive) != artifact['sha256']:
        raise RuntimeError('Prepare the verified private Python archive before packaging.')
    documents = kind == 'documents'
    requirements = root / ('engine/documents-requirements-lock.txt' if documents else 'engine/requirements-lock.txt')
    output = root / ('engine/runtime/documents' if documents else 'engine/runtime/prepared') / target_platform
    output.mkdir(parents=True, exist_ok=True)
    manifest_file = output / 'manifest.json'
    packed = output / f'{kind}-runtime.tar.gz'
    identity = {'version': 2 if documents else 1, 'platform': target_platform, 'python_sha256': artifact['sha256'],
                'requirements_sha256': digest(requirements)}
    document_probe = root / 'engine/document_runtime_probe.py'
    modules = ()
    if documents:
        identity['validation_sha256'] = digest(document_probe)
        modules = runpy.run_path(str(document_probe))['MODULES']
    if cached_runtime_matches(manifest_file, packed, identity, modules):
        print('Prepared engine already verified; no dependency installation needed.')
        return output
    tools = root / '.tools'
    tools.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='prepared-engine-', dir=tools) as temporary:
        staging = Path(temporary)
        with tarfile.open(archive) as source:
            source.extractall(staging, filter='data')
        python = staging / artifact['executable']
        environment = {k: v for k, v in os.environ.items() if k not in ('PYTHONHOME', 'PYTHONPATH')}
        subprocess.run([str(python), '-I', '-m', 'ensurepip'], check=True, env=environment)
        site = staging / ('python/Lib/site-packages' if target_platform == 'win-x64' else 'python/lib/python3.12/site-packages')
        command = [str(python), '-I', '-m', 'pip', 'install', '--disable-pip-version-check',
                   '--only-binary=:all:', '--no-compile', '--target', str(site), '-r', str(requirements)]
        if wheelhouse:
            command += ['--no-index', '--find-links', str(wheelhouse)]
        subprocess.run(command, check=True, env=environment)
        if documents:
            patch_pillow_apng(site)
        probe = (document_probe.read_bytes().decode('utf-8')
                 if documents else 'import numpy, av, psutil, faster_whisper, ctranslate2, onnxruntime; '
                 + ('import openvino, openvino_genai; ' if target_platform == 'win-x64' else '')
                 + 'print("Prepared engine imports verified.")')
        # pip and build tools are unnecessary in the shipping engine.
        remove_build_tools(site)
        # Verify what will actually ship, after removing the build tools.
        subprocess.run([str(python), '-I', '-c', probe], check=True, env=environment)
        partial = output / f'{kind}-runtime.tar.gz.part'
        with tarfile.open(partial, 'w:gz', compresslevel=6) as bundle:
            bundle.add(staging / 'python', arcname='python')
        partial.replace(packed)
    identity.update(file=packed.name, size=packed.stat().st_size, sha256=digest(packed))
    manifest_file.write_text(json.dumps(identity, indent=2) + '\n', encoding='utf-8')
    print(f'Prepared engine: {packed.stat().st_size / 1048576:.1f} MiB. No pip or venv on customer PCs.')
    return output


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--platform', required=True, choices=['win-x64', 'mac-arm64', 'mac-x64'])
    parser.add_argument('--wheelhouse', type=Path)
    parser.add_argument('--kind', choices=['engine', 'documents'], default='engine')
    args = parser.parse_args()
    expected = 'win-x64' if os.name == 'nt' else 'mac-arm64' if platform.machine() == 'arm64' else 'mac-x64'
    if args.platform != expected:
        raise SystemExit('Build the prepared engine on its target platform.')
    prepare(Path(__file__).resolve().parents[1], args.platform, args.wheelhouse, args.kind)
