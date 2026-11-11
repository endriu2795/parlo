"""Offline Qwen3-TTS CustomVoice synthesis; model files must already be verified."""
import os
from pathlib import Path
import re
import tempfile
import subprocess

_model = None
_identity = None


def chunks(text, limit=400):
    """Bound autoregressive generations, preserving all characters in order."""
    output, current = [], ''
    for part in re.findall(r'\S+\s*|\s+', text):
        while len(part) > limit:
            if current:
                output.append(current)
                current = ''
            output.append(part[:limit])
            part = part[limit:]
        if current and len(current) + len(part) > limit:
            output.append(current)
            current = ''
        current += part
    if current:
        output.append(current)
    return output


def synthesize(request, path):
    global _model, _identity
    for name in ('HF_HUB_OFFLINE', 'TRANSFORMERS_OFFLINE', 'HF_HUB_DISABLE_TELEMETRY', 'DO_NOT_TRACK'):
        os.environ[name] = '1'
    import numpy as np
    import soundfile as sf
    import torch
    from qwen_tts import Qwen3TTSModel
    from document_tools import ffmpeg_binary

    device = request['qwen_device']
    if device not in ('cpu', 'cuda'):
        raise ValueError('device')
    if device == 'cuda':
        if not torch.cuda.is_available():
            raise RuntimeError('cuda_unavailable')
        if torch.cuda.get_device_properties(0).total_memory < 7.5 * 1024**3:
            raise RuntimeError('vram')
    identity = (str(Path(request['qwen_model']).resolve()), device)
    if identity != _identity:
        _model, _identity = None, None
        if torch.cuda.is_available():
            torch.cuda.empty_cache()
        torch.set_num_threads(4)
        _model = Qwen3TTSModel.from_pretrained(identity[0], device_map='cuda:0' if device == 'cuda' else 'cpu',
                    dtype=torch.float16 if device == 'cuda' else torch.float32,
                    attn_implementation='sdpa', local_files_only=True, trust_remote_code=False)
        _identity = identity
    handle, temporary = tempfile.mkstemp(suffix='.wav', prefix='.qwen-', dir=path.parent)
    os.close(handle)
    try:
        with sf.SoundFile(temporary, 'w', samplerate=24000, channels=1, subtype='PCM_16') as output:
            with torch.inference_mode():
                for text in chunks(request['text']):
                    if not text.strip():
                        continue
                    audio, rate = _model.generate_custom_voice(text=text, language=request['qwen_language'],
                        speaker=request['voice'], instruct=request.get('instruct', ''),
                        non_streaming_mode=True, max_new_tokens=2048)
                    if rate != 24000 or len(audio) != 1:
                        raise ValueError('audio_format')
                    samples = np.asarray(audio[0], dtype=np.float32).reshape(-1)
                    if not samples.size or not np.isfinite(samples).all():
                        raise ValueError('audio_empty')
                    output.write(samples)
        subprocess.run([str(ffmpeg_binary()), '-v', 'error', '-nostdin', '-y', '-i', temporary,
                        '-codec:a', 'libmp3lame', '-b:a', '128k', str(path)], check=True, capture_output=True,
                        creationflags=0x08000000 if os.name == 'nt' else 0)
    except Exception:
        _model, _identity = None, None
        if torch.cuda.is_available():
            torch.cuda.empty_cache()
        raise
    finally:
        Path(temporary).unlink(missing_ok=True)
    # CustomVoice does not expose word alignment. Never fabricate subtitles.
    return []
