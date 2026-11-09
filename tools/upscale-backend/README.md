# Local TensorRT upscaler backend

PhotoCraft discovers this backend at `external-backends/upscale` beside its executable. The
directory contains `backend.json`, `catalog.json`, `runtime/`, `python/`, `models/` and a
generated `cache/`. It can be moved as a unit; keep the model files and runtime inside it.
TensorRT engines are built on the target GPU and cached there. The backend runs independently
of ComfyUI and requires an NVIDIA RTX 30-series or newer GPU and a compatible NVIDIA driver.

The files here are the runtime source and manifests. The model weights and the Windows Python
environment are installed separately and are not part of the PhotoCraft repository. Arrange
the three IllustrationJaNai V3 BF16 ONNX files and the five waifu2x V3 art ONNX files at the
paths in `catalog.json`. `export_waifu2x.py` converts the official V3 art checkpoints to the
mixed-precision ONNX format expected by this backend using an isolated developer environment;
the resulting runtime has no PyTorch dependency.

The private Python 3.12 runtime needs NumPy, Pillow, CUDA Python, CUDA NVRTC and TensorRT 11;
the verified versions are recorded in `python/requirements.lock.txt` in the prepared runtime.
Copy the Python distribution and all required package DLLs into `python/`, with an isolated
`python312._pth` that references only paths within that directory. Copy the `.py` and `.cu`
runtime sources into `runtime/`, and put the two JSON manifests at the backend root. Verify the
installation with `python/python.exe runtime/worker.py --probe` before using it in PhotoCraft.

The illustration model and waifu2x checkpoints have their own licences; obtain and retain those
terms with the model files. These external models are not redistributed by the app source tree.
