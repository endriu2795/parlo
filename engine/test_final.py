import tempfile
import time
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock, patch
import worker


class FinalPipelineTests(unittest.TestCase):
    def test_two_sources_use_one_inference_and_only_channel_labels(self):
        import numpy as np
        events = []
        activity = SimpleNamespace(speaker=lambda start, end: 'Tu' if start < 2 else 'Speaker')
        samples = np.ones(48000, dtype=np.float32)
        words = [SimpleNamespace(start=1., end=1.5, word=' Ciao.'), SimpleNamespace(start=2., end=2.5, word=' Risposta.')]
        inference = Mock(return_value=(iter([SimpleNamespace(end=3., words=words)]), SimpleNamespace(duration=3.)))
        with patch.object(worker, 'model_for', return_value=object()), \
            patch.object(worker, 'mixed_audio', return_value=(samples, activity)), \
            patch.object(worker, 'final_segments', inference), \
            patch.object(worker, 'emit', side_effect=lambda kind, **values: events.append((kind, values))):
            result = worker.run({'diarization': True}, Path('synthetic'))  # Legacy option is ignored.
        self.assertEqual(inference.call_count, 1)
        self.assertEqual(inference.call_args.args[2], 'mixed')
        self.assertEqual([s['speaker'] for s in result], ['Tu', 'Speaker'])
        self.assertEqual([s['text'] for s in result], ['Ciao.', 'Risposta.'])
        partials = [e[1]['segments'] for e in events if e[0] == 'final_partial']
        self.assertEqual(partials[-1], result)
        self.assertTrue(any('progresso fase' in e[1].get('text', '') for e in events))
    def test_progress_keeps_reporting_while_inference_has_no_segments(self):
        events = []
        with patch.object(worker, "emit", side_effect=lambda kind, **values: events.append(values)):
            with worker.Progress() as progress:
                progress.update("Trascrizione della call…")
                time.sleep(2.1)
        self.assertGreaterEqual(len(events), 2)
        self.assertIn("trascorsi 00:02", events[-1]["text"])
        self.assertIn("Trascrizione della call", events[-1]["text"])
        self.assertFalse(progress.thread.is_alive())

    def test_native_power_request_is_released_on_failure(self):
        kernel = SimpleNamespace(GetCurrentProcess=Mock(return_value=42),
            SetProcessInformation=Mock(return_value=1), SetThreadExecutionState=Mock(return_value=1))
        with patch.object(worker.os, "name", "nt"), patch.object(worker.ctypes, "WinDLL", create=True, return_value=kernel):
            with self.assertRaises(RuntimeError):
                with worker.inference_power():
                    raise RuntimeError("synthetic error")
        self.assertEqual([c.args[0] for c in kernel.SetThreadExecutionState.call_args_list], [0x80000001, 0x80000000])
        self.assertEqual(kernel.SetProcessInformation.call_args.args[1], 4)

    def test_memory_pressure_keeps_batches_small_and_preview_threads_bounded(self):
        self.assertEqual(worker.batch_size(1024**3), 1)
        self.assertEqual(worker.batch_size(4 * 1024**3), 2)
        self.assertEqual(worker.batch_size(8 * 1024**3), 8)
        self.assertEqual(worker.batch_size(8 * 1024**3, 64 * 1024**3), 1)
        self.assertEqual(worker.batch_size(0), 1)
        with patch.object(worker.os, "cpu_count", return_value=16), patch('psutil.cpu_count', return_value=8):
            self.assertEqual(worker.cpu_threads(), 8)
            self.assertEqual(worker.cpu_threads(True), 2)
        with patch.object(worker.os, "cpu_count", return_value=1):
            self.assertEqual(worker.cpu_threads(), 1)

    def test_cuda_batch_is_also_bounded_by_system_ram(self):
        plan = {'device': 'cuda'}
        with patch.object(worker, 'available_memory', return_value=1024**3), \
            patch.object(worker.acceleration, 'memory_total', return_value=16 * 1024**3), \
            patch.object(worker.acceleration, 'cuda_batch', return_value=16):
            self.assertEqual(worker.inference_batch(plan), 1)

    def test_batch_rechecks_memory_between_inferences_and_preserves_order(self):
        from faster_whisper import BatchedInferencePipeline
        import numpy as np
        pipeline = worker.adaptive_pipeline(SimpleNamespace(parlo_plan={'device': 'cpu'}))
        batches = []
        def forward(_pipeline, features, tokenizer, metadata, options):
            batches.append(features.tolist())
            self.assertEqual(features.tolist(), metadata)
            return metadata
        with patch.object(worker, 'inference_batch', side_effect=[4, 1, 2, 1]), \
            patch.object(BatchedInferencePipeline, 'forward', forward):
            result = pipeline.forward(np.arange(8), None, list(range(8)), None)
        self.assertEqual(batches, [[0, 1, 2, 3], [4], [5, 6], [7]])
        self.assertEqual(result, list(range(8)))

    def test_memory_failure_retries_only_unfinished_batch_and_restores_timing(self):
        from faster_whisper import BatchedInferencePipeline
        import numpy as np
        pipeline = worker.adaptive_pipeline(SimpleNamespace(parlo_plan={'device': 'cuda'}))
        batches = []
        def forward(current, features, tokenizer, metadata, options):
            batches.append(features.tolist())
            if len(features) > 2 and metadata[0] >= 4:
                current.last_speech_timestamp = 999.
                raise RuntimeError('CUDA out of memory')
            self.assertNotEqual(current.last_speech_timestamp, 999.)
            current.last_speech_timestamp = float(metadata[-1])
            return metadata
        with patch.object(worker, 'inference_batch', return_value=4), \
            patch.object(BatchedInferencePipeline, 'forward', forward), \
            patch.object(worker.acceleration, 'release'), patch.object(worker, 'emit'):
            result = pipeline.forward(np.arange(8), None, list(range(8)), None)
        self.assertEqual(batches, [[0, 1, 2, 3], [4, 5, 6, 7], [4, 5], [6, 7]])
        self.assertEqual(result, list(range(8)))

    def test_batch_does_not_hide_non_memory_errors_or_retry_size_one(self):
        from faster_whisper import BatchedInferencePipeline
        import numpy as np
        for size, message in [(4, 'decoder failure'), (1, 'CUDA out of memory')]:
            pipeline = worker.adaptive_pipeline(SimpleNamespace(parlo_plan={'device': 'cuda'}))
            with patch.object(worker, 'inference_batch', return_value=size), \
                patch.object(BatchedInferencePipeline, 'forward', side_effect=RuntimeError(message)) as forward:
                with self.assertRaisesRegex(RuntimeError, message):
                    pipeline.forward(np.arange(4), None, list(range(4)), None)
                self.assertEqual(forward.call_count, 1)

    def test_mac_idle_sleep_assertion_released_even_when_inference_fails(self):
        awake = Mock()
        with patch.object(worker.os, 'name', 'posix'), patch.object(worker.sys, 'platform', 'darwin'), patch.object(worker.subprocess, 'Popen', return_value=awake) as spawn:
            with self.assertRaises(RuntimeError):
                with worker.inference_power():
                    raise RuntimeError('synthetic failure')
        self.assertEqual(spawn.call_args.args[0][:3], ['/usr/bin/caffeinate', '-i', '-w'])
        awake.terminate.assert_called_once()
        awake.wait.assert_called_once()

if __name__ == "__main__":
    unittest.main()
