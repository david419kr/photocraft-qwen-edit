# Upstream 0.16.0 candidate integration

Status: validation in [draft PR #14](https://github.com/david419kr/photocraft-qwen-edit/pull/14). The PR records final candidate SHA, checks,
Windows artifact and ZIP SHA256. Main merge, tags and publication are not authorized.

## Identity

- Official stable release ID `423446623`, tag `v0.16.0`, published and updated
  `2026-10-25T14:17:53Z`; resolved directly to commit
  `b1794c6e0608ccfc887213dcf58398977c5cb75e`.
- Fork base `bf883b3d0f50049faa7b9ca6b5ba540d591350e1`, branch `update/0.16.0`.
  Merge `2774cb6a2ba4c7d4f101fe00b554a00349243f28` has the base and exact
  release SHA as its two parents; upstream main is not used.
- Admission recheck found no open PR or 0.16.0 branch; latest published fork
  release is `261024-0.15.1` (ID `422569915`). No tag movement/republication observed.
- Prior candidate PR #13 and its SHA/checks remain historical records; no results
  or artifacts are relabeled for this version. Saved cloud environment only.

## Compatibility decisions

Combine both sides of attribution and translation catalogs by context/source key,
retaining fork AI/Korean labels and upstream additions. Keep all regression and
localization floors. Regenerate parity/scorecard from the integrated source.

Adopt upstream's clap CLI split and new desktop launch parser while retaining
AVIF features/help and the dated fork identity in desktop `--version`. Keep new
ASL import alongside AVIF. Adapt prompt-history assignment and metadata tests to
upstream shared `Arc<str>` XMP storage without changing payloads or export policy. Both the AI sampling control regression and upstream
Color panel modes remain. Color persistence stays in the existing fork path.

Upstream introduces an informational update window and engine automation API.
Keep those APIs, but native release transport resolves only this fork's dated
releases with the existing portable updater's tag ordering. The portable updater
continues to own automatic startup checks. Both paths share the existing persisted
`general.automaticUpdateCheck` preference (no duplicate setting), avoiding
an additional startup notification. Existing verified executable replacement and
rollback remain intact. No download points to upstream distributions.

Retain AI backend/model/prompt/history implementations, green annotation removal,
non-destructive layers, Undo/cancel/document-local jobs, model/TE/CFG/Turbo sigma,
HyMT isolation/residency/two-marker full-free, XMP/Metadata None, AVIF and Korean.
Installer/model/ComfyUI pins and license assets are unchanged. Preserve current
background removal wording, tool/color persistence and static CRT.

## Gates and package

Run stable 1.99.1 full-workspace/all-target HEIF Clippy early, and the same gate
with Rust 1.95.0. Require nonempty focused fork regressions, installer/package
and updater tests, upstream Ubuntu/Windows/ARM64/Nix/docs/packaging/SonarCloud,
native/HEIF/fonts/corpus, wasm/web size and architecture checks as configured.
No old exceptions, lowered floors or wholesale warnings suppression apply.
FreeBSD remains manual-only and not required; no FreeBSD job is requested.

Windows x64 MSVC candidate uses Rust 1.95.0 and `+crt-static`, AVIF, exact
18-file/six-root-entry ZIP, licenses, PE imports/version and SHA256 validation.
Artifact provenance records upstream release/tag/SHA, fork base and exact source
SHA. Candidate identity is `261026-0.16.0`; no tag/public release is created.

## Hardware acceptance — not run

Cloud CPU/mock tests and Windows build do not establish actual CUDA, VRAM or UI
acceptance. On separately authorized hardware with the exact ZIP/hash: sequential
Generate/Edit/Fill/Remove/background removal/reroll; source layers and Undo/Redo;
cancellation and cross-document routing; model/TE/CFG/Turbo controls; translation
and repeat-run residency/model-switch two-marker full-free; Korean UI and color
persistence; PNG/AVIF/native history roundtrip, AVIF alpha and Metadata All/None;
updater replacement/rollback. No user PC is used for this candidate task.
