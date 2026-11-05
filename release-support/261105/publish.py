"""Publish the approved, verified Windows artifact without replacing any release."""
import hashlib
import importlib.util
import json
import os
import re
from pathlib import Path
import subprocess
import urllib.request
import zipfile

REPO = 'david419kr/photocraft-qwen-edit'
SOURCE = '6ec7e63bdf14d4ca0d18435d18d2c65cec4af928'
TREE = '1fd1777e9bab0f15e2f65f62495aa29b3d871065'
TAG = '261105-0.20.2'
TITLE = 'PhotoCraft Local Qwen AI - 261105 (upstream 0.20.2)'
UPSTREAM = '7c12f647150f8e7b2e07718f7993b1fcaa57fefe'
VALIDATED_SOURCE = 'f1dfdf24a7ee6ffe6d44484dd51a48f7f4b05828'
VALIDATED_TREE = 'f5d41d1ca9db554c574dfaa0be520f52b9c8602b'
CI_RUNS = [42022951448]
VALIDATED_QWEN_RUN = 42022943308
TEST_ONLY_FILE = 'crates/ui-egui/src/prefs/prefs_ui/tests.rs'
TEST_PARENT_FILE = 'crates/ui-egui/src/prefs/prefs_ui.rs'
TEST_DIFF_SHA256 = '5730cd3565937573f317fc8a3826c2ca79fd7c18305b897d5c37946cc7d287f5'
TEST_BLOB_BEFORE = 'a75ab7ced33e740ad54899241cf678afafb945ac'
TEST_BLOB_AFTER = '86a1cf64a446f11588835731855c2c27bcc0ac21'
TEST_PARENT_BLOB = '0bfb10c1b018f42ce093bf0e6811698d9dd5e72a'
POST_MERGE_CI_RUN = 42032369917
EXPECTED_FOCUSED = {'passed': 847, 'suites': 34, 'manual_ignored': 4}


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
    if not __debug__:
        raise RuntimeError('Publication verification must not run with Python optimization')
    assert os.environ['GITHUB_REPOSITORY'] == REPO
    config = json.loads((Path(__file__).parent/'artifact.json').read_text())
    validation = json.loads((Path(__file__).parent/'validation.json').read_text())
    for pending_file in ('artifact.json', 'validation.json', 'release-notes.md', 'publish.py'):
        pending_text = (Path(__file__).parent/pending_file).read_text(encoding='utf-8')
        assert ('PENDING' + '_') not in pending_text, f'Unresolved release inputs in {pending_file}'
    # Promote the approved original artifact unchanged, even when publication is later.
    assert CI_RUNS and len(SOURCE) == 40 and len(TREE) == 40
    assert len(VALIDATED_SOURCE) == 40 and len(VALIDATED_TREE) == 40
    assert validation['built_source_sha'] == SOURCE
    assert validation['source_sha'] == VALIDATED_SOURCE
    assert run('git', 'rev-parse', VALIDATED_SOURCE+'^{tree}') == VALIDATED_TREE
    assert run('git', 'diff', '--name-only', SOURCE, VALIDATED_SOURCE, '--').splitlines() == [TEST_ONLY_FILE]
    assert run('git', 'rev-parse', SOURCE+':'+TEST_ONLY_FILE) == TEST_BLOB_BEFORE
    assert run('git', 'rev-parse', VALIDATED_SOURCE+':'+TEST_ONLY_FILE) == TEST_BLOB_AFTER
    assert run('git', 'rev-parse', SOURCE+':'+TEST_PARENT_FILE) == TEST_PARENT_BLOB
    assert run('git', 'rev-parse', VALIDATED_SOURCE+':'+TEST_PARENT_FILE) == TEST_PARENT_BLOB
    exact_diff = subprocess.check_output(['git', '-c', 'core.quotePath=false', 'diff', '--no-ext-diff', '--no-textconv', '--binary', '--full-index', '--no-renames', SOURCE, VALIDATED_SOURCE, '--'])
    assert hashlib.sha256(exact_diff).hexdigest() == TEST_DIFF_SHA256
    parent_source = run('git', 'show', SOURCE+':'+TEST_PARENT_FILE)
    assert re.search(r'#\[cfg\(test\)\]\s*mod tests;', parent_source)
    assert validation['test_only_correction'] == dict(
        built_source_sha=SOURCE, built_source_tree=TREE, validated_source_sha=VALIDATED_SOURCE,
        validated_source_tree=VALIDATED_TREE, only_changed_file=TEST_ONLY_FILE,
        exact_diff_sha256=TEST_DIFF_SHA256, before_blob=TEST_BLOB_BEFORE,
        after_blob=TEST_BLOB_AFTER, unchanged_parent_blob=TEST_PARENT_BLOB,
        included_only_under_cfg_test=True)
    assert all(validation['ci_tests'][name]['passed'] > 0 for name in ('ubuntu', 'windows_unit', 'windows_integration', 'without_heif'))
    assert validation['focused_regressions'] == EXPECTED_FOCUSED
    assert EXPECTED_FOCUSED['passed'] > 0 and EXPECTED_FOCUSED['suites'] > 0
    assert run('git', 'rev-parse', 'HEAD') == SOURCE
    assert run('git', 'rev-parse', 'HEAD^{tree}') == TREE
    assert run('git', 'diff', '--name-only') == ''
    pr = api('pulls/22')
    assert pr['merged'] and pr['head']['sha'] == VALIDATED_SOURCE
    merge_sha = pr['merge_commit_sha']
    assert merge_sha == 'f69112e5ae550834899044c8d72eac2083c45947'
    assert validation['post_merge']['sha'] == merge_sha
    assert validation['post_merge']['tree'] == VALIDATED_TREE
    assert validation['post_merge']['ci_run'] == POST_MERGE_CI_RUN
    merge = api('git/commits/'+merge_sha)
    assert [x['sha'] for x in merge['parents']] == ['973eb64ebfb3073851fb9602201645e088c6470f', VALIDATED_SOURCE]
    assert merge['tree']['sha'] == VALIDATED_TREE
    assert api('git/ref/heads/main')['object']['sha'] == merge_sha
    assert Path('release-tag.txt').read_text().strip() == TAG
    upstream_release = json.loads(run('gh', 'api', 'repos/storytold/photocraft/releases/tags/v0.20.2'))
    upstream_ref = json.loads(run('gh', 'api', 'repos/storytold/photocraft/git/ref/tags/v0.20.2'))
    assert (upstream_release['id'], upstream_release['draft'], upstream_release['prerelease'], upstream_release['published_at'], upstream_release['updated_at'], upstream_ref['object']['sha']) == (434651642, False, False, '2026-11-05T01:48:22Z', '2026-11-05T01:48:22Z', UPSTREAM)
    for rid in CI_RUNS + [VALIDATED_QWEN_RUN]:
        result = api(f'actions/runs/{rid}')
        assert result['head_sha'] == VALIDATED_SOURCE and result['status'] == 'completed' and result['conclusion'] == 'success'
    required_ci_jobs = {
        'early Clippy', 'select platforms', 'web build and wasm size gate',
        'test (without heif)', 'corpus tests', 'test (windows-latest, integration)',
        'test (windows-latest, unit)', 'gate / gate (ubuntu-24.04)',
    }
    for rid in CI_RUNS:
        candidate_jobs = {j['name']: j['conclusion'] for j in api(f'actions/runs/{rid}/jobs?per_page=100')['jobs']}
        assert all(candidate_jobs.get(name) == 'success' for name in required_ci_jobs)
    original_run = api(f"actions/runs/{config['run_id']}")
    assert original_run['head_sha'] == SOURCE and original_run['status'] == 'completed'
    original_jobs = {j['name']: j['conclusion'] for j in api(f"actions/runs/{config['run_id']}/jobs?per_page=100")['jobs']}
    assert original_jobs.get('early Clippy') == 'success' and original_jobs.get('windows') == 'success'
    # Original full workflow status is recorded, not misrepresented as successful.
    # Full CI and all Qwen jobs must pass at the separately pinned test-corrected source.
    jobs = api(f'actions/runs/{VALIDATED_QWEN_RUN}/jobs?per_page=100')['jobs']
    assert {j['name']: j['conclusion'] for j in jobs} == {
        'early Clippy': 'success', 'regressions': 'success', 'windows': 'success'}
    artifact = api(f"actions/artifacts/{config['artifact_id']}")
    assert not artifact['expired']
    assert artifact['digest'] == 'sha256:' + config['outer_artifact_sha256']
    assert artifact['name'] == 'qwen-windows-x64-' + SOURCE
    assert artifact['workflow_run']['id'] == config['run_id']
    assert artifact['workflow_run']['head_sha'] == SOURCE
    root = Path('dist')
    build = json.loads((root/'candidate-build.json').read_text(encoding='utf-8-sig'))
    assert config['source_sha'] == SOURCE and config['release_tag'] == TAG
    assert build['source_sha'] == SOURCE
    assert build['upstream_sha'] == UPSTREAM and build['upstream_release_id'] == 434651642
    assert build['upstream_tag'] == 'v0.20.2'
    assert build['fork_base_sha'] == '973eb64ebfb3073851fb9602201645e088c6470f'
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
    assert hashlib.sha256(exe_bytes).hexdigest() == build['executable_sha256'] == config['executable_sha256']
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
    main_push_runs = [r for r in main_runs if r['event'] == 'push' and r['head_sha'] == merge_sha]
    assert main_push_runs and all(r['status'] == 'completed' and r['conclusion'] == 'success' for r in main_push_runs)
    main_ci = [r for r in main_runs if r['name'] == 'CI' and r['head_sha'] == merge_sha and r['event'] == 'push']
    assert len(main_ci) == 1 and main_ci[0]['id'] == POST_MERGE_CI_RUN and main_ci[0]['status'] == 'completed' and main_ci[0]['conclusion'] == 'success'
    main_jobs = {j['name']: j['conclusion'] for j in api(f"actions/runs/{main_ci[0]['id']}/jobs?per_page=100")['jobs']}
    assert all(main_jobs.get(name) == 'success' for name in required_ci_jobs)
    main_run_snapshot = [{k: r[k] for k in ('id', 'name', 'head_sha', 'status', 'conclusion', 'html_url')} for r in main_runs]
    provenance = dict(build, release_tag=TAG, embedded_release_tag=TAG,
                      tag_source_sha=merge_sha, merge_sha=merge_sha, source_tree_sha=TREE,
                      validated_source_sha=VALIDATED_SOURCE, validated_source_tree_sha=VALIDATED_TREE,
                      test_only_correction=validation['test_only_correction'],
                      original_build_workflow_conclusion=original_run['conclusion'],
                      original_build_job_results=original_jobs,
                      post_merge_ci_tests=validation['ci_tests'],
                      candidate_ci_tests=validation.get('candidate_ci_tests', {}),
                      original_built_source_ci=validation['original_built_source_ci'],
                      focused_regressions=validation['focused_regressions'],
                      local_validation=validation['local_checks'],
                      analysis_limits=validation['analysis_limits'],
                      ubuntu_gate='All nine steps passed: fmt, clippy, layers, scorecard, test, panic_hunt, doctest, wasm, i18n.',
                      validation_runs=CI_RUNS + [VALIDATED_QWEN_RUN],
                      original_artifact_id=config['artifact_id'],
                      original_artifact_run_id=config['run_id'],
                      updater_identity_check=updater,
                      independent_artifact_verification=evidence,
                      publication_helper_sha=os.environ['GITHUB_SHA'],
                      publication_run_id=int(os.environ['GITHUB_RUN_ID']),
                      publication_run_attempt=os.environ['GITHUB_RUN_ATTEMPT'],
                      post_merge_ci_policy='Main push CI and every required platform job must complete successfully before publication.',
                      post_merge_main_runs_at_publication=main_run_snapshot,
                      post_merge_validation=validation['post_merge'],
                      actual_gpu_ui_tested=False,
                      approval='Owner reported this candidate works locally and authorized merge and release after CI succeeds.',
                      identity_change='Approved upstream 0.20.2 candidate ZIP promoted unchanged; new tag points to the validated PR merge commit. The built source differs only by the pinned cfg(test)-only correction, with exact complete diff and blob hashes verified; artifact bytes and embedded source identity are unchanged. Existing releases and assets are preserved.',
                      user_reported_local_validation={'passed': True, 'status': 'owner_reported_working_locally', 'candidate_sha': SOURCE, 'artifact_id': config['artifact_id']},
                      hardware_validation='The owner reported local success. Comprehensive actual GPU/VRAM/model inference and interactive UI validation was not independently performed by this release process. No user PC was accessed.')
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
    final_ci = api(f'actions/runs/{POST_MERGE_CI_RUN}')
    assert final_ci['head_sha'] == merge_sha and final_ci['status'] == 'completed' and final_ci['conclusion'] == 'success'
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

