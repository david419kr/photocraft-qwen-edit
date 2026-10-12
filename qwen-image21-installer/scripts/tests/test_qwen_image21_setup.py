# SPDX-License-Identifier: MIT
import hashlib
import importlib.util
import json
import io
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('setup', Path(__file__).resolve().parents[1] / 'qwen-image21-setup.py')
setup = importlib.util.module_from_spec(spec)
spec.loader.exec_module(setup)
PAYLOAD = bytes(range(256)) * 1024


class InstallerTests(unittest.TestCase):
    url = 'https://example.invalid/test-model'

    def test_missing_or_invalid_choices_fail_before_any_environment_work(self):
        with tempfile.TemporaryDirectory() as folder, patch('builtins.input') as ask, patch.object(setup.subprocess, 'run') as run:
            with self.assertRaisesRegex(RuntimeError, 'answer setup questions first'):
                setup.install(Path(folder), SimpleNamespace(models=None, shared=None, translations=None))
            with self.assertRaises(ValueError):
                setup.install(Path(folder), SimpleNamespace(models='3', shared='yes', translations='0'))
            ask.assert_not_called()
            run.assert_not_called()

    def test_resume_verify_and_no_duplicate_download(self):
        with tempfile.TemporaryDirectory(prefix='qwen installer 한글 ') as folder:
            target = Path(folder) / 'weights.gguf'
            target.with_suffix('.gguf.part').write_bytes(PAYLOAD[:8192])
            verified = {}
            digest = hashlib.sha256(PAYLOAD).hexdigest()
            def response(request, timeout):
                self.assertEqual(request.get_header('Range'), 'bytes=8192-')
                result = io.BytesIO(PAYLOAD[8192:])
                result.status = 206
                result.headers = {'Content-Range': f'bytes 8192-{len(PAYLOAD)-1}/{len(PAYLOAD)}'}
                return result
            with patch.object(setup.urllib.request, 'urlopen', side_effect=response) as http:
                setup.download(self.url, target, len(PAYLOAD), digest, verified)
                self.assertEqual(target.read_bytes(), PAYLOAD)
                setup.download(self.url, target, len(PAYLOAD), digest, verified)
                self.assertEqual(http.call_count, 1)

    def test_server_ignoring_range_restarts_instead_of_appending(self):
        with tempfile.TemporaryDirectory() as folder:
            target = Path(folder) / 'weights.gguf'
            target.with_suffix('.gguf.part').write_bytes(b'old partial data')
            response = io.BytesIO(PAYLOAD)
            response.status = 200
            response.headers = {}
            with patch.object(setup.urllib.request, 'urlopen', return_value=response):
                setup.download(self.url, target, len(PAYLOAD), hashlib.sha256(PAYLOAD).hexdigest(), {})
            self.assertEqual(target.read_bytes(), PAYLOAD)

    def test_corrupt_finished_partial_is_preserved_and_not_promoted(self):
        with tempfile.TemporaryDirectory() as folder:
            target = Path(folder) / 'weights.gguf'
            partial = target.with_suffix('.gguf.part')
            partial.write_bytes(b'?' * len(PAYLOAD))
            with self.assertRaisesRegex(RuntimeError, 'Checksum'):
                setup.download(self.url, target, len(PAYLOAD), hashlib.sha256(PAYLOAD).hexdigest(), {})
            self.assertFalse(target.exists())
            self.assertFalse(partial.exists())
            self.assertEqual(len(list(Path(folder).glob('*.bad-*'))), 1)

    def test_different_existing_model_is_not_overwritten(self):
        with tempfile.TemporaryDirectory() as folder:
            target = Path(folder) / 'weights.gguf'
            target.write_bytes(b'existing user model')
            with self.assertRaisesRegex(RuntimeError, 'preserved'):
                setup.download(self.url, target, len(PAYLOAD), hashlib.sha256(PAYLOAD).hexdigest(), {})
            self.assertEqual(target.read_bytes(), b'existing user model')

    def test_select_compatible_stable_abi_wheel(self):
        wheel = {'name': 'sageattention-2.2.0+cu130torch2.10.0andhigher.post6-cp310-abi3-win_amd64.whl'}
        releases = [{'assets': [wheel]}]
        info = {'torch': '2.13.0+cu130', 'cuda': '13.0', 'python': [3, 13, 14]}
        self.assertIs(setup.sage_asset(releases, info), wheel)
        self.assertIsNone(setup.sage_asset(releases, dict(info, cuda='12.8')))
        self.assertIsNone(setup.sage_asset(releases, dict(info, torch='2.9.1+cu130')))
        self.assertIsNone(setup.sage_asset([{'prerelease': True, 'assets': [wheel]}], info))

    def test_unavailable_sage_falls_back_without_replacing_torch(self):
        with tempfile.TemporaryDirectory() as folder:
            calls = []
            with patch.object(setup.subprocess, 'run', return_value=subprocess.CompletedProcess([], 1)), patch.object(setup, 'request_json', return_value=[]), patch.object(setup, 'run', side_effect=lambda args: calls.append(args)):
                enabled = setup.setup_sage('python.exe', Path(folder), {'torch': '2.13.0', 'cuda': '13.0', 'python': [3, 13, 14]}, {})
            self.assertFalse(enabled)
            self.assertEqual(calls[0][-1], 'sageattention')
            self.assertIn('uninstall', calls[0])
            self.assertNotIn('torch', calls[0])

    def test_new_torch_uses_its_public_dependency_metadata(self):
        wheel = {'name': 'sageattention-2.2.0+cu130torch2.10.0andhigher.post6-cp310-abi3-win_amd64.whl',
                 'digest': 'sha256:' + 'a' * 64, 'size': 42, 'browser_download_url': 'https://example.invalid/sage.whl'}
        info = {'torch': '2.14.0+cu130', 'cuda': '13.0', 'python': [3, 13, 14], 'capability': [8, 9]}
        calls = []
        def metadata(url):
            if 'SageAttention' in url:
                return [{'assets': [wheel]}]
            return {'info': {'requires_dist': ['triton~=3.8.0; platform_system == "Linux"']}}
        with tempfile.TemporaryDirectory() as folder:
            results = [subprocess.CompletedProcess([], 1), subprocess.CompletedProcess([], 0, 'SAGE_OK', '')]
            with patch.object(setup.subprocess, 'run', side_effect=results), patch.object(setup, 'request_json', side_effect=metadata), patch.object(setup, 'download'), patch.object(setup, 'ensure_python_headers'), patch.object(setup, 'run', side_effect=lambda args: calls.append(args)):
                self.assertTrue(setup.setup_sage('python.exe', Path(folder), info, {}))
        self.assertIn('triton-windows>=3.8,<3.9', calls[0])
        self.assertIn('--no-deps', calls[0])

    def test_launcher_only_changes_sage_flag_and_is_relocatable(self):
        with tempfile.TemporaryDirectory(prefix='portable 한글 ') as folder:
            root = Path(folder)
            setup.write_launcher(root, False)
            ordinary = (root / 'Run-Qwen-Image21.cmd').read_bytes()
            setup.write_launcher(root, True)
            accelerated = (root / 'Run-Qwen-Image21.cmd').read_bytes()
            self.assertEqual(accelerated.replace(b' --use-sage-attention', b''), ordinary)
            self.assertIn(b'--disable-auto-launch', ordinary)
            self.assertNotIn(b'--port', ordinary)
            self.assertIn(b'%~dp0', ordinary)
            self.assertIn(b'.\\scripts\\qwen-image21-setup.py', ordinary)
            self.assertNotIn(folder.encode(), ordinary)

    def test_choices_allow_zero_and_multiple_but_reject_bad_values(self):
        self.assertEqual(setup.selections('0'), [])
        self.assertEqual(setup.selections('2,1,1'), ['1', '2'])
        for text in ['0,1', '3', 'one', '']:
            with self.assertRaises(ValueError):
                setup.selections(text)

    def test_atomic_state_roundtrip_in_unicode_folder(self):
        with tempfile.TemporaryDirectory(prefix='state 한글 ') as folder:
            target = Path(folder) / 'install-state.json'
            setup.atomic_json(target, {'models': ['1', '2'], 'translations': [], 'sage_attention': False})
            self.assertEqual(json.loads(target.read_text(encoding='utf-8'))['models'], ['1', '2'])
            self.assertFalse(target.with_name(target.name + '.tmp').exists())

    def test_progress_has_percent_speed_eta_and_fits_console(self):
        line = setup.progress_line(3_000_000_000, 6_000_000_000, 100_000_000, 80)
        self.assertIn('50.0%', line)
        self.assertIn('3.00/6.00 GB', line)
        self.assertIn('100.0 MB/s', line)
        self.assertIn('ETA 00:30', line)
        self.assertLess(len(line), 80)
        self.assertLess(len(setup.progress_line(100, 200, 10, 40)), 40)

    def test_live_progress_rewrites_one_line(self):
        class Console(io.StringIO):
            def isatty(self):
                return True
        stream = Console()
        with setup.DownloadProgress(1000, stream=stream) as progress:
            progress.update(500, force=True)
            progress.update(1000, force=True)
        output = stream.getvalue()
        self.assertGreaterEqual(output.count('\r'), 3)
        self.assertEqual(output.count('\n'), 1)
        self.assertIn('100.0%', output)

    def test_redirected_progress_emits_only_one_summary(self):
        stream = io.StringIO()
        with setup.DownloadProgress(1000, 500, stream=stream) as progress:
            for done in [600, 700, 800, 900, 1000]:
                progress.update(done, force=True)
        self.assertEqual(stream.getvalue().count('\n'), 1)
        self.assertNotIn('\r', stream.getvalue())
        self.assertIn('MB/s', stream.getvalue())


if __name__ == '__main__':
    unittest.main()
