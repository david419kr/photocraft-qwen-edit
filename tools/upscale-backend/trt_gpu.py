"""GPU-resident V3 tiling, TTA, alpha processing and TensorRT execution."""
from pathlib import Path
from contextlib import contextmanager
import hashlib
import math
import os
import numpy as np
from cuda.bindings import driver, nvrtc
from trt_runner import cuda, cu

_MODULE = None


def kernels():
    global _MODULE
    if _MODULE is not None:
        return _MODULE
    source = Path(__file__).with_name("prepost.cu").read_bytes()
    cuda(driver.cuInit(0))
    # Use the CUDA runtime's primary context, the same one TensorRT uses.
    cuda(cu.cudaFree(0))
    version = cuda(nvrtc.nvrtcVersion())
    key = hashlib.sha256(source + repr(version).encode() + b"compute_80-fmad-false-v1").hexdigest()
    cache = Path(__file__).resolve().parents[1] / "cache/cuda-prepost"
    cache.mkdir(parents=True, exist_ok=True)
    path = cache / (key + ".ptx")
    if path.exists():
        ptx = path.read_bytes()
    else:
        program = cuda(nvrtc.nvrtcCreateProgram(source, b"prepost.cu", 0, [], []))
        try:
            options = [b"--gpu-architecture=compute_80", b"--std=c++11", b"--fmad=false"]
            status = nvrtc.nvrtcCompileProgram(program, len(options), options)
            if int(status[0]):
                log = bytearray(cuda(nvrtc.nvrtcGetProgramLogSize(program)))
                cuda(nvrtc.nvrtcGetProgramLog(program, log))
                raise RuntimeError(log.decode("utf-8", "replace"))
            ptx = bytearray(cuda(nvrtc.nvrtcGetPTXSize(program)))
            cuda(nvrtc.nvrtcGetPTX(program, ptx))
            ptx = bytes(ptx)
            temporary = path.with_suffix(f".{os.getpid()}.tmp")
            temporary.write_bytes(ptx)
            temporary.replace(path)
        finally:
            cuda(nvrtc.nvrtcDestroyProgram(program))
    module = cuda(driver.cuModuleLoadData(ptx))
    names = ["inspect_alpha", "white_rgb", "border_init", "border_step", "gather_tiles", "merge_tile", "tta_add", "finish_rgb", "finish_alpha", "gather_upscale", "merge_upscale", "resize_alpha"]
    _MODULE = (module, {name: cuda(driver.cuModuleGetFunction(module, name.encode())) for name in names})
    return _MODULE


class Pipeline:
    def __init__(self, profile=False):
        _, self.functions = kernels()
        self.stream = cuda(cu.cudaStreamCreate())
        self.buffers = {}
        self.peak_device_bytes = 0
        self.profile = profile
        self.events = []
        self.stats = {}
        self.last_profile = {}

    def buffer(self, key, size):
        old = self.buffers.get(key)
        if old is None or old[1] < size:
            if old:
                cuda(cu.cudaStreamSynchronize(self.stream))
                cuda(cu.cudaFree(old[0]))
            self.buffers[key] = (cuda(cu.cudaMalloc(max(1, size))), max(1, size))
        self.peak_device_bytes = max(self.peak_device_bytes, sum(v[1] for v in self.buffers.values()))
        return self.buffers[key][0]

    @contextmanager
    def stage(self, name):
        if not self.profile:
            yield
            return
        start = cuda(cu.cudaEventCreate())
        end = cuda(cu.cudaEventCreate())
        cuda(cu.cudaEventRecord(start, self.stream))
        try:
            yield
        finally:
            cuda(cu.cudaEventRecord(end, self.stream))
            self.events.append((name, start, end))

    def launch(self, name, count, pointers, ints):
        values = [np.array(int(v), dtype=np.uint64) for v in pointers]
        values += [np.array(int(v), dtype=np.int32) for v in ints]
        args = np.array([v.ctypes.data for v in values], dtype=np.uint64)
        cuda(driver.cuLaunchKernel(self.functions[name], (count + 255) // 256, 1, 1, 256, 1, 1, 0,
                                  self.stream, args.ctypes.data, 0))
        self.stats["kernel_launches"] += 1

    def upload(self, key, value):
        value = np.ascontiguousarray(value, dtype=np.float32)
        ptr = self.buffer(key, value.nbytes)
        cuda(cu.cudaMemcpyAsync(ptr, value.ctypes.data, value.nbytes, cu.cudaMemcpyKind.cudaMemcpyHostToDevice, self.stream))
        self.stats["h2d_bytes"] += value.nbytes
        self.stats["h2d_image_copies"] += 1
        return ptr

    def download(self, ptr, shape, dtype=np.float32, control=False):
        value = np.empty(shape, dtype=dtype)
        cuda(cu.cudaMemcpyAsync(value.ctypes.data, ptr, value.nbytes, cu.cudaMemcpyKind.cudaMemcpyDeviceToHost, self.stream))
        cuda(cu.cudaStreamSynchronize(self.stream))
        self.stats["d2h_bytes"] += value.nbytes
        self.stats["d2h_control_copies" if control else "d2h_image_copies"] += 1
        return value

    def tiled(self, source, model, h, w, tile, batch, rotate=0, horizontal=0, vertical=0, alpha_source=False):
        th, tw = (w, h) if rotate else (h, w)
        step = tile - 20
        nh = max(1, math.ceil((th + 16 - tile) / step) + 1)
        nw = max(1, math.ceil((tw + 16 - tile) / step) + 1)
        ph, pw = ((nh - 1) * step + tile) * 2, ((nw - 1) * step + tile) * 2
        pixels = self.buffer("tiles", 3 * ph * pw * 4)
        weights = self.buffer("weights", ph * pw * 4)
        cuda(cu.cudaMemsetAsync(pixels, 0, 3 * ph * pw * 4, self.stream))
        cuda(cu.cudaMemsetAsync(weights, 0, ph * pw * 4, self.stream))
        feed = self.buffer("feed", min(batch, nh * nw) * 3 * tile * tile * 4)
        side = tile * 2 - 32
        for start in range(0, nh * nw, batch):
            count = min(batch, nh * nw - start)
            with self.stage("gather_ms"):
                self.launch("gather_tiles", count * 3 * tile * tile, (source, feed),
                            (h, w, th, tw, tile, step, nw, start, count, rotate, horizontal, vertical, alpha_source))
            with self.stage("inference_ms"):
                output, shape = model.infer_device((count, 3, tile, tile), feed, self.stream)
            if shape != (count, 3, side, side):
                raise RuntimeError(f"Unexpected tile output: {shape}")
            self.stats["inference_calls"] += 1
            with self.stage("merge_ms"):
                for b in range(count):
                    index = start + b
                    self.launch("merge_tile", side * side, (output, pixels, weights, self.flag),
                                (side, ph, pw, index // nw * step * 2, index % nw * step * 2, b))
        return pixels, ph, pw

    def convert(self, rgb, alpha, model, alpha_model, tile=640, batch=4, tta=True, keep_alpha=True):
        if rgb.ndim != 3 or rgb.shape[0] != 3 or min(rgb.shape[1:]) < 1:
            raise ValueError("Expected a nonempty CHW RGB input")
        h, w = rgb.shape[1:]
        if alpha is not None and alpha.shape != (1, h, w):
            raise ValueError("Alpha shape mismatch")
        tile -= tile % 32
        if tile < 64 or batch < 1:
            raise ValueError("Invalid tile or batch")
        self.stats = dict(h2d_bytes=0, d2h_bytes=0, h2d_image_copies=0, d2h_image_copies=0,
                          d2h_control_copies=0, tile_host_copies=0, inference_calls=0, kernel_launches=0)
        self.flag = self.buffer("flag", 4)
        cuda(cu.cudaMemsetAsync(self.flag, 0, 4, self.stream))
        with self.stage("upload_ms"):
            source = self.upload("rgb", rgb)
            ap = self.upload("alpha", alpha) if alpha is not None else None
        nonopaque = False
        if keep_alpha and ap is not None:
            with self.stage("alpha_inspect_ms"):
                self.launch("inspect_alpha", h * w, (ap, self.flag), (h * w,))
            flag = int(self.download(self.flag, (1,), np.int32, control=True)[0])
            if flag & 2:
                raise RuntimeError("Invalid alpha")
            nonopaque = bool(flag & 1)
        if not keep_alpha and ap is not None:
            white = self.buffer("white", rgb.size * 4)
            with self.stage("alpha_prepare_ms"):
                self.launch("white_rgb", rgb.size, (source, ap, white), (h * w,))
            source = white
        elif nonopaque:
            if alpha_model is None:
                raise ValueError("Alpha model required")
            a = self.buffer("border_a", rgb.size * 4)
            b = self.buffer("border_b", rgb.size * 4)
            ma = self.buffer("mask_a", h * w * 4)
            mb = self.buffer("mask_b", h * w * 4)
            with self.stage("alpha_prepare_ms"):
                self.launch("border_init", h * w, (source, ap, a, ma), (h * w,))
                for i in range(16):
                    self.launch("border_step", h * w, (a, ma, b, mb), (h, w, i == 15))
                    a, b, ma, mb = b, a, mb, ma
            source = a
        oh, ow = h * 2, w * 2
        result = self.buffer("result", 3 * oh * ow * 4)
        variants = [(r, hx, vy) for r in (0, 1) for hx, vy in ((0, 0), (1, 0), (0, 1), (1, 1))] if tta else [(0, 0, 0)]
        for index, (r, hx, vy) in enumerate(variants):
            pixels, ph, pw = self.tiled(source, model, h, w, tile, batch, r, hx, vy)
            with self.stage("tta_ms"):
                self.launch("tta_add", oh * ow, (pixels, result), (oh, ow, ph, pw, r, hx, vy, index == 0))
        with self.stage("tta_ms"):
            self.launch("finish_rgb", 3 * oh * ow, (result,), (3 * oh * ow, len(variants)))
        alpha_result = None
        if keep_alpha and ap is not None:
            alpha_result = self.buffer("alpha_result", oh * ow * 4)
            if nonopaque:
                pixels, ph, pw = self.tiled(ap, alpha_model, h, w, tile, batch, alpha_source=True)
            with self.stage("alpha_finish_ms"):
                self.launch("finish_alpha", oh * ow, (pixels, alpha_result), (oh, ow, ph, pw, not nonopaque))
        with self.stage("download_ms"):
            y = self.download(result, (3, oh, ow))
            aa = self.download(alpha_result, (1, oh, ow)) if alpha_result is not None else None
        flag = int(self.download(self.flag, (1,), np.int32, control=True)[0])
        if flag & 2:
            raise RuntimeError("Nonfinite TensorRT output")
        self.last_profile = {}
        for name, start, end in self.events:
            ms = cuda(cu.cudaEventElapsedTime(start, end))
            self.last_profile[name] = self.last_profile.get(name, 0.0) + ms
            cuda(cu.cudaEventDestroy(start))
            cuda(cu.cudaEventDestroy(end))
        self.events.clear()
        return y, aa

    def close(self):
        if self.stream is not None:
            cuda(cu.cudaStreamSynchronize(self.stream))
            for pointer, _ in self.buffers.values():
                cuda(cu.cudaFree(pointer))
            self.buffers.clear()
            for _, start, end in self.events:
                cuda(cu.cudaEventDestroy(start))
                cuda(cu.cudaEventDestroy(end))
            self.events.clear()
            cuda(cu.cudaStreamDestroy(self.stream))
            self.stream = None
