import json
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock, patch
import numpy as np
import worker
import acceleration


class DictionaryTests(unittest.TestCase):
    def setUp(self):
        self.settings = {'language': 'it', 'dictionary': [{'term': 'PARLO'}]}

    def test_dictionary_provides_only_recognition_hints(self):
        self.assertEqual(worker.dictionary_options({}), {})
        self.assertEqual(worker.dictionary_options({'dictionary': [{'term': ' PARLO '}, {'term': 'API'}]}), {'hotwords': 'PARLO, API'})
        model = SimpleNamespace(parlo_plan={'device': 'cpu', 'label': 'CPU'})
        word = SimpleNamespace(start=0., end=1., word=' Presenzio')
        segments = iter([SimpleNamespace(end=1., words=[word])])
        activity = SimpleNamespace(speaker=lambda *args: 'Tu')
        with patch.object(worker, 'mixed_audio', return_value=(np.ones(16000, dtype=np.float32), activity)), patch.object(worker, 'model_for', return_value=model), patch.object(worker, 'final_segments', return_value=(segments, SimpleNamespace(duration=1.))), patch.object(worker, 'emit'):
            result = worker.run_final(self.settings, Path('synthetic'), SimpleNamespace(update=lambda *args: None))
        self.assertEqual(result[0]['text'], 'Presenzio')
        self.assertNotIn('original_text', result[0])

    def test_final_inference_receives_hotwords(self):
        model = SimpleNamespace(transcribe=Mock(return_value=([], None)), parlo_plan={'backend': 'faster', 'device': 'cpu'})
        with patch.object(worker, 'available_memory', return_value=0):
            worker.final_segments(model, np.zeros(10, dtype=np.float32), 'mixed', self.settings)
        self.assertEqual(model.transcribe.call_args.kwargs['hotwords'], 'PARLO')

    def test_openvino_receives_hotwords_without_reading_a_model(self):
        model = acceleration.OpenVINOModel.__new__(acceleration.OpenVINOModel)
        model.pipeline = SimpleNamespace(generate=Mock(return_value=SimpleNamespace(words=[], texts=[''])))
        timeline = SimpleNamespace(get_original_time=lambda value, **kwargs: value)
        with patch.object(acceleration, 'speech_windows', return_value=([(np.zeros(16000, dtype=np.float32), 0)], timeline)), patch.object(acceleration.transcript_quality, 'suspicious_decode', return_value=False):
            segments, _ = model.transcribe(np.zeros(16000, dtype=np.float32), hotwords='PARLO', language='it')
            list(segments)
        self.assertEqual(model.pipeline.generate.call_args.kwargs['hotwords'], 'PARLO')

    def test_cpp_prompt_is_not_disabled_by_zero_context(self):
        model = acceleration.CppModel.__new__(acceleration.CppModel)
        with tempfile.TemporaryDirectory() as directory:
            model.root = Path(directory)
            model.mid, model.temporary, model.binary = 'synthetic', directory, model.root / 'synthetic-cli'
            model.parlo_plan = {'device': 'cpu', 'threads': 1}
            timeline = SimpleNamespace(get_original_time=lambda value, **kwargs: value)
            def command(args, **kwargs):
                output = Path(args[args.index('-of')+1]).with_suffix('.json')
                output.write_text(json.dumps({'transcription': []}), encoding='utf-8')
                return SimpleNamespace(returncode=0, stderr='')
            with patch.object(acceleration, 'speech_windows', return_value=([(np.zeros(160, dtype=np.float32), 0)], timeline)), patch.object(acceleration.subprocess, 'run', side_effect=command) as run:
                segments, _ = model.transcribe(np.zeros(160, dtype=np.float32), hotwords='PARLO')
                list(segments)
            args = run.call_args.args[0]
            self.assertEqual(args[args.index('--prompt')+1], 'PARLO')
            self.assertNotEqual(args[args.index('-mc')+1], '0')


if __name__ == '__main__':
    unittest.main()
