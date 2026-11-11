"""Offline hardware selection and adapters. No audio leaves this process."""
from __future__ import annotations
import gc
import ctypes
import ctypes.util
import hashlib
import importlib.metadata
import json
import os
import platform
import subprocess
import tempfile
import wave
from pathlib import Path
from types import SimpleNamespace
import transcript_quality

_hardware = None
_dll_handles = []
PROFILE_VERSION = 1

def ui_text(italian, english):
    return english if os.environ.get('PARLO_UI_LANGUAGE') == 'en' else italian


def vulkan_devices():
    """Query the OS Vulkan loader; never install or alter a graphics driver."""
    if platform.system() == 'Darwin': return []
    try:
        library = ctypes.WinDLL('vulkan-1.dll') if os.name == 'nt' else ctypes.CDLL(ctypes.util.find_library('vulkan') or 'libvulkan.so.1')
        class Application(ctypes.Structure):
            _fields_ = [('type',ctypes.c_uint32),('next',ctypes.c_void_p),('name',ctypes.c_char_p),('version',ctypes.c_uint32),
                        ('engine',ctypes.c_char_p),('engine_version',ctypes.c_uint32),('api',ctypes.c_uint32)]
        class InstanceInfo(ctypes.Structure):
            _fields_ = [('type',ctypes.c_uint32),('next',ctypes.c_void_p),('flags',ctypes.c_uint32),('application',ctypes.POINTER(Application)),
                        ('layers',ctypes.c_uint32),('layer_names',ctypes.c_void_p),('extensions',ctypes.c_uint32),('extension_names',ctypes.c_void_p)]
        application = Application(0,None,b'PARLO',1,b'PARLO',1,1 << 22)
        info = InstanceInfo(1,None,0,ctypes.pointer(application),0,None,0,None)
        instance = ctypes.c_void_p()
        library.vkCreateInstance.argtypes = [ctypes.POINTER(InstanceInfo),ctypes.c_void_p,ctypes.POINTER(ctypes.c_void_p)]
        library.vkDestroyInstance.argtypes = [ctypes.c_void_p,ctypes.c_void_p]
        library.vkEnumeratePhysicalDevices.argtypes = [ctypes.c_void_p,ctypes.POINTER(ctypes.c_uint32),ctypes.c_void_p]
        library.vkGetPhysicalDeviceProperties.argtypes = [ctypes.c_void_p,ctypes.c_void_p]
        if library.vkCreateInstance(ctypes.byref(info),None,ctypes.byref(instance)) != 0: return []
        try:
            count = ctypes.c_uint32()
            if library.vkEnumeratePhysicalDevices(instance,ctypes.byref(count),None) != 0 or count.value > 32: return []
            devices = (ctypes.c_void_p * count.value)()
            if library.vkEnumeratePhysicalDevices(instance,ctypes.byref(count),devices) != 0: return []
            result = []
            for index, device in enumerate(devices):
                properties = ctypes.create_string_buffer(4096)
                library.vkGetPhysicalDeviceProperties(device,properties)
                import struct
                api,driver,vendor,device_id,kind = struct.unpack_from('<IIIII',properties.raw)
                if kind not in (1,2) or api < (1 << 22 | 2 << 12): continue
                result.append({'backend':'cpp','device':str(index),'name':properties.raw[20:276].split(b'\0')[0].decode('utf-8','replace'),
                               'vendor':vendor,'driver':str(driver),'device_id':device_id})
            return result
        finally: library.vkDestroyInstance(instance,None)
    except (OSError, AttributeError): return []

def memory_available():
    try:
        import psutil
        return psutil.virtual_memory().available
    except ImportError:
        return 0

def memory_total():
    try:
        import psutil
        return psutil.virtual_memory().total
    except ImportError:
        return 0

def threads(live=False):
    logical = os.cpu_count() or 4
    try:
        import psutil
        physical = psutil.cpu_count(logical=False) or logical
    except ImportError:
        physical = max(1, logical // 2)
    return max(1, min(2 if live else 16, physical, max(1, logical - 2)))

def root_for(settings):
    return Path(os.environ.get('CALLNOTES_ENGINE_DIR') or Path(settings['whisper_model']).parent.parent)

def model_id(settings):
    path = Path(settings['whisper_model'])
    name = path.name
    known = name in ('whisper-turbo', 'whisper-small', 'whisper-tiny')
    return name.removeprefix('whisper-') if known and path.resolve() == (root_for(settings) / 'models' / name).resolve() else 'custom'

def cuda_libraries():
    """NVIDIA Python wheels are private to our venv; no system PATH changes."""
    if os.name != 'nt':
        return
    import site
    extra = Path(os.environ.get('CALLNOTES_ENGINE_DIR', '.')) / 'cuda'
    paths = [extra / package / 'nvidia' / package / 'bin' for package in ('cublas', 'cudnn')]
    for folder in site.getsitepackages():
        for name in ('cublas', 'cudnn'):
            path = Path(folder) / 'nvidia' / name / 'bin'
            paths.append(path)
    for path in paths:
        if path.is_dir() and str(path) not in os.environ.get('PATH', '').split(os.pathsep):
            _dll_handles.append(os.add_dll_directory(str(path)))
            os.environ['PATH'] = str(path) + os.pathsep + os.environ.get('PATH', '')

def openvino_import():
    # Inference must not opt into telemetry previously enabled by another app.
    # Use OpenVINO's own no-op telemetry implementation in this worker only.
    import sys
    if 'openvino_telemetry' not in sys.modules:
        from types import ModuleType
        stub = ModuleType('openvino_telemetry')
        class Telemetry:
            def __init__(self, *args, **kwargs):
                self.backend = SimpleNamespace(app_name=kwargs.get('app_name', ''), app_version=kwargs.get('app_version', ''))
            def __getattr__(self, name): return lambda *args, **kwargs: None
        stub.Telemetry = Telemetry
        stub.__path__ = []
        backend = ModuleType('openvino_telemetry.backend')
        backend.backend_ga4 = None
        sys.modules['openvino_telemetry'] = stub
        sys.modules['openvino_telemetry.backend'] = backend
    import openvino
    return openvino

def hardware(refresh=False):
    global _hardware
    if _hardware is not None and not refresh:
        return _hardware
    cuda_libraries()
    data = {'platform': platform.system(), 'architecture': platform.machine(),
            'processor': platform.processor(), 'logical_cores': os.cpu_count() or 1, 'threads': threads(), 'gpus': [], 'versions': {}}
    try:
        import psutil
        data['memory_bytes'] = psutil.virtual_memory().total
    except ImportError:
        data['memory_bytes'] = 0
    try:
        import ctranslate2
        for index in range(ctranslate2.get_cuda_device_count()):
            try: types = sorted(ctranslate2.get_supported_compute_types('cuda', index))
            except Exception: types = ['int8_float16']
            data['gpus'].append({'backend': 'cuda', 'device': str(index), 'name': f'NVIDIA CUDA {index}', 'compute_types': types})
    except Exception:
        pass
    try:
        core = openvino_import().Core()
        for device in core.available_devices:
            if device.startswith('GPU'):
                name = core.get_property(device, 'FULL_DEVICE_NAME')
                try: driver = str(core.get_property(device, 'DRIVER_VERSION'))
                except Exception: driver = 'unknown'
                data['gpus'].append({'backend': 'openvino', 'device': device, 'name': name, 'driver': driver})
    except Exception:
        pass
    if data['platform'] == 'Darwin' and data['architecture'] in ('arm64', 'aarch64'):
        data['gpus'].append({'backend': 'cpp', 'device': '0', 'name': 'Apple Metal'})
    for package in ('ctranslate2', 'openvino-genai'):
        try: data['versions'][package] = importlib.metadata.version(package)
        except importlib.metadata.PackageNotFoundError: pass
    vulkan = vulkan_devices()
    for gpu in vulkan:
        # OpenVINO/CUDA already identify the same Intel/NVIDIA hardware.
        if gpu['vendor'] == 32902 and any(p['backend']=='openvino' for p in data['gpus']): continue
        if gpu['vendor'] == 4318 and any(p['backend']=='cuda' for p in data['gpus']): continue
        data['gpus'].append(gpu)
    data['fingerprint'] = hashlib.sha256(json.dumps(data, sort_keys=True).encode()).hexdigest()
    data['vulkan'] = vulkan
    _hardware = data
    return data

def candidates(settings, live=False):
    cpu = {'backend': 'faster', 'device': 'cpu', 'compute': 'int8', 'threads': threads(live), 'label': 'CPU · INT8'}
    if (live and settings.get('live_compute_mode', 'auto') == 'cpu') or settings.get('_force_faster'):
        return [cpu]
    root, mid = root_for(settings), model_id(settings)
    result = []
    for gpu in hardware()['gpus']:
        if gpu['backend'] == 'cuda':
            compute = next((c for c in ('int8_float16','float16','int8_float32','float32') if c in gpu.get('compute_types',['int8_float16'])), 'float32')
            result.append({'backend': 'faster', 'device': 'cuda', 'index': int(gpu['device']),
                           'compute': compute, 'threads': threads(), 'label': gpu['name'] + ' · CUDA'})
        elif gpu['backend'] == 'openvino' and (root / 'models' / f'whisper-{mid}-openvino' / 'presentio-model.json').is_file():
            result.append({'backend': 'openvino', 'device': gpu['device'], 'threads': threads(), 'label': gpu['name'] + ' · OpenVINO'})
    if (root / 'models' / f'whisper-{mid}-openvino' / 'presentio-model.json').is_file():
        result.append({'backend': 'openvino', 'device': 'CPU', 'threads': threads(), 'label': 'CPU · OpenVINO INT8'})
    cpp = root / 'whisper-cpp' / 'runtime.json'
    if cpp.is_file() and mid != 'custom':
        try: manifest = json.loads(cpp.read_text(encoding='utf-8-sig'))
        except (OSError, ValueError): manifest = {}
        if (root / 'models' / f'ggml-{mid}.bin').is_file() and manifest.get('backend') in ('vulkan', 'metal'):
            devices = hardware().get('vulkan', []) if manifest['backend'] == 'vulkan' else [g for g in hardware()['gpus'] if g['backend']=='cpp']
            for gpu in devices:
                result.append({'backend': 'cpp', 'device': gpu['device'], 'threads': threads(), 'label': gpu['name'] + ' · ' + manifest['backend'].capitalize()})
            if os.name != 'nt':
                result.append({'backend': 'cpp', 'device': 'cpu', 'threads': threads(), 'label': 'CPU · whisper.cpp INT8'})
    native_ready = (root / 'models' / f'whisper-{mid}-openvino/openvino_encoder_model.xml').is_file() or (root / 'models' / f'ggml-{mid}.bin').is_file()
    base_ready = Path(settings['whisper_model']).joinpath('model.bin').is_file()
    if native_ready and not base_ready:
        result = [p for p in result if p['backend'] != 'faster']
    plans = result + ([cpu] if base_ready or not native_ready else [])
    if live:
        # Preview never silently heats the CPU after a GPU error. CPU is opt-in.
        return [{**p, 'threads': threads(True), 'live': True} for p in plans if p['device'].lower() != 'cpu']
    return [p for p in plans if p['device'] == 'cpu' or p['device'] == 'CPU'] if settings.get('compute_mode') == 'cpu' else plans

def candidate_key(candidate):
    return ':'.join(str(candidate.get(k, '')) for k in ('backend', 'device', 'index', 'threads'))

def cuda_batch(plan):
    try:
        result = subprocess.run(['nvidia-smi', '--query-gpu=memory.free,memory.total', '--format=csv,noheader,nounits'],
            capture_output=True, text=True, timeout=2, creationflags=0x08000000 if os.name == 'nt' else 0)
        free, total = (int(value.strip()) for value in result.stdout.splitlines()[plan.get('index', 0)].split(','))
        # Keep room for the desktop/other apps; the model is already loaded.
        budget = max(0, free - max(1024, total * .15))
        return next(size for size in (16, 8, 4, 2, 1) if size == 1 or size * 512 <= budget)
    except (OSError, ValueError, IndexError, subprocess.TimeoutExpired):
        return 1

def profile_for(settings):
    try:
        data = json.loads((root_for(settings) / 'hardware-profile.json').read_text(encoding='utf-8'))
        if data.get('version') == PROFILE_VERSION and data.get('fingerprint') == hardware()['fingerprint']:
            profile = data.get('models', {}).get(model_id(settings))
            if profile and profile.get('resources', resource_signature(settings)) == resource_signature(settings):
                return profile
    except (OSError, ValueError):
        pass
    return None

def resource_signature(settings):
    root = root_for(settings)
    resources = {}
    for label, path in [('cpp', root / 'whisper-cpp/runtime.json'),
                        ('openvino', root / 'models' / f'whisper-{model_id(settings)}-openvino/presentio-model.json')]:
        try: resources[label] = json.loads(path.read_text(encoding='utf-8-sig'))
        except (OSError, ValueError): pass
    return hashlib.sha256(json.dumps(resources, sort_keys=True).encode()).hexdigest()

def selected_candidates(settings, live=False):
    plans = candidates(settings, live)
    if not live:
        profile = profile_for(settings)
        if profile:
            if settings.get('compute_mode') == 'cpu':
                cpu_results = [r for r in profile.get('results', []) if r.get('plan', {}).get('device', '').lower() == 'cpu']
                if cpu_results:
                    best = min(cpu_results, key=lambda r: r['inference_seconds'])
                    profile = {**best, 'selected': best['key']}
            winner = profile.get('selected')
            calibrated = profile.get('plan', {})
            for plan in plans:
                if all(plan.get(k) == calibrated.get(k) for k in ('backend', 'device', 'index')):
                    count = calibrated.get('threads')
                    if isinstance(count, int) and 1 <= count <= 16 and count <= (os.cpu_count() or 1):
                        plan['threads'] = count
            plans.sort(key=lambda p: candidate_key(p) != winner)
    return plans

def load(settings, plan):
    if plan['backend'] == 'openvino':
        return OpenVINOModel(settings, plan)
    if plan['backend'] == 'cpp':
        if plan.get('live'):
            from cpp_live import LiveModel
            return LiveModel(settings, plan)
        return CppModel(settings, plan)
    from faster_whisper import WhisperModel
    model = WhisperModel(settings['whisper_model'], device=plan['device'], device_index=plan.get('index', 0),
        compute_type=plan['compute'], cpu_threads=plan['threads'], num_workers=1, local_files_only=True)
    model.parlo_plan = plan
    return model

def speech_windows(samples, options):
    """Pack speech into encoder windows instead of padding every short phrase."""
    from faster_whisper.vad import VadOptions, get_speech_timestamps, SpeechTimestampsMap
    import numpy as np
    if options.get('vad_filter', True):
        params = dict(options.get('vad_parameters', {}))
        params['max_speech_duration_s'] = 28.
        intervals = get_speech_timestamps(samples, VadOptions(**params))
    else:
        intervals = [{'start': 0, 'end': len(samples)}] if len(samples) else []
    split = []
    for interval in intervals:
        for start in range(interval['start'], interval['end'], 28 * 16000):
            split.append({'start': start, 'end': min(interval['end'], start + 28 * 16000)})
    timeline = SpeechTimestampsMap(split, 16000)
    def generate():
        pieces, length, offset = [], 0, 0.
        for interval in split:
            count = interval['end'] - interval['start']
            if pieces and length + count > 28 * 16000:
                yield np.concatenate(pieces), offset
                offset += length / 16000
                pieces, length = [], 0
            pieces.append(samples[interval['start']:interval['end']])
            length += count
        if pieces: yield np.concatenate(pieces), offset
    return generate(), timeline

class OpenVINOModel:
    def __init__(self, settings, plan):
        openvino_import()
        import openvino_genai
        self.parlo_plan = plan
        path = root_for(settings) / 'models' / f'whisper-{model_id(settings)}-openvino'
        cache = root_for(settings) / 'openvino-cache'
        cache.mkdir(parents=True, exist_ok=True)
        options = {'word_timestamps': True, 'CACHE_DIR': str(cache)}
        if plan['device'] == 'CPU':
            options['INFERENCE_NUM_THREADS'] = plan['threads']
        self.pipeline = openvino_genai.WhisperPipeline(str(path), plan['device'], **options)

    def transcribe(self, samples, **options):
        """VAD bounds each window; keep word timestamps in original call time."""
        language = options.get('language')
        windows, timeline = speech_windows(samples, options)
        def generate():
            # Process at most 28 seconds; partial text is available throughout a
            # long call, and a GPU failure can be retried without losing capture.
            for window, offset in windows:
                duration = len(window) / 16000
                kwargs = {'return_timestamps': True, 'word_timestamps': True, 'task': 'transcribe', 'num_beams': 1,
                          'repetition_penalty': 1.05, 'no_repeat_ngram_size': 12,
                          'max_new_tokens': min(440, max(64, int(duration * 14) + 32))}
                if language: kwargs['language'] = f'<|{language}|>'
                if options.get('hotwords'): kwargs['hotwords'] = options['hotwords']
                result = self.pipeline.generate(window, **kwargs)
                quality = None
                if transcript_quality.suspicious_decode(result.words or [], duration):
                    # Retry this window once on the same GPU, not the whole call
                    # on CPU. No global language/model change and no extra model.
                    result = self.pipeline.generate(window, **{**kwargs, 'repetition_penalty': 1.12, 'no_repeat_ngram_size': 8})
                    quality = transcript_quality.RETRY_WARNING
                words = [SimpleNamespace(start=timeline.get_original_time(offset + min(duration, max(0., w.start_ts))),
                          end=timeline.get_original_time(offset + min(duration, max(0., w.start_ts, w.end_ts)), is_end=True), word=w.word,
                          quality=quality) for w in result.words or []]
                if result.texts and result.texts[0].strip() and not words:
                    raise RuntimeError('OpenVINO non ha restituito i timestamp delle parole')
                yield SimpleNamespace(start=timeline.get_original_time(offset),
                    end=timeline.get_original_time(offset + duration, is_end=True), words=words, text=''.join(w.word for w in words))
        return generate(), SimpleNamespace(duration=len(samples) / 16000)

class CppModel:
    def __init__(self, settings, plan):
        self.parlo_plan, self.root, self.mid = plan, root_for(settings), model_id(settings)
        self.temporary = settings.get('_temporary_dir', str(self.root))
        self.binary = self.root / 'whisper-cpp' / ('whisper-cli.exe' if os.name == 'nt' else 'whisper-cli')
        if not self.binary.is_file(): raise RuntimeError('Runtime whisper.cpp non installato')

    def transcribe(self, samples, **options):
        import numpy as np
        windows, timeline = speech_windows(samples, options)
        def generate():
            with tempfile.TemporaryDirectory(prefix='parlo-', dir=self.temporary) as folder:
                path, output = Path(folder) / 'window.wav', Path(folder) / 'output'
                total = 0
                with wave.open(str(path), 'wb') as wav:
                    wav.setparams((1, 2, 16000, 0, 'NONE', 'not compressed'))
                    for window, offset in windows:
                        wav.writeframes((np.clip(window, -1, 1) * 32767).astype('<i2').tobytes())
                        total += len(window)
                if not total: return
                command = [str(self.binary), '-m', str(self.root / 'models' / f'ggml-{self.mid}.bin'),
                            '-f', str(path), '-of', str(output), '-ojf', '-l', options.get('language') or 'auto',
                            '-t', str(self.parlo_plan['threads']), '-bs', '1', '-bo', '1', '-mc', '224' if options.get('hotwords') else '0']
                if options.get('hotwords'): command.extend(['--prompt', options['hotwords'], '--carry-initial-prompt'])
                cpu = self.parlo_plan['device'] == 'cpu'
                if cpu: command.append('-ng')
                result = subprocess.run(command, capture_output=True, encoding='utf-8', errors='replace', timeout=max(180, int(total / 16000 * 2)),
                            creationflags=0x08000000 if os.name == 'nt' else 0,
                            env={**os.environ, **({} if cpu else {'GGML_VK_VISIBLE_DEVICES': self.parlo_plan['device']})})
                if result.returncode: raise RuntimeError('whisper.cpp GPU: ' + result.stderr[-1200:])
                if not cpu and ('whisper_backend_init_gpu: using ' not in result.stderr or 'failed to initialize' in result.stderr):
                    raise RuntimeError('whisper.cpp non ha attivato il backend GPU')
                data = json.loads(output.with_suffix('.json').read_text(encoding='utf-8'))
                for segment in data['transcription']:
                    words = []
                    for token in segment.get('tokens', []):
                        if token.get('id', 50257) >= 50257: continue
                        timing = token.get('offsets', segment['offsets'])
                        words.append(SimpleNamespace(start=timeline.get_original_time(max(0, timing['from']) / 1000),
                            end=timeline.get_original_time(max(0, timing['to']) / 1000, is_end=True), word=token['text']))
                    if segment['text'].strip() and not words:
                        raise RuntimeError('whisper.cpp non ha restituito i timestamp dei token')
                    yield SimpleNamespace(start=timeline.get_original_time(segment['offsets']['from'] / 1000),
                        end=timeline.get_original_time(segment['offsets']['to'] / 1000, is_end=True), words=words, text=segment['text'])
        return generate(), SimpleNamespace(duration=len(samples) / 16000)

def release():
    gc.collect()
