"""TensorRT-only validation runner, deliberately independent of torch/nunif."""
import math
from pathlib import Path
import numpy as np
import tensorrt as trt
from cuda.bindings import runtime as cu

LOGGER = trt.Logger(trt.Logger.WARNING)


def cuda(result):
    if int(result[0]) != 0:
        raise RuntimeError(f"CUDA: {result[0]}")
    return result[1] if len(result) == 2 else (result[1:] if len(result) > 2 else None)


class Network:
    def __init__(self, engine_file):
        self.logger = LOGGER
        self.runtime = trt.Runtime(self.logger)
        self.engine = self.runtime.deserialize_cuda_engine(Path(engine_file).read_bytes())
        if self.engine is None:
            raise RuntimeError("Could not load engine")
        self.input_name = next(self.engine.get_tensor_name(i) for i in range(self.engine.num_io_tensors)
                               if self.engine.get_tensor_mode(self.engine.get_tensor_name(i)) == trt.TensorIOMode.INPUT)
        self.output_name = next(self.engine.get_tensor_name(i) for i in range(self.engine.num_io_tensors)
                                if self.engine.get_tensor_mode(self.engine.get_tensor_name(i)) == trt.TensorIOMode.OUTPUT)
        self.context = self.engine.create_execution_context(trt.ExecutionContextAllocationStrategy.USER_MANAGED)
        if self.context is None:
            raise RuntimeError("Could not create execution context")
        self.stream = cuda(cu.cudaStreamCreate())
        self.buffers = {}
        self.peak_device_bytes = 0

    def buffer(self, key, size):
        old = self.buffers.get(key)
        if old is None or old[1] < size:
            if old:
                cuda(cu.cudaFree(old[0]))
            self.buffers[key] = (cuda(cu.cudaMalloc(max(1, size))), max(1, size))
        self.peak_device_bytes = max(self.peak_device_bytes, sum(v[1] for v in self.buffers.values()))
        return self.buffers[key][0]

    def __call__(self, value):
        value = np.ascontiguousarray(value, dtype=np.float32)
        if not self.context.set_input_shape(self.input_name, value.shape):
            raise RuntimeError(f"Input shape rejected: {value.shape}")
        need = self.context.update_device_memory_size_for_shapes()
        self.context.device_memory = int(self.buffer("activation", need))
        shape = tuple(self.context.get_tensor_shape(self.output_name))
        if any(x <= 0 for x in shape):
            raise RuntimeError(f"Unresolved output: {shape}")
        output = np.empty(shape, dtype=np.float32)
        ip = self.buffer("input", value.nbytes)
        op = self.buffer("output", output.nbytes)
        self.context.set_tensor_address(self.input_name, int(ip))
        self.context.set_tensor_address(self.output_name, int(op))
        cuda(cu.cudaMemcpyAsync(ip, value.ctypes.data, value.nbytes, cu.cudaMemcpyKind.cudaMemcpyHostToDevice, self.stream))
        if not self.context.execute_async_v3(int(self.stream)):
            raise RuntimeError("TensorRT execution failed")
        cuda(cu.cudaMemcpyAsync(output.ctypes.data, op, output.nbytes, cu.cudaMemcpyKind.cudaMemcpyDeviceToHost, self.stream))
        cuda(cu.cudaStreamSynchronize(self.stream))
        if not np.isfinite(output).all():
            raise RuntimeError("Nonfinite inference output")
        return output

    def infer_device(self, shape, input_pointer, stream):
        """Enqueue inference on device buffers; no tile host transfer or synchronization."""
        shape = tuple(shape)
        if getattr(self, "_device_shape", None) != shape:
            # TensorRT context shape changes must not race an in-flight execution.
            cuda(cu.cudaStreamSynchronize(stream))
        if not self.context.set_input_shape(self.input_name, shape):
            raise RuntimeError(f"Input shape rejected: {shape}")
        self._device_shape = shape
        need = self.context.update_device_memory_size_for_shapes()
        self.context.device_memory = int(self.buffer("activation", need))
        output_shape = tuple(self.context.get_tensor_shape(self.output_name))
        if any(v <= 0 for v in output_shape):
            raise RuntimeError(f"Unresolved output: {output_shape}")
        output = self.buffer("output", math.prod(output_shape) * 4)
        self.context.set_tensor_address(self.input_name, int(input_pointer))
        self.context.set_tensor_address(self.output_name, int(output))
        if not self.context.execute_async_v3(int(stream)):
            raise RuntimeError("TensorRT execution failed")
        return output, output_shape

    def close(self):
        pipeline = getattr(self, "_gpu_pipeline", None)
        if pipeline is not None:
            pipeline.close()
            self._gpu_pipeline = None
        if self.context is not None:
            self.context = None
        for pointer, _ in self.buffers.values():
            cuda(cu.cudaFree(pointer))
        self.buffers.clear()
        if self.stream is not None:
            cuda(cu.cudaStreamDestroy(self.stream))
            self.stream = None
        self.engine = None
        self.runtime = None


def tiled(x, model, requested_tile, batch):
    tile = requested_tile - requested_tile % 32
    if tile < 64:
        raise ValueError("Invalid V3 tile")
    _, h, w = x.shape
    scale, offset, blend = 2, 16, 8
    step = tile - 20
    nh = max(1, math.ceil((h + 16 - tile) / step) + 1)
    nw = max(1, math.ceil((w + 16 - tile) / step) + 1)
    ph, pw = (nh - 1) * step + tile, (nw - 1) * step + tile
    padded = np.pad(x, ((0, 0), (8, ph - h - 8), (8, pw - w - 8)), mode="edge")
    pixels = np.zeros((3, ph * 2, pw * 2), dtype=np.float32)
    weights = np.zeros((1, ph * 2, pw * 2), dtype=np.float32)
    output_side = tile * 2 - 32
    pos = np.arange(output_side)
    edge = np.minimum(pos + 1, output_side - pos)
    filt = np.minimum(edge[:, None], edge[None, :]).astype(np.float32)
    filt = np.minimum(filt, blend + 1)[None] / np.float32(blend + 1)
    indexes = [(i, j) for i in range(nh) for j in range(nw)]
    for start in range(0, len(indexes), batch):
        subset = indexes[start:start + batch]
        feed = np.stack([padded[:, i * step:i * step + tile, j * step:j * step + tile] for i, j in subset])
        output = model(feed)
        if output.shape != (len(subset), 3, output_side, output_side):
            raise RuntimeError(f"Wrong tile output: {output.shape}")
        for value, (i, j) in zip(output, subset):
            y, z = i * step * 2, j * step * 2
            region = (slice(None), slice(y, y + output_side), slice(z, z + output_side))
            old = weights[region]
            total = old + filt
            ratio = old / total
            pixels[region] = pixels[region] * ratio + value * (1 - ratio)
            weights[region] += filt
    return np.clip(pixels[:, :h * 2, :w * 2], 0, 1)


def rgb_infer(x, model, tile, batch, tta):
    if not tta:
        return tiled(x, model, tile, batch)
    avg = None
    for rotate in (False, True):
        base = np.rot90(x, 1, axes=(1, 2)) if rotate else x
        for horizontal, vertical in ((False, False), (True, False), (False, True), (True, True)):
            value = base
            if vertical: value = np.flip(value, 1)
            if horizontal: value = np.flip(value, 2)
            value = tiled(value, model, tile, batch)
            if horizontal: value = np.flip(value, 2)
            if vertical: value = np.flip(value, 1)
            if rotate: value = np.rot90(value, -1, axes=(1, 2))
            avg = value.copy() if avg is None else avg + value
    return np.clip(avg * np.float32(1 / 8), 0, 1)


def pad_alpha(rgb, alpha):
    h, w = alpha.shape[-2:]
    mask = (alpha[0] > 0).astype(np.float32)
    missing = mask < 1
    rgb = rgb.copy()
    rgb[:, missing] = 0
    for _ in range(16):
        m = np.pad(mask, 1)
        r = np.pad(rgb, ((0, 0), (1, 1), (1, 1)))
        mw = sum(m[i:i + h, j:j + w] for i in range(3) for j in range(3))
        sums = sum(r[:, i:i + h, j:j + w] for i in range(3) for j in range(3))
        border = sums / (mw[None] + np.float32(1e-7))
        rgb[:, missing] = border[:, missing]
        mask = (mw > 0).astype(np.float32)
        missing = mask < 1
    return np.clip(rgb, 0, 1)


def convert_numpy_reference(rgb, alpha, model, alpha_model, tile=640, batch=4, tta=True, keep_alpha=True):
    if not keep_alpha:
        if alpha is not None: rgb = rgb * alpha + (1 - alpha)
        return rgb_infer(rgb, model, tile, batch, tta), None
    nonopaque = alpha is not None and not np.all(alpha == 1)
    if nonopaque: rgb = pad_alpha(rgb, alpha)
    result = rgb_infer(rgb, model, tile, batch, tta)
    if alpha is None:
        a = None
    elif nonopaque:
        a = tiled(np.repeat(alpha, 3, axis=0), alpha_model, tile, batch).mean(axis=0, keepdims=True)
    else:
        a = np.ones((1, result.shape[1], result.shape[2]), dtype=np.float32)
    return result, a


def convert(rgb, alpha, model, alpha_model, tile=640, batch=4, tta=True, keep_alpha=True):
    """Default execution keeps pre/post processing on the GPU."""
    from trt_gpu import Pipeline
    pipeline = getattr(model, "_gpu_pipeline", None)
    if pipeline is None:
        pipeline = model._gpu_pipeline = Pipeline()
    return pipeline.convert(rgb, alpha, model, alpha_model, tile, batch, tta, keep_alpha)
