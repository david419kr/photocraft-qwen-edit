# Qwen color-persistence candidate: 261022-2-0.12.0

This is a focused backport onto fork main `06a0ed41`, which already contains
upstream v0.12.0 (`6a2d027e`). Cargo's upstream version remains `0.12.0`.
The candidate's embedded identity is `release-tag.txt`: `261022-2-0.12.0`.
The existing updater and portable packager already accept same-day revisions;
no parser, installer, model, translation or package-layout changes are needed.

## Scope and provenance

Port the color-persistence changes from upstream PR
[storytold/photocraft#8485](https://github.com/storytold/photocraft/pull/8485):
`6dacefce`, `cbb0406c`, `432b1e3f`, and `52ed7ed2`.
These changes are pending upstream; this candidate does not merge upstream main.

- Restore foreground/background RGBA pairs for pixels and masks before the first frame.
- Remember exact CMYK picks and finite HDR/alpha values, including normal-exit changes.
- Reject malformed, non-numeric and overflowing preset colors before applying the brush.
- Preserve the v0.12.0 fork's existing tool/brush lifecycle; do not import later upstream
  tool-brush ownership changes or unrelated main features.
- Regenerate the scorecard from this fork's own preferences and retain its parity rows.

## Validation and release boundary

The candidate workflow includes this branch for Linux fork regressions and Windows
x64 Rust 1.95/static-CRT/AVIF builds. Its existing ancestry checks remain enabled.
The focused runner additionally requires nonzero color-persistence and tool-preset tests.
Each Windows ZIP must be built from the candidate SHA, embed `261022-2-0.12.0`,
and pass the existing portable verifier and SHA256 checks. Artifact provenance is
recorded by the workflow; an older ZIP must not be renamed as this release.

Local validation results are recorded in the PR. Upstream-branch test counts are
not evidence for this fork. Windows/CUDA/VRAM acceptance remains explicit until run.
Check reopening pixel and mask colors, exact CMYK picks, swap/default commands,
Qwen model/translation settings and the AI editing bar on the candidate package.

This PR prepares the release identity and candidate build. It does not create a
Git tag, publish a release, merge main, or replace the existing `261022-0.12.0` assets.
