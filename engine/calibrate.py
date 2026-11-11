"""Compare local engines on bundled PUBLIC speech, in isolated processes."""
from pathlib import Path
import argparse
import datetime
import json
import os
import subprocess
import sys
import time
import wave
import acceleration

def probe(root, mid, plan):
    import numpy as np
    import worker
    with wave.open(str(root / 'benchmark.wav')) as wav:
        if (wav.getnchannels(), wav.getsampwidth(), wav.getframerate()) != (1, 2, 16000):
            raise ValueError('Campione di calibrazione non valido')
        samples = np.frombuffer(wav.readframes(wav.getnframes()), dtype='<i2').astype(np.float32) / 32768
    settings = {'whisper_model': str(root / 'models' / f'whisper-{mid}'), 'language': 'en'}
    started = time.monotonic()
    with worker.inference_power():
        model = acceleration.load(settings, plan)
        loaded = time.monotonic() - started
        measurements = []
        for _ in range(2):
            started = time.monotonic()
            segments, _ = worker.final_segments(model, samples, 'mixed', settings)
            segments = list(segments)
            text = ' '.join(s.text for s in segments).lower()
            if not all(word in text for word in ('americans', 'country', 'ask')):
                raise RuntimeError('Test del parlato pubblico non superato')
            if not any(s.words for s in segments):
                raise RuntimeError('Timestamp delle parole non disponibili')
            measurements.append(time.monotonic() - started)
    return {'plan': plan, 'key': acceleration.candidate_key(plan), 'label': plan['label'],
            'load_seconds': round(loaded, 2), 'inference_seconds': round(sum(measurements) / len(measurements), 2),
            'realtime_factor': round(sum(measurements) / len(measurements) / (len(samples) / 16000), 3),
            'sample_seconds': len(samples) / 16000}

def calibrate(root, mid):
    settings = {'whisper_model': str(root / 'models' / f'whisper-{mid}'), 'compute_mode': 'auto'}
    hw = acceleration.hardware(refresh=True)
    plans = acceleration.candidates(settings)
    expanded = []
    for plan in plans:
        if plan['device'].lower() == 'cpu':
            for count in sorted({min(4, hw['threads']), min(8, hw['threads']), hw['threads']}):
                expanded.append({**plan, 'threads': count, 'label': plan['label'] + f' · {count} thread'})
        else: expanded.append(plan)
    results, failures = [], []
    for plan in expanded:
        print('Calibrazione: ' + plan['label'], flush=True)
        try:
            child = subprocess.run([sys.executable, str(Path(__file__).resolve()), '--engine', str(root), '--model', mid,
                '--probe', json.dumps(plan)], capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=240,
                creationflags=0x08000000 if os.name == 'nt' else 0,
                env={**os.environ, 'CALLNOTES_ENGINE_DIR': str(root), 'PYTHONIOENCODING': 'utf-8'})
            if child.returncode:
                raise RuntimeError(child.stderr[-1500:])
            result = json.loads(child.stdout.splitlines()[-1])
            results.append(result)
            print(f"  {result['inference_seconds']} s per {result['sample_seconds']} s di audio", flush=True)
        except (OSError, ValueError, RuntimeError, subprocess.TimeoutExpired) as exc:
            failures.append({'key': acceleration.candidate_key(plan), 'error': str(exc)[-1500:]})
            print('  non disponibile: ' + str(exc)[-300:], flush=True)
    if not results: raise RuntimeError('Nessun motore supera la calibrazione')
    # Prefer measured throughput. A tiny clip is always padded by Whisper;
    # its RTF is NOT a promise of the time for a twenty-minute meeting.
    winner = min(results, key=lambda r: r['inference_seconds'])
    path = root / 'hardware-profile.json'
    try: existing = json.loads(path.read_text(encoding='utf-8'))
    except (OSError, ValueError): existing = {}
    models = existing.get('models', {}) if existing.get('fingerprint') == hw['fingerprint'] else {}
    models[mid] = {**winner, 'selected': winner['key'], 'results': results, 'failures': failures,
                   'resources': acceleration.resource_signature(settings),
                   'calibrated_at': datetime.datetime.now(datetime.timezone.utc).isoformat()}
    profile = {'version': acceleration.PROFILE_VERSION, 'fingerprint': hw['fingerprint'], 'hardware': hw, 'models': models}
    temporary = path.with_suffix('.json.tmp')
    temporary.write_text(json.dumps(profile, ensure_ascii=False, indent=2), encoding='utf-8')
    temporary.replace(path)
    print('Motore scelto: ' + winner['label'], flush=True)

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--engine', type=Path, required=True)
    parser.add_argument('--model', choices=['turbo', 'small', 'tiny'], default='turbo')
    parser.add_argument('--probe')
    args = parser.parse_args()
    if args.probe: print(json.dumps(probe(args.engine.resolve(), args.model, json.loads(args.probe)), ensure_ascii=False))
    else: calibrate(args.engine.resolve(), args.model)
