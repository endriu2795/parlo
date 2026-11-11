"""Measure short-window preview on public audio, never the user's microphone."""
from pathlib import Path
import json, os, subprocess, sys, tempfile, time, urllib.request, wave
import av
root=Path(__file__).resolve().parents[1]
url='https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-segmentation-models/1-two-speakers-en.wav'
with tempfile.TemporaryDirectory(prefix='parlo-live-') as tmp:
    directory=Path(tmp);source=directory/'fixture.wav';urllib.request.urlretrieve(url,source)
    pcm=bytearray()
    with av.open(str(source)) as container:
        resampler=av.AudioResampler(format='s16',layout='mono',rate=16000)
        for frame in container.decode(audio=0):
            for converted in resampler.resample(frame):pcm.extend(converted.to_ndarray().tobytes())
        for converted in resampler.resample(None):pcm.extend(converted.to_ndarray().tobytes())
    settings={'whisper_model':str(root/'.tools/engine/models/whisper-turbo'),'live_whisper_model':str(root/'.tools/engine/models/whisper-tiny'),'language':'en'}
    worker=subprocess.Popen([sys.executable,'-u',str(root/'engine/worker.py'),'--daemon'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True,encoding='utf-8',env={**os.environ,'PYTHONIOENCODING':'utf-8'})
    def request(cursors):
        started=time.monotonic();worker.stdin.write(json.dumps({'settings':settings,'directory':tmp,'live':True,'cursors':cursors})+'\n');worker.stdin.flush();streams=0
        for line in worker.stdout:
            event=json.loads(line)
            if event['type']=='error':raise RuntimeError(event['message'])
            if event['type']=='live_source':streams+=1
            if event['type']=='result':return event,round(time.monotonic()-started,3),streams
        raise RuntimeError('Worker exited')
    try:
        _,warm,_=request({'warmup':True});cursors={};report=[];texts=[]
        for seconds in [3,6,9,12,16]:
            for source_name in ['microphone','system']:
                with wave.open(str(directory/f'{source_name}.wav'),'wb') as w:w.setnchannels(1);w.setsampwidth(2);w.setframerate(16000);w.writeframes(pcm[:seconds*32000])
            event,elapsed,streams=request(cursors)
            assert all(event['cursors'][source]>=cursors.get(source,0) for source in ['system','microphone'])
            cursors=event['cursors'];texts.extend(event['segments'])
            assert streams==2 and event['streamed']
            report.append({'audio_seconds':seconds,'both_sources_inference_seconds':elapsed,'segments':len(event['segments'])})
        assert texts and set(s['source'] for s in texts)=={'system','microphone'}
        assert all(s['speaker']==('Tu' if s['source']=='microphone' else 'Audio call') for s in texts)
        output={'source':url,'model':'tiny multilingual','warmup_seconds':warm,'windows':report}
        (root/'.tools/qa/live-latency-report.json').write_text(json.dumps(output,indent=2),encoding='utf-8');print(json.dumps(output,indent=2),flush=True)
    finally:worker.terminate();worker.wait(timeout=20)
