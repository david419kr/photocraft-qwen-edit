"""Correct release identity while preserving functional source and superseded assets."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import urllib.request
import zipfile

REPO = 'david419kr/photocraft-qwen-edit'
SOURCE = '69e1a3330fc9db52191b341a705c60fd3c60dc00'
CANDIDATE = '69e1a3330fc9db52191b341a705c60fd3c60dc00'
TREE = 'd54958575a54fe3afa4da26ee4f1bdf583ab0ccc'
UPSTREAM = 'b1794c6e0608ccfc887213dcf58398977c5cb75e'
TAG = '261028-0.16.0'
TITLE = 'PhotoCraft Local Qwen AI - 261028 (upstream 0.16.0)'
RUNS = [40601496265,40601496240,40601495910,40601584345]
EMBEDDED_TAG = TAG
CORRECTION_RUN = 40608550862
CORRECTION_CONTROLLER = '80e049db782c84ed5baa771e817ee90ce5c353dd'
CORRECTION_ARTIFACT = 12782700784
NEW_ZIP_SHA256 = 'acc6ea321dabc107b3f6c1843c152cb3240a0cb0aae9201e40e5877cf13172af'
BACKUPS = {
    'superseded-embedded261026-photocraft-qwen-edit-261028-0.16.0-windows-x64.zip': (683284632, 'sha256:a4046ece066202eca4a5ed0cde45a5718e1648bab122a69000ff3f84de8abcde'),
    'superseded-embedded261026-photocraft-qwen-edit-261028-0.16.0-windows-x64.zip.sha256': (683284725, 'sha256:92693032607438c2424b746ae4fa8bf0388f943889c7f73c2c562f06d2e6216e'),
    'superseded-embedded261026-release-provenance.json': (683284776, 'sha256:66d43d184572e8db5a7952d9f064d80262c1db5d50e176d368f1e3843632f239'),
}


def run(*args, **kwargs):
    return subprocess.check_output(args, text=True, encoding='utf-8', **kwargs).strip()


def api(path, payload=None, method='PATCH'):
    args = ['gh', 'api', f'repos/{REPO}/{path}']
    if payload is not None:
        args += ['--method', method, '--input', '-']
        return json.loads(run(*args, input=json.dumps(payload)))
    return json.loads(run(*args))


def digest(path):
    with path.open('rb') as f:
        return hashlib.file_digest(f, 'sha256').hexdigest()


def main():
    assert os.environ['GITHUB_REPOSITORY'] == REPO
    assert run('git','rev-parse','HEAD') == CANDIDATE
    assert run('git','rev-parse','HEAD^{tree}') == TREE
    assert run('git','rev-parse',CANDIDATE+'^{tree}') == TREE
    assert api('git/ref/heads/main')['object']['sha'] == SOURCE
    pr = api('pulls/15')
    assert pr['merged'] and pr['merge_commit_sha'] == SOURCE and pr['head']['sha'] == 'b5fa9a0c963697fc7bfaf9c3cbceddbdcd7a2757'
    assert run('git','diff','--name-only') == ''
    assert run('git','rev-parse',SOURCE+'^{tree}') == TREE
    assert Path('release-tag.txt').read_text().strip() == '261026-0.16.0'
    for rid in RUNS:
        r = api(f'actions/runs/{rid}')
        assert r['head_sha'] == CANDIDATE and r['conclusion'] == 'success'
    corrected_run = api(f'actions/runs/{CORRECTION_RUN}')
    assert corrected_run['head_sha'] == CORRECTION_CONTROLLER and corrected_run['conclusion'] == 'success'
    identity = json.loads(Path('dist/identity-override.json').read_text(encoding='utf-8-sig'))
    assert identity['product_source_sha'] == SOURCE and identity['controller_sha'] == CORRECTION_CONTROLLER
    assert identity['original_release_tag'] == '261026-0.16.0' and identity['requested_release_tag'] == TAG
    assert identity['functional_source_unchanged'] and identity['metadata_override_file'] == 'release-tag.txt'
    upstream = json.loads(run('gh','api','repos/storytold/photocraft/releases/423446623'))
    assert upstream['tag_name'] == 'v0.16.0' and not upstream['draft'] and not upstream['prerelease']
    assert json.loads(run('gh','api','repos/storytold/photocraft/commits/v0.16.0'))['sha'] == UPSTREAM
    root = Path('dist'); archive = root/f'photocraft-qwen-edit-{EMBEDDED_TAG}-windows-x64.zip'
    build = json.loads((root/'candidate-build.json').read_text(encoding='utf-8-sig'))
    assert build['source_sha'] == CANDIDATE and build['upstream_sha'] == UPSTREAM
    assert build['upstream_release_id'] == 423446623 and build['rustflags'] == '-C target-feature=+crt-static'
    assert build['fork_base_sha'] == 'bf883b3d0f50049faa7b9ca6b5ba540d591350e1'
    assert digest(archive) == NEW_ZIP_SHA256
    assert build['zip'] == archive.name and build['zip_sha256'] == digest(archive)
    assert (root/(archive.name+'.sha256')).read_text(encoding='utf-8-sig').split()[0] == digest(archive)
    assert EMBEDDED_TAG in (root/'candidate-version.txt').read_text(encoding='utf-8-sig')
    with zipfile.ZipFile(archive) as z:
        exe_bytes = z.read('photocraft.exe')
        assert f'Release: {TAG}' in z.read('README.txt').decode('utf-8')
        assert f'PhotoCraft portable fork {TAG}' in z.read('portable.txt').decode('utf-8')
    assert hashlib.sha256(exe_bytes).hexdigest() == build['executable_sha256']
    assert CANDIDATE.encode() in exe_bytes and EMBEDDED_TAG.encode() in exe_bytes
    exe = root/'verified-photocraft.exe'; exe.write_bytes(exe_bytes)
    spec = importlib.util.spec_from_file_location('portable','scripts/verify-qwen-portable.py')
    verifier = importlib.util.module_from_spec(spec); spec.loader.exec_module(verifier)
    assert verifier.verify(archive,exe) == 18
    original_archive_name = archive.name
    assert archive.name == f'photocraft-qwen-edit-{TAG}-windows-x64.zip'
    Path(str(archive)+'.sha256').write_text(f'{digest(archive)}  {archive.name}\n', encoding='utf-8')
    jobs = api(f'actions/runs/{CORRECTION_RUN}/jobs')['jobs']
    assert all(j['conclusion'] not in ('failure','cancelled','timed_out') for j in jobs)
    assert next(j for j in jobs if j['name']=='windows')['conclusion']=='success'
    candidate_status = [{k:j[k] for k in ['id','name','status','conclusion']} for j in jobs]
    subprocess.run(['python',str(Path(__file__).parent/'check-updater-identity.py')],check=True)
    updater_check = json.loads((root/'updater-identity-verification.json').read_text())
    assert updater_check['installed_tag']==TAG and not updater_check['equal_version_offer']
    provenance = dict(build, release_tag=TAG, zip=archive.name, original_candidate_zip=original_archive_name, embedded_release_tag=EMBEDDED_TAG, zip_sha256=digest(archive), candidate_jobs_at_publication=candidate_status, tag_source_sha=SOURCE, validated_candidate_sha=CANDIDATE,
                      source_tree_sha=TREE, validation_runs=RUNS,
                      original_artifact_run_id=CORRECTION_RUN, original_artifact_id=CORRECTION_ARTIFACT,
                      binary_rebuilt_for_publication=True, rebuild_scope="Explicit release identity and build-date metadata only; functional source unchanged", identity_override=identity, updater_identity_check=updater_check, superseded_assets=BACKUPS, corrected_from_zip_sha256="a4046ece066202eca4a5ed0cde45a5718e1648bab122a69000ff3f84de8abcde", correction_approved_at_utc="2026-10-28T00:20:00Z",
                      independent_artifact_verification_run=int(os.environ["GITHUB_RUN_ID"]),
                      publication_helper_sha=os.environ['GITHUB_SHA'],
                      publication_run_attempt=os.environ['GITHUB_RUN_ATTEMPT'], actual_gpu_ui_tested=False, user_reported_working=True,
                      user_reported_scope='Owner accepted the functional build and explicitly authorized this metadata-only rebuild/republication; no claim of owner retesting this new ZIP', identity_correction='Executable, internal README/portable marker, public tag and exact updater asset now all identify261028-0.16.0',
                      validation_record='https://github.com/david419kr/photocraft-qwen-edit/pull/15',
                      windows_nextest_leak_annotations={'unit':2,'integration':1}, historical_check_records_preserved=True,
                      sonar_workflow_conclusion='success; no claim beyond workflow result',
                      hardware_validation='user-reported working; no agent-run user-PC or actual CUDA/VRAM tests',
                      approval_at_utc='2026-10-27T23:56:00Z',
                      main_tree_equals_candidate=True, merged_source_ci_success=True)
    provenance_path = root/'release-provenance.json'
    provenance_path.write_text(json.dumps(provenance,indent=2)+'\n',encoding='utf-8')
    notes = (Path(__file__).parent/'release-notes.md').read_text(encoding='utf-8')
    files = [archive,Path(str(archive)+'.sha256'),provenance_path]
    new_expected = {p.name:'sha256:'+digest(p) for p in files}
    expected = dict(new_expected, **{name:details[1] for name,details in BACKUPS.items()})
    old = api('releases/423732880')
    old_assets = {a['id']:a['digest'] for a in old['assets']}
    refs = api('git/matching-refs/tags/'+TAG)
    exact = [r for r in refs if r['ref']=='refs/tags/'+TAG]
    if not exact:
        api('git/refs',{'ref':'refs/tags/'+TAG,'sha':SOURCE},'POST')
    assert api('git/ref/tags/'+TAG)['object'] == {'sha':SOURCE,'type':'commit','url':f'https://api.github.com/repos/{REPO}/git/commits/{SOURCE}'}
    release=api('releases/426118135')
    assert release['draft'], 'Never overwrite an existing public release'
    assert release['name']==TITLE and release['target_commitish']==SOURCE
    assert {a['name']:(a['id'],a['digest']) for a in release['assets']} == BACKUPS, 'Existing superseded assets must be preserved exactly; inspect any partial new upload'
    assert release['tag_name']==TAG or release['tag_name'].startswith('untagged-')
    rid=release['id']
    release = api(f'releases/{rid}', {'tag_name':TAG,'target_commitish':SOURCE,'name':TITLE,'body':notes,'draft':True,'prerelease':False})
    assert release['tag_name']==TAG
    assert release['body'].replace('\r\n','\n').strip()==notes.strip()
    assert release['target_commitish']==SOURCE and not release['prerelease']
    present={a['name']:a for a in release['assets']}
    assert len(present)==len(release['assets']) and set(present)<=set(expected)
    for name,a in present.items(): assert a['state']=='uploaded' and a['digest']==expected[name]
    for p in files:
        if p.name not in present:
            subprocess.run(['gh','release','upload',TAG,str(p),'--repo',REPO],check=True)
    release=api(f'releases/{rid}')
    assert len(release['assets'])==6 and {a['name']:a['digest'] for a in release['assets']}==expected
    assert {a['name']:(a['id'],a['digest']) for a in release['assets'] if a['name'] in BACKUPS}==BACKUPS
    assert api('git/ref/heads/main')['object']['sha']==SOURCE
    release=api(f'releases/{rid}',{'tag_name':TAG,'target_commitish':SOURCE,'name':TITLE,'body':notes,'draft':False,'prerelease':False,'make_latest':'true'})
    assert not release['draft'] and not release['prerelease']
    latest=api('releases/latest'); assert latest['id']==rid
    public=root/'public-download';public.mkdir(exist_ok=True)
    for a in release['assets']:
        p=public/a['name'];urllib.request.urlretrieve(a['browser_download_url'],p)
        assert 'sha256:'+digest(p)==expected[a['name']]
    assert verifier.verify(public/archive.name,exe)==18
    assert json.loads((public/'release-provenance.json').read_text(encoding='utf-8')) == json.loads(json.dumps(provenance))
    assert (public/(archive.name+'.sha256')).read_text(encoding='utf-8-sig').split()[0] == digest(public/archive.name)
    assert api('git/ref/tags/'+TAG)['object']['sha']==SOURCE
    assert {a['id']:a['digest'] for a in api('releases/423732880')['assets']}==old_assets
    result=dict(provenance,release_id=rid,release_url=release['html_url'],published_at=release['published_at'],
                public_downloads_verified=True,previous_release_unchanged=True,superseded_assets_preserved=True,
                assets=[{k:a[k] for k in ['name','browser_download_url','size','digest']} for a in release['assets']])
    (root/'publication-verification.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
    print(json.dumps(result,indent=2))


if __name__=='__main__':
    main()
