PhotoCraft Local Qwen AI build **261112**, based on upstream **0.23.1**. The date identifies this fork's build; the upstream number tracks the PhotoCraft release integrated into it.

## Download and start

1. Download the Windows x64 ZIP below and extract it into a writable folder. Read the included `README.txt`.
2. Run `install-qwen-image21.cmd`, choose the model and optional translation setup, and wait for installation to complete.
3. Launch `photocraft.exe` and follow the guide to connect the local ComfyUI installation.

AI generation targets Windows x64 with an NVIDIA RTX GPU and a CUDA 13 compatible NVIDIA driver. Initial installation requires internet access and separate model downloads. Translation is optional. Existing ComfyUI, models and external upscalers are not upgraded by this release.

## Included

- Upstream 0.23.1 updates, with complete-catalog translations for the new Count and text-panel labels.
- Qwen generation, editing, Fill and removal; non-destructive layers, Undo/Redo, cancellation and document-local jobs.
- Model/text-encoder/CFG/Turbo controls, HyMT isolation and residency, XMP and Metadata None privacy including native saves, and AVIF support.
- Selected-tool/native-color persistence, Quick Mask Selected Areas, local selection erase, PR #27 external upscalers, portable updater and pinned installer dependencies.

Cloud checks passed for the released source. Actual CUDA inference, VRAM behavior, interactive Windows UI/IME and real external-upscaler inference remain separate hardware checks. See [PR #30](https://github.com/david419kr/photocraft-qwen-edit/pull/30) for validation details and limitations.

## Guides

- [English guide](https://github.com/david419kr/photocraft-qwen-edit/blob/261112-0.23.1/README.md)
- [한국어 안내](https://github.com/david419kr/photocraft-qwen-edit/blob/261112-0.23.1/README.ko.md)

The ZIP contains the executable, portable marker, offline README, installer launcher, scripts and licenses. It excludes personal settings, images, models and external runtimes. The executable uses the static MSVC CRT; no separate CRT package is bundled. The original tested ZIP is published without rebuilding or repacking. A SHA256 sidecar and detailed release provenance are attached.
