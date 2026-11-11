"""Regression tests: requested downloads, native CPU fallback and resumable files."""
import hashlib
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch


def load(filename):
    spec = importlib.util.spec_from_file_location(filename.replace('-', '_'), Path(__file__).with_name(filename))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


models = load('setup-engine.py')
accelerator = load('setup-accelerator.py')


class ModelSetupTests(unittest.TestCase):
    def test_default_download_does_not_request_tiny_or_small(self):
        with tempfile.TemporaryDirectory() as temp, patch.object(models, 'load_accelerator') as prepare, patch.object(models, 'ensure_faster_model') as preview:
            models.setup(Path(temp), 'turbo')
            prepare.return_value.setup.assert_called_once_with(Path(temp), 'turbo', compute_mode='auto')
            preview.assert_not_called()

    def test_enabled_preview_downloads_only_its_selected_model(self):
        with tempfile.TemporaryDirectory() as temp, patch.object(models, 'load_accelerator') as prepare, patch.object(models, 'ensure_faster_model') as preview:
            models.setup(Path(temp), 'small', live_model='tiny')
            prepare.return_value.setup.assert_called_once_with(Path(temp), 'small', compute_mode='auto')
            preview.assert_called_once_with(Path(temp), 'tiny')

    def test_preview_selection_does_not_download_final_or_gpu_models(self):
        with tempfile.TemporaryDirectory() as temp, patch.object(models, 'load_accelerator') as prepare, patch.object(models, 'ensure_faster_model') as preview:
            models.setup(Path(temp), 'tiny', preview_only=True)
            prepare.assert_not_called()
            preview.assert_called_once_with(Path(temp), 'tiny')

    def test_existing_model_is_reused_without_network(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            target = root / 'models/whisper-small'
            target.mkdir(parents=True)
            (target / 'model.bin').touch()
            (target / 'config.json').touch()
            (target / 'tokenizer.json').touch()
            with patch('model_download.download_model') as download:
                models.ensure_faster_model(root, 'small')
                download.assert_not_called()

    def test_hardware_downloads_only_one_model_format(self):
        hw = {'gpus': [{'backend': 'openvino'}, {'backend': 'cpp'}]}
        with tempfile.TemporaryDirectory() as temp:
            runtime = Path(temp) / 'runtime.json'
            runtime.touch()
            self.assertEqual(accelerator.preferred_backend(hw, runtime, 'auto'), 'openvino')
            self.assertEqual(accelerator.preferred_backend(hw, runtime, 'cpu'), 'faster')
            self.assertEqual(accelerator.preferred_backend({'gpus': [{'backend': 'cpp'}]}, runtime, 'auto'), 'cpp')

    def test_windows_vulkan_prepares_cpu_adapter_of_same_model_without_extra_models(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)
            (root/'models').mkdir();(root/'models/ggml-turbo.bin').touch()
            hw={'gpus':[{'backend':'cpp'}]}
            runtime=root/'whisper-cpp';runtime.mkdir();(runtime/'runtime.json').write_text('{}')
            with patch.object(accelerator.acceleration,'hardware',return_value=hw), \
                patch.object(accelerator.acceleration,'profile_for',return_value={'selected':'cpp:0'}), \
                patch.dict(accelerator.os.environ,{'PARLO_RUNTIME_BUNDLE':str(root/'no-bundle')}), \
                patch('model_download.ensure_faster_model') as cpu:
                accelerator.setup(root,'turbo')
                if accelerator.os.name=='nt':cpu.assert_called_once_with(root,'turbo')
                else:cpu.assert_not_called()


class ResumableDownloadTests(unittest.TestCase):
    def response(self, data, status=200, content_range=None):
        response = io.BytesIO(data)
        response.status = status
        response.headers = {'Content-Length': str(len(data))}
        if content_range: response.headers['Content-Range'] = content_range
        return response

    def test_interrupted_download_resumes_and_checks_hash(self):
        with tempfile.TemporaryDirectory() as temp:
            target = Path(temp) / 'library.whl'
            target.with_suffix('.whl.part').write_bytes(b'ab')
            with patch.object(accelerator.urllib.request, 'urlopen', return_value=self.response(b'cd', 206, 'bytes 2-3/4')) as request:
                accelerator.download('https://example.test/library', target, 4, hashlib.sha256(b'abcd').hexdigest())
            self.assertEqual(request.call_args.args[0].get_header('Range'), 'bytes=2-')
            self.assertEqual(target.read_bytes(), b'abcd')

    def test_server_ignoring_range_restarts_without_appending(self):
        with tempfile.TemporaryDirectory() as temp:
            target = Path(temp) / 'library.whl'
            target.with_suffix('.whl.part').write_bytes(b'ab')
            with patch.object(accelerator.urllib.request, 'urlopen', return_value=self.response(b'abcd')):
                accelerator.download('https://example.test/library', target, 4, hashlib.sha256(b'abcd').hexdigest())
            self.assertEqual(target.read_bytes(), b'abcd')

    def test_corrupt_download_is_never_published(self):
        with tempfile.TemporaryDirectory() as temp:
            target = Path(temp) / 'library.whl'
            with patch.object(accelerator.urllib.request, 'urlopen', return_value=self.response(b'wrong')), self.assertRaisesRegex(RuntimeError, 'Verifica'):
                accelerator.download('https://example.test/library', target, 5, hashlib.sha256(b'right').hexdigest())
            self.assertFalse(target.exists())
            self.assertFalse(target.with_suffix('.whl.part').exists())

    def test_unverified_partial_is_resumed_instead_of_published(self):
        with tempfile.TemporaryDirectory() as temp:
            target = Path(temp) / 'sample.wav'
            target.with_suffix('.wav.part').write_bytes(b'ab')
            with patch.object(accelerator.urllib.request, 'urlopen', return_value=self.response(b'cd', 206, 'bytes 2-3/4')) as request:
                accelerator.download('https://example.test/sample', target)
            request.assert_called_once()
            self.assertEqual(target.read_bytes(), b'abcd')

    def test_finished_verified_partial_is_published_without_network(self):
        with tempfile.TemporaryDirectory() as temp:
            target = Path(temp) / 'library.whl'
            target.with_suffix('.whl.part').write_bytes(b'abcd')
            with patch.object(accelerator.urllib.request, 'urlopen') as request:
                accelerator.download('https://example.test/library', target, 4, hashlib.sha256(b'abcd').hexdigest())
            request.assert_not_called()
            self.assertEqual(target.read_bytes(), b'abcd')


if __name__ == '__main__':
    unittest.main()
