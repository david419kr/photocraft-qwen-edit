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
SOURCE = 'b4ce47cf1782b0f63ebc501b31c5300169fbb096'
TREE = 'b004830b60ab0d1d5d61b28af3be9067aa19d7cb'
TAG = '261107-0.21.0'
TITLE = 'PhotoCraft Local Qwen AI - 261107 (upstream 0.21.0)'
UPSTREAM = 'efb7ebd0c0dab1fc108ee4642aa47c22694271a8'
VALIDATED_SOURCE = 'b4ce47cf1782b0f63ebc501b31c5300169fbb096'
VALIDATED_TREE = 'b004830b60ab0d1d5d61b28af3be9067aa19d7cb'
CI_RUNS = [42311791820]
VALIDATED_QWEN_RUN = 42311785190
POST_MERGE_CI_RUN = 42320159817
EXPECTED_FOCUSED = {'passed': 951, 'suites': 36, 'manual_ignored': 4}


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


def verify_post_merge(merge_sha, required_ci_jobs):
    """Permit only the owner's one-release, pinned Ubuntu runner interruption."""
    assert TAG == '261107-0.21.0'
    assert merge_sha == '7f360316cc207bc64f52cd67a92be53459ca7de5'
    assert POST_MERGE_CI_RUN == 42320159817
    runs = api('actions/runs?head_sha='+merge_sha+'&per_page=100')['workflow_runs']
    pushes = [r for r in runs if r['event'] == 'push' and r['head_sha'] == merge_sha]
    assert pushes and all(r['status'] == 'completed' for r in pushes)
    ci = [r for r in pushes if r['name'] == 'CI']
    assert len(ci) == 1 and ci[0]['id'] == POST_MERGE_CI_RUN
    assert ci[0]['conclusion'] == 'failure' and ci[0]['run_attempt'] == 1
    assert all(r['conclusion'] == 'success' for r in pushes if r['id'] != POST_MERGE_CI_RUN)
    jobs = api(f'actions/runs/{POST_MERGE_CI_RUN}/jobs?per_page=100')['jobs']
    names = {j['name']: j for j in jobs}
    assert len(names) == len(jobs)
    ubuntu = names['gate / gate (ubuntu-24.04)']
    assert ubuntu['id'] == 128245237315 and ubuntu['run_id'] == POST_MERGE_CI_RUN
    assert ubuntu['head_sha'] == merge_sha and ubuntu['run_attempt'] == 1
    assert ubuntu['status'] == 'completed' and ubuntu['conclusion'] == 'failure'
    steps = ubuntu['steps']
    gate = next(i for i, step in enumerate(steps) if step['name'] == 'Gate')
    assert gate == 12 and len(steps) == 18
    assert all(x['status'] == 'completed' and x['conclusion'] == 'success' for x in steps[:gate])
    assert steps[gate]['status'] == 'in_progress' and steps[gate]['conclusion'] is None
    assert steps[gate]['completed_at'] is None
    assert [x['name'] for x in steps[gate+1:]] == [
        'Lockfile unchanged',
        'Post Run Swatinem/rust-cache@d1527c8f67ef9af357290475d4af4b0fab8f12e0',
        'Post Check out craft-i18n', 'Post Check out craft-fonts',
        'Post Run actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1']
    assert all(x['status'] == 'pending' and x['conclusion'] is None for x in steps[gate+1:])
    others = required_ci_jobs - {'gate / gate (ubuntu-24.04)'}
    assert all(names.get(name, {}).get('conclusion') == 'success' for name in others)
    assert all(j['status'] == 'completed' and j['conclusion'] in ('success', 'skipped')
               for j in jobs if j['id'] != 128245237315)
    return [{k: r[k] for k in ('id', 'name', 'head_sha', 'status', 'conclusion', 'html_url')} for r in runs]


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
    assert SOURCE == VALIDATED_SOURCE and TREE == VALIDATED_TREE
    assert all(validation['ci_tests'][name]['passed'] > 0 for name in ('ubuntu', 'windows_unit', 'windows_integration', 'without_heif'))
    assert validation['focused_regressions'] == EXPECTED_FOCUSED
    assert EXPECTED_FOCUSED['passed'] > 0 and EXPECTED_FOCUSED['suites'] > 0
    assert run('git', 'rev-parse', 'HEAD') == SOURCE
    assert run('git', 'rev-parse', 'HEAD^{tree}') == TREE
    assert run('git', 'diff', '--name-only') == ''
    pr = api('pulls/23')
    assert pr['merged'] and pr['head']['sha'] == VALIDATED_SOURCE
    merge_sha = pr['merge_commit_sha']
    assert merge_sha == '7f360316cc207bc64f52cd67a92be53459ca7de5'
    assert validation['post_merge']['sha'] == merge_sha
    assert validation['post_merge']['tree'] == VALIDATED_TREE
    assert validation['post_merge']['ci_run'] == POST_MERGE_CI_RUN
    merge = api('git/commits/'+merge_sha)
    assert [x['sha'] for x in merge['parents']] == ['f69112e5ae550834899044c8d72eac2083c45947', VALIDATED_SOURCE]
    assert merge['tree']['sha'] == VALIDATED_TREE
    assert api('git/ref/heads/main')['object']['sha'] == merge_sha
    assert Path('release-tag.txt').read_text().strip() == TAG
    upstream_release = json.loads(run('gh', 'api', 'repos/storytold/photocraft/releases/tags/v0.21.0'))
    upstream_ref = json.loads(run('gh', 'api', 'repos/storytold/photocraft/git/ref/tags/v0.21.0'))
    assert (upstream_release['id'], upstream_release['draft'], upstream_release['prerelease'], upstream_release['published_at'], upstream_release['updated_at'], upstream_ref['object']['sha']) == (436962270, False, False, '2026-11-06T20:22:03Z', '2026-11-06T20:22:03Z', UPSTREAM)
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
    assert original_run['head_sha'] == SOURCE and original_run['status'] == 'completed' and original_run['conclusion'] == 'success'
    original_jobs = {j['name']: j['conclusion'] for j in api(f"actions/runs/{config['run_id']}/jobs?per_page=100")['jobs']}
    assert original_jobs.get('early Clippy') == 'success' and original_jobs.get('windows') == 'success'
    # The promoted bytes and all candidate checks share the same exact source.
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
    assert build['upstream_sha'] == UPSTREAM and build['upstream_release_id'] == 436962270
    assert build['upstream_tag'] == 'v0.21.0'
    assert build['fork_base_sha'] == 'f69112e5ae550834899044c8d72eac2083c45947'
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
    assert validation['post_merge']['ubuntu_waiver']['job_id'] == 128245237315
    assert validation['post_merge']['ubuntu_waiver']['scope'] == TAG
    main_run_snapshot = verify_post_merge(merge_sha, required_ci_jobs)
    provenance = dict(build, release_tag=TAG, embedded_release_tag=TAG,
                      tag_source_sha=merge_sha, merge_sha=merge_sha, source_tree_sha=TREE,
                      validated_source_sha=VALIDATED_SOURCE, validated_source_tree_sha=VALIDATED_TREE,
                      original_build_workflow_conclusion=original_run['conclusion'],
                      original_build_job_results=original_jobs,
                      candidate_ci_tests=validation['ci_tests'],
                      post_merge_ci_tests=validation['post_merge'].get('tests', {}),
                      focused_regressions=validation['focused_regressions'],
                      local_validation=validation['local_checks'],
                      analysis_limits=validation['analysis_limits'],
                      ubuntu_gate='Candidate Ubuntu passed all nine gate steps. Post-merge Ubuntu was interrupted by hosted-runner communication loss and waived by the owner for this Windows release only.',
                      validation_runs=CI_RUNS + [VALIDATED_QWEN_RUN],
                      original_artifact_id=config['artifact_id'],
                      original_artifact_run_id=config['run_id'],
                      updater_identity_check=updater,
                      independent_artifact_verification=evidence,
                      publication_helper_sha=os.environ['GITHUB_SHA'],
                      publication_run_id=int(os.environ['GITHUB_RUN_ID']),
                      publication_run_attempt=os.environ['GITHUB_RUN_ATTEMPT'],
                      post_merge_ci_policy='Only Ubuntu job 128245237315 in run 42320159817 is waived for 261107-0.21.0 after verified runner communication loss. All other required jobs and non-CI workflow wrappers must succeed; the exact interruption is rechecked immediately before publication.',
                      post_merge_main_runs_at_publication=main_run_snapshot,
                      post_merge_validation=validation['post_merge'],
                      actual_gpu_ui_tested=False,
                      approval='Owner authorized the exact reported 0.21.0 candidate and explicitly waived the interrupted post-merge Ubuntu job for this Windows release. This release exception does not alter CI configuration or waive future Ubuntu checks.',
                      identity_change='Approved upstream 0.21.0 candidate ZIP promoted unchanged; new tag points to the validated PR merge commit with an identical source tree. Built and validated candidate source are identical. Existing releases and assets are preserved.',
                      user_reported_local_validation={'status': 'not_reported', 'candidate_sha': SOURCE, 'artifact_id': config['artifact_id']},
                      hardware_validation='Actual GPU/VRAM/model inference and interactive UI acceptance remain unrun. Hosted mocks and software rendering are not equivalent to hardware acceptance. No user PC was accessed.')
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
    verify_post_merge(merge_sha, required_ci_jobs)
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

