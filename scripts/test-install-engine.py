"""Provisioning lifecycle tests; no models, network or personal audio."""
import importlib.util
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('install_engine', Path(__file__).with_name('install-engine.py'))
installer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(installer)


class EnvironmentTests(unittest.TestCase):
    def test_locked_legacy_environment_is_not_removed(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            environment = root / 'venv'
            environment.mkdir()
            (environment / 'previous').touch()
            original = Path.rename

            def rename(path, target):
                if path == environment:
                    raise PermissionError('environment locked')
                return original(path, target)

            with patch.object(installer, 'uses_current_runtime', return_value=False), patch.object(Path, 'rename', rename):
                with self.assertRaises(PermissionError):
                    with installer.engine_environment(root):
                        self.fail('Must not replace a locked environment')
            self.assertTrue((environment / 'previous').exists())
            self.assertFalse((root / 'venv-installing.json').exists())

    def test_interrupted_migration_is_restored_before_retry(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'venv').mkdir()
            (root / 'venv/incomplete').touch()
            backup = root / '.venv-previous-test'
            backup.mkdir()
            (backup / 'previous').touch()
            journal = root / 'venv-installing.json'
            journal.write_text('{"backup":".venv-previous-test"}')
            installer.recover_interrupted_install(root, root / 'venv', journal)
            self.assertTrue((root / 'venv/previous').exists())
            self.assertFalse((root / 'venv/incomplete').exists())
            self.assertFalse(journal.exists())

    def test_journal_cannot_reference_external_backup(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            journal = root / 'venv-installing.json'
            journal.write_text('{"backup":"../.venv-previous-outside"}')
            with self.assertRaises(RuntimeError):
                installer.recover_interrupted_install(root, root / 'venv', journal)
            self.assertTrue(journal.exists())

    def test_clean_install_uses_included_python_and_path_with_spaces(self):
        with tempfile.TemporaryDirectory(prefix='PARLO setup ') as temp:
            root = Path(temp)
            with patch.object(installer, 'uses_current_runtime', return_value=False), patch.object(installer.subprocess, 'run') as run:
                with installer.engine_environment(root) as python:
                    self.assertEqual(python, installer.executable(root / 'venv'))
                run.assert_called_once_with([installer.sys.executable, '-I', '-m', 'venv', str(root / 'venv')], check=True)

    def test_legacy_environment_restored_after_pip_or_model_failure(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'venv').mkdir()
            (root / 'venv/previous').write_text('working engine')

            def create(*args, **kwargs):
                (root / 'venv').mkdir()
                (root / 'venv/new').write_text('incomplete engine')

            with patch.object(installer, 'uses_current_runtime', return_value=False), patch.object(installer.subprocess, 'run', side_effect=create):
                with self.assertRaisesRegex(RuntimeError, 'download failed'):
                    with installer.engine_environment(root):
                        raise RuntimeError('download failed')
            self.assertEqual((root / 'venv/previous').read_text(), 'working engine')
            self.assertFalse((root / 'venv/new').exists())
            self.assertFalse(list(root.glob('.venv-previous-*')))

    def test_legacy_environment_restored_after_venv_creation_failure(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'venv').mkdir()
            (root / 'venv/previous').touch()
            with patch.object(installer, 'uses_current_runtime', return_value=False), patch.object(installer.subprocess, 'run', side_effect=subprocess.CalledProcessError(1, 'venv')):
                with self.assertRaises(subprocess.CalledProcessError):
                    with installer.engine_environment(root):
                        self.fail('Must not reach dependency installation')
            self.assertTrue((root / 'venv/previous').exists())

    def test_valid_environment_reused_without_recreation(self):
        with tempfile.TemporaryDirectory() as temp:
            with patch.object(installer, 'uses_current_runtime', return_value=True), patch.object(installer.subprocess, 'run') as run:
                with installer.engine_environment(Path(temp)):
                    pass
                run.assert_not_called()

    def test_successful_migration_removes_only_previous_environment(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'venv').mkdir()
            (root / 'venv/previous').touch()
            (root / 'models').mkdir()
            (root / 'models/model.bin').write_bytes(b'preserved')
            with patch.object(installer, 'uses_current_runtime', return_value=False), patch.object(installer.subprocess, 'run', side_effect=lambda *a, **k: (root / 'venv').mkdir()):
                with installer.engine_environment(root):
                    (root / 'venv/new').touch()
            self.assertTrue((root / 'venv/new').exists())
            self.assertFalse(list(root.glob('.venv-previous-*')))
            self.assertEqual((root / 'models/model.bin').read_bytes(), b'preserved')


@unittest.skipUnless(installer.os.name == 'nt', 'Windows dependency bundle')
class WheelhouseTests(unittest.TestCase):
    def fixture(self, root):
        requirements = root / 'requirements.txt'
        requirements.write_bytes(b'example==1.0\n')
        folder = root / 'wheels'
        folder.mkdir()
        wheel = folder / 'example-1.0-py3-none-any.whl'
        wheel.write_bytes(b'fixture wheel bytes')
        manifest = {'version':1,'platform':'win-x64',
            'requirements_sha256':hashlib.sha256(requirements.read_bytes()).hexdigest(),
            'files':[{'name':wheel.name,'size':wheel.stat().st_size,
                'sha256':hashlib.sha256(wheel.read_bytes()).hexdigest()}]}
        (folder/'manifest.json').write_text(json.dumps(manifest), encoding='utf-8')
        return folder, requirements, wheel, manifest

    def test_valid_bundle_and_corrupt_wheel(self):
        with tempfile.TemporaryDirectory() as temp:
            folder, requirements, wheel, _ = self.fixture(Path(temp))
            with patch.object(installer.os, 'name', 'nt'):
                installer.validate_wheelhouse(folder, requirements)
                wheel.write_bytes(b'corrupted wheel data')
                with self.assertRaisesRegex(RuntimeError, 'danneggiata'):
                    installer.validate_wheelhouse(folder, requirements)

    def test_modified_requirements_extra_file_and_path_escape_rejected(self):
        for change in ['requirements', 'extra', 'escape']:
            with self.subTest(change=change), tempfile.TemporaryDirectory() as temp:
                folder, requirements, _, manifest = self.fixture(Path(temp))
                if change == 'requirements': requirements.write_bytes(b'example==2.0\n')
                elif change == 'extra': (folder/'extra.whl').write_bytes(b'extra')
                else:
                    manifest['files'][0]['name'] = '../outside.whl'
                    (folder/'manifest.json').write_text(json.dumps(manifest), encoding='utf-8')
                with patch.object(installer.os, 'name', 'nt'), self.assertRaises(RuntimeError):
                    installer.validate_wheelhouse(folder, requirements)

    def test_bundle_mismatch_is_rejected_before_environment_changes(self):
        with tempfile.TemporaryDirectory() as temp:
            folder, _, _, _ = self.fixture(Path(temp))
            with patch.object(installer.sys, 'argv', ['install-engine.py','--engine',temp,'--wheelhouse',str(folder)]), patch.object(installer, 'engine_environment') as environment:
                with self.assertRaises(RuntimeError): installer.main()
                environment.assert_not_called()


if __name__ == '__main__':
    unittest.main()
