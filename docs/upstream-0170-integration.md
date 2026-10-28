# Upstream 0.17.0 integration candidate

This is a candidate on `update/0.17.0`, based on the owner's latest main
`de3663c8a60adcb95d5e0f2b11c97c446b58c42c`. Main merge, release tags and public
publication are not authorized. The held 0.16.0 deployment is not resumed.

## Release and source identity

- Upstream repository: `storytold/photocraft`, read-only.
- Stable release ID: `426949298`; tag: `v0.17.0`; draft/prerelease: false.
- Published: `2026-10-28T17:39:12Z`.
- Exact resolved release commit: `b8c3f3aa8e2918032cadc0e042664414a03be333`.
- Admission recheck confirmed that release identity and fork main. No open PR or
  existing 0.17.0 update branch existed. The release was not a main ancestor.
- Integration uses a merge commit with the approved main and exact release SHA
  as parents. Upstream main and the former private fork are not inputs.
- Candidate source/check SHA and final identity recheck are recorded in the
  draft PR and Actions provenance; this document is part of that source tree.
- Embedded candidate identity: `261029-2-0.17.0` (Tokyo date, supported revision
  2). The executable, `release-tag.txt`, generated `README.txt` and ZIP must agree.
- Previous published portable version: `261028-2-0.16.0`.

## Integration decisions

Upstream extensively extracted engine commands, session code, UI, widgets and
tests into submodules. Preserve those module boundaries and move the fork's
changes into the corresponding definitions. Combine translation catalogs by
context/source key, keeping fork terminology and upstream additions. Preserve
upstream tests and retain the fork's additional assertions in the moved suites.

Keep Qwen Generate/Edit/Fill/removal, green annotation, optional per-fill colour
matching, non-destructive layers, Undo, cancellation and document-local jobs.
Keep current main's reference attachment/AVIF input, approximately 2K frame
budget, approval-based Edit resize, elongated Fill framing, independent removal
groups, and experimental external cutout preference. The optional external
backend remains distinct from Qwen and upstream's local selection/matting path.
No ComfyUI, model, translation-node or installer pin upgrade is included.

Keep model/encoder/CFG/Turbo controls, current main's per-translation worker
release policy, isolated HyMT runtime, loader residency and two-marker full-free.
Keep XMP prompt history, Metadata All/None and PNG alpha-LSB cleaning, AVIF,
Korean labels, tool/colour persistence and Windows static CRT. Retain the FXC
return fix and compact general layer-batch shader specifically for software
D3D12; do not pin an adapter or bypass GPU composition.

Adapt the fork bar to egui 0.37 IDs, accessibility roles, shared combo boxes and
shared docking. AI `ui.set` fields participate in the new parse-before-apply
structure, so invalid combined patches do not partially mutate state.

Upstream's new self-installer assumes upstream semantic tags and packages.
The fork's dated portable updater continues to own installation, replacement,
rollback and restart. Preserve engine update checks and the informational
window, using only fork releases; do not initialize the upstream suite installer
or offer upstream binaries. The existing persisted automatic-update preference
and single startup owner remain.

## Validation and provenance

Both Rust 1.95.0 and current stable whole-workspace/all-target HEIF Clippy run
early with `-D warnings`. Upstream's new explicit-import rules require porting
fork imports. Rust 1.95 additionally detects redundant boolean expressions and
an unnecessary closure drop; repair expressions without warnings suppression.
AI prompt history is rendered only in the UI phase, never hidden logic ticks.
Headless Korean AI UI tests use a pinned real NanumGothic fixture with its OFL
notice; missing-glyph panic checks remain enabled, including without craft-fonts.
The UI inspection JSON is assembled in smaller parts rather than raising the
macro recursion limit.

The draft PR records exact source SHA, counts and pass/fail/ignored/not-run
results. Require focused fork regressions (zero selected tests fail), native,
HEIF, fonts, corpus, wasm/web size, architecture, formatting and applicable
upstream platform workflows. No 0.7 exception, lowered floor or omitted gate
applies. FreeBSD stays optional/manual-only; no FreeBSD run is requested.

The Windows candidate workflow uses Rust 1.95.0, x64 MSVC, `+crt-static` and AVIF.
It verifies executable version, PE/imports, the exact 18-file/six-root-entry ZIP,
licenses, package identity and SHA256. Its `candidate-build.json` records source,
fork base, upstream release/tag/SHA, toolchain, flags, ZIP/executable hashes and
run URL. Artifact availability alone does not imply all checks passed.
`build_only` is for rebuilding an already validated exact source SHA; it does
not waive required gates or relabel another SHA's results.

## Local acceptance still required

CI and CPU/mock tests do not establish actual CUDA, VRAM or interactive UI
acceptance. Verify the downloaded ZIP's SHA256 and source, then extract to a
separate writable folder. Keep existing models, ComfyUI, settings and documents.
Sequentially exercise Generate, Edit with reference and resize approval, Fill,
empty removal, colour matching, both background-removal paths and reroll;
check masks, source visibility, Undo/Redo, cancellation and document switching.
Check Korean UI, Ctrl+Enter, hover-wheel sampling, model/encoder/CFG/Turbo controls,
translation/repeat-run residency and model-switch full-free, PNG/AVIF/native
history roundtrip and Metadata All/None. Check shared docking and the portable
updater's replacement/rollback on Windows. No user PC is accessed by this task.
