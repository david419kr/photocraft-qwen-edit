# Upstream 0.23.1 integration candidate

## Provenance and scope

- Published upstream release ID `442467332`, tag `v0.23.1`, resolved commit `45066c48292e92229ada3459a83453439514c83d`; draft=false, prerelease=false, published 2026-11-12 05:07:51 UTC. The fetched tag matches the release target. The preceding `v0.23.0` remains `9963efcb2f04867b56c564f876befd412b6c3c4e`.
- Fork base `efe4a26f6ae4535f0b9803f55bb3fa5a08ea0456`, tree `83ecef8a8b730ca6c9d060110ad649a1b8610c20`. PR #29 is merged and published as `261112-0.23.0`, release ID `442507991`; its approved publication does not authorize this candidate's publication.
- No open fork PR or remote `update/0.23.1` branch existed at preparation. The exact release SHA was not an ancestor of fork main. Merge that SHA with two parents on `update/0.23.1`; upstream is read-only.
- Candidate version `261112-0.23.1`. Preserve Rust 1.95.0, Windows x64 MSVC static CRT and AVIF. No ComfyUI/model upgrades, upstream-main merge, main merge, tag or public release.

## Integration review

Read AGENTS.md, the complete Library update handoff, current ComfyUI documentation, 0.7.0/0.23.0 integration records, update-state and PR #29's completed deployment record. Current code and explicit user instructions supersede the handoff's historical green annotation, automatic Poisson and translator unload descriptions.

Resolve only the three textual conflicts: combine the new zoom-out icon attribution with the fork's language icon exclusion; retain indexed PNG XMP and Metadata None/alpha-LSB cleaning wrappers while applying upstream's checked pixel-count multiplication in the shared implementation; export both the fork's integer field and upstream's whole-value field. Review automatically merged export, canvas, history, shortcut, engine registration and GPU planning paths. Upstream Quick Export metadata filtering continues through the fork's stricter None export options.

Upstream PR #13257's automation GPU nextest filter is now part of this exact release. The fork already carried its isolated repair from 0.23.0. Retain it once, with max-four scheduling and all fork Windows overrides unchanged; no post-release code is attributed to the wrong version.

Preserve generation/edit/fill/blank removal with original RGB and black/white guides, non-destructive layers and Undo/Redo, cancellation and document-local jobs, model/TE/CFG/Turbo sigma, HyMT isolation/residency/two-marker full-free, XMP/Metadata None including native saves, AVIF, compressed multilingual catalogs/Korean, static CRT, installer pins/licenses/updater, selected-tool/native-color persistence, Quick Mask Selected Areas, local selection erase and PR #27 external upscalers. Dependency changes in Cargo.lock are workspace version changes only; external dependency pins remain unchanged.

Extend the existing candidate workflow's branch guards and upstream/base provenance to 0.23.1. Keep all full PR checks, explicitly enable the full sweep for this candidate, and leave FreeBSD manual-only. PR #7 is not rerun. Exclude the newly added upstream release-nightly cron: fork updates are scheduled externally, and this task does not authorize a new scheduler. Keep its manual/release-branch behavior. Existing successful publication infrastructure is reference only; this task stops at draft PR/artifact.

## Validation status

Early full-workspace/all-target locked HEIF Clippy comparisons on latest stable and Rust 1.95.0 are preparation diagnostics. Exact final-source results, failures/corrections, full PR checks and Windows artifact hashes are recorded in the draft PR and its Actions links. No previous-version result is reused as candidate acceptance. The regression harness rejects empty filters. Preserve all tests, warning gates and floors.

The managed cloud environment initially required a startup wait; it then became ready. Rust was installed in workspace-owned directories. The cloud image does not advertise native GTK/Wayland/xkbcommon development packages or sudo; hosted jobs retain their existing dependency installation and remain the full platform acceptance path. Record any local dependency failure separately, not as a code pass or a global blocker.

## Outside-cloud acceptance not performed

No user PC was accessed. Actual CUDA Qwen Generate/Edit/Fill/blank Remove/background removal/reroll, output quality, feather boundaries, interactive Windows cancellation/layers/Undo, model/TE/CFG/Turbo controls, Korean IME/Ctrl+Enter/hover-wheel, real HyMT VRAM pressure/residency/two-marker full-free, PNG/AVIF/native metadata All/None save/reopen UI, Quick Mask/local erase, and real IllustrationJaNai/waifu2x/TensorRT inference remain untested here. Cloud mock/software-GPU checks cannot certify those hardware/UI results.

## Initial preparation evidence

Initial merge `46d50d4688e5405ee1861c94ec9c906bb761c3d9` is in draft [PR #30](https://github.com/david419kr/photocraft-qwen-edit/pull/30). PR merge `57d7b70e3366dc4a14007bd13b209be8b1a61e52` has the identical full tree `9c89e7cce96b3ee8abacc90f845527d792899306`. Local full stable 1.99.1 Clippy passed in 6m49s. Installer tests: 14 passed; package/updater Python suite: 11 passed, three Windows-only skipped; CI-reuse policy: 19 passed. Formatting/diff checks passed. Rust 1.95 and full final-source hosted tests remain pending here; later measured results belong in the PR. External dependency lock entries and the entire nextest configuration match fork base exactly.

The initial local xtask run passed all 13 library tests and 88/89 binary tests, failing complete-catalog coverage on six new Count/Character/Paragraph source keys. Add those six translated entries to each of the 33 complete language catalogs without changing existing translations or Hindi's partial status. The same suites then pass all 102 tests. The actual shared-lookup report covers 2,583/2,583 keys in every complete catalog. Layers pass for 30 crates and the generated scorecard is current. This correction changes embedded catalogs, so final-source platform tests and packaging must run for the new SHA; initial CI/artifacts are historical only.
