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

Preparation diagnostics (not final-SHA acceptance): CI-reuse policy 19 passed; installer 14 passed; package/updater Python suite 14 discovered, 11 passed and 3 Windows-only skipped. Full locked workspace/all-target HEIF Clippy passed on latest stable 1.99.1 (7m 11s after the corrections below). The full Rust 1.95.0 comparison also passed (42m 49s). These preparation checks do not substitute for final-SHA hosted gates.

Initial full-Clippy diagnostics exposed an obsolete translation-report helper reference, its now-unused optional-checkout functions, catalog test iteration against the new lazy embedded snapshot, and a new upstream preview fixture missing the fork's tool-color argument. These were corrected directly; the test fixtures and full warning gate remain. Missing embedded catalogs now fail translation tests rather than taking upstream's optional-checkout skip path. Lockfile reconciliation explicitly retains `oxideav-av1` 0.1.20, `oxideav-core` 0.1.39 and `oxideav-heif` 0.0.10 from the fork base.

The initial xtask run executed 89 binary tests: 87 passed, with failures for new-label coverage and generated scorecard freshness (the separate library target passed all 13 tests). Complete-catalog additions now cover 2,577/2,577 source keys in all 33 complete languages through the actual shared lookup, with zero malformed rows; Hindi retains its pre-existing partial status. Reuse exact existing label translations where applicable, and translate the new clone-source, document-state, pen/path and brush messages. Hebrew, Hungarian, Georgian and Turkish catalog-form validation follows the shared CLDR category counts. After corrections, the same compiled xtask suites passed all 102 tests (13 library + 89 binary). Layering passed for 30 crates; panel-command routing passed; the regenerated scorecard records 211 preferences. Runtime tests and final generated parity validation remain required.

The draft PR records the final candidate SHA, exact measured test counts and Actions/artifact URLs. Full upstream PR gates (Ubuntu, Windows, HEIF/font variants, corpus, WASM/web, lint and structural checks) and focused fork regressions must pass for that candidate. FreeBSD remains manual-only and non-required; PR #7 is not rerun. Exact-tree PR CI reuse applies only to a later separately approved main merge, never to skipping PR tests.

## Acceptance outside the cloud

Not performed here: actual CUDA Qwen Generate/Edit/Fill/blank Remove/background removal/reroll; output quality, feather boundaries, cancellation, layer visibility and Undo/Redo through the interactive Windows UI; model/TE/CFG/Turbo controls; Korean Ctrl+Enter/hover-wheel behavior; translator reload/residency and two-marker model-switch full-free under real VRAM pressure; PNG/AVIF/native prompt-history All/None save/reopen; Quick Mask Selected Areas and native-color local erase; real IllustrationJaNai and waifu2x/TensorRT inference, tiling/alpha/TTA/GPU selection, document resizing and one-step Undo/Redo. Follow `docs/upstream-070-integration.md` with the exact artifact SHA. Existing historical GPU results do not certify this candidate.

## First candidate validation

Initial merge candidate `91a01ca764e578d1b96435fd5046a0aa2f456f04` is preserved in draft [PR #29](https://github.com/david419kr/photocraft-qwen-edit/pull/29). Both hosted latest-stable Clippy gates and the hosted Rust 1.95 Clippy gate passed. The web build/size gate and HEIF-disabled test job passed; full platform tests were still running when this checkpoint was written.

The expanded local i18n filter executed 146 tests: 138 passed and eight strict first-frame glyph tests failed without the optional craft-fonts build input. Source inspection confirmed that the eager per-character provider is installed only with that font input; the older focused filter did not include these live tests. Full upstream PR CI already supplies the pinned fonts. Supply the identical `b076417b5880bd4154bd22f00695b1d28c6bea83` font input to candidate regressions, with `CRAFT_FONTS_REQUIRED=1`, rather than dropping live tests or permitting missing glyphs. The Windows packaging job and runtime/model dependencies are unchanged. Final corrective-SHA results belong in the PR.

The first candidate’s HEIF-disabled job executed 2,350 tests, all passed, with 18 skipped. Its browser release passed the existing shipped-file gate (WASM 33,407,517 raw bytes, distributed as 14,947,909 gzip bytes). These results are historical once the corrective commit is pushed and must not be substituted for its full gates. Local generated menu inventory validation reported 668/668 wired entries and left `docs/parity-checklist.md` unchanged; this measures menu wiring, not complete application behavior.

With the required pinned fonts, the local localization rerun passed all 146 tests (zero failed or ignored), including the eight previous strict-glyph failures. The first candidate corpus job also passed 7,536 tests across 48 targets, with 45 ignored. Keep these initial-run records alongside the corrective candidate’s complete rerun evidence.

The first Windows artifact passed the static-CRT import, version, allowlist, provenance and package checks, plus three Windows updater/rollback tests. Historical [artifact 13905713864](https://github.com/david419kr/photocraft-qwen-edit/actions/runs/43129281565/artifacts/13905713864), from `91a01ca764e578d1b96435fd5046a0aa2f456f04`, contains `photocraft-qwen-edit-261112-0.23.0-windows-x64.zip` with SHA256 `729908091c454c6faa4e55c607d7649de3131cf4d1baaa4735ddc2a82063bc1b`. The downloaded ZIP hash was independently checked against its build metadata. This artifact is superseded by the corrective candidate and is not final acceptance.

A full local UI run exposed a catalog migration regression outside the focused localization filter: 22 catalogs retained the retired fill-rasterization source key as well as its already-translated replacement. Remove only those obsolete rows; retain the replacement translations and upstream regression test. Additional local PSD-import failures report zero admissible memory: this cloud sandbox exposes cgroup membership `/` but mount root `/..`, which the upstream detector cannot resolve and deliberately treats as unreadable. Do not weaken that guard or count those failures as passes; use the full hosted platform results for candidate acceptance.

The local full UI sweep completed with 7,031 passed, 23 failed and 24 ignored. The retired-key correction then passed both upstream fill-rasterization tests (zero failed/ignored). Twenty other failures explicitly reported the sandbox's zero-memory PSD budget; two background PSD/PSDT-open tests retained their original document instead of receiving the refused import. These local failures remain recorded separately from hosted acceptance. Candidate workflow syntax, manual-only FreeBSD configuration and all three candidate-provenance tests passed after the font-input correction.
