"""Local inference worker. JSON Lines on stdout, no network or persistent voice profiles."""
from __future__ import annotations
import argparse
import json
import os
import sys
import wave
import threading
import time
import ctypes
import subprocess
from contextlib import contextmanager
from pathlib import Path
import acceleration
from acceleration import ui_text
import transcript_quality

# Libraries may otherwise check or download remote model metadata.
os.environ["HF_HUB_OFFLINE"] = "1"
os.environ["HF_HUB_DISABLE_TELEMETRY"] = "1"
os.environ["DO_NOT_TRACK"] = "1"
_models = {}
_output_lock = threading.Lock()

def cpu_threads(live=False):
    return acceleration.threads(live)

def available_memory():
    free = acceleration.memory_available()
    if free:
        return free
    if os.name == "nt":
        class Memory(ctypes.Structure):
            _fields_ = [("length", ctypes.c_ulong), ("load", ctypes.c_ulong)] + [
                (name, ctypes.c_ulonglong) for name in ("total", "available", "page_total", "page_available", "virtual_total", "virtual_available", "extended")]
        memory = Memory()
        memory.length = ctypes.sizeof(memory)
        if ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(memory)):
            return memory.available
    return 0  # Unknown: use the conservative single-window path.

def batch_size(free_bytes, total_bytes=0, gpu=False):
    # Estimates for extra inference buffers, after loading audio and the model.
    # Leave at least 3 GiB or 20% of RAM available for the rest of the system.
    budget = max(0, free_bytes - max(3 * 1024**3, total_bytes * .2))
    per_window = (128 if gpu else 512) * 1024**2
    sizes = (16, 8, 4, 2, 1) if gpu else (8, 4, 2, 1)
    return next(size for size in sizes if size == 1 or size * per_window <= budget)


def inference_batch(plan):
    cuda = plan.get('device') == 'cuda'
    ram_limit = batch_size(available_memory(), acceleration.memory_total(), gpu=cuda)
    return min(ram_limit, acceleration.cuda_batch(plan)) if cuda else ram_limit


def memory_failure(error):
    return isinstance(error, MemoryError) or any(message in str(error).lower() for message in (
        'out of memory', 'failed to allocate', 'cannot allocate memory', 'bad_alloc',
        'cublas_status_alloc_failed', 'cudnn_status_alloc_failed'))


def adaptive_pipeline(model):
    from faster_whisper import BatchedInferencePipeline

    class AdaptivePipeline(BatchedInferencePipeline):
        """Split the library's batches without changing its VAD/timestamp mapping.

        forward's signature is from the pinned faster-whisper 1.2.1 runtime.
        Only an unfinished sub-batch is retried, so committed text is preserved.
        """
        def __init__(self):
            super().__init__(model)
            self.plan = model.parlo_plan
            self.ceiling = 16 if self.plan['device'] == 'cuda' else 8

        def forward(self, features, tokenizer, chunks_metadata, options):
            results, offset = [], 0
            while offset < len(features):
                size = min(self.ceiling, inference_batch(self.plan), len(features) - offset)
                timestamp = self.last_speech_timestamp
                failed = False
                try:
                    output = super().forward(features[offset:offset + size], tokenizer,
                        chunks_metadata[offset:offset + size], options)
                except Exception as error:
                    if size == 1 or not memory_failure(error):
                        raise
                    failed = True
                if failed:
                    # Leave the exception scope before releasing failed buffers.
                    self.last_speech_timestamp = timestamp
                    self.ceiling = max(1, size // 2)
                    acceleration.release()
                    emit('stage', text=ui_text(f'Memoria occupata: riduco il batch a {self.ceiling} e continuo…', f'Memory under pressure: continuing with batch size {self.ceiling}…'))
                    continue
                results.extend(output)
                offset += size
            return results

    return AdaptivePipeline()

@contextmanager
def inference_power():
    """Keep native inference responsive while the WebView is minimized.

    Only system sleep is inhibited, not the display or the user's power plan.
    """
    kernel = None
    awake = None
    if os.name == "nt":
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.GetCurrentProcess.restype = ctypes.c_void_p
        kernel.SetProcessInformation.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p, ctypes.c_ulong]
        kernel.SetThreadExecutionState.argtypes = [ctypes.c_ulong]
        kernel.SetThreadExecutionState.restype = ctypes.c_ulong
        class Throttling(ctypes.Structure):
            _fields_ = [("version", ctypes.c_ulong), ("control", ctypes.c_ulong), ("state", ctypes.c_ulong)]
        throttle = Throttling(1, 1, 0)
        kernel.SetProcessInformation(kernel.GetCurrentProcess(), 4, ctypes.byref(throttle), ctypes.sizeof(throttle))
        kernel.SetThreadExecutionState(0x80000001)
    elif sys.platform == 'darwin':
        # Inhibit idle system sleep only during inference. -w also releases the
        # assertion automatically if the worker exits unexpectedly.
        try:
            awake = subprocess.Popen(['/usr/bin/caffeinate', '-i', '-w', str(os.getpid())], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        except OSError:
            pass
    try:
        yield
    finally:
        if kernel:
            kernel.SetThreadExecutionState(0x80000000)
        if awake:
            awake.terminate()
            awake.wait()

class Progress:
    def __init__(self):
        self.started = time.monotonic()
        self.text = "Preparazione del motore locale…"
        self.lock = threading.Lock()
        self.stopped = threading.Event()
        self.thread = threading.Thread(target=self.heartbeat, daemon=True)
    def update(self, text):
        with self.lock:
            self.text = text
        self.publish()
    def publish(self):
        with self.lock:
            text = self.text
        elapsed = round(time.monotonic() - self.started)
        emit("stage", text=ui_text(f"{text} · trascorsi {elapsed // 60:02}:{elapsed % 60:02}", f"{text} · elapsed {elapsed // 60:02}:{elapsed % 60:02}"))
    def heartbeat(self):
        while not self.stopped.wait(2):
            self.publish()
    def __enter__(self):
        self.thread.start()
        return self
    def __exit__(self, *args):
        self.stopped.set()
        self.thread.join()

def emit(kind: str, **values):
    with _output_lock:
        print(json.dumps({"type": kind, **values}, ensure_ascii=False), flush=True)


def dictionary_options(settings):
    terms = ', '.join(entry['term'].strip() for entry in settings.get('dictionary', []) if entry.get('term', '').strip())
    return {'hotwords': terms} if terms else {}



def group_words(words: list[dict], source: str) -> list[dict]:
    result: list[dict] = []
    pending_space = ''
    for word in words:
        text = pending_space + word["text"]
        pending_space = ''
        if not text.strip():
            pending_space = text
            continue
        start, end = round(word["start"] * 1000), round(word["end"] * 1000)
        speaker = word["speaker"]
        quality = word.get('quality')
        # whisper.cpp emits subword tokens; a speaker/time limit must wait
        # for a space, including when that space arrives in a separate token.
        continues_word = bool(result and not text[0].isspace())
        if result and (continues_word or (result[-1]["speaker"] == speaker and result[-1].get('quality') == quality and start - result[-1]["end_ms"] < 2000 and end - result[-1]["start_ms"] < 20000)):
            result[-1]["text"] += text
            result[-1]["end_ms"] = max(end, result[-1]["end_ms"])
        else:
            result.append({"start_ms": max(0, start), "end_ms": max(0, end), "speaker": speaker, "text": text, "source": source})
            if quality: result[-1]['quality'] = quality
    for segment in result:
        segment["text"] = segment["text"].strip()
    return transcript_quality.review_segments(result)

def read_audio(audio_path: Path, start: float = 0., end: float | None = None):
    import numpy as np
    with wave.open(str(audio_path), "rb") as f:
        if (f.getnchannels(), f.getsampwidth(), f.getframerate()) != (1, 2, 16000):
            raise ValueError("Il motore richiede WAV mono PCM16 a 16 kHz")
        first = min(f.getnframes(), round(start * 16000))
        f.setpos(first)
        count = f.getnframes() - first if end is None else max(0, min(f.getnframes(), round(end * 16000)) - first)
        samples = np.frombuffer(f.readframes(count), dtype="<i2").astype(np.float32) / 32768.
    return samples

def prepare_microphone_audio(samples, source: str, settings: dict):
    """Recover a weak input for inference, with bounded gain and peak headroom.

    read_audio owns the float buffer; adapting it never alters the captured WAV.
    Estimate voice level from short frames, rather than averaging long pauses.
    """
    import numpy as np
    if source != "microphone" or not settings.get("microphone_voice_boost", True) or not len(samples):
        return samples
    levels = []
    peak = 0.
    for start in range(0, len(samples), 30 * 16000):
        block = samples[start:start + 30 * 16000]
        peak = max(peak, float(np.max(np.abs(block))))
        count = len(block) // 1600
        if count:
            frames = block[:count * 1600].reshape(count, 1600)
            levels.extend(np.sqrt(np.mean(np.square(frames), axis=1)))
        elif len(block):
            levels.append(float(np.sqrt(np.mean(np.square(block)))))
    if peak < .0001 or not levels:
        return samples
    voice_rms = float(np.percentile(levels, 90))
    gain = max(1., min(12., .06 / max(voice_rms, .000001), .95 / peak))
    if gain > 1.:
        samples *= gain
    return samples

def speech_filter(source: str, settings: dict, live=False):
    # Keep the system track's detection unchanged. Quiet microphone speech needs
    # less confidence to enter a speech interval, while retaining the silence filter.
    quiet_voice = source in ("microphone", "mixed") and settings.get("microphone_voice_boost", True)
    return {"threshold": .35 if quiet_voice else .5, "min_silence_duration_ms": 2000 if live else 500}

def remove_echo(microphone, system):
    """Conservative delay/gain cancellation against the known far-end signal.

    This is not full acoustic echo cancellation. Only strongly correlated blocks
    are changed; independent local speech remains in the residual. Delays up to
    250 ms are searched with FFT correlation, without another neural model.
    """
    import numpy as np
    block_size, delay = 8000, 4000
    for start in range(0, len(microphone), block_size):
        block = microphone[start:start + block_size]
        count = len(block)
        if count < 320 or float(np.dot(block, block)) < 1e-8:
            continue
        reference = np.zeros(count + delay, dtype=np.float32)
        first = max(0, start - delay)
        reference[first - (start - delay):] = system[first:start + count]
        powers = np.cumsum(np.concatenate(([0.], reference.astype(np.float64)**2)))
        energies = powers[count:] - powers[:-count]
        if float(np.max(energies)) < count * .0001**2:
            continue
        size = 1 << (len(reference) + count - 2).bit_length()
        correlations = np.fft.irfft(np.fft.rfft(reference, size) * np.fft.rfft(block[::-1], size), size)[count - 1:count + delay]
        confidence = correlations / np.sqrt(np.maximum(energies * float(np.dot(block, block)), 1e-20))
        offset = int(np.argmax(confidence))
        if confidence[offset] >= .65:
            gain = float(correlations[offset] / max(energies[offset], 1e-20))
            if 0 < gain <= 4:
                block -= gain * reference[offset:offset + count]
    return microphone

def frame_levels(samples):
    import numpy as np
    count = (len(samples) + 319) // 320
    result = np.zeros(count, dtype=np.float32)
    full = len(samples) // 320
    for first in range(0, full, 2048):
        last = min(full, first + 2048)
        frames = samples[first * 320:last * 320].reshape(last - first, 320)
        result[first:last] = np.sqrt(np.mean(np.square(frames), axis=1))
    if len(samples) % 320:
        result[-1] = np.sqrt(np.mean(np.square(samples[full * 320:])))
    return result

def speech_activity(samples, threshold):
    """Use the bundled Silero detector to distinguish speech from channel volume.

    This runs on each captured channel after echo removal, without transcribing
    twice or fetching a model. Short padding accommodates word timing jitter.
    """
    import numpy as np
    from faster_whisper.vad import get_speech_timestamps, VadOptions
    count = (len(samples) + 319) // 320
    frames = np.zeros(count, dtype=np.int32)
    if len(samples) and float(np.max(np.abs(samples))) >= .0001:
        intervals = get_speech_timestamps(samples, VadOptions(threshold=threshold,
            min_speech_duration_ms=80, min_silence_duration_ms=100, speech_pad_ms=30))
        for interval in intervals:
            frames[max(0, interval['start'] // 320):min(count, (interval['end'] + 319) // 320)] = 1
    return np.concatenate(([0], np.cumsum(frames, dtype=np.int32)))

class SourceActivity:
    def __init__(self, microphone, system, has_microphone, has_system):
        import numpy as np
        self.has_microphone, self.has_system = has_microphone, has_system
        # One source has an unambiguous label. Avoid two RMS buffers and their
        # cumulative sums for the entire call (and skip both channel VADs).
        if not (has_microphone and has_system):
            return
        mic, pc = frame_levels(microphone), frame_levels(system)
        self.floor = max(.006, float(np.percentile(mic, 10)) * 2) if len(mic) else .006
        self.mic = np.concatenate(([0.], np.cumsum(mic.astype(np.float64)**2)))
        self.pc = np.concatenate(([0.], np.cumsum(pc.astype(np.float64)**2)))
        self.mic_speech = speech_activity(microphone, .35) if has_microphone and has_system else None
        self.pc_speech = speech_activity(system, .5) if has_microphone and has_system else None
    def speaker(self, start, end):
        if not self.has_system:
            return "Tu"
        if not self.has_microphone:
            return "Speaker"
        import math
        first = max(0, min(len(self.mic) - 2, math.floor(start / .02)))
        last = max(first + 1, min(len(self.mic) - 1, math.ceil(end / .02)))
        local_voice = (self.mic_speech[last] - self.mic_speech[first]) / (last - first)
        remote_voice = (self.pc_speech[last] - self.pc_speech[first]) / (last - first)
        if local_voice >= .4 and remote_voice < .25:
            return "Tu"
        if remote_voice >= .4 and local_voice < .25:
            return "Speaker"
        mic = math.sqrt(max(0., self.mic[last] - self.mic[first]) / (last - first))
        pc = math.sqrt(max(0., self.pc[last] - self.pc[first]) / (last - first))
        return "Tu" if mic > self.floor and mic >= pc * .65 else "Speaker"

def prepare_system_audio(samples):
    import numpy as np
    if len(samples):
        levels = frame_levels(samples)
        active = levels[levels > .002]
        if len(active):
            gain = min(12., .06 / max(float(np.percentile(active, 80)), .000001), .95 / max(float(np.max(np.abs(samples))), .000001))
            samples *= gain
    return samples


def mixed_audio(directory, settings, start=0., end=None):
    import numpy as np
    tracks = {}
    for source in ("microphone", "system"):
        path = directory / f"{source}.wav"
        if path.is_file():
            tracks[source] = read_audio(path, start, end)
    if not tracks and (directory / "registrazione.wav").is_file():
        samples = read_audio(directory / "registrazione.wav", start, end)
        return samples, SourceActivity(samples, samples, False, True)
    if len(tracks) == 1:
        source, samples = next(iter(tracks.items()))
        if source == 'microphone':
            prepare_microphone_audio(samples, source, settings)
        else:
            prepare_system_audio(samples)
        if len(samples):
            peak = float(np.max(np.abs(samples)))
            if peak > .95:
                samples *= .95 / peak
        return samples, SourceActivity(samples, samples, source == 'microphone', source == 'system')
    length = max((len(track) for track in tracks.values()), default=0)
    def aligned(source):
        track = tracks.pop(source, None)
        if track is None:
            return np.zeros(length, dtype=np.float32)
        return track if len(track) == length else np.pad(track, (0, length - len(track)))
    mic, pc = aligned('microphone'), aligned('system')
    del tracks  # Avoid retaining extra long-call buffers during model loading.
    if length:
        if (directory / "microphone.wav").is_file() and (directory / "system.wav").is_file():
            remove_echo(mic, pc)
        prepare_microphone_audio(mic, "microphone", settings)
        prepare_system_audio(pc)
    activity = SourceActivity(mic, pc, (directory / "microphone.wav").is_file(), (directory / "system.wav").is_file())
    mic += pc
    if length:
        peak = float(np.max(np.abs(mic)))
        if peak > .95:
            mic *= .95 / peak
    return mic, activity

def model_for(settings, live=False, force_cpu=False):
    selected = dict(settings)
    if live:
        selected['whisper_model'] = settings.get('live_whisper_model', settings['whisper_model'])
    if force_cpu:
        selected['compute_mode'] = 'cpu'
        # Native-only model installations must fall back to that format's CPU
        # adapter, rather than requiring an unrequested CTranslate2 download.
        selected['_force_faster'] = Path(selected['whisper_model']).joinpath('model.bin').is_file()
    errors = []
    plans = acceleration.selected_candidates(selected, live)
    if live and not plans:
        raise RuntimeError('GPU non disponibile per questo modello. Prepara il modello dell’anteprima oppure scegli esplicitamente CPU nelle impostazioni')
    for plan in plans:
        key = (selected['whisper_model'], live, acceleration.candidate_key(plan))
        if key in _models:
            if hasattr(_models[key], 'temporary'):
                _models[key].temporary = selected.get('_temporary_dir', str(acceleration.root_for(selected)))
            return _models[key]
        emit('stage', text=ui_text('Caricamento trascrizione · ', 'Loading transcription · ') + plan['label'])
        _models.clear()
        acceleration.release()
        try:
            _models[key] = acceleration.load(selected, plan)
            return _models[key]
        except Exception as exc:
            errors.append(str(exc))
            emit('stage', text=plan['label'] + ui_text(' non disponibile; provo il motore successivo…', ' unavailable. Trying the next engine…'))
    raise RuntimeError('Impossibile avviare la trascrizione locale: ' + '; '.join(errors))

def live(settings: dict, directory: Path, cursors: dict):
    """One mixed rolling window; use channels for Tu/Speaker, never voice clustering."""
    import numpy as np
    durations = []
    try:
        for source in ('microphone', 'system'):
            path = directory / f'{source}.wav'
            if path.is_file():
                with wave.open(str(path), 'rb') as file:
                    durations.append(file.getnframes() / 16000)
    except (EOFError, wave.Error):
        return [], cursors
    duration = max(durations, default=0.)
    cursor = cursors.get('mixed', 0.)
    if duration - cursor < 2.5:
        return [], cursors
    start = max(0., cursor - .7)
    end = min(duration, cursor + 8.)
    stable = end - .7
    try:
        samples, activity = mixed_audio(directory, settings, start, end)
    except (EOFError, wave.Error):
        return [], cursors
    words = []
    if len(samples) and float(np.max(np.abs(samples))) >= .0001:
        model = model_for(settings, True)
        segments, _ = model.transcribe(samples, language=None if settings['language'] == 'auto' else settings['language'],
            word_timestamps=True, vad_filter=True, vad_parameters=speech_filter('mixed', settings, live=True),
            condition_on_previous_text=False, beam_size=1, hallucination_silence_threshold=2.,
            repetition_penalty=1.05, no_repeat_ngram_size=12, **dictionary_options(settings))
        for segment in segments:
            for word in segment.words or []:
                midpoint = start + (word.start + word.end) / 2
                if cursor <= midpoint < stable:
                    words.append({'start': start + word.start, 'end': start + word.end, 'text': word.word,
                        'speaker': activity.speaker(word.start, word.end), 'quality': getattr(word, 'quality', None)})
    result = group_words(words, 'mixed')
    emit('live_source', source='mixed', segments=result)
    return result, {'mixed': stable}

def run(settings: dict, directory: Path) -> list[dict]:
    with inference_power(), Progress() as progress:
        return run_final(settings, directory, progress)

def final_segments(model, samples, source, settings):
    options = dict(language=None if settings["language"] == "auto" else settings["language"],
        word_timestamps=True, vad_filter=True, condition_on_previous_text=False, beam_size=1,
        hallucination_silence_threshold=2., vad_parameters=speech_filter(source, settings),
        repetition_penalty=1.05, no_repeat_ngram_size=12, **dictionary_options(settings))
    plan = getattr(model, 'parlo_plan', {'backend': 'faster', 'device': 'cpu'})
    if plan.get('backend') == 'faster':
        # The outer batch is a ceiling; forward rechecks RAM/VRAM before every
        # inference, shrinking or growing sub-batches within that ceiling.
        return adaptive_pipeline(model).transcribe(samples,
            batch_size=16 if plan['device'] == 'cuda' else 8, **options)
    return model.transcribe(samples, **options)

def run_final(settings: dict, directory: Path, progress) -> list[dict]:
    import numpy as np
    progress.update(ui_text('Preparazione audio unico…', 'Preparing combined audio…'))
    samples, activity = mixed_audio(directory, settings)
    if not len(samples) or float(np.max(np.abs(samples))) < .0001:
        return []
    progress.update(ui_text('Caricamento del modello di trascrizione…', 'Loading transcription model…'))
    settings = {**settings, '_temporary_dir': str(directory)}
    model = model_for(settings)
    for attempt in range(2):
        plan = getattr(model, 'parlo_plan', {'device': 'cpu', 'label': 'CPU'})
        progress.update(ui_text('Trascrizione della call · ', 'Transcribing call · ') + plan['label'])
        words = []
        last_partial = time.monotonic()
        try:
            segments, info = final_segments(model, samples, 'mixed', settings)
            for segment in segments:
                progress.update(ui_text(f"Trascrizione della call · {plan['label']} · progresso fase {min(100, round(100 * segment.end / max(info.duration, 1)))}%", f"Transcribing call · {plan['label']} · stage progress {min(100, round(100 * segment.end / max(info.duration, 1)))}%"))
                for word in segment.words or []:
                    words.append({'start': word.start, 'end': word.end, 'text': word.word,
                        'speaker': activity.speaker(word.start, word.end), 'quality': getattr(word, 'quality', None)})
                if time.monotonic() - last_partial >= 10:
                    emit('final_partial', segments=group_words(words, 'mixed'))
                    last_partial = time.monotonic()
            break
        except Exception:
            if attempt or (plan['device'] == 'cpu' and plan.get('backend', 'faster') == 'faster'):
                raise
            # Retry the whole call, replacing provisional text; never append a
            # CPU retry to the GPU's partial output (which would duplicate it).
            emit('final_partial', segments=[])
            progress.update(ui_text('Accelerazione interrotta: riprendo sulla CPU…', 'Acceleration interrupted. Resuming on CPU…'))
        # Outside except: its traceback can otherwise retain the failed GPU
        # pipeline while loading the CPU model, causing a second memory failure.
        del model
        _models.clear()
        acceleration.release()
        model = model_for(settings, force_cpu=True)
    result = group_words(words, 'mixed')
    emit('final_partial', segments=result)
    progress.update(ui_text('Salvataggio della trascrizione…', 'Saving transcript…'))
    return result

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--directory", type=Path)
    parser.add_argument("--daemon", action="store_true")
    args = parser.parse_args()
    if args.daemon:
        for line in sys.stdin:
            try:
                request = json.loads(line)
                if request.get("cursors", {}).get("warmup"):
                    import numpy as np
                    from faster_whisper.vad import get_vad_model
                    model = model_for(request["settings"], True)
                    get_vad_model()
                    # Warm encoder/decoder on silence; never read the microphone.
                    segments, _ = model.transcribe(np.zeros(3 * 16000, dtype=np.float32), language="en", vad_filter=False, word_timestamps=True, beam_size=1, condition_on_previous_text=False)
                    list(segments)
                    emit("result", warmed=True)
                elif request.get("live"):
                    segments, cursors = live(request["settings"], Path(request["directory"]), request.get("cursors", {}))
                    emit("result", segments=segments, cursors=cursors, streamed=True)
                else:
                    emit("result", segments=run(request["settings"], Path(request["directory"])))
            except Exception as exc:
                emit("error", message=str(exc))
        return 0
    if not args.directory:
        parser.error("--directory richiesto senza --daemon")
    settings = json.loads(sys.stdin.read())
    try:
        emit("result", segments=run(settings, args.directory))
    except Exception as exc:
        emit("error", message=str(exc))
        return 1
    return 0

if __name__ == "__main__":
    sys.exit(main())
