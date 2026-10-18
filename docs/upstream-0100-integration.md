# Upstream 0.10.0 candidate integration

Status: candidate validation in progress. Main merge, tags and public publication are not authorized.

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

The preceding candidate's downloaded Windows artifact `12128417199` was independently
verified: exactly 18 files / six roots, embedded licenses, x64 PE, no dynamic CRT
imports, EXE SHA256 `d0cc7b7b89ecb50be2365443e7943e6cb3ce5eb3777c30267624c81c2f8dffb1`,
and inner ZIP SHA256 `5c21c71a4bdee8399c7a48592d7f577b3eb1714fa3f8bf0a9b36c5ff06d2f335`.
This evidence applies only to `85ca762a`; the workflow repair requires fresh final-head
checks and an artifact. Current results remain in PR #7.
