Date build **261022**, based on upstream **PhotoCraft 0.12.0**. The date tracks this fork; the version suffix tracks the upstream base.

## Download and start
Download the Windows x64 ZIP, extract it, and read the included **README.txt**. For a new setup, run **install-qwen-image21.cmd**, press Enter for the default folder, and answer the setup questions before downloads begin. Start the generated **0. Run-Qwen-Image21** shortcut, then **photocraft.exe**.

Requires **Windows x64 + NVIDIA RTX** and a CUDA 13 compatible driver. ComfyUI and model files are downloaded separately. Existing ComfyUI installations, models and personal settings do not need to be replaced. Optional translation: **Hy-MT2 1.8B is recommended** for lower VRAM use; 7B uses more VRAM.

## Included
- Updates the editor to upstream PhotoCraft 0.12.0 while preserving the local Qwen workflow.
- Adds upstream History Brush improvements and history source selection, retaining Korean history labels.
- Preserves private-metadata stripping and prompt metadata export, and keeps AVIF available in the new format-specific Open filters.
- Makes background removal prompts apply to all subject types, preserving original shapes and edges without inventing hair or fur details.
- Preserves remembered tool selection with temporary spring-loaded and pen-eraser switching.
- Retains local Qwen generation, editing, Generative Fill/removal, optional prompt translation, prompt history, AVIF, Korean UI, non-destructive layers, Undo and cancellation.
- Keeps automatic update checks, manual checking in **Help → About PhotoCraft**, and safe executable replacement with rollback. Save modified documents before installing.

This update does not change ComfyUI, model or translation-node versions. Automatic installation replaces only **photocraft.exe**; it does not reinstall models or overwrite personal settings.

## Guides
[English guide and screenshots](https://github.com/david419kr/photocraft-qwen-edit/blob/main/README.md)  
[한국어 가이드 및 스크린샷](https://github.com/david419kr/photocraft-qwen-edit/blob/main/README.ko.md)

ZIP layout: **licenses/**, **scripts/**, **install-qwen-image21.cmd**, **photocraft.exe**, **portable.txt**, **README.txt**. Personal settings, documents and model weights are excluded. The Windows executable includes its CRT. A SHA256 sidecar and build provenance are provided.



Live CUDA/VRAM and interactive UI acceptance has not been performed for this build. Cloud validation and build provenance are recorded in [PR #9](https://github.com/david419kr/photocraft-qwen-edit/pull/9).
