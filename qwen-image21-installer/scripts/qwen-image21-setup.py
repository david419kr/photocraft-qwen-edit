#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Configure official Windows portable for Qwen Image 2.1; stdlib only."""
import argparse
import ast
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import socket
import subprocess
import sys
import time
import zipfile
import urllib.error
import urllib.parse
import urllib.request

UA = 'PhotoCraft-Qwen-Setup'
QWEN_REPO = 'Comfy-Org/Qwen-Image-2.1'
MODEL_FILES = {'1': 'diffusion_models/qwen_image_2.1_int8_convrot.safetensors',
               '2': 'diffusion_models/qwen_image_2.1_turbo_int8_convrot.safetensors'}
SHARED_FILES = ['text_encoders/qwen3vl_8b_int8_convrot.safetensors', 'vae/qwen_image_2.1_vae_bf16.safetensors']
TRANSLATORS = {'1': ('mradermacher/Hy-MT2-1.8B-heretic-i1-GGUF', 'Hy-MT2-1.8B-heretic.i1-Q4_K_M.gguf'),
               '2': ('alexwkleung/Hy-MT2-7B-Abliterated-GGUF', 'HY-MT2-7B-Abliterated-Q4_K_M.gguf')}


def section(title):
    print('\n' + '=' * 64 + '\n  ' + title + '\n' + '=' * 64 + '\n', flush=True)


def request_json(url):
    with urllib.request.urlopen(urllib.request.Request(url, headers={'User-Agent': UA}), timeout=60) as response:
        return json.load(response)


def sha256(path):
    digest = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for chunk in iter(lambda: stream.read(8 << 20), b''):
            digest.update(chunk)
    return digest.hexdigest()


def atomic_json(path, value):
    temporary = path.with_name(path.name + '.tmp')
    temporary.write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding='utf-8')
    temporary.replace(path)


def progress_line(done, total, speed, width=80):
    percent = 100 * done / total if total else 100
    remaining = max(0, total - done)
    if speed > 0:
        seconds = int(remaining / speed)
        eta = f'{seconds // 60:02d}:{seconds % 60:02d}'
    else:
        eta = '--:--'
    details = f'{percent:5.1f}% {done / 1e9:.2f}/{total / 1e9:.2f} GB  {speed / 1e6:.1f} MB/s  ETA {eta}'
    if width < 76:
        details = f'{percent:5.1f}% {speed / 1e6:.1f} MB/s  ETA {eta}'
    bar_width = min(26, max(4, width - len(details) - 4))
    filled = min(bar_width, max(0, int(bar_width * done / total))) if total else bar_width
    return ('[' + '#' * filled + '-' * (bar_width - filled) + '] ' + details)[:max(1, width - 1)]


class DownloadProgress:
    """Refresh one console line; redirected output gets a single summary."""
    def __init__(self, total, initial=0, stream=None):
        self.total, self.initial, self.done = total, initial, initial
        self.stream = sys.stdout if stream is None else stream
        self.owned_stream = False
        # PowerShell may pipe native stdout even in a visible console. Write progress
        # directly to that console to avoid turning carriage returns into log lines.
        if stream is None and os.name == 'nt' and not self.stream.isatty():
            try:
                self.stream = open('CONOUT$', 'w', encoding='utf-8', buffering=1)
                self.owned_stream = True
            except OSError:
                pass
        self.live = self.stream.isatty()
        self.started = time.monotonic()
        self.last_refresh = 0.0
        self.previous_width = 0

    def __enter__(self):
        self.update(self.initial, force=True)
        return self

    def update(self, done, force=False):
        self.done = done
        now = time.monotonic()
        if not self.live or (not force and now - self.last_refresh < 0.2):
            return
        elapsed = max(0.001, now - self.started)
        speed = max(0, done - self.initial) / elapsed
        try:
            width = os.get_terminal_size(self.stream.fileno()).columns
        except (OSError, ValueError, AttributeError):
            width = 80
        line = progress_line(done, self.total, speed, width)
        self.stream.write('\r' + line + ' ' * max(0, self.previous_width - len(line)))
        self.stream.flush()
        self.previous_width, self.last_refresh = len(line), now

    def __exit__(self, exc_type, exc_value, traceback):
        if self.live:
            self.update(self.done, force=True)
            self.stream.write('\n')
        else:
            speed = max(0, self.done - self.initial) / max(0.001, time.monotonic() - self.started)
            self.stream.write('  ' + progress_line(self.done, self.total, speed) + '\n')
        self.stream.flush()
        if self.owned_stream:
            self.stream.close()
            # Preserve a single transfer summary when stdout is being logged/piped.
            speed = max(0, self.done - self.initial) / max(0.001, time.monotonic() - self.started)
            print('  ' + progress_line(self.done, self.total, speed), flush=True)


def download(url, target, expected_size, expected_hash, verified):
    """Resume into a .part file; promote only after exact size + SHA256 validation."""
    target = Path(target)
    target.parent.mkdir(parents=True, exist_ok=True)
    key = str(target.resolve())
    if target.is_file():
        stamp = {'size': target.stat().st_size, 'mtime_ns': target.stat().st_mtime_ns, 'sha256': expected_hash}
        if stamp['size'] == expected_size and (verified.get(key) == stamp or sha256(target) == expected_hash):
            verified[key] = stamp
            print(f'[OK] Already installed: {target.name}', flush=True)
            return
        raise RuntimeError(f'A different file already exists: {target}. It was preserved. Move it elsewhere and rerun setup.')
    partial = target.with_name(target.name + '.part')
    if partial.exists() and partial.stat().st_size > expected_size:
        partial.rename(partial.with_name(partial.name + '.bad-' + str(time.time_ns())))
    for attempt in range(4):
        offset = partial.stat().st_size if partial.exists() else 0
        if offset == expected_size:
            break
        headers = {'User-Agent': UA}
        if offset:
            headers['Range'] = f'bytes={offset}-'
        try:
            with urllib.request.urlopen(urllib.request.Request(url, headers=headers), timeout=120) as response:
                if response.status == 206:
                    content_range = response.headers.get('Content-Range', '')
                    if not content_range.startswith(f'bytes {offset}-'):
                        raise RuntimeError('The server returned an incorrect download resume position.')
                    mode = 'ab'
                elif response.status == 200:
                    offset, mode = 0, 'wb'
                else:
                    raise RuntimeError(f'Unexpected HTTP status {response.status}')
                if shutil.disk_usage(target.parent).free < expected_size - offset + (1 << 30):
                    raise RuntimeError(f'Not enough free disk space: {target.parent}')
                print(f'\n[Download] {target.name}', flush=True)
                with partial.open(mode) as output, DownloadProgress(expected_size, offset) as progress:
                    # Stop at the trusted file size; do not wait for EOF on a keep-alive relay.
                    while offset < expected_size:
                        chunk = response.read(min(8 << 20, expected_size - offset))
                        if not chunk:
                            break
                        output.write(chunk)
                        offset += len(chunk)
                        progress.update(offset)
            if partial.stat().st_size == expected_size:
                break
        except (urllib.error.URLError, TimeoutError, ConnectionError) as error:
            if attempt == 3:
                raise RuntimeError('Download interrupted. Rerun setup to resume.') from error
        time.sleep(1)
    if not partial.exists() or partial.stat().st_size != expected_size:
        raise RuntimeError(f'Download incomplete: {partial}')
    print(f'  Verifying SHA256: {target.name}', flush=True)
    if sha256(partial) != expected_hash:
        partial.rename(partial.with_name(partial.name + '.bad-' + str(time.time_ns())))
        raise RuntimeError(f'Checksum mismatch: {target.name}. Rerun setup to download a fresh copy.')
    partial.replace(target)
    verified[key] = {'size': target.stat().st_size, 'mtime_ns': target.stat().st_mtime_ns, 'sha256': expected_hash}


def selections(text):
    items = [part for part in re.split(r'[\s,]+', text.strip()) if part]
    if not items or any(item not in ('0', '1', '2') for item in items) or ('0' in items and len(items) != 1):
        raise ValueError('Enter 0, 1, 2, or 1,2.')
    return sorted(set(items) - {'0'})


def choose(title, supplied, default):
    if supplied is not None:
        return selections(supplied)
    while True:
        answer = input(f'{title} [{default}]: ').strip() or default
        try:
            return selections(answer)
        except ValueError as error:
            print(error)


def run(arguments, timeout=None):
    print('[Run] ' + subprocess.list2cmdline([str(a) for a in arguments]), flush=True)
    subprocess.run([str(a) for a in arguments], check=True, timeout=timeout)


def portable_env():
    environment = dict(os.environ)
    for name in ['CUDA_PATH', 'CUDA_HOME', 'TRITON_PTXAS_PATH', 'TRITON_PTXAS_BLACKWELL_PATH', 'CC', 'CXX', 'HIP_PATH']:
        environment.pop(name, None)
    return environment


PROBE = r'''
import importlib.metadata as m, json, sys, torch
info = {'python': list(sys.version_info[:3]), 'torch': torch.__version__, 'cuda': torch.version.cuda,
        'gpu_available': torch.cuda.is_available()}
if info['gpu_available']:
    info['gpu'] = torch.cuda.get_device_name(0)
    info['capability'] = list(torch.cuda.get_device_capability(0))
    torch.ones(1, device='cuda').sum().item()
info['torch_triton_requirement'] = [r for r in (m.requires('torch') or []) if 'triton' in r.lower()]
print('QWEN_PROBE=' + json.dumps(info))
'''
SAGE_PROBE = r'''
import torch
from sageattention import sageattn
q = torch.randn(1, 4, 128, 128, dtype=torch.float16, device='cuda')
k = torch.randn_like(q); v = torch.randn_like(q)
out = sageattn(q, k, v, tensor_layout='HND', is_causal=False)
reference = torch.nn.functional.scaled_dot_product_attention(q, k, v)
torch.cuda.synchronize()
assert torch.isfinite(out).all().item(), 'Sage returned non-finite values'
similarity = torch.nn.functional.cosine_similarity(out.flatten().float(), reference.flatten().float(), dim=0).item()
assert similarity > 0.98, f'Sage correctness check failed: cosine={similarity}'
print('SAGE_OK')
'''


def version_pair(value):
    match = re.match(r'(\d+)\.(\d+)', value)
    if not match:
        raise ValueError(f'Unsupported version: {value}')
    return tuple(map(int, match.groups()))


def sage_asset(releases, info):
    torch_version = version_pair(info['torch'])
    cuda_major = int(info['cuda'].split('.')[0])
    py_major, py_minor = info['python'][:2]
    for release in releases:
        if release.get('prerelease') or release.get('draft'):
            continue
        for asset in release.get('assets', []):
            name = asset['name']
            match = re.search(r'\+cu(\d{3})torch(\d+\.\d+\.\d+)(andhigher)?[^-]*-cp(\d)(\d+)-(abi3|cp\d+)-win_amd64\.whl$', name)
            if not match or int(match[1]) // 10 != cuda_major:
                continue
            minimum_torch = version_pair(match[2])
            torch_ok = torch_version[0] == minimum_torch[0] and (torch_version >= minimum_torch if match[3] else torch_version == minimum_torch)
            minimum_py = (int(match[4]), int(match[5]))
            py_ok = (py_major, py_minor) >= minimum_py if match[6] == 'abi3' else (py_major, py_minor) == minimum_py
            if torch_ok and py_ok:
                return asset
    return None


def ensure_python_headers(python, cache, info, verified):
    embedded = Path(python).parent.resolve()
    major, minor = info['python'][:2]
    if (embedded / 'include/Python.h').exists() and (embedded / f'libs/python{major}{minor}.lib').exists():
        return
    if (major, minor) != (3, 13):
        raise RuntimeError('Triton headers/libraries are unavailable for this embedded Python version.')
    name = 'python_3.13.2_include_libs.zip'
    digest = 'e3e8c3abb17ea8de1cd2cd264ad15aae3de5eb7dc61ab37cb1a3d07f9c86238d'
    archive_path = cache / name
    url = 'https://github.com/woct0rdho/triton-windows/releases/download/v3.0.0-windows.post1/' + name
    download(url, archive_path, 555256, digest, verified)
    with zipfile.ZipFile(archive_path) as archive:
        for item in archive.infolist():
            parts = Path(item.filename).parts
            destination = (embedded / item.filename).resolve()
            if not parts or parts[0].lower() not in ('include', 'libs') or not destination.is_relative_to(embedded):
                raise RuntimeError('Unsafe path in the Python support ZIP.')
        archive.extractall(embedded)
    if not (embedded / 'include/Python.h').exists() or not (embedded / 'libs/python313.lib').exists():
        raise RuntimeError('Python support headers/libraries were not installed correctly.')


def setup_sage(python, cache, info, verified):
    test = [python, '-I', '-c', SAGE_PROBE]
    try:
        existing = subprocess.run(test, capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=120, env=portable_env())
        (cache / 'sage-probe.log').write_text((existing.stdout or '') + (existing.stderr or ''), encoding='utf-8')
        if existing.returncode == 0:
            ensure_python_headers(python, cache, info, verified)
            print('[OK] Existing SageAttention passed the CUDA kernel test.', flush=True)
            return True
        releases = request_json('https://api.github.com/repos/woct0rdho/SageAttention/releases?per_page=8')
        asset = sage_asset(releases, info)
        if not asset:
            raise RuntimeError('No Windows Sage wheel matches this Python / PyTorch / CUDA environment.')
        expected_hash = (asset.get('digest') or '').removeprefix('sha256:')
        if not re.fullmatch(r'[0-9a-f]{64}', expected_hash):
            raise RuntimeError('The Sage wheel has no SHA256 digest.')
        wheel = cache / asset['name']
        download(asset['browser_download_url'], wheel, asset['size'], expected_hash, verified)
        # Match the installed Torch family, without allowing pip to replace Torch.
        torch_pair = version_pair(info['torch'])
        mapping = {(2, 4): '3.1', (2, 5): '3.1', (2, 6): '3.2', (2, 7): '3.3',
                   (2, 8): '3.4', (2, 9): '3.5', (2, 10): '3.6', (2, 11): '3.6',
                   (2, 12): '3.7', (2, 13): '3.7'}
        requirements = list(info.get('torch_triton_requirement', []))
        # Windows Torch wheels omit Linux-only Triton dependencies; consult the same Torch release metadata.
        if not requirements:
            try:
                release_info = request_json('https://pypi.org/pypi/torch/' + info['torch'].split('+')[0] + '/json')
                requirements = release_info['info'].get('requires_dist') or []
            except (urllib.error.URLError, OSError, ValueError, KeyError):
                requirements = []
        matches = [re.search(r'triton\s*(?:==|~=)\s*(\d+\.\d+)', line) for line in requirements]
        required = next((match for match in matches if match), None)
        triton_version = required[1] if required else mapping.get(torch_pair)
        if tuple(info['capability']) < (8, 0):
            triton_version = '3.2'
        if not triton_version:
            raise RuntimeError('Could not determine a compatible Triton version for this PyTorch release.')
        minor = int(triton_version.split('.')[1])
        requirement = f'triton-windows>={triton_version},<3.{minor + 1}'
        ensure_python_headers(python, cache, info, verified)
        run([python, '-s', '-m', 'pip', 'install', '--only-binary=:all:', '--no-deps', requirement, wheel])
        checked = subprocess.run(test, capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=120, env=portable_env())
        (cache / 'sage-probe.log').write_text(checked.stdout + checked.stderr, encoding='utf-8')
        if checked.returncode != 0:
            raise RuntimeError('Sage CUDA kernel test failed. See .qwen-setup/sage-probe.log.')
        print('[OK] SageAttention installed and passed the CUDA kernel test.', flush=True)
        return True
    except (RuntimeError, OSError, subprocess.SubprocessError, urllib.error.URLError, ValueError, KeyError, TypeError, zipfile.BadZipFile) as error:
        print(f'\n[Fallback] SageAttention is unavailable: {error}\n  Using PyTorch attention instead. Setup will continue.\n', flush=True)
        # A broken Sage import can prevent ComfyUI startup even without the Sage flag.
        run([python, '-s', '-m', 'pip', 'uninstall', '-y', 'sageattention'])
        return False


def check_nodes(root):
    expected = {'comfy_extras/nodes_qwen.py': ['TextEncodeQwenImage21', 'QwenImage21Cache'],
                'comfy_extras/nodes_post_processing.py': ['ResizeImageMaskNode'],
                'comfy_extras/nodes_images.py': ['GetImageSize'],
                'comfy_extras/nodes_custom_sampler.py': ['ManualSigmas', 'SamplerCustom', 'KSamplerSelect']}
    for relative, names in expected.items():
        source = root / 'ComfyUI' / relative
        tree = ast.parse(source.read_text(encoding='utf-8-sig'))
        classes = {node.name for node in ast.walk(tree) if isinstance(node, ast.ClassDef)}
        missing = set(names) - classes
        if missing:
            raise RuntimeError(f'Required ComfyUI nodes are missing: {sorted(missing)}. Update the official ComfyUI installation.')
    print('[OK] Required Qwen 2.1 / Turbo nodes are included in this ComfyUI installation.', flush=True)


def hf_files(repo, files):
    data = request_json('https://huggingface.co/api/models/' + repo + '?blobs=true')
    index = {item['rfilename']: item for item in data['siblings']}
    result = []
    for filename in files:
        item = index.get(filename, {})
        lfs = item.get('lfs', {})
        digest = lfs.get('sha256', '')
        if not re.fullmatch(r'[0-9a-f]{64}', digest) or not lfs.get('size'):
            raise RuntimeError(f'Model file or verification metadata is missing: {repo}/{filename}')
        revision = data['sha']
        url = f'https://huggingface.co/{repo}/resolve/{revision}/' + urllib.parse.quote(filename)
        result.append((filename, url, lfs['size'], digest))
    return result


def write_launcher(root, sage):
    option = ' --use-sage-attention' if sage else ''
    text = ('@echo off\r\nrem SPDX-License-Identifier: MIT\r\nsetlocal\r\n'
            'cd /d "%~dp0"\r\n' + ''.join(f'set "{name}="\r\n' for name in ['HIP_PATH', 'CUDA_PATH', 'CUDA_HOME', 'TRITON_PTXAS_PATH', 'TRITON_PTXAS_BLACKWELL_PATH', 'CC', 'CXX']) +
            '".\\python_embeded\\python.exe" -s -X utf8 ".\\scripts\\qwen-image21-setup.py" check-port\r\n'
            'if errorlevel 1 goto done\r\n'
            '".\\python_embeded\\python.exe" -s ComfyUI\\main.py --windows-standalone-build --disable-auto-launch' + option + '\r\n'
            ':done\r\npause\r\n')
    (root / 'Run-Qwen-Image21.cmd').write_bytes(text.encode('ascii'))


def install(root, options):
    if any(getattr(options, name, None) is None for name in ('models', 'shared', 'translations')):
        raise RuntimeError('Run install-qwen-image21.cmd to answer setup questions first, or supply --models, --shared and --translations.')
    models = choose('Select Qwen models', options.models, '2')
    shared = bool(models) or options.shared == 'yes'
    translations = choose('Select translation models', options.translations, '0')
    root = root.resolve()
    python = root / 'python_embeded/python.exe'
    cache = root / '.qwen-setup'
    cache.mkdir(exist_ok=True)
    state_file = cache / 'install-state.json'
    try:
        state = json.loads(state_file.read_text(encoding='utf-8')) if state_file.exists() else {}
    except (OSError, ValueError):
        print('[Setup] Saved state could not be read; using a fresh state.', flush=True)
        state = {}
    if not isinstance(state, dict):
        state = {}
    verified = state.setdefault('verified_downloads', {})
    # Updating packages while this exact installation is running is unsafe.
    command = ['$p=Get-CimInstance Win32_Process; $p | Where-Object {$_.Name -match "python" -and $_.CommandLine -match "main.py"} | Select-Object -ExpandProperty ExecutablePath']
    running = subprocess.run(['powershell.exe', '-NoProfile', '-Command', command[0]], capture_output=True, text=True)
    if str(python).lower() in running.stdout.lower():
        raise RuntimeError('Close ComfyUI running from this installation folder, then rerun setup.')
    info_result = subprocess.run([python, '-I', '-c', PROBE], capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=120)
    if info_result.returncode:
        raise RuntimeError('The official portable failed its PyTorch/CUDA check:\n' + info_result.stderr)
    info = json.loads(next(line.removeprefix('QWEN_PROBE=') for line in info_result.stdout.splitlines() if line.startswith('QWEN_PROBE=')))
    atomic_json(cache / 'environment.json', info)
    if not info['gpu_available'] or not info.get('cuda', '').startswith('13.'):
        raise RuntimeError('A CUDA 13 compatible NVIDIA GPU and driver are required. See .qwen-setup/environment.json.')
    section('[2/5] Acceleration and required nodes')
    print(f"GPU: {info['gpu']}; Python {info['python']}; Torch {info['torch']}; CUDA {info['cuda']}", flush=True)
    sage = setup_sage(python, cache, info, verified)
    state['sage_attention'] = sage
    check_nodes(root)
    section('[3/5] Selected configuration')
    print(f"Image models: {','.join(models) or 'manual'} | Shared encoder/VAE: {'yes' if shared else 'no'} | Translation: {','.join(translations) or 'none'}", flush=True)
    state.update(models=models, shared=shared, translations=translations, completed=False)
    atomic_json(state_file, state)
    write_launcher(root, sage)
    section('[4/5] Download and configure')
    wanted = [MODEL_FILES[key] for key in models] + (SHARED_FILES if shared else [])
    for filename, url, size, digest in (hf_files(QWEN_REPO, wanted) if wanted else []):
        download(url, root / 'ComfyUI/models' / filename, size, digest, verified)
        atomic_json(state_file, state)
    if translations:
        print('Installing the dedicated Python/CUDA environment for translation.', flush=True)
        run(['powershell.exe', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', root / 'scripts/install-comfyui-hymt2.ps1', '-PortableRoot', root])
        for key in translations:
            repo, filename = TRANSLATORS[key]
            for _, url, size, digest in hf_files(repo, [filename]):
                download(url, root / 'ComfyUI/models/LLM/GGUF' / filename, size, digest, verified)
                atomic_json(state_file, state)
    state['completed'] = True
    atomic_json(state_file, state)
    section('[5/5] Installation complete')
    print(f"  Attention : {'SageAttention' if sage else 'PyTorch'}\n  Run       : {root / 'Run-Qwen-Image21.cmd'}\n  Setup     : {root / 'install-qwen-image21.cmd'}\n  Address   : http://127.0.0.1:8188\n  Browser   : Auto-launch disabled\n", flush=True)
    if not models and not shared:
        print('[Next] Add an image model, the official text encoder, and VAE before generating images.')
    (root / 'Qwen-Image21-Setup-Info.txt').write_text(
        'Run: Run-Qwen-Image21.cmd\nAdd models / translation: install-qwen-image21.cmd\n'
        'Address: http://127.0.0.1:8188 (PhotoCraft default endpoint)\nBrowser auto-launch: disabled\n'
        'Image models: ComfyUI/models/diffusion_models\nText encoder: ComfyUI/models/text_encoders\nVAE: ComfyUI/models/vae\n'
        'Translation GGUF: ComfyUI/models/LLM/GGUF\n'
        'Official filenames:\n' + '\n'.join(MODEL_FILES.values()) + '\n' + '\n'.join(SHARED_FILES) + '\n', encoding='utf-8-sig')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('action', choices=['install', 'check-port'])
    parser.add_argument('--root', type=Path)
    parser.add_argument('--models')
    parser.add_argument('--shared', choices=['yes', 'no'])
    parser.add_argument('--translations')
    options = parser.parse_args()
    if options.action == 'check-port':
        try:
            with socket.create_connection(('127.0.0.1', 8188), timeout=1):
                print('Port 8188 is already in use. Close the existing ComfyUI server, then try again.')
                return 1
        except OSError:
            return 0
    if options.root is None:
        parser.error('--root is required for installation')
    try:
        install(options.root, options)
        return 0
    except (EOFError, KeyboardInterrupt):
        print('Setup cancelled. Rerun the installer to reuse completed downloads.', file=sys.stderr)
        return 130
    except (OSError, RuntimeError, ValueError, StopIteration, subprocess.SubprocessError, SyntaxError) as error:
        print(f'\n[Error] Setup failed: {error}\nYour selections and completed downloads were preserved. Rerun install-qwen-image21.cmd.', file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
