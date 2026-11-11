import json
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock, patch
import numpy as np
import acceleration as a
import worker

class AccelerationTests(unittest.TestCase):
    def test_windows_vulkan_has_verified_faster_cpu_fallback_instead_of_generic_cpp_cpu(self):
        (self.root/'whisper-cpp').mkdir()
        (self.root/'whisper-cpp/runtime.json').write_text('{"backend":"vulkan"}')
        (self.root/'models').mkdir()
        (self.root/'models/ggml-turbo.bin').touch()
        (self.root/'models/whisper-turbo').mkdir()
        (self.root/'models/whisper-turbo/model.bin').touch()
        hw={'gpus':[],'vulkan':[{'device':'0','name':'Test Vulkan'}]}
        with patch.object(a.os,'name','nt'),patch.object(a,'hardware',return_value=hw):
            plans=a.candidates(self.settings)
            self.assertEqual([(p['backend'],p['device']) for p in plans],[('cpp','0'),('faster','cpu')])
            self.assertEqual([p['backend'] for p in a.candidates({**self.settings,'compute_mode':'cpu'})],['faster'])
    def test_native_only_openvino_uses_cpu_without_downloading_another_format(self):
        path = self.root / 'models/whisper-turbo-openvino'
        path.mkdir(parents=True)
        (path / 'presentio-model.json').write_text('{}')
        (path / 'openvino_encoder_model.xml').touch()
        with patch.object(a, 'hardware', return_value={'gpus': [{'backend': 'openvino', 'device': 'GPU', 'name': 'Intel'}]}):
            self.assertEqual([p['backend'] for p in a.candidates(self.settings)], ['openvino', 'openvino'])
            self.assertEqual([p['device'] for p in a.candidates({**self.settings, 'compute_mode': 'cpu'})], ['CPU'])
        with patch.object(a, 'selected_candidates', return_value=[{'backend': 'openvino', 'device': 'CPU', 'threads': 4, 'label': 'CPU'}]) as plans, patch.object(a, 'load', return_value=SimpleNamespace()) , patch.object(worker, 'emit'):
            worker.model_for(self.settings, force_cpu=True)
            self.assertFalse(plans.call_args.args[0]['_force_faster'])

    def setUp(self):
        self.folder = tempfile.TemporaryDirectory()
        self.root = Path(self.folder.name)
        self.settings = {'whisper_model': str(self.root / 'models/whisper-turbo'), 'compute_mode': 'auto', 'language': 'en'}
        self.environment = patch.dict(a.os.environ, {'CALLNOTES_ENGINE_DIR': str(self.root)})
        self.environment.start()
    def tearDown(self):
        self.environment.stop()
        self.folder.cleanup()
        worker._models.clear()

    def test_auto_gpu_detection_cpu_override_and_preview(self):
        path = self.root / 'models/whisper-turbo-openvino'
        path.mkdir(parents=True)
        (path / 'presentio-model.json').write_text('{}')
        hw = {'gpus': [{'backend': 'cuda', 'device': '0', 'name': 'NVIDIA'}, {'backend': 'openvino', 'device': 'GPU', 'name': 'Intel'}]}
        with patch.object(a, 'hardware', return_value=hw):
            plans = a.candidates(self.settings)
            self.assertEqual([p['backend'] for p in plans], ['faster', 'openvino', 'openvino', 'faster'])
            self.assertEqual([p['device'] for p in a.candidates({**self.settings, 'compute_mode': 'cpu'})], ['CPU', 'cpu'])
            self.assertEqual([p['device'] for p in a.candidates(self.settings, live=True)], ['cpu'])
            self.assertEqual(a.candidates({**self.settings, 'whisper_model': '/custom/model'})[-1]['backend'], 'faster')

    def test_calibration_can_choose_cpu_and_invalidates_on_hardware_change(self):
        plans = [{'backend':'faster','device':'cuda','index':0,'threads':8}, {'backend':'faster','device':'cpu','threads':10}]
        cpu = {**plans[1], 'threads':4}
        profile = {'version':a.PROFILE_VERSION,'fingerprint':'one','models':{'turbo':{'selected':a.candidate_key(cpu),'plan':cpu}}}
        (self.root / 'hardware-profile.json').write_text(json.dumps(profile))
        with patch.object(a, 'hardware', return_value={'fingerprint':'one'}), patch.object(a, 'candidates', side_effect=lambda *args: [dict(p) for p in plans]):
            selected = a.selected_candidates(self.settings)
            self.assertEqual(selected[0]['device'], 'cpu')
            self.assertEqual(selected[0]['threads'], 4)
        with patch.object(a, 'hardware', return_value={'fingerprint':'changed'}):
            self.assertIsNone(a.profile_for(self.settings))

    def test_small_utterances_pack_without_losing_original_timestamps(self):
        samples = np.arange(6 * 16000, dtype=np.float32)
        intervals = [{'start':16000,'end':32000}, {'start':64000,'end':80000}]
        with patch('faster_whisper.vad.get_speech_timestamps', return_value=intervals):
            windows, timeline = a.speech_windows(samples, {})
            windows = list(windows)
        self.assertEqual(len(windows), 1)
        np.testing.assert_array_equal(windows[0][0], np.concatenate((samples[16000:32000],samples[64000:80000])))
        self.assertEqual(timeline.get_original_time(.5), 1.5)
        self.assertEqual(timeline.get_original_time(1.5), 4.5)
        self.assertEqual(timeline.get_original_time(1., is_end=True), 2.)
        windows, _ = a.speech_windows(np.ones(70*16000,dtype=np.float32), {'vad_filter':False})
        self.assertEqual([len(w) // 16000 for w, _ in windows], [28,28,14])

    def test_gpu_load_failure_falls_back_with_same_model(self):
        gpu={'backend':'faster','device':'cuda','index':0,'threads':4,'label':'GPU'}
        cpu={'backend':'faster','device':'cpu','threads':4,'label':'CPU'}
        loaded = SimpleNamespace(parlo_plan=cpu)
        with patch.object(a,'selected_candidates',return_value=[gpu,cpu]), patch.object(a,'load',side_effect=[RuntimeError('missing CUDA'),loaded]) as load, patch.object(worker,'emit'):
            self.assertIs(worker.model_for(self.settings),loaded)
        self.assertEqual(load.call_args_list[0].args[0]['whisper_model'], load.call_args_list[1].args[0]['whisper_model'])

    def test_cached_cpp_uses_the_next_calls_temporary_directory(self):
        plan={'backend':'cpp','device':'0','threads':4,'label':'GPU'}
        model=SimpleNamespace(temporary='old-call')
        with patch.object(a,'selected_candidates',return_value=[plan]),patch.object(a,'load',return_value=model),patch.object(worker,'emit'):
            worker.model_for({**self.settings,'_temporary_dir':'call-one'})
            self.assertIs(worker.model_for({**self.settings,'_temporary_dir':'call-two'}),model)
        self.assertEqual(model.temporary,'call-two')

    def test_inference_failure_replaces_partial_text_before_cpu_retry(self):
        gpu = SimpleNamespace(parlo_plan={'backend':'openvino','device':'GPU','label':'Intel'})
        cpu = SimpleNamespace(parlo_plan={'backend':'faster','device':'cpu','label':'CPU'})
        def broken():
            yield SimpleNamespace(end=1.,words=[SimpleNamespace(start=0.,end=1.,word=' A')])
            raise RuntimeError('GPU memory')
        final = iter([SimpleNamespace(end=2.,words=[SimpleNamespace(start=0.,end=2.,word=' Complete')])])
        events=[]
        with patch.object(worker,'mixed_audio',return_value=(np.ones(32000,dtype=np.float32),SimpleNamespace(speaker=lambda *_:'Tu'))), \
            patch.object(worker,'model_for',side_effect=[gpu,cpu]) as load, \
            patch.object(worker,'final_segments',side_effect=[(broken(),SimpleNamespace(duration=2.)),(final,SimpleNamespace(duration=2.))]), \
            patch.object(worker,'emit',side_effect=lambda kind,**data:events.append((kind,data))):
            result = worker.run(self.settings,self.root)
        self.assertEqual(result[0]['text'],'Complete')
        self.assertTrue(load.call_args.kwargs['force_cpu'])
        self.assertIn(('final_partial',{'segments':[]}),events)

    def test_cuda_batch_uses_free_vram_and_handles_missing_driver_tool(self):
        with patch.object(a.subprocess,'run',return_value=SimpleNamespace(stdout='7000, 8192\n2000, 8192\n')):
            self.assertEqual(a.cuda_batch({'index':0}),8)
            self.assertEqual(a.cuda_batch({'index':1}),1)
        with patch.object(a.subprocess,'run',return_value=SimpleNamespace(stdout='20000, 24576\n4000, 24576\n')):
            self.assertEqual(a.cuda_batch({'index':0}),16)
            self.assertEqual(a.cuda_batch({'index':1}),1)
        with patch.object(a.subprocess,'run',return_value=SimpleNamespace(stdout='N/A, N/A')):
            self.assertEqual(a.cuda_batch({}),1)
        with patch.object(a.subprocess,'run',side_effect=FileNotFoundError):
            self.assertEqual(a.cuda_batch({}),1)

if __name__ == '__main__': unittest.main()
