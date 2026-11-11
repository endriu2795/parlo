"""Exercise the generated NSIS installer using a unique, isolated QA identity.

The executable/resources and installer logic match the release. Only the QA
identity, fixture data paths, output paths, compression and version marker are
changed. No production application, registration or data folder is touched.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time
import uuid
import winreg

project = Path(__file__).resolve().parents[1]
root = project / '.tools/qa' / ('distribution-' + uuid.uuid4().hex[:10])
root.mkdir(parents=True)
assert root.resolve().is_relative_to((project / '.tools/qa').resolve())
installation = root / 'installed app'
profile = root / 'profile data'
roaming = root / 'roaming data'
engine = profile / 'engine'
product = 'PARLO QA ' + root.name
binary = 'parlo-qa-' + root.name
registry = 'Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\' + product
compiler = Path(os.environ['LOCALAPPDATA']) / 'tauri/NSIS/makensis.exe'
source = project / 'src-tauri/target/release/nsis/x64'
template = (source / 'installer.nsi').read_text(encoding='utf-8-sig')
events = []

def log(name, **data):
    entry = dict(test=name, **data)
    events.append(entry)
    (root / 'report.json').write_text(json.dumps(events, indent=2), encoding='utf-8')
    print(json.dumps(entry), flush=True)

def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

def run(command, timeout=300, **options):
    flags = subprocess.CREATE_NO_WINDOW
    result = subprocess.run(command, creationflags=flags, capture_output=True, text=True,
                            encoding='utf-8', errors='replace', timeout=timeout, **options)
    if result.returncode:
        raise RuntimeError(f'{Path(command[0]).name}: exit {result.returncode}\n{result.stdout[-2500:]}\n{result.stderr[-2500:]}')
    return result

def build(version, marker, purge=False):
    folder = root / ('package-' + marker)
    folder.mkdir()
    for filename in ['utils.nsh', 'FileAssociation.nsh']:
        shutil.copyfile(source / filename, folder / filename)
    values = {'MANUFACTURER':'PARLO QA', 'PRODUCTNAME':product, 'BUNDLEID':'local.parlo.qa.' + root.name,
              'MAINBINARYNAME':binary, 'VERSION':version, 'VERSIONWITHBUILD':version+'.0',
              'OUTFILE':str(folder / 'setup.exe'), 'ALLOWDOWNGRADES':'false'}
    text = template
    for key, value in values.items():
        text, count = re.subn(r'^!define ' + key + r' ".*"$', lambda _: f'!define {key} "{value}"', text, flags=re.M)
        assert count == 1, key
    # Compression does not affect install/upgrade/uninstall behavior; disabled
    # to avoid repeatedly recompressing the same 190 MiB prepared runtime.
    text = text.replace('SetCompressor /SOLID "lzma"', 'SetCompress off')
    text = text.replace('  File "${MAINBINARYSRCPATH}"', '  File "/oname=${MAINBINARYNAME}.exe" "${MAINBINARYSRCPATH}"')
    text = text.replace('"$APPDATA\\${BUNDLEID}"', '"' + str(roaming) + '"')
    text = text.replace('"$LOCALAPPDATA\\${BUNDLEID}"', '"' + str(profile) + '"')
    (folder / 'qa-version.txt').write_text(marker)
    text = text.replace('  ; Copy resources', f'  File "{folder / "qa-version.txt"}"\n  ; Copy resources')
    text = text.replace('  ; Delete resources', '  Delete "$INSTDIR\\qa-version.txt"\n  ; Delete resources')
    if purge:
        text = text.replace('Section Uninstall', 'Section Uninstall\n  StrCpy $DeleteAppDataCheckboxState 1')
    (folder / 'installer.nsi').write_text(text, encoding='utf-8')
    run([str(compiler), '/V2', str(folder / 'installer.nsi')], cwd=folder)
    log('package_compiled', version=version, marker=marker, purge_fixture=purge)
    return folder / 'setup.exe'

def install(package, update=False):
    # NSIS requires /D to be the final, unquoted command-line option.
    command = f'"{package}" /S /NS ' + ('/UPDATE ' if update else '') + '/D=' + str(installation)
    run(command, timeout=240, cwd=root)

def version():
    with winreg.OpenKey(winreg.HKEY_CURRENT_USER, registry) as key:
        return winreg.QueryValueEx(key, 'DisplayVersion')[0]

def check_fixtures(before):
    for filename, checksum in before.items():
        assert Path(filename).is_file() and digest(Path(filename)) == checksum, filename

def uninstall(update=False):
    command = f'"{installation / "uninstall.exe"}" /S ' + ('/UPDATE ' if update else '') + '_?=' + str(installation)
    run(command, cwd=root)
    assert not (installation / (binary + '.exe')).exists()

try:
    first = build('0.1.0', 'first')
    same = build('0.1.0', 'reinstalled')
    newer = build('0.1.1', 'upgrade')
    purge = build('0.1.1', 'purge-fixture', purge=True)
    assert not installation.exists()
    install(first)
    assert version() == '0.1.0'
    assert digest(installation / (binary + '.exe')) == digest(project / 'src-tauri/target/release/callnotes.exe')
    for asset in ['engine-runtime.tar.gz','manifest.json']:
        assert digest(installation / 'engine/runtime/prepared/win-x64' / asset) == digest(project / 'engine/runtime/prepared/win-x64' / asset)
    log('fresh_install', passed=True)
    profile.mkdir(exist_ok=True)
    roaming.mkdir(exist_ok=True)
    # Synthetic archive, preferences, model and credentials sentinel (no account).
    fixtures = {profile / 'archive.sqlite':b'QA archive sentinel', profile / 'settings.json':b'{"theme":"light"}',
                engine / 'models/fixture.bin':b'QA model sentinel', roaming / 'qa-account-sentinel':b'NOT A REAL CREDENTIAL'}
    for path, contents in fixtures.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(contents)
    before = {str(path):digest(path) for path in fixtures}
    install(same)
    check_fixtures(before)
    assert (installation / 'qa-version.txt').read_text() == 'reinstalled'
    log('same_version_reinstall_preserves_data', passed=True)
    install(newer, update=True)
    assert version() == '0.1.1'
    assert (installation / 'qa-version.txt').read_text() == 'upgrade'
    check_fixtures(before)
    log('upgrade_preserves_archive_settings_models', passed=True)
    minimal = dict(os.environ, PATH=r'C:\Windows\System32;C:\Windows', CALLNOTES_ENGINE_DIR=str(engine),
                   PARLO_ENGINE_BUNDLE=str(installation / 'engine/runtime/prepared/win-x64'), HF_HUB_OFFLINE='1')
    minimal.pop('PYTHONHOME', None)
    minimal.pop('PYTHONPATH', None)
    run([str(installation / (binary+'.exe')), '--prepare-engine-runtime'], timeout=360, env=minimal)
    python = engine / 'prepared-python/python/python.exe'
    run([str(python), '-I', '-c', 'import sys,importlib.util,numpy,av,psutil,faster_whisper,ctranslate2,onnxruntime,openvino_genai; assert sys.prefix == sys.base_prefix; assert importlib.util.find_spec("pip") is None'], env=minimal)
    check_fixtures(before)
    log('installed_runtime_without_system_python_or_pip', passed=True)
    # The selected model is populated from a PUBLIC pretrained model already
    # present in the developer engine. No call data or profile is copied.
    model = engine / 'models/whisper-tiny'
    shutil.copytree(project / '.tools/engine/models/whisper-tiny', model, copy_function=os.link,
                    ignore=shutil.ignore_patterns('.cache'))
    script = 'from faster_whisper import WhisperModel; model=WhisperModel(__import__("sys").argv[1],device="cpu",compute_type="int8",cpu_threads=2,local_files_only=True); segments,_=model.transcribe(__import__("sys").argv[2],language="en"); text=" ".join(s.text for s in segments); assert "country" in text.lower()'
    run([str(python), '-I', '-c', script, str(model), str(project / '.tools/engine/benchmark.wav')], env=minimal)
    log('installed_runtime_offline_public_inference', passed=True)
    # Retrying an existing included runtime must not reinstall it.
    marker = engine / 'prepared-python/.presentio-prepared'
    stamp = marker.stat().st_mtime_ns
    run([str(installation / (binary+'.exe')), '--prepare-engine-runtime'], env=minimal)
    assert marker.stat().st_mtime_ns == stamp
    log('runtime_reused_offline', passed=True)
    uninstall()
    check_fixtures(before)
    assert python.is_file()
    log('uninstall_keeps_data_by_default', passed=True)
    install(purge)
    uninstall(update=True)
    check_fixtures(before)
    log('uninstall_during_update_keeps_data_even_with_purge_selected', passed=True)
    install(purge)
    uninstall()
    assert not profile.exists() and not roaming.exists()
    log('explicit_purge_only_removes_fixture_data', passed=True)
    try:
        winreg.OpenKey(winreg.HKEY_CURRENT_USER, registry)
        raise AssertionError('QA uninstall registration left behind')
    except FileNotFoundError:
        pass
    log('qa_registration_cleaned', passed=True)
    print('REPORT ' + str(root / 'report.json'), flush=True)
finally:
    # Cleanup only our QA registration/install; never use production names.
    if (installation / 'uninstall.exe').is_file() and (installation / (binary+'.exe')).is_file():
        uninstall()
