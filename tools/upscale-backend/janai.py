"""Fourfold GPU inference and overlap blending using the shared TensorRT runtime."""
import math
import numpy as np
from trt_gpu import Pipeline
from trt_runner import cuda, cu


def convert(rgb, alpha, model, tile, overlap, tiling, progress):
    h, w = rgb.shape[1:]
    th = tile if tiling else math.ceil(h / 32) * 32
    tw = tile if tiling else math.ceil(w / 32) * 32
    step_y, step_x = th - overlap if tiling else th, tw - overlap if tiling else tw
    ny = max(1, math.ceil((h - th) / step_y) + 1)
    nx = max(1, math.ceil((w - tw) / step_x) + 1)
    oh, ow = h * 4, w * 4
    p = Pipeline()
    p.stats = dict(h2d_bytes=0, d2h_bytes=0, h2d_image_copies=0, d2h_image_copies=0,
                   d2h_control_copies=0, tile_host_copies=0, inference_calls=0, kernel_launches=0)
    try:
        p.flag = p.buffer("flag", 4)
        cuda(cu.cudaMemsetAsync(p.flag, 0, 4, p.stream))
        source, a = p.upload("rgb", rgb), p.upload("alpha", alpha)
        p.launch("inspect_alpha", h * w, (a, p.flag), (h * w,))
        status = int(p.download(p.flag, (1,), np.int32, control=True)[0])
        if status & 2:
            raise RuntimeError("Invalid alpha")
        if status & 1:
            x, y = p.buffer("border_a", rgb.size * 4), p.buffer("border_b", rgb.size * 4)
            ma, mb = p.buffer("mask_a", h * w * 4), p.buffer("mask_b", h * w * 4)
            p.launch("border_init", h * w, (source, a, x, ma), (h * w,))
            for i in range(16):
                p.launch("border_step", h * w, (x, ma, y, mb), (h, w, i == 15))
                x, y, ma, mb = y, x, mb, ma
            source = x
        feed = p.buffer("feed", 3 * th * tw * 4)
        out = p.buffer("result", 3 * oh * ow * 4)
        weights = p.buffer("weights", oh * ow * 4)
        cuda(cu.cudaMemsetAsync(out, 0, 3 * oh * ow * 4, p.stream))
        cuda(cu.cudaMemsetAsync(weights, 0, oh * ow * 4, p.stream))
        for iy in range(ny):
            for ix in range(nx):
                top, left = iy * step_y, ix * step_x
                progress((iy * nx + ix) / (ny * nx))
                p.launch("gather_upscale", 3 * th * tw, (source, feed), (h, w, th, tw, top, left))
                result, shape = model.infer_device((1, 3, th, tw), feed, p.stream)
                if shape != (1, 3, th * 4, tw * 4):
                    raise RuntimeError(f"Unexpected model output: {shape}")
                p.launch("merge_upscale", th * tw * 16, (result, out, weights, p.flag),
                         (oh, ow, th * 4, tw * 4, top * 4, left * 4, overlap * 4 if tiling else 0,
                          iy > 0, ix > 0, iy + 1 < ny, ix + 1 < nx))
        p.launch("finish_rgb", 3 * oh * ow, (out,), (3 * oh * ow, 1))
        resized_alpha = p.buffer("alpha_result", oh * ow * 4)
        p.launch("resize_alpha", oh * ow, (a, resized_alpha), (h, w, 4))
        result, output_alpha = p.download(out, (3, oh, ow)), p.download(resized_alpha, (1, oh, ow))
        if int(p.download(p.flag, (1,), np.int32, control=True)[0]) & 2:
            raise RuntimeError("Nonfinite model output")
        return result, output_alpha
    finally:
        p.close()
