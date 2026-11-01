# Upstream 0.19.0 integration candidate

## Source and authorization

Read-only upstream `storytold/photocraft`: stable release ID `430808229`, tag
`v0.19.0`, direct commit `2df9f807702a935184041355a86405380bda8274`, published
and updated `2026-11-01T19:36:04Z`. Verified against the live API before merging.
Fork base `e43084b400e19c8d39c6c5409df7da99fb88e2ca` on
`david419kr/photocraft-qwen-edit/main`; branch `update/0.19.0`. Integration uses
a merge commit of this exact release SHA. No open PR or candidate branch existed.
The previous completed release is `261031-0.18.0`, ID `429852989`; candidate
`70d6f1e99628015169dafae6157f4cea65e447ac` was merged and published already.
Historical 0.7.0 release ID `409339821` / source
`79261615956ab21334e49ca1d384390272e647da` was deployed as `261013-0.7.0`
from `df715bc41646f7d434a07be774297eaa71e3f444`. Its lint exception is not reused.

Only integration fixes, tests, candidate push, a draft PR and Windows artifacts
are authorized. Main merge, tags and publication require new user approval.
The final candidate SHA and matching test/artifact results are recorded in the
PR and Actions provenance; this source document does not claim a passing build.

## Integration decisions

- Preserve original-image latent masked sampling, unmarked source ROI and the
  black/white selection guide. Historical green annotation and translator
  residency wording do not override current code and tests. Keep Qwen Generate,
  reference Edit/resize approval, Fill/removal, background removal, reroll,
  nondestructive layers, Undo, cancellation and document-scoped jobs.
- Preserve model/TE/CFG/Turbo sigma settings, per-request translator release,
  isolated HyMT worker, loader-cache policy and two-marker full-free protocol.
  No ComfyUI, model or installer pin upgrade is part of this change.
- Combine upstream colour-channel command registration and filter targeting
  with the fork's ComfyUI registration and optional per-fill harmonization.
- Preserve the Brush fallback for a fresh profile and the last-selected-tool
  persistence contract; upstream now defaults new profiles to Move.
- Combine upstream held zoom edits with fork numeric speed/hover-wheel handling.
  Retain upstream input bounds and IME truncation behaviour.
- Keep the dated fork portable updater and its replacement/rollback ownership.
  Upstream's semantic-tag installer cannot install this distribution. Keep its
  manual recheck notification fix in the informational engine-check window.
- Retain XMP/prompt-history, Metadata None stripping, streamed PNG alpha-LSB
  cleaning, AVIF and Korean labels across upstream TIFF/PSD and UI changes.
- Preserve fork distribution README and attribution while adding upstream asset
  credits. Keep all translation keys required by the fork's update window.
  Restricted ArtCraft trademark assets remain excluded.
- Regenerate scorecard from the combined source. Keep every existing floor and
  all upstream tests, including the complete testkit sweep on this PR.

## Validation and artifact policy

Check current stable full-workspace/all-target HEIF Clippy and Rust 1.95 early.
Run focused fork regressions with zero-test rejection, installer/package tests,
full upstream native/platform/font/corpus/WASM/web-size/format/layers/parity and
scorecard gates at the final candidate SHA. FreeBSD remains manual-only and
non-required; Ubuntu and all other applicable gates remain enabled.

Windows packaging uses Rust 1.95, x64 MSVC and static CRT, verifies PE imports
and executable `--version`, and checks the exact 18-file/six-root portable ZIP,
licenses and SHA256. Build identity starts at `261102-0.19.0` (Tokyo date).
Artifacts include source/base/upstream identities and expire after 14 days.
No public release workflow is dispatched.

## Separate local acceptance

Cloud CPU/mock tests and Windows CI do not establish real CUDA/VRAM or interactive
UI acceptance. No user PC is accessed. On a separate user request, verify ZIP
SHA/source, extract into a separate folder, and test Generate, reference Edit,
Fill/empty removal, background removal, reroll, Undo/Redo, cancellation and
cross-document routing. Check Korean UI, Ctrl+Enter, hover wheel, model/TE/CFG/
Turbo settings, translation release and two-marker full-free; save/reopen
PNG/AVIF/native history and compare Metadata All/None. Preserve installed
ComfyUI/models/settings and check portable replacement/rollback independently.
