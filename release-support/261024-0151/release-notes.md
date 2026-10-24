Date build **261024**, based on upstream **PhotoCraft 0.15.1**. The date tracks this fork; the version suffix tracks the upstream base.

## Download and start
Download the Windows x64 ZIP, extract it, and read the included **README.txt**. For a new setup, run **install-qwen-image21.cmd**, press Enter for the default folder, and answer the setup questions before downloads begin. Start the generated **0. Run-Qwen-Image21** shortcut, then **photocraft.exe**.

Requires **Windows x64 + NVIDIA RTX** and a CUDA 13 compatible driver. ComfyUI and model files are downloaded separately. Existing installations, models and personal settings do not need to be replaced. Optional translation: **Hy-MT2 1.8B is recommended** for lower VRAM use; 7B uses more VRAM.

## Included
- PhotoCraft 0.15.1 improves handling of malformed image, font, metadata and vector inputs, confirms ICC profile identity by its bytes, and improves linked-file path resolution.
- Retains local Qwen generation, editing, Fill/removal, prompt translation/history, AVIF, Korean UI, remembered foreground/background colors, non-destructive layers, Undo and cancellation, including the simplified background-removal prompt.
- Keeps automatic update checks, manual checking in **Help → About PhotoCraft**, and safe executable replacement with rollback. Save modified documents before installing.

Automatic installation replaces only **photocraft.exe**; it does not reinstall models or overwrite personal settings. AVIF export remains SDR-focused.

## Guides
[English guide and screenshots](https://github.com/david419kr/photocraft-qwen-edit/blob/main/README.md)

[한국어 가이드 및 스크린샷](https://github.com/david419kr/photocraft-qwen-edit/blob/main/README.ko.md)

ZIP layout: **licenses/**, **scripts/**, **install-qwen-image21.cmd**, **photocraft.exe**, **portable.txt**, **README.txt**. Personal settings, documents and model weights are excluded. The Windows executable includes its CRT. A SHA256 sidecar and build provenance are provided.

This release promotes the exact tested candidate ZIP after user approval. [Validation and source provenance](https://github.com/david419kr/photocraft-qwen-edit/pull/13) are recorded with the attached release provenance.
