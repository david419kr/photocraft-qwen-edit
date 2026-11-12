# PhotoCraft Local Qwen AI 261112-0.23.0

Windows x64 portable release based on the published upstream PhotoCraft v0.23.0 release, retaining the fork's Qwen workflows and PR #27 external upscalers.

- Preserves generation/editing/Fill/removal, non-destructive layers, Undo/Redo, cancellation and document-local jobs.
- Preserves model/TE/CFG/Turbo controls, HyMT isolation/residency, XMP and Metadata None privacy including native saves, AVIF and language coverage.
- Retains selected-tool/native-color persistence, Quick Mask Selected Areas, local selection erase, portable updater, installer pins and licenses.
- Includes upstream PR #13257's isolated GPU test scheduling correction, with the original failures and repair provenance recorded in [PR #29](https://github.com/david419kr/photocraft-qwen-edit/pull/29).
- External ComfyUI/model/upscaler installations are unchanged. No model weights or external runtimes are bundled.

## Validation and provenance

All required final-candidate cloud checks passed: Ubuntu workspace 19,049; strict Vulkan/lavapipe sweep 1,941; Windows unit 15,392 and integration 3,558; no-HEIF 2,350; corpus 7,536; focused fork regressions 1,167 checks across 40 nonempty, overlapping filters. These are separate suite counts, not a unique-test total. Skip counts, doctests, installer/updater checks and full evidence are recorded in PR #29. Full Clippy passed on Rust 1.95.0 and latest stable 1.99.1. FreeBSD remains manual-only and was not run.

The exact tested candidate is `e7000434acc7588005880e6a3e8460ccf10c0776`. The release tag points to merged main `efe4a26f6ae4535f0b9803f55bb3fa5a08ea0456`; both trees equal `83ecef8a8b730ca6c9d060110ad649a1b8610c20`. Upstream release ID 442232444 / v0.23.0 resolves to `9963efcb2f04867b56c564f876befd412b6c3c4e`.

The original Rust 1.95 Windows x64 MSVC static-CRT candidate ZIP is promoted without rebuilding or repacking. Exact-tree PR validation is reused; duplicate main CI is not a release prerequisite. The publisher verifies source identity, all required PR/candidate checks, positive focused test counts, updater identity, package contents, executable architecture/static CRT/version, and public downloaded bytes. Detailed provenance is attached as `release-provenance.json`.

ZIP SHA256: `000c6a47bb6c07ef29a18755f3d530210f8832cd532c54bcb0d25291c9d4e691`.

Actual CUDA Qwen inference, VRAM residency/full-free, interactive Windows UI/IME, cancellation/Undo, save/reopen privacy behavior and real external-upscaler inference were not performed in cloud. Software-GPU and mock tests do not replace these local hardware checks.

Download the Windows x64 ZIP and SHA256 sidecar. Existing releases and assets are preserved.
