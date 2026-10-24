# Upstream 0.15.1 candidate integration

Status: candidate validation in [draft PR #13](https://github.com/david419kr/photocraft-qwen-edit/pull/13). The draft PR records the final SHA, workflow results,
Windows download and ZIP SHA256. Main merge, tags and public release need separate
version-specific approval.

## Identity and merge provenance

- Upstream official release ID `422333392`, tag `v0.15.1`, published
  `2026-10-23T22:44:33Z`, draft=false, prerelease=false.
- Resolved tag commit: `60c87b9884fdae0e464614d482a3bc435cbbc578`.
- Fork main base: `79c22ad465fb63d33da0326dd07806e456376b66`.
- `update/0.15.1` starts from that base and merges that exact release commit
  with two parents (merge `9219ece5de52f83c8961f341c3ba90a59a9cc1f7`). No upstream main integration. Upstream remains read-only.
- Admission recheck: no open PR, no existing 0.15.1 branch, no running Actions;
  latest fork release `261024-0.14.0` ID `422167100`. No retag observed.
  Prior upstream v0.14.0 remains `ff4738810134d7f7e68b49f2e4f231b686282ff3`.
- Work is confined to the saved photocraft-qwen-edit cloud environment.

## Integration decisions

The three textual conflicts were CI, FreeBSD triggers and the engine lockfile
entry. Combine upstream's Ubuntu shared `cargo xtask ci` gate with the fork's
early full-workspace stable Clippy gate; retain Windows, corpus, web, docs,
packaging and Nix checks. FreeBSD stays manual-only/non-required; scheduled
perf/Windows ARM64 triggers remain absent. Keep reqwest alongside upstream's
roxmltree 0.21.1 update in the engine lockfile.

Upstream c3aa3f9e5 removes or weakens localization assertions. Retain the prior
UI tests and coverage requirements for this candidate, including the Korean and
preference-label tests. The new informational i18n report remains available.
No test removal, lowered floor or blanket warning suppression is used.

PR #2's Clippy cleanup is not an ancestor by its original commit IDs, but its
source edits are already present (reverse patch check succeeds); do not replay
it. Both Rust 1.95.0 and current stable 1.99.1 are checked with full-workspace,
all-targets HEIF Clippy and warnings as errors.

The core AI/backend/model/prefs/history, color persistence, AVIF, metadata,
installer and updater paths retain current main's implementations. Review the
shared engine registration, codec limits, menus/widgets and new selection
refinement integration together with focused regressions. Preserve Generate,
Edit, Fill, green annotation removal, non-destructive layers, Undo/cancellation,
document-local jobs, model/TE/CFG/Turbo sigma, HyMT isolation/residency/two-marker
full-free, XMP/Metadata None, Korean, static CRT, pins, licenses and updater.
The main background-removal prompt without hair/fur references is retained.

## Validation and artifacts

Run the nonzero-test-enforcing fork regression runner, Rust 1.95/stable Clippy,
format/layers/wasm/parity/scorecard, installer/package/updater tests and applicable
upstream CI on the final candidate SHA. The 261024 hotfix CI skip and historical
0.7.0 Clippy exception do not apply. Exact results belong in the draft PR and
Actions records; an artifact alone is not acceptance.

Windows candidate identity: `261024-0.15.1`, Rust 1.95.0, x64 MSVC static CRT,
AVIF. The packager validates PE architecture/imports/version, exact 18-file and
six-root-entry allowlist, licenses and ZIP SHA256. Provenance contains release
ID/tag/SHA, fork base, source SHA, toolchain, flags and hashes. Retention: 14 days.

## Hardware acceptance (not run)

Actual CUDA/VRAM/interactive UI testing requires separate authorization and is
not established by cloud CPU/mock checks. Useful checks with the exact ZIP in a
separate folder: Generate/Edit/Fill/Remove/reroll; original layers and Undo/Redo;
cancel and cross-document routing; foreground/background persistence; translated
prompts and repeated-run residency/full-free; Korean UI; PNG/AVIF/native history
save/reopen and Metadata All/None; updater rollback. No user PC is used here.
