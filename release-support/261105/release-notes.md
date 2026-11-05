Date build **261105**, based on upstream **PhotoCraft 0.20.2**. The date tracks this fork; the version suffix tracks the upstream base.

## Download and start

Download **photocraft-qwen-edit-261105-0.20.2-windows-x64.zip**, extract it into a writable folder, and read the included **README.txt**. Run **install-qwen-image21.cmd**, press Enter for the displayed default folder, and answer the setup questions before downloads begin. Start the generated **0. Run-Qwen-Image21** shortcut, wait for ComfyUI, then run **photocraft.exe**.

Requires **Windows x64 + NVIDIA RTX** and a **CUDA 13 compatible NVIDIA driver**. Initial installation needs internet access. ComfyUI and model files are downloaded separately, not bundled in this ZIP; generation and translation then run locally. Optional translation: **Hy-MT2 1.8B is recommended** for its lower VRAM use; 7B uses more VRAM and may require unloading or offloading. Keep the compatible ComfyUI backend running: the AI bar appears only when the required service, models and nodes are detected.

## Included

**From upstream 0.20.2:**
- Native scratch-disk storage can move cold pixel and Undo-history tiles out of RAM. Configure it in Preferences → Scratch Disks. New preferences use a configurable 70% RAM budget.
- Native opening of supported single-part, flat scanline EXR files decodes one compression block at a time, preserving source dimensions.
- TIFF Options for Save As and Save a Copy: None/LZW/ZIP compression, image pyramids, transparency and discarding layers.
- OpenType numeral choices, plus Layers-panel filters for label color, linked/missing/embedded Smart Objects and Artboard.
- Improvements to Distort/Perspective handles, dialog color previews and cancellation, Windows/Linux Ctrl+Alt+wheel zoom, and centering small images when Overscroll is off.

[Full upstream changelog](https://github.com/storytold/photocraft/compare/v0.20.1...v0.20.2)

**Retained Local Qwen AI features:**
- Local Qwen Image 2.1 prompt generation, whole-image editing, Generative Fill/removal, background removal and reroll. Results use new layers with Undo/Redo support.
- Optional English prompt translation and ordered prompt history, including original text, translations and approximate fill positions. Use a metadata-capable format and **Metadata: All** when exporting to retain history; **None** removes it.
- Custom model/text-encoder settings, per-model steps/CFG and Normal/Turbo profiles, model residency and release on model changes. Installer pins remain unchanged.
- AVIF opening, saving and export; Korean UI additions; Ctrl+Enter generation; numeric-field wheel input; restored last-selected tool and foreground/background colors. Upstream shortcuts and export defaults remain intact.
- The native 64-bit PSD import safety ceiling remains **20 GiB of estimated decoded pixel memory**, with available-memory checks and lower custom limits still enforced. This is not a guarantee that every document below that ceiling fits RAM or VRAM.

**Known limitations:** AVIF export targets SDR; this portable build does not include optional HEIF support or the optional craft-fonts bundle. Scratch storage does not make every format, filter or renderer streamable, and unsupported EXR variants use the whole-file decoder with its own limits. See [scratch-disk behavior and limitations](https://github.com/david419kr/photocraft-qwen-edit/blob/6ec7e63bdf14d4ca0d18435d18d2c65cec4af928/docs/scratch-disks.md) and [EXR streaming scope](https://github.com/david419kr/photocraft-qwen-edit/blob/6ec7e63bdf14d4ca0d18435d18d2c65cec4af928/docs/streaming-io.md). The owner reported that this candidate works locally; this does not establish comprehensive GPU/VRAM or interactive AI validation on every supported configuration.

## Guides

[English guide and screenshots](https://github.com/david419kr/photocraft-qwen-edit/blob/6ec7e63bdf14d4ca0d18435d18d2c65cec4af928/README.md)  
[한국어 가이드 및 스크린샷](https://github.com/david419kr/photocraft-qwen-edit/blob/6ec7e63bdf14d4ca0d18435d18d2c65cec4af928/README.ko.md)  
[Detailed installer guide](https://github.com/david419kr/photocraft-qwen-edit/blob/6ec7e63bdf14d4ca0d18435d18d2c65cec4af928/qwen-image21-installer/README.md)

ZIP layout: **licenses/**, **scripts/**, **install-qwen-image21.cmd**, **photocraft.exe**, **portable.txt**, **README.txt**. Personal settings, documents, ComfyUI installations and model weights are excluded. The Windows executable includes its CRT. A **SHA256 sidecar** is provided; exact source/build hashes and validation evidence belong in the release provenance attachment.
