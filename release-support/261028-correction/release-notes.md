Date build **261028**, based on upstream **PhotoCraft 0.16.0**.

## Version correction
This release was temporarily unpublished and rebuilt to correct its embedded identity. The executable, internal README, portable marker, public tag and Windows ZIP filename now all identify **261028-0.16.0**. Functional code remains the tested merged source **69e1a3330fc9db52191b341a705c60fd3c60dc00**; only release/build-date metadata changed. The Git tag was not moved.

The initial upload incorrectly embedded 261026 despite the 261028 public filename. Its ZIP SHA256 was `a4046ece066202eca4a5ed0cde45a5718e1648bab122a69000ff3f84de8abcde`. Those three original assets remain preserved with **superseded-embedded261026-** names and warning labels for audit; **do not install them**. The corrected ZIP, SHA256 sidecar and provenance use their normal names below. This correction is explicit, rather than a silent asset overwrite.

## Download and update
Download **photocraft-qwen-edit-261028-0.16.0-windows-x64.zip**, extract it into a writable folder and follow **README.txt**. New setups start with **install-qwen-image21.cmd**. Requires Windows x64 + NVIDIA RTX and a CUDA 13 compatible driver; ComfyUI and models are separate downloads.

Existing installations, including the initial misidentified upload, can update through the fork's normal updater. It selects the corrected asset by its exact filename and SHA256, ignores the superseded backups, and offers no update when installed/latest identities both equal 261028-0.16.0. Save documents before updating. Automatic installation replaces only **photocraft.exe**; existing models and personal settings are retained.

Corrected ZIP SHA256: **acc6ea321dabc107b3f6c1843c152cb3240a0cb0aae9201e40e5877cf13172af**.

## Included
- Fixed Size rectangular and elliptical selections retain full dimensions when initially placed across the canvas boundary, matching Photoshop. Moving the outline back inside recovers the complete selection.
- Current Qwen Fill color controls, explicit reversible Poisson matching, original-source restoration, Remove after Fill and layer-menu re-entry, plus the FXC return-path fix.
- Software D3D12 mixed-layer composition repair, while retaining existing hardware/Vulkan shader paths. Test-only CMYK opaque-white reference correction.
- Local Qwen generation/editing/Fill/removal, translation, AVIF, Korean UI and persistent foreground/background colors.

## Validation and provenance
- [Merged source and PR #15](https://github.com/david419kr/photocraft-qwen-edit/pull/15): **69e1a3330fc9db52191b341a705c60fd3c60dc00**.
- Existing [merged-source CI](https://github.com/david419kr/photocraft-qwen-edit/actions/runs/40601496265) and [Qwen gate](https://github.com/david419kr/photocraft-qwen-edit/actions/runs/40601584345) passed: Windows unit 9,245 passed, Windows integration 2,394 passed; focused Qwen checks 31 nonempty groups, 604 passes. These validate the unchanged functional source.
- [Metadata-only Windows rebuild](https://github.com/david419kr/photocraft-qwen-edit/actions/runs/40608550862) uses Rust 1.95.0, x64 MSVC static CRT and AVIF. The release identity is an explicit input, also written to the build workspace's release-tag.txt; that metadata-only override is attached to provenance. No product code changes or repeated full CI.
- Release verification checks executable version, source/hash, README/portable identity, AMD64 PE, CRT imports, exact package/license contents, installer replacement/rollback and production updater comparison/asset selection. Public ZIP, sidecar and provenance are downloaded again and verified.
- FreeBSD was not run; fuzz was skipped by the original workflow policy. This cloud task did not run actual CUDA inference, VRAM or interactive hardware acceptance. The owner accepted the functional build and explicitly approved this metadata correction.

The **release-provenance.json** asset records old/new hashes, the explicit identity override, source/controller SHAs and correction/verification runs. Prior releases remain unchanged.

[English guide](https://github.com/david419kr/photocraft-qwen-edit/blob/main/README.md) · [한국어 가이드](https://github.com/david419kr/photocraft-qwen-edit/blob/main/README.ko.md)
