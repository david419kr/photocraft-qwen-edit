"""Developer-only conversion of the official V3 art models to mixed-precision ONNX.

Requires the official nunif checkout, its PyTorch export dependencies, onnx and
onnxconverter-common. The standalone inference runtime does not need PyTorch.
"""
import argparse
from pathlib import Path

import onnx
from onnxconverter_common import float16
import waifu2x  # registers the official model architecture
from nunif.models import load_model


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("models", type=Path, help="official V3 art checkpoint directory")
    parser.add_argument("output", type=Path, help="destination for ONNX files")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    sensitive = ["Pow", "ReduceMean", "Sqrt", "Reciprocal", "Softmax", "ReduceSum", "Div"]
    blocked = list(set(float16.DEFAULT_OP_BLOCK_LIST) | set(sensitive))
    for name in ["scale2x", "noise0_scale2x", "noise1_scale2x", "noise2_scale2x", "noise3_scale2x"]:
        checkpoint = args.models / (name + ".pth")
        model, _ = load_model(str(checkpoint), map_location="cpu", weights_only=True)
        full = args.output / (name + ".fp32.onnx")
        model.export_onnx(str(full), dynamo=True)
        graph = float16.convert_float_to_float16(onnx.load(str(full)), keep_io_types=True, op_block_list=blocked)
        onnx.save(graph, str(args.output / (name + ".onnx")))


if __name__ == "__main__":
    main()
