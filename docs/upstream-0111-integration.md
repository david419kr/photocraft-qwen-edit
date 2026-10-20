# Upstream 0.11.1 candidate integration

Status: candidate preparation; checks and Windows artifact are pending. The draft
PR will record the exact final candidate SHA, run IDs and artifact hashes.
No prior release's test results establish this candidate's validation.

## Source identity

- Fork: `david419kr/photocraft-qwen-edit`.
- Fork main base: `a3088336c4e9817885135bd21e0308a7553fe8cd`.
- Official upstream release: `v0.11.1`, ID `418788549`, published
  `2026-10-20T19:34:02Z`, not a draft or prerelease.
- Resolved lightweight tag: `46e538b3a2bacb533e89c7a30a99bbf2d91c3381`.
- Branch: `update/0.11.1`, merging that exact release with both parents retained.
- Admission recheck: no open PR, no existing branch for this version; latest
  workflow runs completed. Previous published fork release: `261019-0.10.0`,
  release ID `417302286`. Its background-removal prompt remains in this base.
- Original Library handoff was materialized and read in the consuming saved
  cloud environment. No user PC or excluded private repository was used.

## Integration decisions

- Keep upstream's validated launch argument parser and the fork's CLI identity,
  ComfyUI startup and updater wiring.
- Combine upstream JPEG EXIF export with the fork's Metadata None stripping of
  EXIF, XMP, ICC and DPI. Extend regression assertions for Save for Web and Quick
  Export; All retains upstream's camera metadata behavior.
- Combine indexed PNG transparency handling and stereo support with prompt XMP
  retention and metadata-clean alpha handling. Test XMP and stereo together.
- Retain numeric hover-wheel and integer controls alongside upstream's new
  percentage/length conversion. Combine Serbian translations and icon notices.
- Qwen graph/routing/model/TE/CFG/Turbo logic, HyMT isolation/residency/two-marker
  full-free, prompt history, installer pins and background-removal prompt stay
  unchanged from fork main. Shared integration paths require fresh tests.
- Preserve AVIF, Korean UI, remembered tools, updater, static CRT and package
  notices. No ComfyUI/model upgrade or independent feature work.
- Existing Clippy cleanup is already in main; do not replay it. Run Rust 1.95.0
  and current stable full-workspace/all-targets HEIF Clippy with warnings denied.
- FreeBSD stays manual-only and excluded from required gates. Preserve Ubuntu,
  native variants, corpus, wasm/web, Windows ARM64, Nix, docs and packaging checks.

## Validation and local acceptance

The PR and exact-head CI/artifact records are authoritative. Until those records
show success, this document does not assert passing validation. The Windows
workflow records source/base/upstream SHA, release ID/tag, compiler/target,
static CRT imports, package contract, build log, EXE and inner ZIP SHA256.

Actual CUDA inference, VRAM reuse/full-free timing, interactive Korean UI,
Generate/Edit/Fill/Remove/reroll, Undo/cancel and cross-document routing, AVIF and
prompt-history save/reopen, Metadata All/None and updater installation remain
local acceptance checks. Use a separate candidate folder and existing model
installations. Cloud mock tests are not live GPU/UI evidence.

Main merge, tags and public releases require separate user approval.
