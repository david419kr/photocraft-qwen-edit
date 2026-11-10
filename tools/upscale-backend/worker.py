"""Standalone PhotoCraft external upscaler; all paths resolve inside its installation."""
import hashlib
import json
import math
import os
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
os.environ["PATH"] = str(ROOT / "python") + os.pathsep + os.environ.get("PATH", "")
import numpy as np
import tensorrt as trt
from cuda.bindings import runtime as cu
from trt_runner import Network, cuda


def devices():
    result = []
    for i in range(cuda(cu.cudaGetDeviceCount())):
        props = cuda(cu.cudaGetDeviceProperties(i))
        if props.major >= 8:
            result.append({"id": i, "name": bytes(props.name).split(b"\0", 1)[0].decode("utf-8", "replace"),
                           "capability": f"{props.major}.{props.minor}"})
    return result


def engine(model, shape, batch, workspace, gpu):
    key = hashlib.sha256(model.read_bytes()).hexdigest()[:24]
    props = cuda(cu.cudaGetDeviceProperties(gpu))
    identity = hashlib.sha256(repr((bytes(props.uuid.bytes), props.major, props.minor, cuda(cu.cudaDriverGetVersion()),
                                    trt.__version__, shape, batch, workspace)).encode()).hexdigest()[:20]
    dest = ROOT / "cache" / "tensorrt" / f"{key}-{identity}.engine"
    if dest.exists():
        return dest
    dest.parent.mkdir(parents=True, exist_ok=True)
    logger = trt.Logger(trt.Logger.WARNING)
    builder = trt.Builder(logger)
    network = builder.create_network(0)
    parser = trt.OnnxParser(network, logger)
    if not parser.parse_from_file(str(model)):
        raise RuntimeError("ONNX parse failed: " + "\n".join(str(parser.get_error(i)) for i in range(parser.num_errors)))
    config = builder.create_builder_config()
    config.set_memory_pool_limit(trt.MemoryPoolType.WORKSPACE, workspace * 1024 * 1024)
    config.builder_optimization_level = 1
    profile = builder.create_optimization_profile()
    name = network.get_input(0).name
    profile.set_shape(name, (1, 3, *shape), (min(batch, 4), 3, *shape), (batch, 3, *shape))
    config.add_optimization_profile(profile)
    data = builder.build_serialized_network(network, config)
    if data is None:
        raise RuntimeError("TensorRT engine build failed; try a smaller tile or workspace")
    tmp = dest.with_suffix(f".{os.getpid()}.tmp")
    tmp.write_bytes(bytes(data))
    tmp.replace(dest)
    return dest


def main():
    available = devices()
    if "--probe" in sys.argv:
        print(json.dumps(available))
        return
    request = json.loads(Path(os.environ["PHOTOCRAFT_BACKEND_REQUEST"]).read_text(encoding="utf-8"))
    opts = request["options"]
    chosen = opts["gpu"]
    if not available:
        raise RuntimeError("No supported NVIDIA GPU found")
    if chosen == -1:
        chosen = available[0]["id"]
    if chosen not in [x["id"] for x in available]:
        raise RuntimeError("Selected GPU is unavailable")
    cuda(cu.cudaSetDevice(chosen))
    catalog = json.loads((ROOT / "catalog.json").read_text(encoding="utf-8"))
    model = next(x for x in catalog if x["id"] == request["model"])
    h, w = request["height"], request["width"]
    rgb = np.fromfile(request["inputRgb"], dtype="<f4").reshape(h, w, 3).transpose(2, 0, 1).copy()
    alpha = np.fromfile(request["inputAlpha"], dtype="<f4").reshape(1, h, w)
    cancel = Path(os.environ["PHOTOCRAFT_BACKEND_CANCEL"])
    progress_path = Path(request["progress"])
    def progress(value, message="Upscaling"):
        if cancel.exists():
            raise RuntimeError("Cancelled")
        tmp = progress_path.with_suffix(".tmp")
        tmp.write_text(json.dumps({"value": value, "message": message}), encoding="utf-8")
        tmp.replace(progress_path)
    networks = []
    try:
        progress(0.01, "Preparing GPU engine")
        if model["restoration"]:
            tile = opts["tile"] - opts["tile"] % 32
            step = tile - 20
            tiles = max(1, math.ceil((h + 16 - tile) / step) + 1) * max(1, math.ceil((w + 16 - tile) / step) + 1)
            batch = min(opts["batch"], tiles)
            noise = opts["noise"]
            modelpath = ROOT / "models" / "waifu2x" / ("scale2x.onnx" if noise < 0 else f"noise{noise}_scale2x.onnx")
            selected = Network(engine(modelpath, (tile, tile), batch, opts["workspaceMib"], chosen)); networks.append(selected)
            alpha_model = selected
            if noise >= 0 and opts["alpha"]:
                class AlphaModel:
                    def infer_device(self, shape, pointer, stream):
                        if not hasattr(self, "network"):
                            path = ROOT / "models" / "waifu2x" / "scale2x.onnx"
                            self.network = Network(engine(path, (tile, tile), batch, opts["workspaceMib"], chosen))
                            networks.append(self.network)
                        return self.network.infer_device(shape, pointer, stream)
                alpha_model = AlphaModel()
            from trt_gpu import Pipeline
            pipe = Pipeline()
            try:
                output, out_alpha = pipe.convert(rgb, alpha, selected, alpha_model, tile=tile, batch=opts["batch"], tta=opts["tta"], keep_alpha=opts["alpha"])
            finally:
                pipe.close()
        else:
            tile, overlap = opts["tile"], opts["overlap"]
            shape = (tile, tile) if opts["tiling"] else (math.ceil(h / 32) * 32, math.ceil(w / 32) * 32)
            modelpath = ROOT / model["file"]
            selected = Network(engine(modelpath, shape, 1, opts["workspaceMib"], chosen)); networks.append(selected)
            from janai import convert
            output, out_alpha = convert(rgb, alpha, selected, tile, overlap, opts["tiling"], lambda x: progress(.03 + x * .85))
        progress(.91, "Saving result")
        np.ascontiguousarray(output.transpose(1, 2, 0), dtype="<f4").tofile(request["outputRgb"])
        if out_alpha is not None:
            np.ascontiguousarray(out_alpha, dtype="<f4").tofile(request["outputAlpha"])
        Path(os.environ["PHOTOCRAFT_BACKEND_RESULT"]).write_text(json.dumps({"width": w * model["scale"], "height": h * model["scale"], "hasAlpha": out_alpha is not None}), encoding="utf-8")
    finally:
        for net in networks:
            net.close()


if __name__ == "__main__":
    main()
