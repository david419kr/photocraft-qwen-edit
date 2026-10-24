"""Publish approved 0.15.1 unchanged, preserving tested source and embedded identity."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import urllib.request
import zipfile

REPO = 'david419kr/photocraft-qwen-edit'
SOURCE = 'bf883b3d0f50049faa7b9ca6b5ba540d591350e1'
CANDIDATE = 'c7eefcb15accd4487afb8f6e8c2fcaa7fd2c9a42'
TREE = '271aab8f8e6559a275205af189b6e4aa7a66a0cf'
UPSTREAM = '60c87b9884fdae0e464614d482a3bc435cbbc578'
TAG = '261024-0.15.1'
TITLE = 'PhotoCraft Local Qwen AI - 261024 (upstream 0.15.1)'
RUNS = [40043736428,40043740136,40043740141,40043739996,40043740157,40043740124,40043740165]


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
    pr = api('pulls/13')
    assert pr['merged'] and pr['merge_commit_sha'] == SOURCE and pr['head']['sha'] == CANDIDATE
    assert run('git','diff','--name-only') == ''
    assert run('git','rev-parse',SOURCE+'^{tree}') == TREE
    assert Path('release-tag.txt').read_text().strip() == TAG
    for rid in RUNS:
        r = api(f'actions/runs/{rid}')
        assert r['head_sha'] == CANDIDATE and r['conclusion'] == 'success'
    upstream = json.loads(run('gh','api','repos/storytold/photocraft/releases/422333392'))
    assert upstream['tag_name'] == 'v0.15.1' and not upstream['draft'] and not upstream['prerelease']
    assert json.loads(run('gh','api','repos/storytold/photocraft/commits/v0.15.1'))['sha'] == UPSTREAM
    root = Path('dist'); archive = root/f'photocraft-qwen-edit-{TAG}-windows-x64.zip'
    build = json.loads((root/'candidate-build.json').read_text(encoding='utf-8-sig'))
    assert build['source_sha'] == CANDIDATE and build['upstream_sha'] == UPSTREAM
    assert build['upstream_release_id'] == 422333392 and build['rustflags'] == '-C target-feature=+crt-static'
    assert build['fork_base_sha'] == '79c22ad465fb63d33da0326dd07806e456376b66'
    assert digest(archive) == 'ea05c0b48fed7d2dd2a0a10a0eee09a900cc3de9425acc05f31b86882478f17f'
    assert build['zip'] == archive.name and build['zip_sha256'] == digest(archive)
    assert (root/(archive.name+'.sha256')).read_text(encoding='utf-8-sig').split()[0] == digest(archive)
    assert TAG in (root/'candidate-version.txt').read_text(encoding='utf-8-sig')
    with zipfile.ZipFile(archive) as z: exe_bytes = z.read('photocraft.exe')
    assert hashlib.sha256(exe_bytes).hexdigest() == build['executable_sha256']
    assert CANDIDATE.encode() in exe_bytes and TAG.encode() in exe_bytes
    exe = root/'verified-photocraft.exe'; exe.write_bytes(exe_bytes)
    spec = importlib.util.spec_from_file_location('portable','scripts/verify-qwen-portable.py')
    verifier = importlib.util.module_from_spec(spec); spec.loader.exec_module(verifier)
    assert verifier.verify(archive,exe) == 18
    provenance = dict(build, release_tag=TAG, tag_source_sha=SOURCE, validated_candidate_sha=CANDIDATE,
                      source_tree_sha=TREE, validation_runs=RUNS,
                      original_artifact_run_id=40043736428, original_artifact_id=12540874548,
                      binary_rebuilt_for_publication=False,
                      independent_artifact_verification_run=int(os.environ["GITHUB_RUN_ID"]),
                      publication_helper_sha=os.environ['GITHUB_SHA'],
                      publication_run_attempt=os.environ['GITHUB_RUN_ATTEMPT'], actual_gpu_ui_tested=False, user_reported_working=True,
                      user_reported_scope='User tested 0.15.1 and reported it works; exact CUDA/VRAM/UI scenarios were not specified',
                      validation_record='https://github.com/david419kr/photocraft-qwen-edit/pull/13',
                      sonar_analysis='not run: token absent; wrapper success is not a scan pass',
                      hardware_validation='user-reported working; no agent-run user-PC or actual CUDA/VRAM tests',
                      approval_at_utc='2026-10-24T06:50:00Z',
                      main_tree_equals_candidate=True, post_merge_ci_not_an_extra_gate=True)
    provenance_path = root/'release-provenance.json'
    provenance_path.write_text(json.dumps(provenance,indent=2)+'\n',encoding='utf-8')
    notes = (Path(__file__).parent/'release-notes.md').read_text(encoding='utf-8')
    files = [archive,Path(str(archive)+'.sha256'),provenance_path]
    expected = {p.name:'sha256:'+digest(p) for p in files}
    old = api('releases/422167100')
    old_assets = {a['id']:a['digest'] for a in old['assets']}
    refs = api('git/matching-refs/tags/'+TAG)
    exact = [r for r in refs if r['ref']=='refs/tags/'+TAG]
    if not exact:
        api('git/refs',{'ref':'refs/tags/'+TAG,'sha':SOURCE},'POST')
    assert api('git/ref/tags/'+TAG)['object'] == {'sha':SOURCE,'type':'commit','url':f'https://api.github.com/repos/{REPO}/git/commits/{SOURCE}'}
    releases = api('releases?per_page=100')
    matches = [r for r in releases if r['tag_name']==TAG]
    assert len(matches)<=1
    if matches:
        release=matches[0]
        assert release['draft'], 'Never overwrite an existing public release'
    else:
        release=api('releases',{'tag_name':TAG,'target_commitish':SOURCE,'name':TITLE,'body':notes,'draft':True,'prerelease':False},'POST')
    rid=release['id']
    assert release['name']==TITLE and release['body'].replace('\r\n','\n').strip()==notes.strip()
    assert release['target_commitish']==SOURCE and not release['prerelease']
    present={a['name']:a for a in release['assets']}
    assert len(present)==len(release['assets']) and set(present)<=set(expected)
    for name,a in present.items(): assert a['state']=='uploaded' and a['digest']==expected[name]
    for p in files:
        if p.name not in present:
            subprocess.run(['gh','release','upload',TAG,str(p),'--repo',REPO],check=True)
    release=api(f'releases/{rid}')
    assert len(release['assets'])==3 and {a['name']:a['digest'] for a in release['assets']}==expected
    assert api('git/ref/heads/main')['object']['sha']==SOURCE
    release=api(f'releases/{rid}',{'draft':False,'make_latest':'true'})
    assert not release['draft'] and not release['prerelease']
    latest=api('releases/latest'); assert latest['id']==rid
    public=root/'public-download';public.mkdir(exist_ok=True)
    for a in release['assets']:
        p=public/a['name'];urllib.request.urlretrieve(a['browser_download_url'],p)
        assert 'sha256:'+digest(p)==expected[a['name']]
    assert verifier.verify(public/archive.name,exe)==18
    assert json.loads((public/'release-provenance.json').read_text(encoding='utf-8')) == provenance
    assert (public/(archive.name+'.sha256')).read_text(encoding='utf-8-sig').split()[0] == digest(public/archive.name)
    assert api('git/ref/tags/'+TAG)['object']['sha']==SOURCE
    assert {a['id']:a['digest'] for a in api('releases/422167100')['assets']}==old_assets
    result=dict(provenance,release_id=rid,release_url=release['html_url'],published_at=release['published_at'],
                public_downloads_verified=True,previous_release_unchanged=True,
                assets=[{k:a[k] for k in ['name','browser_download_url','size','digest']} for a in release['assets']])
    (root/'publication-verification.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
    print(json.dumps(result,indent=2))


if __name__=='__main__':
    main()
