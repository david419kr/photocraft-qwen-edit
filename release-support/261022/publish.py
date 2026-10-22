"""Publish the approved 0.11.1 source, preserving date-only build identity provenance."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import urllib.request
import zipfile

REPO = 'david419kr/photocraft-qwen-edit'
SOURCE = '312561fff855408f9ae28a6d9d8b51ef13a5550b'
CANDIDATE = '03d79a744aa22b5dc9517956d398ae62f63ec159'
TREE = '267db4303650ce74a23622f473371eb1a0fbad1b'
UPSTREAM = '46e538b3a2bacb533e89c7a30a99bbf2d91c3381'
TAG = '261022-0.11.1'
TITLE = 'PhotoCraft Local Qwen AI - 261022 (upstream 0.11.1)'
RUNS = [39503442677,39503436689,39503475840,39503442859,39503442807,39503443273]


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
    assert run('git','rev-parse','HEAD') == SOURCE
    assert run('git','rev-parse','HEAD^{tree}') == TREE
    assert run('git','rev-parse',CANDIDATE+'^{tree}') == TREE
    assert api('git/ref/heads/main')['object']['sha'] == SOURCE
    pr = api('pulls/8')
    assert pr['merged'] and pr['merge_commit_sha'] == SOURCE and pr['head']['sha'] == CANDIDATE
    assert run('git','diff','--name-only') == 'release-tag.txt'
    assert Path('release-tag.txt').read_text().strip() == TAG
    for rid in RUNS:
        r = api(f'actions/runs/{rid}')
        assert r['head_sha'] == CANDIDATE and r['conclusion'] == 'success'
    upstream = json.loads(run('gh','api','repos/storytold/photocraft/releases/418788549'))
    assert upstream['tag_name'] == 'v0.11.1' and not upstream['draft'] and not upstream['prerelease']
    assert json.loads(run('gh','api','repos/storytold/photocraft/git/ref/tags/v0.11.1'))['object']['sha'] == UPSTREAM
    root = Path('dist'); archive = root/f'photocraft-qwen-edit-{TAG}-windows-x64.zip'
    build = json.loads((root/'candidate-build.json').read_text(encoding='utf-8-sig'))
    assert build['source_sha'] == SOURCE and build['upstream_sha'] == UPSTREAM
    assert build['upstream_release_id'] == 418788549 and build['rustflags'] == '-C target-feature=+crt-static'
    assert build['zip'] == archive.name and build['zip_sha256'] == digest(archive)
    assert TAG in (root/'candidate-version.txt').read_text(encoding='utf-8-sig')
    with zipfile.ZipFile(archive) as z: exe_bytes = z.read('photocraft.exe')
    assert hashlib.sha256(exe_bytes).hexdigest() == build['executable_sha256']
    assert SOURCE.encode() in exe_bytes and TAG.encode() in exe_bytes
    exe = root/'verified-photocraft.exe'; exe.write_bytes(exe_bytes)
    spec = importlib.util.spec_from_file_location('portable','scripts/verify-qwen-portable.py')
    verifier = importlib.util.module_from_spec(spec); spec.loader.exec_module(verifier)
    assert verifier.verify(archive,exe) == 18
    provenance = dict(build, release_tag=TAG, tag_source_sha=SOURCE, validated_candidate_sha=CANDIDATE,
                      source_tree_sha=TREE, validation_runs=RUNS,
                      build_identity_override={'path':'release-tag.txt','committed_value':'261021-0.11.1','build_value':TAG},
                      publication_helper_sha=os.environ['GITHUB_SHA'],
                      publication_run_attempt=os.environ['GITHUB_RUN_ATTEMPT'], actual_gpu_ui_tested=False)
    provenance_path = root/'release-provenance.json'
    provenance_path.write_text(json.dumps(provenance,indent=2)+'\n',encoding='utf-8')
    notes = (Path(__file__).parent/'release-notes.md').read_text(encoding='utf-8')
    files = [archive,Path(str(archive)+'.sha256'),provenance_path]
    expected = {p.name:'sha256:'+digest(p) for p in files}
    old = api('releases/417302286')
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
    assert api('git/ref/tags/'+TAG)['object']['sha']==SOURCE
    assert {a['id']:a['digest'] for a in api('releases/417302286')['assets']}==old_assets
    result=dict(provenance,release_id=rid,release_url=release['html_url'],published_at=release['published_at'],
                public_downloads_verified=True,previous_release_unchanged=True,
                assets=[{k:a[k] for k in ['name','browser_download_url','size','digest']} for a in release['assets']])
    (root/'publication-verification.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
    print(json.dumps(result,indent=2))


if __name__=='__main__':
    main()
