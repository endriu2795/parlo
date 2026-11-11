"""Check resident final inference and growing-WAV live drafts on public audio only."""
from pathlib import Path
import json, os, subprocess, sys, time, urllib.request, wave
import av

root=Path(__file__).resolve().parents[1]
directory=root/'.tools'/'qa'/'stream-fixture'
directory.mkdir(parents=True,exist_ok=True)
source=directory/'fixture.wav'
url='https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-segmentation-models/1-two-speakers-en.wav'
urllib.request.urlretrieve(url,source)
pcm=bytearray()
with av.open(str(source)) as container:
    resampler=av.AudioResampler(format='s16',layout='mono',rate=16000)
    for frame in container.decode(audio=0):
        for converted in resampler.resample(frame):pcm.extend(converted.to_ndarray().tobytes())
    for converted in resampler.resample(None):pcm.extend(converted.to_ndarray().tobytes())
engine=root/'.tools'/'engine'
settings={'whisper_model':str(engine/'models'/'whisper-turbo'),'language':'en','diarization':False}
settings['live_whisper_model']=str(engine/'models'/'whisper-small')
def spawn():
    return subprocess.Popen([sys.executable,'-u',str(root/'engine'/'worker.py'),'--daemon'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True,encoding='utf-8',env={**os.environ,'PYTHONIOENCODING':'utf-8'})
def request(worker,live=False,cursors=None):
    started=time.monotonic();worker.stdin.write(json.dumps({'settings':settings,'directory':str(directory),'live':live,'cursors':cursors or {}})+'\n');worker.stdin.flush()
    loads=0
    for line in worker.stdout:
        event=json.loads(line)
        if event['type']=='stage' and 'Caricamento' in event['text']:loads+=1
        if event['type']=='error':raise RuntimeError(event['message'])
        if event['type']=='result':return event,time.monotonic()-started,loads
    raise RuntimeError('Worker exited')
def write(data):
    with wave.open(str(directory/'system.wav'),'wb') as w:w.setnchannels(1);w.setsampwidth(2);w.setframerate(16000);w.writeframes(data)
workers=[]
try:
    live_worker=spawn();workers.append(live_worker)
    write(pcm[:8*32000]);first,t1,l1=request(live_worker,True)
    write(pcm);second,t2,l2=request(live_worker,True,first['cursors'])
    assert l1==1 and l2==0,'Live model was not reused'
    assert second['cursors']['mixed']>first['cursors']['mixed']
    assert first['segments'] or second['segments'],'No live text from spoken fixture'
    assert all(s['speaker']=='Speaker' for s in first['segments']+second['segments'])
    assert min(s['start_ms'] for s in second['segments'])>=int((first['cursors']['mixed']-1)*1000)
    final_worker=spawn();workers.append(final_worker)
    final,t3,l3=request(final_worker);again,t4,l4=request(final_worker)
    assert final['segments']==again['segments'],'Channel labels contaminated the next call'
    assert {s['speaker'] for s in final['segments']}=={'Speaker'}
    report={'source':url,'live_first_seconds':round(t1,2),'live_next_seconds':round(t2,2),'live_segments':len(first['segments'])+len(second['segments']),'final_first_seconds':round(t3,2),'final_next_seconds':round(t4,2),'models_reused':True,'speakers_scoped':True}
    (root/'.tools'/'qa'/'stream-report.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
    print(json.dumps(report,indent=2),flush=True)
finally:
    for worker in workers:
        worker.terminate();worker.wait(timeout=30)
    for file in (source,directory/'system.wav'):
        if file.exists():file.unlink()
    directory.rmdir()
