"""Verify and promote original candidate bytes. Default is verify-only; never rebuild.

Run from a clean checkout of config.build_sha, with helper files in a separate
publisher checkout. Publication requires --publish, complete enabled config, and an exact publisher branch push.
"""
import argparse
import ast
import hashlib
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import re
import subprocess
import urllib.request
import zipfile

HERE = Path(__file__).resolve().parent
C = {}
ROOT = Path('dist')


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def run(*args, **kw):
    return subprocess.check_output(args, text=True, encoding='utf-8', **kw).strip()


def api(path, payload=None, method='PATCH', repo=None):
    args = ['gh', 'api', f"repos/{repo or C['repo']}/{path}"]
    if payload is not None:
        args += ['--method', method, '--input', '-']
        return json.loads(run(*args, input=json.dumps(payload)))
    return json.loads(run(*args))


def pages(path, key=None):
    result = []
    for page in range(1, 101):
        data = api(path + ('&' if '?' in path else '?') + f'per_page=100&page={page}')
        rows = data[key] if key else data
        result.extend(rows)
        if len(rows) < 100:
            return result
    raise RuntimeError('Pagination exceeded safe limit')


def digest(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def load(path):
    return json.loads(Path(path).read_text(encoding='utf-8-sig'))


def write(path, value):
    Path(path).write_text(json.dumps(value, indent=2, sort_keys=True)+'\n', encoding='utf-8')


def validate_config(config):
    require('PENDING_' not in json.dumps(config), 'Unresolved config: publication disabled')
    for key in ('feature_sha', 'base_sha', 'feature_tree', 'build_sha', 'build_tree', 'merge_sha', 'upstream_sha', 'fork_base_sha'):
        require(re.fullmatch('[0-9a-f]{40}', config[key]), f'Invalid {key}')
    for key in ('artifact_sha256', 'regression_artifact_sha256', 'zip_sha256', 'executable_sha256'):
        require(re.fullmatch('[0-9a-f]{64}', config[key]), f'Invalid {key}')
    for key in ('pr', 'pr_ci_run', 'candidate_run', 'artifact_id', 'regression_artifact_id'):
        require(type(config[key]) is int and config[key] > 0, f'Invalid {key}')
    require(re.fullmatch(r'\d{6}-(?:(?:[2-9]|[1-9]\d+)-)?0\.23\.1', config['tag']), 'Invalid release tag')
    require(config['metadata_allowlist'] == [], 'Metadata allowance changed')
    require(config['focused_groups'] == 40, 'Focused group requirement changed')
    require(type(config['publish_enabled']) is bool, 'publish_enabled must be boolean')
    require(config['publish_tag'] == (config['tag'] if config['publish_enabled'] else ''), 'Configured publication tag mismatch')
    require(config['publisher_branch'].startswith('publish/') and not any(x in config['publisher_branch'] for x in ('*', '?', '[', ']', '..', ' ', '~', '^', ':', '\\')), 'Invalid publisher branch')


def checked_run(rid, sha, path, event=None):
    obj = api(f'actions/runs/{rid}')
    require(obj['head_sha'] == sha and obj['path'] == path, 'Workflow source mismatch')
    require(obj['repository']['full_name'] == C['repo'], 'Wrong workflow repository')
    require(obj['status'] == 'completed' and obj['conclusion'] == 'success', 'Workflow not successful')
    require(event is None or obj['event'] == event, 'Unexpected workflow event')
    jobs = pages(f"actions/runs/{rid}/attempts/{obj['run_attempt']}/jobs", 'jobs')
    require(jobs and len({j['name'] for j in jobs}) == len(jobs), 'Missing or duplicate jobs')
    require(all(j['status'] == 'completed' for j in jobs), 'Unfinished jobs')
    return obj, jobs


def verify_post_merge_jobs(jobs):
    """Skipped matrix/reusable jobs may retain unexpanded GitHub display names."""
    provenance_name = f"CI provenance (PR 0, {C['merge_sha']})"
    successful = [j for j in jobs if j['conclusion'] == 'success']
    require(len(successful) == 1 and successful[0]['name'] == provenance_name,
            'Post-main CI must reuse PR, provenance only')
    require(all(j['status'] == 'completed' and
                j['conclusion'] == ('success' if j['name'] == provenance_name else 'skipped')
                for j in jobs), 'Post-main CI not clean reuse')
    require(len(jobs) > 1, 'Post-main skipped jobs missing')
    # gh's ANSI guard applies even to captured stdout. Keep raw logs private.
    log = run('gh', 'api', '--allow-escape-sequences', f"repos/{C['repo']}/actions/jobs/{successful[0]['id']}/logs")
    # This is emitted by the executed proof script only after server-side reuse
    # validation, not by workflow YAML or a step name. Exact PR checks remain above.
    require(any(line.endswith('Reusing successful PR validation of this exact tree.')
                for line in log.splitlines()), 'Post-main reuse proof missing from job log')


def source_gates():
    require(os.environ.get('GITHUB_REPOSITORY') == C['repo'], 'Wrong repository context')
    require(run('git', 'rev-parse', 'HEAD') == C['build_sha'], 'Wrong build checkout')
    require(not run('git', 'diff', 'HEAD', '--name-only'), 'Tracked working files changed')
    for sha, tree in ((C['build_sha'], C['build_tree']), (C['feature_sha'], C['feature_tree'])):
        require(run('git', 'rev-parse', sha+'^{tree}') == tree, 'Source tree mismatch')
    changed = run('git', 'diff', '--name-only', C['feature_sha'], C['build_sha']).splitlines()
    require(set(changed) == set(C['metadata_allowlist']), 'Build differs beyond exact release metadata allowance')
    require(C['build_sha'] == C['feature_sha'] and C['build_tree'] == C['feature_tree'], 'Artifact must be from exact tested PR source')
    require(Path('release-tag.txt').read_text().strip() == C['tag'], 'Build tag mismatch')
    for ancestor in (C['upstream_sha'], C['fork_base_sha'], C['base_sha']):
        subprocess.run(['git', 'merge-base', '--is-ancestor', ancestor, C['feature_sha']], check=True)
    pr = api(f"pulls/{C['pr']}")
    require(pr['merged'] and pr['head']['sha'] == C['feature_sha'] and pr['merge_commit_sha'] == C['merge_sha'], 'PR not merged as pinned')
    require(pr['base']['ref'] == 'main' and pr['base']['repo']['full_name'] == C['repo'], 'Wrong PR destination')
    merge = api('git/commits/'+C['merge_sha'])
    require([p['sha'] for p in merge['parents']] == [C['base_sha'], C['feature_sha']], 'Merge parents mismatch')
    require(merge['tree']['sha'] == C['feature_tree'], 'Merge must have exact feature tree')
    require(api('git/ref/heads/main')['object']['sha'] == C['merge_sha'], 'Main advanced or differs')
    ci, jobs = checked_run(C['pr_ci_run'], C['feature_sha'], '.github/workflows/ci.yml', 'pull_request')
    require(ci['updated_at'] <= pr['merged_at'], 'PR CI reran after merge')
    runs = pages('actions/workflows/ci.yml/runs?event=pull_request&head_sha='+C['feature_sha'], 'workflow_runs')
    matching = [r for r in runs if r['head_branch'] == pr['head']['ref'] and r['head_repository']['id'] == pr['head']['repo']['id']]
    require(matching and max(r['id'] for r in matching) == C['pr_ci_run'], 'Pinned PR CI is not latest')
    names = {j['name']: j['conclusion'] for j in jobs}
    require(all(names.get(name) == 'success' for name in C['required_pr_jobs']), 'Required PR CI job not green, including Ubuntu')
    require(all(j['conclusion'] in ('success', 'skipped') for j in jobs), 'PR CI has unsuccessful job')
    proofs = [re.fullmatch(r'CI provenance \(PR (\d+), ([0-9a-f]{40})\)', j['name']) for j in jobs if j['conclusion'] == 'success']
    proofs = [p for p in proofs if p]
    require(len(proofs) == 1 and int(proofs[0][1]) == C['pr'], 'Missing PR provenance')
    test_merge = api('git/commits/'+proofs[0][2])
    require([p['sha'] for p in test_merge['parents']] == [C['base_sha'], C['feature_sha']] and test_merge['tree']['sha'] == C['feature_tree'], 'PR tested stale or different merge tree')
    candidate, candidate_jobs = checked_run(C['candidate_run'], C['build_sha'], '.github/workflows/qwen-candidate.yml')
    candidate_runs = pages('actions/workflows/qwen-candidate.yml/runs?head_sha='+C['build_sha'], 'workflow_runs')
    matching_candidates = [r for r in candidate_runs if r['head_branch'] == candidate['head_branch']]
    require(matching_candidates and max(r['id'] for r in matching_candidates) == C['candidate_run'], 'Pinned candidate is not latest for source and branch')
    require({j['name']: j['conclusion'] for j in candidate_jobs} == {'early Clippy': 'success', 'regressions': 'success', 'windows': 'success'}, 'Candidate jobs not all green')
    # Exact merged tree and parents were verified above. User forbids duplicate main CI waiting.
    upstream = api('releases/tags/'+C['upstream_tag'], repo=C['upstream_repo'])
    require((upstream['id'], upstream['draft'], upstream['prerelease'], upstream['published_at'], upstream['updated_at']) == (C['upstream_release_id'], False, False, C['upstream_published_at'], C['upstream_updated_at']), 'Upstream release changed')
    require(api('git/ref/tags/'+C['upstream_tag'], repo=C['upstream_repo'])['object']['sha'] == C['upstream_sha'], 'Upstream tag changed')
    return {'pr_ci': ci['html_url'], 'candidate_run': C['candidate_run'], 'post_merge_ci': 'not required: exact PR/merge tree equality verified', 'pr_test_merge': proofs[0][2]}


def download_artifact_bytes(rid, archive):
    # ZIP bytes may contain ESC; preserve them verbatim in a binary file only.
    # https://cli.github.com/manual/gh_api documents this explicit opt-in.
    with archive.open('wb') as out:
        subprocess.run(['gh', 'api', '--allow-escape-sequences',
                        f"repos/{C['repo']}/actions/artifacts/{rid}/zip"],
                       stdout=out, check=True)


def artifact(rid, name, sha256, destination):
    obj = api(f'actions/artifacts/{rid}')
    require(not obj['expired'] and obj['name'] == name and obj['digest'] == 'sha256:'+sha256, 'Artifact identity/digest mismatch')
    require(obj['workflow_run']['id'] == C['candidate_run'] and obj['workflow_run']['head_sha'] == C['build_sha'], 'Artifact not from pinned candidate')
    archive = ROOT/f'{rid}.zip'
    download_artifact_bytes(rid, archive)
    require(digest(archive) == sha256, 'Downloaded artifact digest mismatch')
    destination.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(archive) as z:
        require(len(z.namelist()) == len(set(z.namelist())), 'Duplicate artifact paths')
        for item in z.infolist():
            p = PurePosixPath(item.filename)
            require(not p.is_absolute() and '..' not in p.parts and '\\' not in item.filename and ':' not in item.filename, 'Unsafe archive path')
            require((item.external_attr >> 16) & 0o170000 != 0o120000, 'Artifact symlink rejected')
        z.extractall(destination)
    return obj


def focused_checks():
    artifact(C['regression_artifact_id'], 'qwen-regressions-'+C['build_sha'], C['regression_artifact_sha256'], ROOT/'regressions')
    results = load(ROOT/'regressions/results.json')
    tree = ast.parse(Path('scripts/check-qwen-regressions.py').read_text())
    cases = next(ast.literal_eval(n.value) for n in ast.walk(tree) if isinstance(n, ast.Assign) and any(isinstance(t, ast.Name) and t.id == 'cases' for t in n.targets))
    require(len(results) == len(cases) == C['focused_groups'], 'Missing focused groups')
    require([r['name'] for r in results] == [case[0] for case in cases], 'Focused group identity differs')
    for result, (_, args) in zip(results, cases):
        require(result['command'] == ['cargo', 'test', '--locked', *args], 'Focused command differs')
        require(result['source_sha'] == C['build_sha'] and result['ok'] is True and result['exit_code'] == 0 and result['passed'] > 0 and result['failed'] == 0, 'Focused group not positive green')
        log = (ROOT/'regressions'/(result['name']+'.log')).read_text(encoding='utf-8')
        counts = re.findall(r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored', log)
        require(counts and [sum(int(row[i]) for row in counts) for i in range(3)] == [result['passed'], 0, result['ignored']], 'Focused log counts mismatch')
    return results


def verify_binary(executable):
    output = run('pwsh', '-NoProfile', '-File', str(HERE/'verify-binary.ps1'), '-Executable', str(executable.resolve()))
    evidence = json.loads(output)
    require(C['tag'] in evidence['version'] and 'upstream 0.23.1' in evidence['version'] and 'PhotoCraft Local Qwen AI ' in evidence['version'], 'Executable identity mismatch')
    return evidence


def verify_dist():
    artifact(C['artifact_id'], 'qwen-windows-x64-'+C['build_sha'], C['artifact_sha256'], ROOT)
    build = load(ROOT/'candidate-build.json')
    expected = {'source_sha': C['build_sha'], 'upstream_sha': C['upstream_sha'], 'upstream_tag': C['upstream_tag'], 'upstream_release_id': C['upstream_release_id'], 'fork_base_sha': C['fork_base_sha'], 'target': 'x86_64-pc-windows-msvc', 'rustflags': '-C target-feature=+crt-static', 'zip_sha256': C['zip_sha256'], 'executable_sha256': C['executable_sha256'], 'zip': f"photocraft-qwen-edit-{C['tag']}-windows-x64.zip"}
    require(all(build.get(k) == v for k, v in expected.items()), 'Build metadata mismatch')
    archive = ROOT/expected['zip']
    require(digest(archive) == C['zip_sha256'], 'ZIP digest mismatch')
    exe = ROOT/'verified-photocraft.exe'
    with zipfile.ZipFile(archive) as z:
        data = z.read('photocraft.exe')
        require(hashlib.sha256(data).hexdigest() == C['executable_sha256'], 'EXE digest mismatch')
        require(C['build_sha'].encode() in data and C['tag'].encode() in data, 'Missing embedded identity')
        require(f"Release: {C['tag']}" in z.read('README.txt').decode() and f"PhotoCraft portable fork {C['tag']}" in z.read('portable.txt').decode(), 'Package tag mismatch')
        require(len({name.split('/')[0] for name in z.namelist()}) == 6, 'ZIP root contract mismatch')
        exe.write_bytes(data)
    spec = importlib.util.spec_from_file_location('portable', 'scripts/verify-qwen-portable.py')
    verifier = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(verifier)
    require(verifier.verify(archive, exe) == 18, 'ZIP contract differs')
    evidence = verify_binary(exe)
    require((ROOT/'candidate-version.txt').read_text(encoding='utf-8-sig').strip() == evidence['version'], 'Original/replayed version mismatch')
    subprocess.run(['python', str(HERE/'check-updater-identity.py')], check=True)
    updater = load(ROOT/'updater-identity-verification.json')
    require(updater['installed_tag'] == C['tag'] and not updater['equal_version_offer'], 'Updater identity mismatch')
    return build, archive, exe, verifier, evidence, updater


def release_assets(release):
    return {str(a['id']): {k: a.get(k) for k in ('name', 'digest', 'size', 'state')} for a in release['assets']}


def publish(files, provenance, evidence, verifier, exe):
    tag = C['tag']
    expected = {p.name: 'sha256:'+digest(p) for p in files}
    releases = pages('releases')
    previous = {str(r['id']): {'tag_name': r['tag_name'], 'draft': r['draft'], 'prerelease': r['prerelease'], 'body': r['body'], 'name': r['name'], 'assets': release_assets(r)} for r in releases if r['tag_name'] != tag}
    matches = [r for r in releases if r['tag_name'] == tag]
    require(len(matches) <= 1, 'Multiple releases for tag')
    # Public releases are read-only even on retry. Never patch or upload to them.
    require(not matches or matches[0]['draft'], 'Public release exists: stop and verify it read-only; never overwrite')
    refs = api('git/matching-refs/tags/'+tag)
    exact = [r for r in refs if r['ref'] == 'refs/tags/'+tag]
    require(not exact or exact[0]['object']['sha'] == C['merge_sha'], 'Existing tag points elsewhere')
    source_gates()
    if not exact:
        api('git/refs', {'ref': 'refs/tags/'+tag, 'sha': C['merge_sha']}, 'POST')
    notes = (HERE/'release-notes.md').read_text(encoding='utf-8').replace('{{TAG}}', tag)
    require('PENDING_' not in notes, 'Unresolved release notes')
    release = matches[0] if matches else api('releases', {'tag_name': tag, 'target_commitish': C['merge_sha'], 'name': C['title'], 'body': notes, 'draft': True, 'prerelease': False}, 'POST')
    require(release['target_commitish'] == C['merge_sha'] and release['draft'] and not release['prerelease'] and release['body'] == notes and release['name'] == C['title'], 'Draft does not match approved content')
    present = {a['name']: a for a in release['assets']}
    require(len(present) == len(release['assets']) and set(present) <= set(expected), 'Unexpected draft assets')
    for name, asset in present.items():
        require(asset['state'] == 'uploaded' and asset['digest'] == expected[name], 'Existing draft asset differs; do not overwrite')
    for path in files:
        if path.name not in present:
            subprocess.run(['gh', 'release', 'upload', tag, str(path), '--repo', C['repo']], check=True)
    release = api(f"releases/{release['id']}")
    require(release['draft'] and len(release['assets']) == len(files) and {a['name']: a['digest'] for a in release['assets']} == expected, 'Uploaded assets mismatch')
    source_gates()
    require(api('git/ref/tags/'+tag)['object']['sha'] == C['merge_sha'], 'Tag changed')
    release = api(f"releases/{release['id']}", {'draft': False, 'prerelease': False, 'make_latest': 'true'})
    require(not release['draft'] and not release['prerelease'] and api('releases/latest')['id'] == release['id'], 'Latest publication failed')
    public = ROOT/'public-download'
    public.mkdir(exist_ok=True)
    for asset in release['assets']:
        path = public/asset['name']
        urllib.request.urlretrieve(asset['browser_download_url'], path)
        require('sha256:'+digest(path) == expected[asset['name']], 'Public bytes differ')
    require(verifier.verify(public/files[0].name, exe) == 18, 'Public package contract mismatch')
    with zipfile.ZipFile(public/files[0].name) as z:
        public_exe = public/'photocraft.exe'
        public_exe.write_bytes(z.read('photocraft.exe'))
    require(verify_binary(public_exe)['version'] == evidence['version'], 'Public version differs')
    require(load(public/'release-provenance.json') == provenance, 'Public provenance differs')
    for rid, old in previous.items():
        now = api('releases/'+rid)
        require({k: now[k] for k in ('tag_name', 'draft', 'prerelease', 'body', 'name')} == {k: old[k] for k in ('tag_name', 'draft', 'prerelease', 'body', 'name')} and release_assets(now) == old['assets'], 'Older release changed')
    write(ROOT/'publication-verification.json', {'release_url': release['html_url'], 'release_id': release['id'], 'public_downloads_verified': True, 'previous_releases_unchanged': True, 'version': evidence['version'], 'asset_digests': expected})
    print(release['html_url'])


def publish_authorization():
    require(C['publish_enabled'] is True, 'Publication disabled in config')
    require(C['publish_tag'] == C['tag'] and os.environ.get('RELEASE_PUBLISH_APPROVED') == C['tag'], 'Exact publication tag required')
    require(os.environ.get('GITHUB_EVENT_NAME') == 'push', 'Publication requires publisher branch push')
    require(os.environ.get('GITHUB_REF') == 'refs/heads/'+C['publisher_branch'], 'Wrong publisher branch')


def main():
    global C
    require(__debug__, 'Python optimization disables production updater assertions; do not use -O')
    parser = argparse.ArgumentParser()
    parser.add_argument('--publish', action='store_true')
    args = parser.parse_args()
    C = load(HERE/'config.json')
    validate_config(C)
    if args.publish:
        publish_authorization()
    ROOT.mkdir(exist_ok=True)
    sources = source_gates()
    focused = focused_checks()
    build, archive, exe, verifier, evidence, updater = verify_dist()
    # Stable across retries: no publication run IDs/timestamps in uploaded provenance.
    provenance = {'build': build, 'release_tag': C['tag'], 'build_source_sha': C['build_sha'], 'build_tree': C['build_tree'], 'feature_sha': C['feature_sha'], 'feature_tree': C['feature_tree'], 'release_tag_commit': C['merge_sha'], 'metadata_only_differences': C['metadata_allowlist'], 'source_identity': 'Build source is the exact tested PR head; build, PR and merged main trees are identical.', 'validation': sources, 'focused_regressions': focused, 'artifact_id': C['artifact_id'], 'artifact_outer_sha256': C['artifact_sha256'], 'regression_artifact_id': C['regression_artifact_id'], 'regression_artifact_outer_sha256': C['regression_artifact_sha256'], 'binary_verification': evidence, 'updater_identity': updater, 'actual_gpu_ui_tested': False, 'actual_ai_model_inference_tested': False, 'analysis_limits': {'actual_sonarcloud_scan_ran': None, 'coverage_generation_ran': None, 'sonarcloud_note': 'Not claimed by this publisher; wrapper status is not analysis evidence.', 'manual_fuzz_performance_included': False, 'freebsd_validation_included': False, 'hardware_ui_model_inference': 'Not tested by this cloud pipeline.'}, 'post_main_policy': 'Latest complete PR CI including Ubuntu and Windows passed; exact merge tree equality checked directly. No duplicate main CI is required. No failure waiver.', 'package_policy': 'Original CI ZIP bytes promoted unchanged. External erase/upscale backends and models are optional separate user installations; no models bundled.'}
    write(ROOT/'release-provenance.json', provenance)
    write(ROOT/'verified-artifact.json', evidence)
    if args.publish:
        publish_authorization()
        publish([archive, Path(str(archive)+'.sha256'), ROOT/'release-provenance.json'], provenance, evidence, verifier, exe)
    else:
        print('Verification passed. No tag or release created; publication not requested.')


if __name__ == '__main__':
    main()
