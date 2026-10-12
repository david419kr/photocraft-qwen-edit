# Qwen Image 2.1 Portable Installer

## Quick start

1. **Extract the ZIP** into a folder of your choice. Do not run a CMD directly inside the ZIP.
2. Double-click **`install-qwen-image21.cmd`**. Python, Git, and 7-Zip do not need to be installed system-wide.
3. Choose the installation folder. The default is `ComfyUI_windows_portable` beside the installer. A previously selected folder is remembered.
4. Choose image models: `1` Standard (25 steps), `2` Turbo (8 steps), `1,2` both, or `0` to add models manually.
5. Selecting an image model also downloads the official INT8 text encoder and VAE. With `0`, you can choose whether to download these shared files.
6. Choose optional translation: `1` Hy-MT2 1.8B Heretic i1-Q4_K_M, `2` Hy-MT2 7B Abliterated Q4_K_M, `1,2` both, or `0` none.

After setup, double-click **`0. Run-Qwen-Image21.lnk` in the installer folder**.

- Default address: **`http://127.0.0.1:8188`**.
- The browser **does not open automatically**. You can open the address manually when needed.
- A successful SageAttention CUDA test enables Sage; otherwise the launcher uses PyTorch attention.
- If port 8188 is already in use, the launcher asks you to close the existing server.
- Run `install-qwen-image21.cmd` again to add models or translation. Existing models are preserved, even when you select `0`.
- Rerunning setup reuses the existing ComfyUI version and verified downloads.

## Folder layout

Before installation, the only CMD at the top level is the setup entry point. Support scripts are kept in `scripts`:

```text
install-qwen-image21.cmd        <- double-click this to install
README.md
LICENSE-MIT
scripts/
  install-qwen-image21.ps1
  qwen-image21-setup.py
  install-comfyui-hymt2.ps1
```

Setup creates these shortcuts **in the folder containing the installer CMD**, including when the portable is installed elsewhere:

```text
0. Run-Qwen-Image21.lnk          -> portable/Run-Qwen-Image21.cmd
1. diffusion_models.lnk         -> portable/ComfyUI/models/diffusion_models
2. text_encoders.lnk            -> portable/ComfyUI/models/text_encoders
3. vae.lnk                      -> portable/ComfyUI/models/vae
```

The portable folder contains its own `install-qwen-image21.cmd`, `Run-Qwen-Image21.cmd`, and support `scripts` folder. Upgrading from an older flat layout preserves old portable support files in `.qwen-setup/legacy-support` and old installer support files in `scripts/legacy`.

## Installation details

- Downloads the official [NVIDIA CUDA 13 portable v0.39.0](https://github.com/Comfy-Org/ComfyUI/releases/tag/v0.39.0), `ComfyUI_windows_portable_nvidia.7z`. This exact release tag is pinned; new installs do not follow `latest`. ComfyUI source and WebUI remain intact.
- Selects a compatible [SageAttention Windows wheel](https://github.com/woct0rdho/SageAttention/releases) and `triton-windows` based on Python, PyTorch, CUDA, and GPU compatibility. Triton versions come primarily from the matching PyTorch release metadata.
- Installs embedded Python `include`/`libs` support when needed, following the [Triton portable Python instructions](https://github.com/woct0rdho/triton-windows#8-special-notes-for-comfyui-with-embeded-python).
- Uses `--no-deps` for Sage/Triton installation, preserving the official PyTorch version. If Sage fails, the broken Sage package is removed and the launcher uses PyTorch.
- Checks the official built-in Qwen/Turbo nodes. No additional custom Qwen nodes are needed.
- Optional translation uses the [Hy-MT2 residency fork](https://github.com/david419kr/ComfyUI-Hy-MT2/tree/22ea3d7ebc8938515c0f60dbcd991edbb46ae8d5), pinned to commit `22ea3d7ebc8938515c0f60dbcd991edbb46ae8d5`, with its dedicated Python/CUDA environment. Translation workers and image loader cache entries survive switching between translation and image jobs while memory permits. Explicit full unload clears both. Clean Git installations use a detached pinned checkout; divergent or modified sources are preserved and require manual reconciliation.
- Setup does not automatically generate Qwen images or run translation inference. Its Sage test uses small random CUDA attention tensors.
- The updated PhotoCraft client explicitly releases both image and translation models when either selector changes, then orders subsequent generation behind a lightweight ComfyUI queue marker. Repeated translation/generation with the same models keeps the residency caches. Older PhotoCraft builds benefit from residency but do not send selector-triggered unload requests.

Requires Windows x64 and a CUDA 13 compatible NVIDIA GPU and driver. A Sage compatibility failure uses PyTorch instead; lack of CUDA support itself requires correcting the GPU/driver environment.

## Downloads and portability

- Interrupted downloads resume from `.part` files. Completed files are checked against their expected size and SHA256.
- Hugging Face model URLs are pinned to the repository revision returned when metadata is fetched.
- The official portable archive and 7-Zip extractor are also checksum-verified. Large archives use **64-bit 7za**, all logical CPU cores, and a RAM budget of up to **80% of currently available physical memory** (also capped at 80% of total RAM). These settings are measured on the destination PC at extraction time, not copied from the developer's PC. The 32-bit bootstrap only unpacks the small 7-Zip tool bundle.
- 7-Zip may use fewer workers when the archive structure does not permit more parallel decoding. The configured RAM budget is a limit, not a forced allocation.
- Model downloads show an updating single-line bar with percentage, received/total GB, MB/s, and ETA. Redirected logs receive one summary instead of repeated progress lines.
- Shortcuts are numbered for sorting. Old unnumbered shortcuts are preserved under `scripts/legacy-shortcuts` on rerun.
- Selections and download verification records: `.qwen-setup/install-state.json`.
- Environment details: `.qwen-setup/environment.json`. Sage test output: `.qwen-setup/sage-probe.log`.
- An existing model with a different checksum is preserved, and setup explains how to proceed.
- One image model plus shared encoder/VAE: about **17.3 GB** of model downloads. Both image models plus shared files: about **24.5 GB**. ComfyUI disk usage is additional.
- Spaces and Unicode folder names are supported. Launch CMDs use relative paths. If you move the installation, rerun setup to refresh shortcuts and repair the optional translation environment as needed.
- CUDA/compiler environment variable cleanup applies only to the launched process. System environment variables are unchanged.

### Adding models manually

Use the folder shortcuts created by setup, or place files directly in these folders:

| Folder | File |
|---|---|
| `ComfyUI/models/diffusion_models` | `qwen_image_2.1_int8_convrot.safetensors` or `qwen_image_2.1_turbo_int8_convrot.safetensors` |
| `ComfyUI/models/text_encoders` | `qwen3vl_8b_int8_convrot.safetensors` |
| `ComfyUI/models/vae` | `qwen_image_2.1_vae_bf16.safetensors` |
| `ComfyUI/models/LLM/GGUF` | `Hy-MT2-1.8B-heretic.i1-Q4_K_M.gguf` / `HY-MT2-7B-Abliterated-Q4_K_M.gguf` |

Official Qwen files: <https://huggingface.co/Comfy-Org/Qwen-Image-2.1/tree/main>

## Unattended setup

From an extracted bundle:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\install-qwen-image21.ps1 `
  -PortableRoot "E:\AI Tools\ComfyUI_windows_portable" `
  -Models "1,2" -TranslationModels "1"
```

Use `-Models 0 -DownloadShared Yes` or `No` when adding image models manually. `-InstallerRoot` can explicitly choose where setup shortcuts are created; the CMD supplies its own folder automatically.

## Development checks

```powershell
python -X utf8 scripts/tests/test_qwen_image21_setup.py -v
```

Focused checks cover resumable downloads, ignored Range requests, checksums, existing-file preservation, wheel compatibility, PyTorch fallback, the relative launcher path and browser suppression, selections, and state persistence. Integration checks verify layout migration and the four real Windows shortcut targets. They do not repeat large model downloads or image generation for a layout-only change.
