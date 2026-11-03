"""Publish the approved, verified Windows artifact without replacing any release."""
from datetime import datetime, timedelta, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import urllib.request
import zipfile

REPO = 'david419kr/photocraft-qwen-edit'
SOURCE = 'cef30b514d9de4e9c954013550dc6c897581e86f'
TREE = 'ab3b87a7f7fd08b041d7009b567b35a6926e078e'
TAG = '261103-0.20.1'
TITLE = 'PhotoCraft Local Qwen AI - 261103 (upstream 0.20.1)'
UPSTREAM = '327c4cc82e737a3e0af7c147a477126d77525495'
CI_RUNS = [41575998369, 41575998480, 41575998343, 41575998289, 41575998070, 41576033011]


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
    validation = json.loads((Path(__file__).parent/'validation.json').read_text())
    assert datetime.now(timezone(timedelta(hours=9))).strftime('%y%m%d') == TAG.split('-')[0]
    assert validation['source_sha'] == SOURCE
    assert all(validation['ci_tests'][name]['passed'] > 0 for name in ('ubuntu', 'windows_unit', 'windows_integration', 'without_heif'))
    assert validation['focused_regressions'] == {'passed': 804, 'suites': 33, 'manual_ignored': 4}
    assert run('git', 'rev-parse', 'HEAD') == SOURCE
    assert run('git', 'rev-parse', 'HEAD^{tree}') == TREE
    assert run('git', 'diff', '--name-only') == ''
    pr = api('pulls/19')
    assert pr['merged'] and pr['head']['sha'] == SOURCE
    merge_sha = pr['merge_commit_sha']
    assert merge_sha == '6ee15434818f1229569b8f650e7316e27a8d827c'
    merge = api('git/commits/'+merge_sha)
    assert [x['sha'] for x in merge['parents']] == ['730a400e7468e5a19f0e5f9cf81142e23ff749c9', SOURCE]
    assert merge['tree']['sha'] == TREE
    assert api('git/ref/heads/main')['object']['sha'] == merge_sha
    assert Path('release-tag.txt').read_text().strip() == TAG
    upstream_release = json.loads(run('gh', 'api', 'repos/storytold/photocraft/releases/tags/v0.20.1'))
    upstream_ref = json.loads(run('gh', 'api', 'repos/storytold/photocraft/git/ref/tags/v0.20.1'))
    assert (upstream_release['id'], upstream_release['draft'], upstream_release['prerelease'], upstream_release['published_at'], upstream_release['updated_at'], upstream_ref['object']['sha']) == (432155025, False, False, '2026-11-03T01:24:40Z', '2026-11-03T01:24:40Z', UPSTREAM)
    for rid in CI_RUNS + [config['run_id']]:
        result = api(f'actions/runs/{rid}')
        assert result['head_sha'] == SOURCE and result['status'] == 'completed' and result['conclusion'] == 'success'
    required_ci_jobs = {
        'early Clippy', 'select platforms', 'web build and wasm size gate',
        'test (without heif)', 'corpus tests', 'test (windows-latest, integration)',
        'test (windows-latest, unit)', 'gate / gate (ubuntu-24.04)',
    }
    ci_jobs = {j['name']: j['conclusion'] for j in api('actions/runs/41575998369/jobs?per_page=100')['jobs']}
    assert all(ci_jobs.get(name) == 'success' for name in required_ci_jobs)
    jobs = api(f"actions/runs/{config['run_id']}/jobs")['jobs']
    assert {j['name']: j['conclusion'] for j in jobs} == {
        'early Clippy': 'success', 'regressions': 'success', 'windows': 'success'}
    artifact = api(f"actions/artifacts/{config['artifact_id']}")
    assert not artifact['expired']
    assert artifact['name'] == 'qwen-windows-x64-' + SOURCE
    assert artifact['workflow_run']['id'] == config['run_id']
    assert artifact['workflow_run']['head_sha'] == SOURCE
    root = Path('dist')
    build = json.loads((root/'candidate-build.json').read_text(encoding='utf-8-sig'))
    assert build['source_sha'] == SOURCE
    assert build['upstream_sha'] == UPSTREAM and build['upstream_release_id'] == 432155025
    assert build['upstream_tag'] == 'v0.20.1'
    assert build['fork_base_sha'] == '730a400e7468e5a19f0e5f9cf81142e23ff749c9'
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
    main_runs = api('actions/runs?head_sha='+merge_sha+'&per_page=100')['workflow_runs']
    main_run_snapshot = [{k: r[k] for k in ('id', 'name', 'head_sha', 'status', 'conclusion', 'html_url')} for r in main_runs]
    provenance = dict(build, release_tag=TAG, embedded_release_tag=TAG,
                      tag_source_sha=merge_sha, merge_sha=merge_sha, source_tree_sha=TREE,
                      candidate_ci_tests=validation['ci_tests'],
                      focused_regressions=validation['focused_regressions'],
                      local_validation=validation['local_checks'],
                      analysis_limits=validation['analysis_limits'],
                      ubuntu_gate='All nine steps passed: fmt, clippy, layers, scorecard, test, panic_hunt, doctest, wasm, i18n.',
                      validation_runs=CI_RUNS + [config['run_id']],
                      original_artifact_id=config['artifact_id'],
                      original_artifact_run_id=config['run_id'],
                      updater_identity_check=updater,
                      independent_artifact_verification=evidence,
                      publication_helper_sha=os.environ['GITHUB_SHA'],
                      publication_run_id=int(os.environ['GITHUB_RUN_ID']),
                      publication_run_attempt=os.environ['GITHUB_RUN_ATTEMPT'],
                      post_merge_ci_policy='Naturally triggered main CI monitored separately; identical merge tree does not delay approved publication.',
                      post_merge_main_runs_at_publication=main_run_snapshot,
                      actual_gpu_ui_tested=False,
                      approval='Owner approved release of completed PR19 candidate cef30b514d9de4e9c954013550dc6c897581e86f with the instruction: ㅇㅋ 릴리즈. Main merge, versioned tag and public release authorized; actual GPU/UI acceptance remains unperformed.',
                      identity_change='None: original validated candidate ZIP promoted unchanged; release tag points to the tree-identical PR merge commit.',
                      user_reported_local_validation={'passed': False, 'status': 'not_performed_for_0.20.1', 'candidate_sha': SOURCE, 'artifact_id': config['artifact_id']},
                      hardware_validation='Actual GPU/VRAM/model inference and interactive UI acceptance remain untested for 0.20.1. No user PC was accessed. Owner approved publication without claiming those validations.')
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
    public_exe = public/'verified-photocraft.exe'
    with zipfile.ZipFile(public/archive.name) as bundle:
        public_exe.write_bytes(bundle.read('photocraft.exe'))
    version_probe = public/'read-version.ps1'
    version_probe.write_text('''param([string]$Executable, [string]$OutputFile)
$ErrorActionPreference = 'Stop'
$p = Start-Process -FilePath $Executable -ArgumentList '--version' -Wait -PassThru -NoNewWindow -RedirectStandardOutput $OutputFile
if ($p.ExitCode -ne 0) { throw 'Public executable version check failed' }
''', encoding='utf-8')
    public_version_path = public/'downloaded-version.txt'
    subprocess.run(['pwsh', '-NoProfile', '-File', str(version_probe.resolve()),
                    '-Executable', str(public_exe.resolve()), '-OutputFile', str(public_version_path.resolve())], check=True)
    public_version = public_version_path.read_text(encoding='utf-8-sig').strip()
    assert public_version == evidence['version'] and TAG in public_version
    assert json.loads((public/'release-provenance.json').read_text()) == provenance
    assert api('git/ref/tags/'+TAG)['object']['sha'] == merge_sha
    for previous_id, assets in previous.items():
        assert {a['id']: a['digest'] for a in api(f'releases/{previous_id}')['assets']} == assets
    result = dict(provenance, release_id=rid, release_url=release['html_url'],
                  published_at=release['published_at'], public_downloads_verified=True,
                  public_downloaded_version=public_version,
                  previous_releases_unchanged=True,
                  assets=[{k: a[k] for k in ['name', 'browser_download_url', 'size', 'digest']} for a in release['assets']])
    (root/'publication-verification.json').write_text(json.dumps(result, indent=2)+'\n', encoding='utf-8')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
