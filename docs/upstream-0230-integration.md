# Upstream 0.23.0 integration candidate

## Provenance and scope

- Upstream stable release ID `442232444`, tag `v0.23.0`, commit `9963efcb2f04867b56c564f876befd412b6c3c4e`. Release list and latest endpoint both report published, non-prerelease; the lightweight tag resolves to that exact SHA.
- Fork base `05d1bbbbcc6da1bea29b205831ddd25bfd798bf9`, reverified before creating `update/0.23.0`. No open fork PR or existing 0.23.0 branch was present. Previous published release: `261110-0.22.0`, ID `440080748`, PR #28; its PR #27 external upscalers are already in the base.
- Merge the exact release with two parents. Upstream remains read-only. No upstream-main merge, user-computer access, ComfyUI/model update, model redistribution, public tag, main merge or release publication is authorized.
- Candidate version `261112-0.23.0`; recheck the date before every executable build. Windows x64 MSVC remains Rust 1.95.0 with static CRT and AVIF.

## Compatibility review

The full supplied update handoff and repository integration documents were read. Current code and tests take precedence over historical green-annotation and translator residency descriptions.

- Preserve Qwen generation/edit/fill/removal, original-reference and black/white-guide routing, native layers and Undo/Redo, cancellation and document-local jobs. Model/TE/CFG/Turbo sigma, HyMT isolation and two-marker explicit full-free, installer pins and external upscaler execution remain fork contracts.
- Incorporate the shared `craft-heif` migration while retaining the separately pinned AVIF decoder and encoder. Reconcile the expanded native format picker without dropping AVIF save/open filters.
- Retain Metadata None removal, including native saves, XMP prompt history for All, and non-mutating export. Incorporate upstream PDF spot-plate export separately from user-supplied XMP.
- Combine upstream spot-channel dot gain with Quick Mask Selected Areas coverage; preserve selected-tool/native-color persistence and local erase.
- Upstream rewrites the GPU blend switch to assign a result and return after the switch. This supersedes the earlier fork FXC post-switch return compatibility fix without removing its guarantee.
- Use upstream craft-i18n lookup, command-id validation and CLDR plural handling. Preserve unconditional compressed catalog embedding, Qwen/upscaler wording and full fork coverage gates. Catalogs incorporate the exact central pin `e02939ef009218e81c44e8ec001fbb7603461de4`; fork overrides are identified by context/source differences from upstream 0.22.0. Serbian saved `sr` migrates to `sr-latn`; aliases remain accepted. Keep both upstream lookup tests and the fork's prior coverage/compression/parser tests. No missing-catalog skip is used for this distribution.
- Retain zopfli compression for catalogs and fonts, existing attribution and notices, portable updater and the package allowlist. External runtimes/weights remain separately installed.

## Validation

Preparation diagnostics (not final-SHA acceptance): CI-reuse policy 19 passed; installer 14 passed; package/updater Python suite 14 discovered, 11 passed and 3 Windows-only skipped. Full locked workspace/all-target HEIF Clippy passed on latest stable 1.99.1 (7m 11s after the corrections below). Rust 1.95.0 comparison remains in progress. These preparation checks do not substitute for final-SHA hosted gates.

Initial full-Clippy diagnostics exposed an obsolete translation-report helper reference, its now-unused optional-checkout functions, catalog test iteration against the new lazy embedded snapshot, and a new upstream preview fixture missing the fork's tool-color argument. These were corrected directly; the test fixtures and full warning gate remain. Missing embedded catalogs now fail translation tests rather than taking upstream's optional-checkout skip path. Lockfile reconciliation explicitly retains `oxideav-av1` 0.1.20, `oxideav-core` 0.1.39 and `oxideav-heif` 0.0.10 from the fork base.

The initial xtask run executed 89 binary tests: 87 passed, with failures for new-label coverage and generated scorecard freshness (the separate library target passed all 13 tests). Complete-catalog additions now cover 2,577/2,577 source keys in all 33 complete languages through the actual shared lookup, with zero malformed rows; Hindi retains its pre-existing partial status. Reuse exact existing label translations where applicable, and translate the new clone-source, document-state, pen/path and brush messages. Hebrew, Hungarian, Georgian and Turkish catalog-form validation follows the shared CLDR category counts. After corrections, the same compiled xtask suites passed all 102 tests (13 library + 89 binary). Layering passed for 30 crates; panel-command routing passed; the regenerated scorecard records 211 preferences. Runtime tests and final generated parity validation remain required.

The draft PR records the final candidate SHA, exact measured test counts and Actions/artifact URLs. Full upstream PR gates (Ubuntu, Windows, HEIF/font variants, corpus, WASM/web, lint and structural checks) and focused fork regressions must pass for that candidate. FreeBSD remains manual-only and non-required; PR #7 is not rerun. Exact-tree PR CI reuse applies only to a later separately approved main merge, never to skipping PR tests.

## Acceptance outside the cloud

Not performed here: actual CUDA Qwen Generate/Edit/Fill/blank Remove/background removal/reroll; output quality, feather boundaries, cancellation, layer visibility and Undo/Redo through the interactive Windows UI; model/TE/CFG/Turbo controls; Korean Ctrl+Enter/hover-wheel behavior; translator reload/residency and two-marker model-switch full-free under real VRAM pressure; PNG/AVIF/native prompt-history All/None save/reopen; Quick Mask Selected Areas and native-color local erase; real IllustrationJaNai and waifu2x/TensorRT inference, tiling/alpha/TTA/GPU selection, document resizing and one-step Undo/Redo. Follow `docs/upstream-070-integration.md` with the exact artifact SHA. Existing historical GPU results do not certify this candidate.
