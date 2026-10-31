# Upstream 0.18.0 integration candidate

## Source identity and authorization

- Read-only upstream: `storytold/photocraft`; stable release ID `429709851`,
  tag `v0.18.0`, resolved commit `8a87a457abb43ac344deeb3fdb67541b91a72ff1`.
- Published and updated: `2026-10-31T04:02:59Z`; verified before integration.
- Fork base: `e7ed50fb5d7285880f872bc0502a1945d83b18c9` on
  `david419kr/photocraft-qwen-edit/main`; branch `update/0.18.0`.
- Previous published release: `261029-2-0.17.0`, ID `427355572`;
  its validated source was `cb32198a851ee6ff40bd34f46f8e0694823b8dc5`.
- Historical v0.7.0: ID `409339821`, SHA
  `79261615956ab21334e49ca1d384390272e647da`, published fork source
  `df715bc41646f7d434a07be774297eaa71e3f444`. Its Clippy exception does not apply.

Integration uses a merge commit with the exact release commit and current fork
main as parents. Admission checks found no existing 0.18.0 branch or open PR.
Only candidate preparation, tests, branch push, draft PR and Windows artifact
are authorized. Main merge, release tags and publication require fresh approval.
The draft PR and Actions provenance record the final candidate SHA, checks and
artifact identity; artifacts from another SHA are not acceptance evidence.

## Merge decisions

Retain current Qwen source and tests: unmarked original ROI plus black/white
selection guide and original-image latent noise masking. Historical green
reference wording does not override the current implementation. Keep Generate,
Edit/reference/resize approval, Fill/removal, native selection coverage,
colour matching, Undo, cancellation, per-document jobs and regeneration.
Keep model/TE/CFG/Turbo sigma settings, isolated translation, current per-request
translator release, loader residency and two-marker full-free. Installation
pins, models and ComfyUI are not upgraded.

Upstream adds banded PNG export. Port Metadata None's full metadata/ICC removal
and alpha-LSB scrubbing into both whole-image and streamed encoding; preserve
ExifOnly's separate policy. Add a streamed regression at 8 and 16 bits. Combine
AVIF support with upstream JPEG XL export and retain AVIF's smaller preview
budget with the new source-profile argument. Combine catalogs by context and
source key, preserving fork translations and upstream additions.
Complete fork menu and preference labels in the five newly complete languages,
and restore missing current selection/tool labels in existing catalogs. Retain
the catalog completeness and placeholder checks without exemptions.

Preserve the dated fork portable updater and its replacement/rollback ownership;
upstream's semantic-tag self-installer cannot install this distribution. Keep
upstream control-listener changes, boxed live strokes and new UI controls/tests.
Retain prior Rust 1.95 compatibility fixes where upstream refactoring overlaps.
Distinguish finite fixed-size selections from nonzero sparse canvas complements:
finite off-canvas marquee pixels survive combining and later moves. Existing
unclipped-marquee regressions retain their full coverage assertions.
README stable distribution wording remains intact. Restricted brand assets
remain excluded and icon attribution covers both additions.

## Validation and delivery

The 0.18.0 PR enables the complete testkit sweep in CI, rather than the daily
subset, so final candidate validation follows the repository gate.

Run current stable and Rust 1.95 whole-workspace/all-target HEIF Clippy early,
then focused fork tests with zero-test rejection, installer/package tests,
upstream native/platform, corpus, fonts, WASM/web size, formatting, layers,
parity and scorecard gates. FreeBSD stays manual-only and non-required; do not
run it. Preserve Ubuntu, other checks and all test/size/corpus floors.

The guarded candidate workflow builds x64 MSVC using Rust 1.95 and static CRT,
checks executable identity, PE imports and exact 18-file/six-root-entry ZIP,
and supplies SHA256 plus source/base/upstream provenance. Candidate identity
starts at `261031-0.18.0`; check the Tokyo build date before every executable build.
No publication workflow is dispatched.

## Separate local acceptance checklist

Cloud CPU/mock and CI results do not establish CUDA, VRAM or interactive UI
acceptance. Only on a separate user request: verify artifact SHA256/source,
extract separately, preserve existing installations and documents; exercise
Generate, reference Edit and resize approval, Fill/empty removal, background
removal and reroll sequentially. Check exact coverage, Undo/Redo, cancellation,
document switching, Korean UI, Ctrl+Enter, hover-wheel controls, model/TE/CFG/
Turbo settings, translation lifecycle and two-marker full-free. Save/reopen
PNG/AVIF/native prompt history and verify Metadata All/None. Check Windows
portable replacement/rollback. No user PC is accessed during candidate work.
