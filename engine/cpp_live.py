"""Persistent whisper.cpp 1.8.3 context for short GPU preview windows.

The ABI below matches the pinned include/whisper.h shipped by our build scripts.
Keep native inference inside the supervised Python worker, never the UI process.
"""
import ctypes as c
import os
import sys
from pathlib import Path
from types import SimpleNamespace


class Heads(c.Structure):
    _fields_ = [('count', c.c_size_t), ('heads', c.c_void_p)]


class ContextParams(c.Structure):
    _fields_ = [('use_gpu', c.c_bool), ('flash_attn', c.c_bool), ('gpu_device', c.c_int),
                ('dtw_token_timestamps', c.c_bool), ('dtw_aheads_preset', c.c_int),
                ('dtw_n_top', c.c_int), ('dtw_aheads', Heads), ('dtw_mem_size', c.c_size_t)]


class VadParams(c.Structure):
    _fields_ = [('threshold', c.c_float), ('min_speech_duration_ms', c.c_int),
                ('min_silence_duration_ms', c.c_int), ('max_speech_duration_s', c.c_float),
                ('speech_pad_ms', c.c_int), ('samples_overlap', c.c_float)]


class Greedy(c.Structure):
    _fields_ = [('best_of', c.c_int)]


class Beam(c.Structure):
    _fields_ = [('beam_size', c.c_int), ('patience', c.c_float)]


class FullParams(c.Structure):
    _fields_ = [
        ('strategy', c.c_int), ('n_threads', c.c_int), ('n_max_text_ctx', c.c_int),
        ('offset_ms', c.c_int), ('duration_ms', c.c_int),
        *[(name, c.c_bool) for name in ('translate', 'no_context', 'no_timestamps', 'single_segment',
            'print_special', 'print_progress', 'print_realtime', 'print_timestamps', 'token_timestamps')],
        ('thold_pt', c.c_float), ('thold_ptsum', c.c_float), ('max_len', c.c_int),
        ('split_on_word', c.c_bool), ('max_tokens', c.c_int), ('debug_mode', c.c_bool),
        ('audio_ctx', c.c_int), ('tdrz_enable', c.c_bool), ('suppress_regex', c.c_char_p),
        ('initial_prompt', c.c_char_p), ('carry_initial_prompt', c.c_bool),
        ('prompt_tokens', c.c_void_p), ('prompt_n_tokens', c.c_int), ('language', c.c_char_p),
        ('detect_language', c.c_bool), ('suppress_blank', c.c_bool), ('suppress_nst', c.c_bool),
        *[(name, c.c_float) for name in ('temperature', 'max_initial_ts', 'length_penalty',
            'temperature_inc', 'entropy_thold', 'logprob_thold', 'no_speech_thold')],
        ('greedy', Greedy), ('beam_search', Beam),
        *[(name, c.c_void_p) for name in ('new_segment_callback', 'new_segment_callback_user_data',
            'progress_callback', 'progress_callback_user_data', 'encoder_begin_callback',
            'encoder_begin_callback_user_data', 'abort_callback', 'abort_callback_user_data',
            'logits_filter_callback', 'logits_filter_callback_user_data', 'grammar_rules')],
        ('n_grammar_rules', c.c_size_t), ('i_start_rule', c.c_size_t), ('grammar_penalty', c.c_float),
        ('vad', c.c_bool), ('vad_model_path', c.c_char_p), ('vad_params', VadParams),
    ]


class Token(c.Structure):
    _fields_ = [('id', c.c_int), ('tid', c.c_int), *[(name, c.c_float) for name in ('p', 'plog', 'pt', 'ptsum')],
                ('t0', c.c_int64), ('t1', c.c_int64), ('t_dtw', c.c_int64), ('vlen', c.c_float)]


_dll_dirs = []
_loading_logs = []


@c.CFUNCTYPE(None, c.c_int, c.c_char_p, c.c_void_p)
def native_log(level, text, user):
    message = (text or b'').decode('utf-8', 'replace')
    # Only initialization diagnostics are retained; decoded text is not logged.
    if _loading_logs is not None and len(_loading_logs) < 300:
        _loading_logs.append(message)
    if level <= 3:
        try:
            sys.stderr.write(message)
        except (OSError, UnicodeError):
            pass


class LiveModel:
    def __init__(self, settings, plan):
        import acceleration
        global _loading_logs
        self.ctx = None
        self.parlo_plan = plan
        root = acceleration.root_for(settings)
        folder = root / 'whisper-cpp'
        if os.name == 'nt':
            _dll_dirs.append(os.add_dll_directory(str(folder)))
        path = folder / ('whisper.dll' if os.name == 'nt' else 'libwhisper.dylib')
        self.lib = lib = c.CDLL(str(path))
        def bind(name, result, *args):
            fn = getattr(lib, name)
            fn.restype, fn.argtypes = result, list(args)
            return fn
        version = bind('whisper_version', c.c_char_p)().decode('ascii')
        if version.split('-')[0] != '1.8.3':
            raise RuntimeError('Runtime anteprima non compatibile: aggiorna il runtime whisper.cpp')
        bind('whisper_log_set', None, type(native_log), c.c_void_p)(native_log, None)
        context = bind('whisper_context_default_params', ContextParams)()
        context.use_gpu, context.gpu_device = True, int(plan['device'])
        bind('whisper_free', None, c.c_void_p)
        bind('whisper_full_default_params', FullParams, c.c_int)
        bind('whisper_full', c.c_int, c.c_void_p, FullParams, c.POINTER(c.c_float), c.c_int)
        bind('whisper_full_n_segments', c.c_int, c.c_void_p)
        bind('whisper_full_n_tokens', c.c_int, c.c_void_p, c.c_int)
        bind('whisper_full_get_token_data', Token, c.c_void_p, c.c_int, c.c_int)
        bind('whisper_full_get_token_text', c.c_char_p, c.c_void_p, c.c_int, c.c_int)
        bind('whisper_token_eot', c.c_int, c.c_void_p)
        bind('whisper_full_get_segment_t0', c.c_int64, c.c_void_p, c.c_int)
        bind('whisper_full_get_segment_t1', c.c_int64, c.c_void_p, c.c_int)
        _loading_logs = []
        try:
            self.ctx = bind('whisper_init_from_file_with_params', c.c_void_p, c.c_char_p, ContextParams)(
                os.fsencode(root / 'models' / f'ggml-{acceleration.model_id(settings)}.bin'), context)
            gpu = any('whisper_backend_init_gpu: using ' in message for message in _loading_logs) and not any(
                'failed to initialize' in message for message in _loading_logs)
            if not self.ctx or not gpu:
                self.close()
                raise RuntimeError('La GPU non si è attivata per l’anteprima; nessun passaggio automatico alla CPU')
        finally:
            _loading_logs = None

    def close(self):
        if self.ctx:
            self.lib.whisper_free(self.ctx)
            self.ctx = None

    def __del__(self):
        try:
            self.close()
        except Exception:
            pass

    def transcribe(self, samples, **options):
        import acceleration
        import numpy as np
        windows, timeline = acceleration.speech_windows(samples, options)
        def generate():
            lib, ctx = self.lib, self.ctx
            for window, offset in windows:
                data = np.ascontiguousarray(window, dtype=np.float32)
                duration = len(data) / 16000
                params = lib.whisper_full_default_params(0)
                params.n_threads = self.parlo_plan['threads']
                params.no_context, params.token_timestamps = True, True
                params.print_special = params.print_progress = params.print_realtime = params.print_timestamps = False
                language = (options.get('language') or 'auto').encode('utf-8')
                prompt = (options.get('hotwords') or '').encode('utf-8')
                params.language, params.initial_prompt = language, prompt or None
                params.n_max_text_ctx = 224 if prompt else 0
                params.carry_initial_prompt = bool(prompt)
                params.greedy.best_of = 1
                if lib.whisper_full(ctx, params, data.ctypes.data_as(c.POINTER(c.c_float)), len(data)):
                    raise RuntimeError('Trascrizione GPU dell’anteprima interrotta')
                eot = lib.whisper_token_eot(ctx)
                for segment in range(lib.whisper_full_n_segments(ctx)):
                    start = lib.whisper_full_get_segment_t0(ctx, segment) / 100
                    end = lib.whisper_full_get_segment_t1(ctx, segment) / 100
                    words, pending, token_start = [], b'', start
                    for index in range(lib.whisper_full_n_tokens(ctx, segment)):
                        token = lib.whisper_full_get_token_data(ctx, segment, index)
                        if token.id >= eot: continue
                        if not pending: token_start = token.t0 / 100 if token.t0 >= 0 else start
                        pending += lib.whisper_full_get_token_text(ctx, segment, index) or b''
                        try:
                            text = pending.decode('utf-8')
                        except UnicodeDecodeError:
                            continue
                        token_end = token.t1 / 100 if token.t1 >= 0 else end
                        words.append(SimpleNamespace(
                            start=timeline.get_original_time(offset + min(duration, max(0, token_start))),
                            end=timeline.get_original_time(offset + min(duration, max(0, token_start, token_end)), is_end=True),
                            word=text))
                        pending = b''
                    if pending:
                        words.append(SimpleNamespace(start=timeline.get_original_time(offset + min(duration, max(0, token_start))),
                            end=timeline.get_original_time(offset + min(duration, max(0, end)), is_end=True), word=pending.decode('utf-8', 'replace')))
                    yield SimpleNamespace(start=timeline.get_original_time(offset + min(duration, max(0, start))),
                        end=timeline.get_original_time(offset + min(duration, max(0, end)), is_end=True),
                        words=words, text=''.join(word.word for word in words))
        return generate(), SimpleNamespace(duration=len(samples) / 16000)
