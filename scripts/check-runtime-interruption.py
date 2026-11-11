"""Force-stop the native runtime preparation, retry and reject corrupt bundles."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time
import uuid

project=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser()
parser.add_argument('--binary',type=Path,default=project/'src-tauri/target/release/callnotes.exe')
args=parser.parse_args()
root=project/'.tools/qa'/('runtime interrupted '+uuid.uuid4().hex[:8])
root.mkdir(parents=True)
assert root.resolve().is_relative_to((project/'.tools/qa').resolve())
bundle=project/'engine/runtime/prepared/win-x64'
engine=root/'engine'
old=engine/'prepared-python'
old.mkdir(parents=True)
(old/'previous-engine-sentinel').write_bytes(b'previous engine')
sentinels={engine/'models/keep.bin':b'public model sentinel',root/'archive.sqlite':b'archive sentinel'}
for filename,data in sentinels.items(): filename.parent.mkdir(parents=True,exist_ok=True);filename.write_bytes(data)
env=dict(os.environ,CALLNOTES_ENGINE_DIR=str(engine),PARLO_ENGINE_BUNDLE=str(bundle),PATH=r'C:\Windows\System32;C:\Windows')
env.pop('PYTHONHOME',None);env.pop('PYTHONPATH',None)
command=[str(args.binary.resolve()),'--prepare-engine-runtime']
flags=subprocess.CREATE_NO_WINDOW
events=[]
def log(name,**data):
    events.append(dict(test=name,**data))
    (root/'report.json').write_text(json.dumps(events,indent=2),encoding='utf-8')
    print(json.dumps(events[-1]),flush=True)
def stages():
    return [p for p in engine.glob('.prepared-*') if p.is_dir() and not p.name.startswith('.prepared-previous-')]
child=subprocess.Popen(command,env=env,creationflags=flags,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
try:
    limit=time.monotonic()+60
    while not stages() and child.poll() is None and time.monotonic()<limit: time.sleep(.01)
    assert stages(),'Native preparation never reached its staging directory'
    interrupted=stages()[0]
    concurrent=subprocess.run(command,env=env,creationflags=flags,capture_output=True,text=True,encoding='utf-8',errors='replace',timeout=30)
    assert concurrent.returncode and child.poll() is None,'Two preparations should not proceed together'
    assert interrupted.exists(),'Concurrent setup deleted an active preparation'
    log('parallel_preparation_blocked',passed=True)
finally:
    if child.poll() is None:child.kill()
    child.wait(timeout=10)
assert interrupted.exists()
assert (old/'previous-engine-sentinel').read_bytes()==b'previous engine'
log('forced_stop_preserves_previous_runtime_and_data',passed=True)
retry=subprocess.run(command,env=env,creationflags=flags,capture_output=True,text=True,encoding='utf-8',errors='replace',timeout=360)
assert retry.returncode==0,retry.stderr
assert (engine/'prepared-python/python/python.exe').is_file()
assert not stages(),'Interrupted staging directory survived a successful retry'
assert any((p/'previous-engine-sentinel').is_file() for p in engine.glob('.prepared-previous-*'))
for filename,data in sentinels.items():assert filename.read_bytes()==data
log('retry_cleans_incomplete_staging_and_preserves_backup',passed=True)
other=root/'corrupt-case'
(other/'prepared-python').mkdir(parents=True)
(other/'prepared-python/previous-engine-sentinel').write_bytes(b'old runtime remains')
corrupt=root/'corrupt-bundle';corrupt.mkdir()
manifest=json.loads((bundle/'manifest.json').read_text())
manifest['size']=1
(corrupt/'manifest.json').write_text(json.dumps(manifest),encoding='utf-8')
(corrupt/'engine-runtime.tar.gz').write_bytes(b'x')
bad_env=dict(env,CALLNOTES_ENGINE_DIR=str(other),PARLO_ENGINE_BUNDLE=str(corrupt))
rejected=subprocess.run(command,env=bad_env,creationflags=flags,capture_output=True,text=True,encoding='utf-8',errors='replace',timeout=30)
assert rejected.returncode!=0
assert (other/'prepared-python/previous-engine-sentinel').read_bytes()==b'old runtime remains'
log('corrupt_bundle_rejected_without_replacing_previous_engine',passed=True)
print('REPORT '+str(root/'report.json'),flush=True)
