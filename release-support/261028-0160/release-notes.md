Date build **261028**, based on upstream **PhotoCraft 0.16.0**. Promotes the exact Windows candidate confirmed working by the owner. No rebuild or repackaging was performed.

## Changes
- Fixed Size rectangular and elliptical selections keep their full dimensions when initially placed across the canvas boundary, matching Photoshop. Moving the outline back inside recovers the complete selection.
- Includes the current Qwen Fill color controls, explicit reversible Poisson matching, restoration from the original source, Remove after Fill and layer-menu re-entry, plus the FXC return-path fix.
- Repairs mixed-layer composition on software D3D12; retains the existing hardware/Vulkan shader paths. Corrects an opaque-white CMYK test reference without changing production CMYK composition.
- Retains local Qwen generation/editing/Fill/removal, translation, AVIF and persistent foreground/background colors.

## Download and update
Download **photocraft-qwen-edit-261028-0.16.0-windows-x64.zip**, extract it into a writable folder and follow **README.txt**. New setups start with **install-qwen-image21.cmd**. Requires Windows x64 + NVIDIA RTX and a CUDA 13 compatible driver; ComfyUI and models are separate downloads.

The in-app updater can discover this release via the fork's latest-release endpoint and verified Windows ZIP digest. Save documents before updating. Automatic installation replaces only **photocraft.exe**; existing models and personal settings are retained.

**Version display/update caveat:** to preserve the exact accepted bytes, the executable and internal README still report **261026-0.16.0**. The public release tag and asset filename are **261028-0.16.0**. Since the updater compares the embedded version, this binary may offer the same update again after installation; no second download is necessary if already using this verified build.

## Source and checks
- Tag/source: `69e1a3330fc9db52191b341a705c60fd3c60dc00` ([merged PR #15](https://github.com/david419kr/photocraft-qwen-edit/pull/15)).
- Windows x64 MSVC, Rust 1.95.0, static CRT (`-C target-feature=+crt-static`), AVIF. Build, executable replacement/rollback tests, PE/import/version checks and ZIP integrity passed.
- Portable ZIP SHA256: `a4046ece066202eca4a5ed0cde45a5718e1648bab122a69000ff3f84de8abcde` (downloaded candidate verified directly).
- [PR-head CI](https://github.com/david419kr/photocraft-qwen-edit/actions/runs/40598594851) passed; the merged source has the identical tree.
- [Merged-source CI](https://github.com/david419kr/photocraft-qwen-edit/actions/runs/40601496265): Ubuntu, Windows unit/doctests (9,245 passed), Clippy, corpus, no-HEIF and web/wasm size passed. Windows integration also passed (2,394 cases); the merged-source CI is fully successful. Fuzz was skipped by workflow policy.
- [Candidate workflow](https://github.com/david419kr/photocraft-qwen-edit/actions/runs/40601584345): Windows build/package and early Clippy passed; Linux focused Qwen regressions, formatting/architecture/WASM/scorecard and installer/package checks all passed.
- Documentation and SonarCloud passed. FreeBSD was not run. Actual CUDA/VRAM/interactive hardware testing was not performed by this cloud task; the owner separately confirmed the ZIP works.

The complete provenance and SHA256 sidecar are attached. Existing releases/assets are unchanged. Normal/Fixed Ratio selection clipping and existing outline movement behavior are preserved; no scorecard count or parity floor was raised.

[English guide](https://github.com/david419kr/photocraft-qwen-edit/blob/main/README.md) · [한국어 가이드](https://github.com/david419kr/photocraft-qwen-edit/blob/main/README.ko.md)
