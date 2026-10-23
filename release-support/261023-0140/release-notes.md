Date build **261023**, based on upstream **PhotoCraft 0.14.0**. The date tracks this fork's verified build; the version suffix tracks the upstream base. This release promotes the tested candidate unchanged.

## Download and start
Download the Windows x64 ZIP, extract it, and read the included **README.txt**. For a new setup, run **install-qwen-image21.cmd**, press Enter for the default folder, and answer the setup questions before downloads begin. Start the generated **0. Run-Qwen-Image21** shortcut, then **photocraft.exe**.

Requires **Windows x64 + NVIDIA RTX** and a CUDA 13 compatible driver. ComfyUI and model files are downloaded separately. Existing ComfyUI installations, models and personal settings do not need to be replaced. Optional translation: **Hy-MT2 1.8B is recommended** for lower VRAM use; 7B uses more VRAM.

## Included
- Updates the native editor to PhotoCraft 0.14.0, including upstream automation, save/recovery and file-format fixes and the editor improvements since 0.12.0.
- Retains local Qwen generation, editing, Fill/removal, background removal, optional prompt translation, prompt history, AVIF, Korean UI, non-destructive layers, Undo and cancellation.
- Preserves foreground/background colors across restarts, including separate pixel/mask pairs, exact CMYK picks and HDR/alpha values, swaps/reset and malformed-preset validation.
- Keeps automatic update checks, manual checking in **Help → About PhotoCraft**, and safe executable replacement with rollback. Save modified documents before installing.

This update does not change ComfyUI, model or translation-node versions. Automatic installation replaces only **photocraft.exe**; it does not reinstall models or overwrite personal settings. AVIF export remains SDR-focused. The user reported the candidate working; detailed hardware/scenario coverage is recorded only where actually measured.

## Guides
[English guide and screenshots](https://github.com/david419kr/photocraft-qwen-edit/blob/main/README.md)

[한국어 가이드 및 스크린샷](https://github.com/david419kr/photocraft-qwen-edit/blob/main/README.ko.md)

ZIP layout: **licenses/**, **scripts/**, **install-qwen-image21.cmd**, **photocraft.exe**, **portable.txt**, **README.txt**. Personal settings, documents and model weights are excluded. The Windows executable includes its CRT. A SHA256 sidecar and build provenance are provided.

Validation and build provenance: [PR #12](https://github.com/david419kr/photocraft-qwen-edit/pull/12).
