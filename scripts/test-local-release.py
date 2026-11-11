"""Focused release-flow checks: synthetic packages, no builds or network calls."""
import importlib.util
import json
from pathlib import Path
from subprocess import CompletedProcess
import tempfile
import unittest
from unittest.mock import patch

import release

spec = importlib.util.spec_from_file_location('local_release', Path(__file__).with_name('local-release.py'))
local = importlib.util.module_from_spec(spec)
spec.loader.exec_module(local)
COMMIT = 'a' * 40
TARGETS = ('windows-x86_64', 'darwin-aarch64')


class LocalReleaseTests(unittest.TestCase):
    def setUp(self):
        tools = release.ROOT / '.tools'
        tools.mkdir(exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(prefix='test-local-release-', dir=tools)
        self.root = Path(self.temporary.name).resolve()
        self.assertTrue(self.root.is_relative_to(release.ROOT.resolve()))
        self.addCleanup(self.temporary.cleanup)
        for mock in [patch.object(local, 'ROOT', self.root), patch.object(release, 'ROOT', self.root),
                     patch.object(release, 'ASSETS', self.root / 'assets'),
                     patch.object(release, 'version', return_value='0.2.3'),
                     patch.object(release, 'validate'), patch.object(local, 'REPOSITORY', 'example/parlo-releases'),
                     patch.dict('os.environ', {'RELEASE_REPOSITORY': 'example/parlo-releases'})]:
            mock.start()
            self.addCleanup(mock.stop)

    def fixture(self, folder, target):
        folder.mkdir(parents=True, exist_ok=True)
        names = local.asset_names(target)
        for name in names: (folder / name).write_bytes(b'synthetic package or signature')
        metadata = {'version': '0.2.3', 'platform': target, 'file': names[0],
                    'signature': (folder / names[1]).read_text(), 'source_commit': COMMIT,
                    'source_dirty': False, 'sha256': {name: local.digest(folder / name) for name in names}}
        (folder / f'update-{target}.json').write_text(json.dumps(metadata))
        return metadata

    def test_changed_binary_or_mixed_commit_is_rejected_before_upload(self):
        target = TARGETS[0]
        folder = local.package_folder(COMMIT, target)
        metadata = self.fixture(folder, target)
        metadata['source_commit'] = 'b' * 40
        file = folder / f'update-{target}.json'
        file.write_text(json.dumps(metadata))
        with patch.object(local, 'github_ready') as network:
            with self.assertRaisesRegex(ValueError, 'commit'): local.upload(COMMIT, target)
            network.assert_not_called()
        metadata['source_commit'] = COMMIT
        file.write_text(json.dumps(metadata))
        (folder / metadata['file']).write_bytes(b'changed after signing')
        with patch.object(local, 'github_ready') as network:
            with self.assertRaisesRegex(ValueError, 'modificato'): local.upload(COMMIT, target)
            network.assert_not_called()

    def test_dirty_build_and_unexpected_filenames_are_rejected(self):
        target = TARGETS[0]
        folder = self.root / 'package'
        metadata = self.fixture(folder, target)
        metadata['source_dirty'] = True
        file = folder / f'update-{target}.json'
        file.write_text(json.dumps(metadata))
        with self.assertRaisesRegex(ValueError, 'committate'): local.validate_package(folder, target, COMMIT)
        metadata['source_dirty'] = False
        metadata['file'] = '../private-source.zip'
        file.write_text(json.dumps(metadata))
        with self.assertRaises(ValueError): local.validate_package(folder, target, COMMIT)

    def test_upload_only_adds_allowlisted_assets_to_matching_draft(self):
        target = TARGETS[0]
        folder = local.package_folder(COMMIT, target)
        self.fixture(folder, target)
        with patch.object(local, 'github_ready'), patch.object(local, 'draft') as draft, patch.object(release, 'gh') as gh, patch('builtins.print'):
            local.upload(COMMIT, target)
            draft.assert_called_once_with(COMMIT, create=True)
            arguments = gh.call_args.args
            self.assertEqual(arguments[:3], ('release', 'upload', 'v0.2.3'))
            uploaded = {Path(value).name for value in arguments[6:]}
            self.assertEqual(uploaded, set(local.asset_names(target) + [f'update-{target}.json']))

    def test_published_version_and_wrong_draft_source_are_refused(self):
        for metadata in [{'isDraft': False}, {'isDraft': True, 'body': '<!-- parlo-source:' + 'b' * 40 + ' -->'}]:
            with self.subTest(metadata=metadata), patch.object(release, 'gh', return_value=CompletedProcess([], 0, json.dumps(metadata), '')) as gh:
                with self.assertRaises(ValueError): local.draft(COMMIT, create=True)
                self.assertEqual(gh.call_count, 1)

    def test_incomplete_draft_never_downloads_or_publishes(self):
        with patch.object(local, 'github_ready'), patch.object(local, 'draft', return_value={'assets': []}), patch.object(release, 'gh') as gh:
            with self.assertRaisesRegex(ValueError, 'incompleta'): local.publish(COMMIT, TARGETS)
            gh.assert_not_called()

    def test_complete_draft_publishes_manifest_after_package_validation(self):
        fixtures = self.root / 'fixtures'
        for target in TARGETS: self.fixture(fixtures, target)
        names = [name for target in TARGETS for name in local.asset_names(target) + [f'update-{target}.json']]
        metadata = {'isDraft': True, 'body': f'<!-- parlo-source:{COMMIT} -->',
                    'assets': [{'name': name} for name in names]}
        mutations = []

        def gh(*args, **kwargs):
            output = ''
            if args[:2] == ('repo', 'view'): output = json.dumps({'visibility': 'PUBLIC'})
            elif args[:2] == ('release', 'view'):
                output = json.dumps({'tagName': 'v0.2.2'} if args[2] == '--repo' else metadata)
            elif args[:2] == ('release', 'download'):
                folder = Path(args[args.index('--dir') + 1])
                for name in names: (folder / name).write_bytes((fixtures / name).read_bytes())
            elif args[:2] == ('release', 'upload'):
                # Finalization must upload only latest.json, not re-upload apps.
                self.assertEqual(len(args), 7)
                self.assertEqual(Path(args[-1]).name, 'latest.json')
                manifest = json.loads(Path(args[-1]).read_text())
                self.assertEqual(set(manifest['platforms']), set(TARGETS))
                mutations.append('manifest')
            elif args[:2] == ('release', 'edit'): mutations.append('publish')
            return CompletedProcess([], 0, output, '')

        with patch.object(local, 'github_ready'), patch.object(release, 'gh', side_effect=gh), patch('builtins.print'): local.publish(COMMIT, TARGETS)
        self.assertEqual(mutations, ['manifest', 'publish'])

    def test_windows_publishes_first_then_mac_extends_same_public_release(self):
        for target in TARGETS: self.fixture(local.package_folder(COMMIT, target), target)
        remote = {}
        state = {'exists': False, 'isDraft': True, 'body': ''}
        manifests = []
        binary_uploads = []

        def gh(*args, **kwargs):
            output = ''
            if args[:2] == ('repo', 'view'): output = json.dumps({'visibility': 'PUBLIC'})
            elif args[:2] == ('release', 'view'):
                if not state['exists']: return CompletedProcess([], 1, '', 'release not found')
                if args[2] == '--repo': output = json.dumps({'tagName': 'v0.2.3'})
                else: output = json.dumps({**state, 'assets': [{'name': name} for name in remote]})
            elif args[:2] == ('release', 'create'):
                self.assertEqual(args[args.index('--target') + 1], COMMIT)
                state.update(exists=True, body=args[args.index('--notes') + 1])
            elif args[:2] == ('release', 'upload'):
                clobber = '--clobber' in args
                for value in args[6 if clobber else 5:]:
                    file = Path(value)
                    if file.name == 'latest.json':
                        manifests.append(json.loads(file.read_text()))
                    else:
                        self.assertFalse(clobber, 'Pacchetti pubblicati non devono essere sostituiti')
                        self.assertNotIn(file.name, remote)
                        binary_uploads.append(file.name)
                    remote[file.name] = file.read_bytes()
            elif args[:2] == ('release', 'download'):
                folder = Path(args[args.index('--dir') + 1])
                for index, value in enumerate(args):
                    if value == '--pattern':
                        name = args[index + 1]
                        (folder / name).write_bytes(remote[name])
            elif args[:2] == ('release', 'edit'): state['isDraft'] = False
            return CompletedProcess([], 0, output, '')

        with patch.object(local, 'github_ready'), patch.object(release, 'gh', side_effect=gh), patch('builtins.print'):
            local.upload(COMMIT, TARGETS[0], publish_now=True)
            self.assertFalse(state['isDraft'])
            self.assertEqual(set(manifests[-1]['platforms']), {TARGETS[0]})
            windows_before = {name: data for name, data in remote.items() if 'windows-x86_64' in name}
            local.upload(COMMIT, TARGETS[1], publish_now=True)
            self.assertEqual(set(manifests[-1]['platforms']), set(TARGETS))
            for name, data in windows_before.items(): self.assertEqual(remote[name], data)
            count = len(binary_uploads)
            local.upload(COMMIT, TARGETS[1], publish_now=True)
            self.assertEqual(len(binary_uploads), count, 'Riprovare non deve ricaricare i pacchetti')

    def test_single_command_detects_native_target_and_mac_free_signing(self):
        for target, adhoc in [(TARGETS[0], False), (TARGETS[1], True)]:
            with self.subTest(target=target), patch.object(local, 'native_platform', return_value=target), \
                 patch.object(local, 'github_ready'), patch.object(local, 'build') as build, \
                 patch.object(release, 'gh', return_value=CompletedProcess([], 1, '', 'not found')), \
                 patch.dict('os.environ', {'APPLE_SIGNING_IDENTITY': ''}):
                local.ship(COMMIT, False)
                args = build.call_args.args[0]
                self.assertTrue(args.upload)
                self.assertTrue(args.publish_now)
                self.assertEqual(args.adhoc, adhoc)

    def test_late_old_platform_cannot_roll_back_newer_release(self):
        with patch.object(local, 'github_ready'), patch.object(local, 'native_platform', return_value=TARGETS[1]), \
             patch.object(local, 'build') as build, \
             patch.object(release, 'gh', return_value=CompletedProcess([], 0, json.dumps({'tagName': 'v0.2.5'}), '')):
            with self.assertRaisesRegex(ValueError, 'v0.2.5'): local.ship(COMMIT, False)
            build.assert_not_called()


if __name__ == '__main__': unittest.main()
