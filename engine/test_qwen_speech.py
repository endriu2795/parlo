"""Synthetic Qwen adapter tests: no model, PyTorch, network or user files."""
import contextlib
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch
import wave

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import qwen_speech as qwen
import qwen_setup as setup
import speech_tools as speech


class QwenTests(unittest.TestCase):
    def tearDown(self):
        qwen._model, qwen._identity = None, None

    def test_long_scripts_preserve_every_character_and_bound_each_generation(self):
        for text in ['  Ciao!\nMondo. ' * 100, 'à' * 1003, '\n\t', '']:
            pieces = qwen.chunks(text)
            self.assertEqual(''.join(pieces), text)
            self.assertTrue(all(0 < len(piece) <= 400 for piece in pieces))

    def test_private_cuda_repair_cannot_replace_cuda_torch_from_pypi(self):
        with tempfile.TemporaryDirectory(prefix='parlo-qwen-setup-') as tmp:
            root = Path(tmp)
            (root / 'requirements.txt').write_text('qwen-tts==0.1.1\n', encoding='utf-8')
            with patch.object(setup, 'run', return_value=b'') as commands, patch.object(setup.subprocess, 'run', return_value=SimpleNamespace(returncode=0)):
                setup.prepare(root, 'cuda', repair=True)
            arguments = [call.args[0] for call in commands.call_args_list]
            self.assertIn('--clear', arguments[0])
            self.assertEqual(Path(arguments[0][-1]), root / 'venv-cuda')
            self.assertIn('https://download.pytorch.org/whl/cu128', arguments[1])
            self.assertIn('https://pypi.org/simple', arguments[2])
            self.assertFalse(any('--force-reinstall' in args for args in arguments))
            self.assertEqual(json.loads((root / 'runtime-cuda.json').read_text())['device'], 'cuda')

    def test_offline_cpu_reuses_model_real_audio_has_no_subtitles_and_cuda_never_falls_back(self):
        class SoundFile:
            def __init__(self, path, mode, samplerate, channels, subtype):
                self.output = wave.open(path, 'wb')
                self.output.setnchannels(channels)
                self.output.setsampwidth(2)
                self.output.setframerate(samplerate)
            def __enter__(self): return self
            def __exit__(self, *args): self.output.close()
            def write(self, samples): self.output.writeframes((samples * 32767).astype('<i2').tobytes())

        model = Mock()
        model.generate_custom_voice.return_value = ([np.zeros(24000, dtype=np.float32)], 24000)
        model_type = Mock()
        model_type.from_pretrained.return_value = model
        torch = SimpleNamespace(cuda=SimpleNamespace(is_available=lambda: False),
            set_num_threads=Mock(), float16='float16', float32='float32',
            inference_mode=contextlib.nullcontext)
        with tempfile.TemporaryDirectory(prefix='parlo-qwen-audio-') as tmp, patch.dict(sys.modules, {
                'torch': torch, 'qwen_tts': SimpleNamespace(Qwen3TTSModel=model_type),
                'soundfile': SimpleNamespace(SoundFile=SoundFile)}):
            root = Path(tmp)
            request = {'provider':'qwen', 'model':'synthetic', 'voice':'ryan', 'text':'Prova sintetica.',
                'qwen_device':'cpu', 'qwen_model':str(root / 'model'), 'qwen_language':'Italian',
                'instruct':'Tono calmo.', 'request_hash':'synthetic'}
            for index in range(2):
                request['output'] = str(root / f'scene-{index}.mp3')
                result = speech.generate(request)
                self.assertAlmostEqual(result['duration_seconds'], 1., delta=.1)
                self.assertFalse(result['word_timings_available'])
                self.assertIsNone(result['subtitles'])
            self.assertEqual(model_type.from_pretrained.call_count, 1)
            options = model_type.from_pretrained.call_args.kwargs
            self.assertTrue(options['local_files_only'])
            self.assertFalse(options['trust_remote_code'])
            self.assertEqual(options['device_map'], 'cpu')
            self.assertEqual(model.generate_custom_voice.call_args.kwargs['instruct'], 'Tono calmo.')
            previous = Path(request['output']).read_bytes()
            request['qwen_device'] = 'cuda'
            with self.assertRaisesRegex(RuntimeError, 'cuda_unavailable'):
                speech.generate(request)
            self.assertEqual(Path(request['output']).read_bytes(), previous)
            self.assertEqual(model_type.from_pretrained.call_count, 1)
            self.assertEqual(list(root.glob('.qwen-*')), [])
            self.assertEqual(list(root.glob('.speech-*')), [])


if __name__ == '__main__':
    unittest.main()
