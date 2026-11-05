# Upstream 0.20.2 integration candidate

## Source and scope

Merge the exact stable upstream release `434651642`, tag `v0.20.2`, commit
`7c12f647150f8e7b2e07718f7993b1fcaa57fefe`, into fork main
`973eb64ebfb3073851fb9602201645e088c6470f` on `update/0.20.2`.
The upstream release remains read-only. Previous candidate records are preserved.
Candidate date/version: `261105-0.20.2` (Asia/Tokyo).
Main merge, tags, public release and user-PC validation are not authorized here.

## Integration decisions

- Keep the fork's effective 20 GiB PSD import ceiling through the new adaptive
  RAM/scratch admission code. Cover byte, source and standalone conversion paths,
  boundary, overflow and high-memory approval; scratch credit cannot bypass it.
- Retain the shared CMYK adjustment coverage helper and independent ink oracle,
  together with upstream's new special-Fill regression.
- Combine WARP compact shader batches with upstream's distinct vertex entry
  points for concurrent FXC compilation.
- Preserve AI cancellation when closing dialogs, upstream live color-preview
  cleanup, ExifOnly export policy and new TIFF options.
- Retain all new OpenType command registrations and Qwen commands; the combined
  live-menu floor is 644. Regenerate parity/scorecard from combined source.
- Retain both fork and upstream attribution/history entries. Restricted ArtCraft
  marks stay excluded. Preserve Korean/AI catalogs, AVIF and metadata cleaning.
- Qwen source-latent Fill/removal, reference inputs, translation isolation and
  full-free markers, model/TE/CFG/Turbo behavior, installer pins, prompt history,
  nondestructive Undo and document-bound jobs remain fork contracts. Historical
  green-reference and residency instructions are subordinate to current code.
- Candidate workflow guards/provenance include this version. Ubuntu stays enabled;
  FreeBSD remains manual-only and is not dispatched. No scheduler is changed.

## Validation and artifact policy

Run current stable full-workspace/all-target HEIF Clippy early and compare with
Rust 1.95.0. No blanket lint suppression, skipped failing tests or reduced floors.
Run the focused fork harness (zero selected tests fail), upstream platform gates,
corpus, fonts/no-HEIF, WASM/web-size, formatting/layers and packaging checks.
The PR records exact source SHA and measured pass/fail/not-run results.

Windows x64 MSVC uses Rust 1.95.0 and static CRT, AVIF enabled. The existing
packager verifies `--version`, PE imports, six-root/18-file allowlist, licenses,
ZIP SHA256 and executable/source provenance. Artifacts are not public releases.

CPU/mock tests and hosted builds do not establish actual CUDA, VRAM or interactive
UI acceptance. Local acceptance remains Generate/Edit/Fill/Remove/background
removal/reroll, cancellation and Undo/Redo, cross-document routing, Korean and
keyboard/wheel controls, translation/model switching and Metadata All/None with
PNG/AVIF/native reopen. Use a separate folder and preserve existing settings,
models and ComfyUI when the user separately requests local validation.

## Validation findings before hosted CI

- Adapted Qwen harmonization's tile comparison to upstream's guarded Tile
  equality, retaining its storage-identity shortcut and pixel comparison.
- Rust 1.95 exposed upstream codepage boolean and ATN layer-style match lints
  that stable 1.99.1 did not report. Equivalent expressions now pass both full
  workspace/all-target/locked HEIF Clippy gates without suppression.
- Rust 1.95 CMYK tests: 59 passed, one timing benchmark ignored; GPU library:
  44 passed. Central memory tests: 20 passed, including the 20 GiB boundary.
- Local I/O suite: 514 passed, 41 failed, two ignored. Remaining failures reject
  tiny PSD imports with limit zero because this sandbox exposes cgroup membership
  without its memory files. The upstream fail-closed detector was not weakened.
  Hosted CI must establish the full I/O result on its real memory environment.
- Installer tests: 14 passed. Packaging Python suite: eight passed, three
  Windows-only tests skipped locally and retained in the Windows workflow.
- Parity and scorecard were regenerated; 644/644 menu entries are live.

These are working-tree diagnostics, not a substitute for the final remote
candidate's hosted gates or a Windows artifact.
