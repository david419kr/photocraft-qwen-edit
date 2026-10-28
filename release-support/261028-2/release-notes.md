Date build **261028-2**, based on upstream **PhotoCraft 0.16.0**. The date and revision identify this fork's build; the semantic version identifies its upstream base.

## Download and start

Download **photocraft-qwen-edit-261028-2-0.16.0-windows-x64.zip**, extract it into a writable folder and follow **README.txt**. For a new setup, run **install-qwen-image21.cmd**, choose the models and optional translation, then start the **0. Run-Qwen-Image21** shortcut and **photocraft.exe**.

The bundled setup requires Windows x64, an NVIDIA RTX GPU and a CUDA 13 compatible driver. ComfyUI and model weights are separate downloads. Optional local prompt translation uses the isolated Hy-MT2 runtime; the 1.8B model uses less memory than 7B.

Existing installations can use the normal update check. Save documents before updating. Automatic installation replaces only **photocraft.exe**, retaining models and personal settings. Equal installed/latest release identities do not offer an update.

## Included

- Live ComfyUI sampling progress, with history-based fallback if the WebSocket disconnects.
- Translation workers released after each translation; Qwen reference-prefix cache uses CPU INT8 storage. These request settings do not impose a process-wide VRAM ceiling.
- Optional reference images and explicit, atomic resizing for large Prompt Edit requests.
- Independent removal regions and native-pixel-preserving background removal. Experimental tiled refinement remains opt-in through the API, outside the default UI path.
- Retained local Qwen generation/editing/Fill/removal, prompt history, translation, AVIF, Korean UI, persistent colors and fixed-size selections.

## Guides

[English guide](https://github.com/david419kr/photocraft-qwen-edit/blob/main/README.md) · [한국어 가이드](https://github.com/david419kr/photocraft-qwen-edit/blob/main/README.ko.md) · [AI behavior and limitations](https://github.com/david419kr/photocraft-qwen-edit/blob/main/docs/comfyui.md)

The portable ZIP contains the executable, portable marker, README, installer launcher, scripts and licenses (18 files, six root entries). Models, ComfyUI installations, personal settings and private documents are excluded. The Microsoft C runtime is statically linked; no separate Visual C++ runtime installation is required. The SHA256 sidecar and **release-provenance.json** record the downloadable bytes and build/check provenance.

Cloud CI and package checks do not establish new CUDA/VRAM measurements or interactive GPU UI acceptance. Existing releases and assets are preserved.
