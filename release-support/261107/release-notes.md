Date build **261107**, based on upstream **PhotoCraft 0.21.0**. The date tracks this fork; the version suffix tracks the upstream base.

## Download and start

Download **photocraft-qwen-edit-261107-0.21.0-windows-x64.zip**, extract it into a writable folder, and read the included **README.txt**. Run **install-qwen-image21.cmd**, press Enter for the displayed default folder, and answer the setup questions before downloads begin. Start the generated **0. Run-Qwen-Image21** shortcut, wait for ComfyUI, then run **photocraft.exe**.

Requires **Windows x64 + NVIDIA RTX** and a **CUDA 13 compatible NVIDIA driver**. Initial installation needs internet access. ComfyUI and model files are downloaded separately, not bundled in this ZIP; generation and translation then run locally. Optional translation: **Hy-MT2 1.8B is recommended** for its lower VRAM use; 7B uses more VRAM and may require unloading or offloading. Keep the compatible ComfyUI backend running: the AI bar appears only when the required service, models and nodes are detected.

## Included

**Upstream 0.21.0 integration:**
- Integrates the exact stable upstream v0.21.0 release while preserving Local Qwen AI generation, editing and Generative Fill.
- Adapts the fork to upstream Place, frame, guide, renderer and background-job APIs, with fallible memory checks before AI/view state changes.
- Preserves the 20 GiB PSD admission ceiling, metadata removal and prompt history across the updated import/export paths, plus the fork's CMYK adjustment Fill behavior.
- Includes updated language catalogs and regression coverage for the integrated source.

[Full upstream changelog](https://github.com/storytold/photocraft/compare/v0.20.2...v0.21.0)

**Retained Local Qwen AI features:**
- Local Qwen Image 2.1 prompt generation, whole-image editing, Generative Fill/removal, background removal and reroll. Results use new layers with Undo/Redo support.
- Optional English prompt translation and ordered prompt history, including original text, translations and approximate fill positions. Use a metadata-capable format and **Metadata: All** when exporting to retain history; **None** removes it.
- Custom model/text-encoder settings, per-model steps/CFG and Normal/Turbo profiles, model residency and release on model changes. Installer pins remain unchanged.
- AVIF opening, saving and export; Korean UI additions; Ctrl+Enter generation; numeric-field wheel input; restored last-selected tool and foreground/background colors. Upstream shortcuts and export defaults remain intact.
- The native 64-bit PSD import safety ceiling remains **20 GiB of estimated decoded pixel memory**, with available-memory checks and lower custom limits still enforced. This is not a guarantee that every document below that ceiling fits RAM or VRAM.

**Known limitations:** AVIF export targets SDR; this portable build does not include optional HEIF support or the optional craft-fonts bundle. Scratch storage does not make every format, filter or renderer streamable, and unsupported EXR variants use the whole-file decoder with its own limits. See [scratch-disk behavior and limitations](https://github.com/david419kr/photocraft-qwen-edit/blob/b4ce47cf1782b0f63ebc501b31c5300169fbb096/docs/scratch-disks.md) and [EXR streaming scope](https://github.com/david419kr/photocraft-qwen-edit/blob/b4ce47cf1782b0f63ebc501b31c5300169fbb096/docs/streaming-io.md). Actual CUDA/VRAM/model inference and interactive UI acceptance remain unrun. Hosted tests and software rendering do not establish hardware acceptance.

## Guides

[English guide and screenshots](https://github.com/david419kr/photocraft-qwen-edit/blob/b4ce47cf1782b0f63ebc501b31c5300169fbb096/README.md)  
[한국어 가이드 및 스크린샷](https://github.com/david419kr/photocraft-qwen-edit/blob/b4ce47cf1782b0f63ebc501b31c5300169fbb096/README.ko.md)  
[Detailed installer guide](https://github.com/david419kr/photocraft-qwen-edit/blob/b4ce47cf1782b0f63ebc501b31c5300169fbb096/qwen-image21-installer/README.md)

ZIP layout: **licenses/**, **scripts/**, **install-qwen-image21.cmd**, **photocraft.exe**, **portable.txt**, **README.txt**. Personal settings, documents, ComfyUI installations and model weights are excluded. The Windows executable includes its CRT. A **SHA256 sidecar** is provided; exact source/build hashes and validation evidence belong in the release provenance attachment.

## Validation and provenance

The published ZIP is the unchanged candidate built from **b4ce47cf1782b0f63ebc501b31c5300169fbb096**, not a rebuild. The tag points to PR #23's merge commit with the identical source tree. Its embedded version/date remain the original **261107-0.21.0** build identity.

Candidate CI passed: Ubuntu 16,080 tests; Windows unit 12,712 and integration 3,258; corpus 6,826; no-HEIF 2,252. The Qwen regression harness passed 951 tests across 36 nonempty groups (groups may overlap). Rust 1.95 and stable 1.99.1 full Clippy passed. Post-merge Ubuntu was interrupted when its GitHub-hosted runner lost communication with the server. The owner explicitly waived that Ubuntu job for this Windows release only; candidate Ubuntu had already passed on the identical source tree. Every other required post-merge job must pass before publication. This is not a claim that post-merge CI was fully green, and no CI workflow or source code was changed for the waiver.

FreeBSD remains manual-only and was not run. Manual fuzz/performance checks were not run. SonarCloud's actual scan/coverage was skipped without SONAR_TOKEN; Nix's actual check was skipped without flake.nix. Successful wrappers do not imply those skipped checks ran.

ZIP SHA256: `f8f028ee27dbcfd2dcf70a0883c5cd703664a042f780fc7d8119322548bfbe5c`
