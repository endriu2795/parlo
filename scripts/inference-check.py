"""End-to-end offline inference check using an upstream public audio fixture."""
from pathlib import Path
import collections
import json
import os
import subprocess
import sys
import urllib.request
import wave
import time
import numpy as np
import av

root=Path(__file__).resolve().parents[1]
directory=root/".tools"/"qa"/"audio"
directory.mkdir(parents=True,exist_ok=True)
source=directory/"fixture.wav"
url="https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-segmentation-models/1-two-speakers-en.wav"
urllib.request.urlretrieve(url,source)
target=directory/"system.wav"
with av.open(str(source)) as container, wave.open(str(target),"wb") as output:
    output.setnchannels(1);output.setsampwidth(2);output.setframerate(16000)
    resampler=av.AudioResampler(format="s16",layout="mono",rate=16000)
    for frame in container.decode(audio=0):
        for converted in resampler.resample(frame):
            output.writeframes(converted.to_ndarray().tobytes())
    for converted in resampler.resample(None):
        output.writeframes(converted.to_ndarray().tobytes())
with wave.open(str(target),"rb") as f:
    duration=f.getnframes()/f.getframerate()
engine=root/".tools"/"engine"
settings={"whisper_model":str(engine/"models"/"whisper-turbo"),"language":"en","diarization":False}
# Synthetic echo of the public fixture, not audio from the user's microphone.
with wave.open(str(target),"rb") as f:
    pcm=np.frombuffer(f.readframes(f.getnframes()),dtype="<i2").astype(np.float32)
echo=np.pad(pcm*.3,(1920,0))[:len(pcm)].astype("<i2")
microphone=directory/"microphone.wav"
with wave.open(str(microphone),"wb") as output:
    output.setnchannels(1);output.setsampwidth(2);output.setframerate(16000);output.writeframes(echo.tobytes())
print(f"Prova offline su campione pubblico: {duration:.1f}s",flush=True)
started=time.monotonic()
result=subprocess.run([sys.executable,"-u",str(root/"engine"/"worker.py"),"--directory",str(directory)],input=json.dumps(settings),capture_output=True,text=True,encoding="utf-8",errors="replace",env={**os.environ,"PYTHONIOENCODING":"utf-8"},timeout=600)
events=[json.loads(line) for line in result.stdout.splitlines() if line.startswith('{')]
if result.returncode:
    print(result.stdout);print(result.stderr);sys.exit(result.returncode)
segments=next(e["segments"] for e in events if e["type"]=="result")
assert segments,"Nessun testo nel campione parlato"
assert all(0<=s["start_ms"]<=s["end_ms"]<=round(duration*1000)+250 for s in segments)
speakers=collections.Counter(s["speaker"] for s in segments)
assert set(speakers)=={"Speaker"}, speakers
summary={"source":url,"duration_seconds":duration,"processing_seconds":round(time.monotonic()-started,2),"segments":len(segments),"speakers":dict(speakers),"first_segment":segments[0],"offline":True,"single_inference":True,"synthetic_echo":True}
(root/".tools"/"qa"/"inference.json").write_text(json.dumps(summary,ensure_ascii=False,indent=2),encoding="utf-8")
print(json.dumps(summary,ensure_ascii=False,indent=2))
# Keep the report, remove fixture audio after the successful verification.
source.unlink();target.unlink();microphone.unlink();directory.rmdir()
