"""Publish the approved integrated release from exact, previously verified CI bytes."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parent
CONFIG = json.loads((ROOT / 'publication-config.json').read_text(encoding='utf-8'))
REPO = 'david419kr/photocraft-qwen-edit'
TAG = CONFIG['tag']


def run(*args, **kwargs):
    return subprocess.check_output(args, text=True, encoding='utf-8', **kwargs).strip()


def api(endpoint, payload=None, method='PATCH'):
    args = ['gh', 'api', f'repos/{REPO}/{endpoint}']
    if payload is None:
        return json.loads(run(*args))
    return json.loads(run(*args, '--method', method, '--input', '-', input=json.dumps(payload)))


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def assert_source():
    assert os.environ['GITHUB_REPOSITORY'] == REPO
    assert run('git', 'rev-parse', 'HEAD') == CONFIG['main_sha']
    assert run('git', 'rev-parse', 'HEAD^{tree}') == CONFIG['tree_sha']
    assert run('git', 'rev-parse', CONFIG['candidate_sha'] + '^{tree}') == CONFIG['tree_sha']
    assert api('git/ref/heads/main')['object']['sha'] == CONFIG['main_sha']
    pr = api('pulls/5')
    assert pr['merged'] and pr['head']['sha'] == CONFIG['pr_head_sha']
    assert pr['merge_commit_sha'] == CONFIG['pr_merge_sha']
    assert run('git', 'diff', '--name-only', CONFIG['pr_head_sha'], 'HEAD') == 'release-tag.txt'
    ref = api('git/ref/tags/' + TAG)['object']
    assert ref['type'] == 'tag' and ref['sha'] == CONFIG['tag_object_sha']
    assert api('git/tags/' + ref['sha'])['object']['sha'] == CONFIG['main_sha']
    assert Path('release-tag.txt').read_text().strip() == TAG
    subprocess.run(['git', 'diff', '--exit-code', CONFIG['base_sha'], 'HEAD', '--',
                    'README.md', 'README.ko.md', 'qwen-image21-installer',
                    '.github/workflows/perf-nightly.yml', '.github/workflows/freebsd.yml',
                    '.github/workflows/windows-arm64.yml'], check=True)


def check_release(release, notes):
    assert release['tag_name'] == TAG and release['target_commitish'] == CONFIG['main_sha']
    assert not release['prerelease'] and release['name'] == CONFIG['title']
    assert release['body'].replace('\r\n', '\n').strip() == notes.strip()


def main():
    assert_source()
    candidate = Path(sys.argv[1]).resolve()
    notes = (ROOT / 'release-notes.md').read_text(encoding='utf-8')
    archive = candidate / f'photocraft-qwen-edit-{TAG}-windows-x64.zip'
    files = [archive, Path(str(archive) + '.sha256'), ROOT / 'release-provenance.json']
    assert digest(archive) == CONFIG['zip_sha256']
    build = json.loads((candidate / 'candidate-build.json').read_text(encoding='utf-8-sig'))
    assert build['source_sha'] == CONFIG['candidate_sha']
    assert build['zip_sha256'] == CONFIG['zip_sha256']
    assert build['executable_sha256'] == CONFIG['exe_sha256']
    version = (candidate / 'candidate-version.txt').read_text(encoding='utf-8-sig')
    assert 'PhotoCraft Local Qwen AI ' + TAG in version
    out = Path('dist'); out.mkdir(exist_ok=True)
    with zipfile.ZipFile(archive) as bundle:
        exe = bundle.read('photocraft.exe')
    assert hashlib.sha256(exe).hexdigest() == CONFIG['exe_sha256']
    assert CONFIG['candidate_sha'].encode() in exe and TAG.encode() in exe
    exe_path = out / 'photocraft.exe'; exe_path.write_bytes(exe)
    spec = importlib.util.spec_from_file_location('portable', 'scripts/verify-qwen-portable.py')
    verifier = importlib.util.module_from_spec(spec); spec.loader.exec_module(verifier)
    assert verifier.verify(archive, exe_path) == 18
    for check in CONFIG['ci_runs']:
        workflow = api(f"actions/runs/{check['id']}")
        assert workflow['head_sha'] == check['sha'] and workflow['conclusion'] == 'success'
    old = api('releases/411304264')
    old_assets = {a['id']: (a['name'], a['digest']) for a in old['assets']}
    expected = {p.name: 'sha256:' + digest(p) for p in files}
    releases = api('releases?per_page=100')
    existing = [r for r in releases if r['tag_name'] == TAG]
    assert len(existing) <= 1
    if existing:
        release = existing[0]
        check_release(release, notes)
        assert release['draft'], 'Existing public release must never be changed'
    else:
        release = api('releases', {'tag_name': TAG, 'target_commitish': CONFIG['main_sha'],
                      'name': CONFIG['title'], 'body': notes, 'draft': True, 'prerelease': False}, method='POST')
    check_release(release, notes)
    present = {a['name']: a for a in release['assets']}
    assert len(present) == len(release['assets']) and set(present) <= set(expected)
    for name, asset in present.items():
        assert asset['state'] == 'uploaded' and asset['digest'] == expected[name], name
    for path in files:
        if path.name not in present:
            subprocess.run(['gh', 'release', 'upload', TAG, str(path), '--repo', REPO], check=True)
    release = api(f"releases/{release['id']}")
    check_release(release, notes)
    assert len(release['assets']) == 3
    assert {a['name']: a['digest'] for a in release['assets']} == expected
    assert_source()
    release = api(f"releases/{release['id']}", {'draft': False, 'make_latest': 'true'})
    check_release(release, notes)
    assert not release['draft']
    public = out / 'public-download'; public.mkdir(exist_ok=True)
    for asset in release['assets']:
        path = public / asset['name']
        urllib.request.urlretrieve(asset['browser_download_url'], path)
        assert 'sha256:' + digest(path) == expected[asset['name']]
    assert verifier.verify(public / archive.name, exe_path) == 18
    after = api('releases/411304264')
    assert old_assets == {a['id']: (a['name'], a['digest']) for a in after['assets']}
    assert after['tag_name'] == old['tag_name'] and after['published_at'] == old['published_at']
    result = dict(CONFIG, release_id=release['id'], release_url=release['html_url'],
                  published_at=release['published_at'], public_downloads_verified=True,
                  previous_release_unchanged=True,
                  assets=[{k: a[k] for k in ['name', 'browser_download_url', 'size', 'digest']} for a in release['assets']])
    (out / 'publication-verification.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
