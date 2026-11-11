"""Regression checks for the shipped voice/runtime contents; no builds or downloads."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import runpy
import tarfile
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('prepare_runtime', Path(__file__).with_name('prepare-runtime.py'))
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)


class PreparedRuntimeTests(unittest.TestCase):
    def test_removing_pip_preserves_piper_and_its_package_metadata(self):
        with tempfile.TemporaryDirectory(prefix='parlo-runtime-test-') as temporary:
            site = Path(temporary) / 'site-packages'
            for name in ('pip', 'pip-25.0.dist-info', 'setuptools', 'setuptools-80.0.dist-info',
                         'piper', 'piper_tts-1.8.0.dist-info'):
                (site / name).mkdir(parents=True)
                (site / name / 'sentinel').write_text(name)
            runtime.remove_build_tools(site)
            self.assertEqual({path.name for path in site.iterdir()}, {'piper', 'piper_tts-1.8.0.dist-info'})
            self.assertEqual((site / 'piper/sentinel').read_text(), 'piper')

    def test_matching_manifest_cannot_hide_missing_piper_in_cached_archive(self):
        with tempfile.TemporaryDirectory(prefix='parlo-runtime-test-') as temporary:
            root = Path(temporary)
            packed, manifest = root / 'runtime.tar.gz', root / 'manifest.json'
            identity = {'version': 2, 'platform': 'win-x64', 'validation_sha256': 'probe-v2'}
            for modules, expected in [(('docx',), False), (('docx', 'piper'), True)]:
                with tarfile.open(packed, 'w:gz') as archive:
                    for module in modules:
                        content = b'# synthetic module, never executed'
                        entry = tarfile.TarInfo(f'python/Lib/site-packages/{module}/__init__.py')
                        entry.size = len(content)
                        archive.addfile(entry, io.BytesIO(content))
                manifest.write_text(json.dumps({**identity, 'sha256': runtime.digest(packed)}))
                self.assertEqual(runtime.cached_runtime_matches(manifest, packed, identity, ('docx', 'piper')), expected)
            stale = {**identity, 'version': 1}
            manifest.write_text(json.dumps({**stale, 'sha256': runtime.digest(packed)}))
            self.assertFalse(runtime.cached_runtime_matches(manifest, packed, identity, ('docx', 'piper')))

    def test_shared_probe_reports_missing_dependency_without_raw_exception_details(self):
        probe = runpy.run_path(str(Path(__file__).resolve().parents[1] / 'engine/document_runtime_probe.py'))
        def importing(name):
            if name == 'piper':
                raise ModuleNotFoundError('private detail must not be copied to diagnostics')
        output = io.StringIO()
        with patch('importlib.import_module', side_effect=importing), contextlib.redirect_stdout(output):
            self.assertEqual(probe['check'](), 1)
        self.assertEqual(json.loads(output.getvalue()), {'ready': False, 'module': 'piper', 'error_type': 'ModuleNotFoundError'})


if __name__ == '__main__':
    unittest.main()
