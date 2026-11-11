"""Long-call benchmark from PUBLIC speech plus synthetic microphone echo."""
import argparse
import json
import os
import sys
import tempfile
import subprocess
import threading
import time
import wave
from pathlib import Path
import numpy as np

root = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(root / 'engine'))
import acceleration
import worker

parser = argparse.ArgumentParser()
parser.add_argument('--minutes', type=float, default=20)
parser.add_argument('--output', type=Path, default=root / '.tools/qa/long-benchmark.json')
args = parser.parse_args()
engine = root / '.tools/engine'
with wave.open(str(engine / 'benchmark.wav')) as wav:
    fixture = np.frombuffer(wav.readframes(wav.getnframes()), dtype='<i2')
# Typical utterance followed by a short pause; no user recording is opened.
cycle = np.concatenate((fixture, np.zeros(16000, dtype='<i2')))
count = round(args.minutes * 60 * 16000)
audio = np.tile(cycle, int(np.ceil(count / len(cycle))))[:count]
settings = {'whisper_model': str(engine / 'models/whisper-turbo'), 'language':'en', 'compute_mode':'auto'}
plans = acceleration.selected_candidates(settings)
print('Benchmark pubblico: ' + str(args.minutes) + ' minuti · ' + plans[0]['label'], flush=True)
started, last_log = time.monotonic(), [0.]
def event(kind, **data):
    if kind == 'stage' and time.monotonic() - last_log[0] >= 30:
        print(data['text'], flush=True)
        last_log[0] = time.monotonic()
worker.emit = event
with tempfile.TemporaryDirectory(prefix='long-benchmark-', dir=root / '.tools/qa') as folder:
    directory = Path(folder)
    for source, pcm in [('system',audio), ('microphone',np.pad((audio.astype(np.float32)*.3).astype('<i2'),(1920,0))[:count])]:
        with wave.open(str(directory / (source + '.wav')), 'wb') as wav:
            wav.setparams((1,2,16000,0,'NONE','not compressed'))
            wav.writeframes(pcm.tobytes())
    del audio, cycle
    # Use the application's persistent-worker protocol. Stop that owned
    # process after its complete result, as the desktop does when its queue
    # empties; do not depend on native GPU objects' interpreter teardown.
    child = subprocess.Popen([sys.executable, '-u', str(root / 'engine/worker.py'), '--daemon'],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
        text=True, encoding='utf-8', errors='replace',
        creationflags=0x08000000 if os.name == 'nt' else 0,
        env={**os.environ, 'CALLNOTES_ENGINE_DIR': str(engine), 'PYTHONIOENCODING': 'utf-8'})
    watchdog = threading.Timer(max(600, count / 16000 * 3), child.kill)
    watchdog.start()
    try:
        child.stdin.write(json.dumps({'settings':settings,'directory':str(directory),'live':False,'cursors':{}}) + '\n')
        child.stdin.flush()
        segments = None
        for line in child.stdout:
            value = json.loads(line)
            if value['type'] == 'stage': event('stage', text=value['text'])
            elif value['type'] == 'error': raise RuntimeError(value.get('message','Errore del benchmark'))
            elif value['type'] == 'result': segments=value['segments'];break
        assert segments is not None, 'Motore interrotto prima del risultato'
    finally:
        watchdog.cancel()
        if child.poll() is None: child.kill()
        child.wait(timeout=10)
        child.stdin.close();child.stdout.close()
    elapsed = time.monotonic() - started
    assert segments and segments[-1]['end_ms'] > count / 16 - 15000, 'Testo finale incompleto'
    assert {s['speaker'] for s in segments} == {'Speaker'}, 'Eco attribuito al microfono'
    assert all(0 <= s['start_ms'] <= s['end_ms'] <= count / 16 + 100 for s in segments), 'Timestamp non validi'
report = {'public_fixture':'whisper.cpp samples/jfk.wav v1.8.3','synthetic_repeated_speech':True,
          'synthetic_echo':True,'audio_seconds':count/16000,'processing_seconds':round(elapsed,2),
          'realtime_factor':round(elapsed / (count/16000),3),'plan':plans[0],
          'segments':len(segments),'words':sum(len(s['text'].split()) for s in segments),
          'last_timestamp_ms':segments[-1]['end_ms']}
args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf-8')
print(json.dumps(report, ensure_ascii=False, indent=2), flush=True)
