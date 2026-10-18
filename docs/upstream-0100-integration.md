# Upstream 0.10.0 candidate integration

Status: application validation and Windows artifact verified at `9ae61f2c6cb53a3683a72ec769d12039ffe0ddf7`.
The final follow-up changes only FreeBSD workflow policy and update documentation/state;
its exact SHA and scoped validation are recorded in PR #7. Main merge, tags and public
publication are not authorized.

## Source identity

- Fork base: `d211a1280e3fd396a8de55a83ffea13ef09b2c8d` (main).
- Upstream release: `v0.10.0`, ID `416221086`, published `2026-10-18T04:01:42Z`, not draft or prerelease.
- Resolved lightweight tag: `8407d9ccb125af52944fca9d228383459f94aaef`.
- Merge base / previously integrated release: `c699e3cda69f4852b43c0b191ab0091aaeff9d6a` (v0.9.0).
- Candidate: `update/0.10.0`; exact head and run identities are recorded in the draft PR and subsequent validation record.
- At start: no open fork PR, no existing 0.10.0 branch and no running/queued Actions. Existing completed release is `261015-0.9.0`, ID `413311718`.

## Semantic resolutions

The merge brings the release tag, not upstream main. The full Library handoff was materialized and read in this cloud environment.

- Retain the fork CLI identity using upstream's broken-pipe-safe stdout writer. Keep ComfyUI session setup together with Linux print-portal setup and the new presentation cap.
- Move all three fork update shell commands into the new engine-owned shell command table. Keep their UI implementations.
- Keep both AVIF and Cineon open extensions and AVIF read/write capabilities. Retain XMP prompt history and Metadata None handling while integrating Lab TIFF and decode-once paths.
- Resolve the automatically merged EXIF-only export option separately from Metadata None: keep camera EXIF/ICC/DPI without XMP/text, while None still strips private metadata and PNG alpha payload bits. Add foreground/background PNG round-trip coverage and UI-to-IO policy assertions. Metadata None shows Embed Color Profile unchecked/disabled, matching the exporter, while retaining the remembered EXIF/All choice.
- Combine preference schema exceptions for alternate shortcuts and ComfyUI model settings. Retain fork tool-selection regression tests alongside expanded upstream temporary-tool tests.
- Preserve all fork translation entries and incorporate upstream additions; remove duplicate catalog keys without dropping AI/update messages.
- Preserve latest stable whole-workspace/all-targets HEIF Clippy before native suites, including the new upstream variant jobs. Keep Rust 1.95 portable and focused compatibility gates, without lint exemptions.
- README.md and README.ko.md remain byte-identical to fork main. Tool inventory stays in docs/native-tools.md; removed scheduled workflows remain unscheduled.
- Qwen routing/model sampling, HyMT workers/full-free protocol and installer pins are unchanged. No ComfyUI/model upgrades.

- Package the newly embedded libfacedetection BSD-3-Clause license and existing SCOWL notices in full inside licenses/NOTICE.txt. The canonical packager and verifier retain exactly 18 files and six root entries.

## Validation

Exact-head CI, focused nonzero regressions and Windows artifact verification are recorded in [draft PR #7](https://github.com/david419kr/photocraft-qwen-edit/pull/7), including full candidate SHA, run IDs, counts and artifact hashes. The PR record is authoritative for validation status; this source document alone is not a green result.

Actual NVIDIA CUDA inference, VRAM residency/free timing, interactive Korean UI, document cancellation/Undo and update-install acceptance remain local GPU checks; cloud tests do not replace them. Existing installations, models and settings are untouched.

## FreeBSD bootstrap repair (2026-10-18)

At candidate `85ca762a1fc996df38c359ecb6f10a67eed3af40`, FreeBSD aarch64 job
`117606663497` failed before project build/tests: the shared workflow fetched
`https://static.rust-lang.org/rustup/dist/aarch64-unknown-freebsd/rustup-init`,
which returns HTTP 404. The x86_64 bootstrap succeeded. Rust's official stable
manifest (2026-10-15, Rust 1.99.1) advertises available aarch64 FreeBSD host tools;
the dated standalone archive returns HTTP 200. This is a bootstrap distinction,
not unsupported Rust compilation or a project-test failure.

The fork-local workflow preserves both VM architectures, fonts, complete workspace
build, all four original test commands and release tarball checks. Only aarch64
uses the official standalone stable installer, verified against the SHA256 from
the same manifest; x86_64 retains rustup. The archive resolver rejects unavailable
targets, unexpected origins and malformed digests, and checksum mismatches fail
before extraction. Two bootstrap regression tests pass locally and run before
both VM jobs. No test is disabled, and no upstream repository is modified.

The Rust 1.95 pre-regression Clippy command now covers the complete workspace,
all targets and HEIF, matching the current-stable gate instead of limiting
compatibility linting to four fork crates. Desktop workspace features retain AVIF.
This found one additional Rust 1.95 `nonminimal_bool` diagnostic in upstream's
`apps/photocraft/src/app_dirs.rs:376`, outside the former four-crate gate. Use the
equivalent `is_none_or` predicate; non-UTF8 and non-staging names remain untouched.
Existing migration/staging tests remain in the complete native suites. No lint
allowance or floor change is used.

The preceding candidate's downloaded Windows artifact `12128417199` was independently
verified: exactly 18 files / six roots, embedded licenses, x64 PE, no dynamic CRT
imports, EXE SHA256 `d0cc7b7b89ecb50be2365443e7943e6cb3ce5eb3777c30267624c81c2f8dffb1`,
and inner ZIP SHA256 `5c21c71a4bdee8399c7a48592d7f577b3eb1714fa3f8bf0a9b36c5ff06d2f335`.
This evidence applies only to `85ca762a`; the workflow repair requires fresh final-head
checks and an artifact. Current results remain in PR #7.

## Validated application and final workflow policy

At `9ae61f2c6cb53a3683a72ec769d12039ffe0ddf7`, full CI `39107185695`, Qwen
candidate/regressions `39107182671`, ARM64 install/run, documentation, packaging
lint and Nix passed. The fork runner executed 383 tests across 20 nonempty groups,
with no failures or ignored tests. Both Rust 1.95 and stable 1.99.1 full-workspace
Clippy passed. Exact suite counts and links are retained in PR #7.

Windows artifact `12128753534` was downloaded and independently verified against
the exact package contract, notices, x64 PE/static-CRT imports and provenance.
Its source SHA remains `9ae61f2c6cb53a3683a72ec769d12039ffe0ddf7`:

- Inner ZIP SHA256: `2dd1984275c95679a8e11afe1bd04c3288698274d74a612987f69537b59479eb`.
- EXE SHA256: `0d9661902b2a9906ddf4d6d6d3a5633ff355c651c3ce8eda988498508cc189ff`.

The user's final 2026-10-18 instruction excludes FreeBSD from this candidate's
required gates and makes it manual-only going forward. In run `39107185848`,
x86_64 passed build, tests and packaging; aarch64 was cancelled, briefly rerun
after an intervening instruction, then cancelled again under the final direction.
It is **not** a passing aarch64 result, and it must not be rerun automatically.
The aarch64 log did verify Rust installation and ongoing project compilation.

The final policy follow-up changes only `.github/workflows/freebsd.yml` and three
update documents/state files. Application code, manifests/lockfile, assets,
installer, build scripts and package inputs are byte-identical to the validated
source. Reuse the existing ZIP with its actual source SHA; do not claim that it
was rebuilt from the policy-only follow-up. Validate workflow syntax and docs at
the final SHA; preserve normal Ubuntu/general CI and Qwen regression workflows.

## Approved public release — 2026-10-18

After reporting that the local candidate works, the user explicitly approved release at
09:51 UTC. PR #7 was merged as `7426eda37bd127b9a0d804a0176322a131313c54`;
its tree exactly equals approved candidate `39b37f95bd54a3135516adec7ef31732da80613c`.
Tag `261018-0.10.0` points to that merge. This approval supersedes the earlier
candidate-only authorization boundary recorded above.

[Public release](https://github.com/david419kr/photocraft-qwen-edit/releases/tag/261018-0.10.0)
ID `416368474` was published at 09:56:13 UTC by successful
[publication run 39117251075](https://github.com/david419kr/photocraft-qwen-edit/actions/runs/39117251075).
It reuses artifact `12128753534`, built from `9ae61f2c6cb53a3683a72ec769d12039ffe0ddf7`.
No binary rebuild, heavy CI rerun or FreeBSD rerun occurred. ZIP SHA256:
`2dd1984275c95679a8e11afe1bd04c3288698274d74a612987f69537b59479eb`.

The publication job and an independent cloud download verified the public ZIP,
EXE hash, 18-file package, sidecar and provenance. GitHub latest-release metadata
has the expected stable tag, Windows asset name, size and SHA256 digest used by
the updater. The previous public release remains unchanged. Publication evidence
artifact: `12132691986`; helper/configuration: branch `publish/261018-0.10.0`.
User-reported local acceptance does not imply specific CUDA/VRAM/UI scenarios
were measured in the cloud; the earlier cloud validation limits still apply.
