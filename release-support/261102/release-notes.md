Date build **261102**, based on upstream **PhotoCraft 0.19.0**. The date identifies this fork's build; the semantic version identifies its upstream base.

## Download and start

Download **photocraft-qwen-edit-261102-0.19.0-windows-x64.zip**, extract it into a writable folder and follow **README.txt**. For a new setup, run **install-qwen-image21.cmd**, choose the models and optional translation, then start the **0. Run-Qwen-Image21** shortcut and **photocraft.exe**.

The bundled setup requires Windows x64, an NVIDIA RTX GPU and a CUDA 13 compatible driver. ComfyUI and model weights are separate downloads. Optional local prompt translation uses the isolated Hy-MT2 runtime; the 1.8B model uses less memory than 7B.

Existing installations can use the normal update check. Save documents before updating. Automatic installation replaces only **photocraft.exe**, retaining models and personal settings. Equal installed/latest release identities do not offer an update.

## Included

- Upstream PhotoCraft 0.19.0, including selected color-channel editing, expanded color pickers, large TIFF/PSD import improvements, and additional interface languages.
- Local Qwen generation, reference-image editing, Fill and removal with non-destructive layers, Undo, cancellation and document-specific jobs. Removal retains original-image latent masking and the unmarked image plus black-and-white guide.
- Model/encoder/CFG/Turbo controls, optional isolated Hy-MT2 translation, large-image resize approval and optional Fill colour matching.
- Metadata None privacy cleanup, prompt-history/XMP preservation, AVIF and Korean UI.
- Remembered tools with the fork's Brush fallback, hover-wheel numeric controls, portable updating and the existing experimental external cutout backend.
- Completed translations for the new preset-import and colour dialogs. Existing ComfyUI, model and installer pins are retained.

Actual GPU/VRAM inference and interactive UI acceptance for this version have not been performed. Automated cloud and Windows checks passed; see the linked AI behavior guide for current limitations.

## Guides

[English guide](https://github.com/david419kr/photocraft-qwen-edit/blob/main/README.md) · [한국어 가이드](https://github.com/david419kr/photocraft-qwen-edit/blob/main/README.ko.md) · [AI behavior and limitations](https://github.com/david419kr/photocraft-qwen-edit/blob/main/docs/comfyui.md)

The portable ZIP contains the executable, portable marker, README, installer launcher, scripts and licenses (18 files, six root entries). Models, ComfyUI installations, personal settings and private documents are excluded. The Microsoft C runtime is statically linked; no separate Visual C++ runtime installation is required. The SHA256 sidecar and **release-provenance.json** record the downloadable bytes and build/check provenance.
