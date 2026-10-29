"""Publish the approved, verified Windows artifact without replacing any release."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import urllib.request
import zipfile

REPO = 'david419kr/photocraft-qwen-edit'
SOURCE = 'cb32198a851ee6ff40bd34f46f8e0694823b8dc5'
TREE = '68a20450ba620994fa973b42d52d56b24b3ae1ed'
TAG = '261029-2-0.17.0'
TITLE = 'PhotoCraft Local Qwen AI - 261029-2 (upstream 0.17.0)'
UPSTREAM = 'b8c3f3aa8e2918032cadc0e042664414a03be333'
CI_RUNS = [40760462201, 40760462134, 40760462117, 40760462097, 40760462086, 40760461578]


def run(*args, **kwargs):
    return subprocess.check_output(args, text=True, encoding='utf-8', **kwargs).strip()


def api(path, payload=None, method='PATCH'):
    args = ['gh', 'api', f'repos/{REPO}/{path}']
    if payload is not None:
        args += ['--method', method, '--input', '-']
        return json.loads(run(*args, input=json.dumps(payload)))
    return json.loads(run(*args))


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def main():
    assert os.environ['GITHUB_REPOSITORY'] == REPO
    config = json.loads((Path(__file__).parent/'artifact.json').read_text())
    assert run('git', 'rev-parse', 'HEAD') == SOURCE
    assert run('git', 'rev-parse', 'HEAD^{tree}') == TREE
    assert run('git', 'diff', '--name-only') == ''
    pr = api('pulls/16')
    assert pr['merged'] and pr['head']['sha'] == SOURCE
    merge_sha = pr['merge_commit_sha']
    assert merge_sha == 'e7ed50fb5d7285880f872bc0502a1945d83b18c9'
    merge = api('git/commits/'+merge_sha)
    assert [x['sha'] for x in merge['parents']] == ['de3663c8a60adcb95d5e0f2b11c97c446b58c42c', SOURCE]
    assert merge['tree']['sha'] == TREE
    assert api('git/ref/heads/main')['object']['sha'] == merge_sha
    assert Path('release-tag.txt').read_text().strip() == TAG
    upstream_release = json.loads(run('gh', 'api', 'repos/storytold/photocraft/releases/tags/v0.17.0'))
    upstream_ref = json.loads(run('gh', 'api', 'repos/storytold/photocraft/git/ref/tags/v0.17.0'))
    assert (upstream_release['id'], upstream_release['draft'], upstream_release['prerelease'], upstream_release['published_at'], upstream_release['updated_at'], upstream_ref['object']['sha']) == (426949298, False, False, '2026-10-28T17:39:12Z', '2026-10-28T17:39:12Z', UPSTREAM)
    for rid in CI_RUNS + [config['run_id']]:
        result = api(f'actions/runs/{rid}')
        assert result['head_sha'] == SOURCE and result['conclusion'] == 'success'
    jobs = api(f"actions/runs/{config['run_id']}/jobs")['jobs']
    assert {j['name']: j['conclusion'] for j in jobs} == {
        'early Clippy': 'success', 'regressions': 'success', 'windows': 'success'}
    root = Path('dist')
    build = json.loads((root/'candidate-build.json').read_text(encoding='utf-8-sig'))
    assert build['source_sha'] == SOURCE
    assert build['upstream_sha'] == UPSTREAM and build['upstream_release_id'] == 426949298
    assert build['upstream_tag'] == 'v0.17.0'
    assert build['fork_base_sha'] == 'de3663c8a60adcb95d5e0f2b11c97c446b58c42c'
    assert build['target'] == 'x86_64-pc-windows-msvc'
    assert build['rustflags'] == '-C target-feature=+crt-static'
    archive = root/f'photocraft-qwen-edit-{TAG}-windows-x64.zip'
    assert build['zip'] == archive.name
    assert digest(archive) == config['zip_sha256'] == build['zip_sha256']
    assert (root/(archive.name+'.sha256')).read_text(encoding='utf-8-sig').strip() == f'{digest(archive)}  {archive.name}'
    assert TAG in (root/'candidate-version.txt').read_text(encoding='utf-8-sig')
    with zipfile.ZipFile(archive) as bundle:
        exe_bytes = bundle.read('photocraft.exe')
        assert f'Release: {TAG}' in bundle.read('README.txt').decode('utf-8')
        assert f'PhotoCraft portable fork {TAG}' in bundle.read('portable.txt').decode('utf-8')
        assert len({name.split('/')[0] for name in bundle.namelist()}) == 6
    assert hashlib.sha256(exe_bytes).hexdigest() == build['executable_sha256']
    assert SOURCE.encode() in exe_bytes and TAG.encode() in exe_bytes
    exe = root/'verified-photocraft.exe'
    exe.write_bytes(exe_bytes)
    spec = importlib.util.spec_from_file_location('portable', 'scripts/verify-qwen-portable.py')
    verifier = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(verifier)
    assert verifier.verify(archive, exe) == 18
    subprocess.run(['python', str(Path(__file__).parent/'check-updater-identity.py')], check=True)
    updater = json.loads((root/'updater-identity-verification.json').read_text())
    assert updater['installed_tag'] == TAG and not updater['equal_version_offer']
    evidence = json.loads(Path('verified-artifact.json').read_text(encoding='utf-8-sig'))
    assert evidence['candidate_sha'] == SOURCE
    assert evidence['zip_sha256'] == build['zip_sha256'] and evidence['static_crt'] and evidence['amd64']
    provenance = dict(build, release_tag=TAG, embedded_release_tag=TAG,
                      tag_source_sha=merge_sha, merge_sha=merge_sha, source_tree_sha=TREE,
                      main_ci_tests={'ubuntu': {'passed': 12658, 'skipped': 95},
                                     'windows_unit': {'passed': 10094, 'skipped': 77},
                                     'windows_integration': {'passed': 2486, 'skipped': 19}},
                      focused_regressions={'passed':751, 'suites':31, 'manual_ignored':3},
                      ubuntu_gate='All nine steps passed: fmt, clippy, layers, scorecard, test, panic_hunt, doctest, wasm, i18n.',
                      validation_runs=CI_RUNS + [config['run_id']],
                      original_artifact_id=config['artifact_id'],
                      original_artifact_run_id=config['run_id'],
                      updater_identity_check=updater,
                      independent_artifact_verification=evidence,
                      publication_helper_sha=os.environ['GITHUB_SHA'],
                      publication_run_id=int(os.environ['GITHUB_RUN_ID']),
                      publication_run_attempt=os.environ['GITHUB_RUN_ATTEMPT'],
                      actual_gpu_ui_tested=False,
                      approval='Owner explicitly approved PR16 main merge followed by public release at 2026-10-29 01:03 UTC.',
                      identity_change='None: original validated candidate ZIP promoted unchanged; release tag points to the tree-identical PR merge commit.',
                      hardware_validation='No user-PC, CUDA/VRAM or interactive GPU UI checks in this cloud release task.')
    provenance_path = root/'release-provenance.json'
    provenance_path.write_text(json.dumps(provenance, indent=2)+'\n', encoding='utf-8')
    notes = (Path(__file__).parent/'release-notes.md').read_text(encoding='utf-8')
    files = [archive, Path(str(archive)+'.sha256'), provenance_path]
    expected = {path.name: 'sha256:'+digest(path) for path in files}
    releases = api('releases?per_page=100')
    previous = {r['id']: {a['id']: a['digest'] for a in r['assets']} for r in releases if r['tag_name'] != TAG}
    matching = [r for r in releases if r['tag_name'] == TAG]
    assert len(matching) <= 1
    refs = api('git/matching-refs/tags/'+TAG)
    exact = [r for r in refs if r['ref'] == 'refs/tags/'+TAG]
    if not exact:
        api('git/refs', {'ref': 'refs/tags/'+TAG, 'sha': merge_sha}, 'POST')
    assert api('git/ref/tags/'+TAG)['object']['sha'] == merge_sha
    if matching:
        release = api(f"releases/{matching[0]['id']}")
        assert release['draft'], 'Never overwrite an existing public release'
        assert release['target_commitish'] == merge_sha
    else:
        release = api('releases', {'tag_name': TAG, 'target_commitish': merge_sha,
                      'name': TITLE, 'body': notes, 'draft': True, 'prerelease': False}, 'POST')
    rid = release['id']
    present = {asset['name']: asset for asset in release['assets']}
    assert len(present) == len(release['assets']) and set(present) <= set(expected)
    for name, asset in present.items():
        assert asset['state'] == 'uploaded' and asset['digest'] == expected[name]
    for path in files:
        if path.name not in present:
            subprocess.run(['gh', 'release', 'upload', TAG, str(path), '--repo', REPO], check=True)
    release = api(f'releases/{rid}')
    assert len(release['assets']) == 3
    assert {a['name']: a['digest'] for a in release['assets']} == expected
    assert api('git/ref/heads/main')['object']['sha'] == merge_sha
    release = api(f'releases/{rid}', {'tag_name': TAG, 'target_commitish': merge_sha,
                  'name': TITLE, 'body': notes, 'draft': False, 'prerelease': False, 'make_latest': 'true'})
    assert not release['draft'] and not release['prerelease']
    assert api('releases/latest')['id'] == rid
    public = root/'public-download'
    public.mkdir(exist_ok=True)
    for asset in release['assets']:
        path = public/asset['name']
        urllib.request.urlretrieve(asset['browser_download_url'], path)
        assert 'sha256:'+digest(path) == expected[asset['name']]
    assert verifier.verify(public/archive.name, exe) == 18
    assert json.loads((public/'release-provenance.json').read_text()) == provenance
    assert api('git/ref/tags/'+TAG)['object']['sha'] == merge_sha
    for previous_id, assets in previous.items():
        assert {a['id']: a['digest'] for a in api(f'releases/{previous_id}')['assets']} == assets
    result = dict(provenance, release_id=rid, release_url=release['html_url'],
                  published_at=release['published_at'], public_downloads_verified=True,
                  previous_releases_unchanged=True,
                  assets=[{k: a[k] for k in ['name', 'browser_download_url', 'size', 'digest']} for a in release['assets']])
    (root/'publication-verification.json').write_text(json.dumps(result, indent=2)+'\n', encoding='utf-8')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
