"""Speech media processing. API keys never enter this process."""
import asyncio
import contextlib
import base64
import hashlib
import json
import math
import os
from pathlib import Path
import sys
import tempfile
import subprocess
import wave

_piper_model = None
_piper_path = None
_kokoro_model = None
_kokoro_path = None


def kokoro(request, path):
    """Synthesize locally with the shared model, retaining it between calls."""
    global _kokoro_model, _kokoro_path
    import numpy as np
    import onnxruntime as ort
    from kokoro_onnx import Kokoro
    from document_tools import ffmpeg_binary
    identity = (request['kokoro_model'], request['kokoro_voices'])
    if _kokoro_path != identity:
        _kokoro_model, _kokoro_path = None, None
        options = ort.SessionOptions()
        options.intra_op_num_threads = 2
        options.inter_op_num_threads = 1
        options.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
        ort.disable_telemetry_events()
        session = ort.InferenceSession(identity[0], sess_options=options, providers=['CPUExecutionProvider'])
        _kokoro_model = Kokoro.from_session(session, identity[1])
        _kokoro_path = identity
    samples, rate = _kokoro_model.create(request['text'], voice=request['voice'], lang='it')
    fd, temporary = tempfile.mkstemp(suffix='.wav', prefix='.kokoro-', dir=path.parent)
    os.close(fd)
    try:
        with wave.open(temporary, 'wb') as output:
            output.setnchannels(1)
            output.setsampwidth(2)
            output.setframerate(rate)
            output.writeframes((np.clip(samples, -1, 1) * 32767).astype('<i2').tobytes())
        subprocess.run([str(ffmpeg_binary()), '-v', 'error', '-nostdin', '-y', '-i', temporary,
                        '-codec:a', 'libmp3lame', '-b:a', '128k', str(path)],
                       check=True, capture_output=True,
                       creationflags=0x08000000 if os.name == 'nt' else 0)
    finally:
        Path(temporary).unlink(missing_ok=True)
    return []


def piper(request, path):
    """Keep one local CPU model resident for the lifetime of this helper."""
    global _piper_model, _piper_path
    import onnxruntime as ort
    from piper import PiperVoice
    from piper.config import PiperConfig
    from document_tools import ffmpeg_binary

    model_path = str(Path(request['piper_model']).resolve())
    config_path = str(Path(request['piper_config']).resolve())
    identity = (model_path, config_path)
    if _piper_path != identity:
        # Release the previous model before loading another quality.
        _piper_model, _piper_path = None, None
        ort.disable_telemetry_events()
        options = ort.SessionOptions()
        options.intra_op_num_threads = 2
        options.inter_op_num_threads = 1
        options.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
        config = PiperConfig.from_dict(json.loads(Path(config_path).read_text(encoding='utf-8')))
        session = ort.InferenceSession(model_path, sess_options=options, providers=['CPUExecutionProvider'])
        _piper_model = PiperVoice(session=session, config=config, download_dir=Path(model_path).parent)
        _piper_path = identity
    fd, temporary = tempfile.mkstemp(suffix='.wav', prefix='.piper-', dir=path.parent)
    os.close(fd)
    try:
        with wave.open(temporary, 'wb') as output:
            _piper_model.synthesize_wav(request['text'], output)
        subprocess.run([str(ffmpeg_binary()), '-v', 'error', '-nostdin', '-y', '-i', temporary,
                        '-codec:a', 'libmp3lame', '-b:a', '128k', str(path)],
                       check=True, capture_output=True,
                       creationflags=0x08000000 if os.name == 'nt' else 0)
    finally:
        Path(temporary).unlink(missing_ok=True)
    # These voice exports do not provide word boundaries. Never invent them.
    return []


def audio_duration(path):
    # Select the shipped FFmpeg, never a system executable or a download.
    from document_tools import ffmpeg_binary
    decoded = subprocess.run([str(ffmpeg_binary()), '-v', 'error', '-nostdin',
                              '-progress', 'pipe:1', '-i', str(path), '-vn', '-f', 'null', '-'],
                             capture_output=True, text=True, check=True,
                             creationflags=0x08000000 if os.name == 'nt' else 0)
    times = [int(line.split('=', 1)[1]) for line in decoded.stdout.splitlines()
             if line.startswith('out_time_us=') and line.split('=', 1)[1].isdigit()]
    duration = max(times, default=0) / 1_000_000
    if duration <= 0:
        raise ValueError('Audio vuoto o durata non valida.')
    return duration


def aligned_words(alignment):
    chars = alignment.get('characters', [])
    starts = alignment.get('character_start_times_seconds', [])
    ends = alignment.get('character_end_times_seconds', [])
    if not (len(chars) == len(starts) == len(ends)):
        raise ValueError('Tempi ElevenLabs incoerenti.')
    words, text, start, end = [], '', 0, 0
    for char, a, b in zip(chars, starts, ends):
        if char.isspace():
            if text:
                words.append({'text': text, 'start': start, 'end': end})
                text = ''
        else:
            if not text:
                start = float(a)
            text += char
            end = float(b)
    if text:
        words.append({'text': text, 'start': start, 'end': end})
    return words


def clock(seconds):
    ms = max(0, round(seconds * 1000))
    return f'{ms // 3600000:02}:{ms // 60000 % 60:02}:{ms // 1000 % 60:02},{ms % 1000:03}'


def subtitles(words):
    # Short readable captions, with timing from the provider rather than estimates.
    groups, group = [], []
    for word in words:
        group.append(word)
        if len(group) >= 9 or word['text'].endswith(('.', '?', '!', ';')):
            groups.append(group)
            group = []
    if group:
        groups.append(group)
    return '\n\n'.join(f"{i}\n{clock(g[0]['start'])} --> {clock(g[-1]['end'])}\n"
                       + ' '.join(w['text'] for w in g)
                       for i, g in enumerate(groups, 1)) + '\n'


async def edge(request, path):
    import edge_tts
    stream = edge_tts.Communicate(request['text'], request['voice'],
                                  rate=request.get('rate', '+0%'),
                                  pitch=request.get('pitch', '+0Hz'),
                                  boundary='WordBoundary')
    words = []
    with path.open('wb') as audio:
        async for event in stream.stream():
            if event['type'] == 'audio':
                audio.write(event['data'])
            elif event['type'] == 'WordBoundary':
                start = event['offset'] / 10_000_000
                words.append({'text': event['text'], 'start': start,
                              'end': start + event['duration'] / 10_000_000})
    return words


def generate(request):
    output = Path(request['output'])
    # Rust validates the destination within the call before invoking this helper.
    # Unique partial files keep a failed request from replacing a previous result.
    fd, temporary = tempfile.mkstemp(suffix='.mp3', prefix='.speech-', dir=output.parent)
    os.close(fd)
    partial = Path(temporary)
    try:
        if request['provider'] == 'edge':
            words = asyncio.run(edge(request, partial))
        elif request['provider'] == 'piper':
            words = piper(request, partial)
        elif request['provider'] == 'kokoro':
            with contextlib.redirect_stdout(sys.stderr):
                words = kokoro(request, partial)
        elif request['provider'] == 'qwen':
            # Third-party progress messages must never enter the NDJSON protocol.
            with contextlib.redirect_stdout(sys.stderr):
                from qwen_speech import synthesize
                words = synthesize(request, partial)
        elif request['provider'] in ('elevenlabs', 'custom'):
            response = request['response']
            partial.write_bytes(base64.b64decode(response['audio_base64'], validate=True))
            try:
                words = aligned_words(response.get('normalized_alignment') or response.get('alignment') or {})
            except (ValueError, TypeError, KeyError):
                words = []
        else:
            raise ValueError('Fornitore voce non supportato.')
        duration = audio_duration(partial)
        if any(not math.isfinite(w['start']) or not math.isfinite(w['end'])
               or w['start'] < 0 or w['end'] < w['start'] or w['end'] > duration + 0.25 for w in words):
            words = []
        warnings = [] if words else ['Audio salvato con durata reale. Il servizio non ha fornito tempi delle parole utilizzabili: sottotitoli non generati. Usa la durata della scena, senza inventare tempi per le parole.']
        for word in words:
            word['end'] = min(word['end'], duration)
        metadata = {key: request[key] for key in ('provider', 'voice', 'model', 'request_hash')}
        metadata.update(duration_seconds=duration, words=words, warnings=warnings,
                        audio_sha256=hashlib.sha256(partial.read_bytes()).hexdigest())
        srt = output.with_suffix('.srt')
        timing = output.with_suffix('.speech.json')
        # Metadata is committed last; incomplete output cannot be reused as a cache hit.
        partial.replace(output)
        if words:
            srt.write_text(subtitles(words), encoding='utf-8')
        else:
            srt.unlink(missing_ok=True)
        timing.write_text(json.dumps(metadata, ensure_ascii=False, indent=2), encoding='utf-8')
        return {'audio': str(output), 'timings': str(timing), 'subtitles': str(srt) if words else None,
                'duration_seconds': duration, 'word_timings_available': bool(words),
                'warnings': warnings, 'cached': False}
    finally:
        partial.unlink(missing_ok=True)


def run(request):
    if request.get('operation') == 'voices':
        import edge_tts
        return [{'id': v['ShortName'], 'name': v['ShortName'].split('-', 2)[-1].removesuffix('Neural'), 'locale': v['Locale'], 'gender': v.get('Gender', '')}
                for v in asyncio.run(edge_tts.list_voices())
                if not request.get('language') or v['Locale'].lower().startswith(request['language'].lower())]
    return generate(request)


if __name__ == '__main__':
    # -I omits the script directory; only import our adjacent, installed helper.
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    sys.stdout.reconfigure(encoding='utf-8')
    def reply(request):
        try:
            return run(request)
        except Exception as error:
            # Never print the request, generated text, HTTP bodies or environment.
            reason = str(error) if request.get('provider') == 'qwen' and str(error) in ('cuda_unavailable', 'vram') else 'Sintesi vocale non completata'
            return {'error': reason, 'type': type(error).__name__}
    if '--server' in sys.argv:
        for line in sys.stdin:
            try:
                request = json.loads(line)
                if request.get('provider') not in ('piper', 'qwen', 'kokoro'):
                    raise ValueError('Il processo persistente supporta solo i motori locali.')
                result = reply(request)
            except (ValueError, TypeError):
                result = {'error': 'Richiesta locale non valida', 'type': 'ValueError'}
            print(json.dumps(result, ensure_ascii=False), flush=True)
    else:
        result = reply(json.load(sys.stdin))
        print(json.dumps(result, ensure_ascii=False))
        if isinstance(result, dict) and result.get('error'):
            sys.exit(1)
