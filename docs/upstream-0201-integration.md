# Upstream 0.20.1 integration candidate

## Source and scope

Read-only upstream `storytold/photocraft`: release ID `432155025`, stable tag
`v0.20.1`, lightweight commit `327c4cc82e737a3e0af7c147a477126d77525495`,
published `2026-11-03T01:24:40Z`. Verified live before integration. Fork base:
`730a400e7468e5a19f0e5f9cf81142e23ff749c9`; branch `update/0.20.1`.
The merge uses that exact release SHA, never upstream main. No competing open
PR, update/0.20.x branch or running Actions existed when work started.

Previous release `261102-0.19.0` (ID `430921994`) and PR #18 are complete;
candidate `ced72073ec8ca6575aa40e7644947bccb2729c74`. The unused empty draft
`430920380` remains preserved. Historical 0.7.0 release ID `409339821`, source
`79261615956ab21334e49ca1d384390272e647da`, was deployed as `261013-0.7.0`
from `df715bc41646f7d434a07be774297eaa71e3f444`; its lint exception does not apply.

Only integration, tests, candidate push, draft PR and Windows artifacts are
authorized. Main merge, tags and public release require new version-specific
approval. Final candidate SHA, PR, checks and downloadable artifact provenance
are recorded together in the PR; this document does not claim completed gates.

## Compatibility review

- Read the full supplied Library handoff and current code/tests. Preserve current
  source-latent masked Fill/removal with unmarked ROI and black/white guide;
  historical green-reference wording is superseded. Preserve reference Edit,
  explicit resize approval, multi-region removal, progress and optional Poisson.
- Keep nondestructive result layers, Undo, cancellation and document-scoped jobs.
  Review upstream cancelled-worker/scratch release and document switching changes
  against those contracts. Keep frozen reroll inputs and metadata history.
- Preserve model/TE/CFG/Turbo sigma configuration, translator isolation and
  per-request unload, loader-cache policy and two-marker full-free. No ComfyUI,
  model or installer pin changes.
- Merge both preference test groups: Qwen migration/invalid-sibling retention
  and upstream integrated-title-bar defaults. Keep last selected tool and colour
  persistence, including the fresh-profile Brush fallback and non-default Move.
- Combine Hindi AI/import/colour/plural translations and upstream multi-action
  import translations. Retain all language coverage and placeholder checks.
- Combine AVIF fixture attribution with upstream synthetic parser fixtures;
  preserve fork notices and exclude restricted ArtCraft marks. Keep AVIF,
  XMP history and Metadata None stripping, including streamed PNG cleaning.
- Retain upstream SVG clipboard, action parsing, live arrow shape/native format,
  memory-management and format fixes. Keep fork portable updater ownership,
  GPU startup, numeric hover wheel and Ctrl+Enter input paths.
- Candidate version is `261103-0.20.1`; semantic Cargo version remains `0.20.1`.
  Update candidate workflow guards, full-sweep CI selection and ZIP provenance.
  README release reference describes the already published 0.19.0, not this
  unapproved candidate.

## Validation policy

Current stable full-workspace/all-target HEIF Clippy and Rust 1.95 Clippy/build
must pass without blanket suppression. Focused fork regressions reject zero
executed tests. Preserve upstream Ubuntu, Windows, ARM64, font/no-HEIF, corpus,
WASM/web-size, formatting, layers, parity, scorecard and packaging gates.
FreeBSD is manual-only and excluded from required gates; do not dispatch it.

Windows packaging uses Rust 1.95, x64 MSVC and static CRT; verifies executable
version and PE imports, exact 18-file/six-root ZIP allowlist, licenses and SHA256.
Artifact metadata contains source/base/upstream identities and expires in 14 days.

Cloud CPU/mock tests and hosted Windows builds do not establish real CUDA/VRAM
or interactive UI acceptance. No user PC is accessed. Separate local acceptance:
verify SHA/ZIP in a separate folder; Generate, reference Edit/resize, Fill/removal,
background removal, reroll, Undo/Redo, cancellation, cross-document routing;
Korean labels/Ctrl+Enter/hover wheel; model/TE/CFG/Turbo; translator release and
two-marker full-free; PNG/AVIF/native history and Metadata All/None. Preserve
existing ComfyUI/models/settings and verify updater replacement/rollback.
