"""Private stdin/stdout worker; runs only in the Hy-MT2 node's dedicated venv."""
import contextlib
import json
import os
import sys
import traceback
from pathlib import Path

def main():
    cuda_bin = Path(os.environ.get("CUDA_PATH", "")) / "bin"
    cuda_dll_directory = os.add_dll_directory(str(cuda_bin)) if os.name == "nt" and cuda_bin.is_dir() else None
    output = sys.stdout
    model = None
    try:
        for line in sys.stdin:
            try:
                request = json.loads(line)
                with contextlib.redirect_stdout(sys.stderr):
                    if request["op"] == "load":
                        from llama_cpp import Llama
                        args = request["args"]
                        if Path(args.get("model_path", "")).name == "Hy-MT2-1.8B-heretic.i1-Q4_K_M.gguf":
                            # This GGUF stores EOS=3 ('$'); its official tokenizer uses 120020.
                            args["kv_overrides"] = {**args.get("kv_overrides", {}), "tokenizer.ggml.eos_token_id": 120020}
                        model = Llama(**args)
                        result = None
                    elif request["op"] == "complete" and model is not None:
                        result = model.create_chat_completion(**request["args"])
                    else:
                        raise ValueError("Unknown operation or model not loaded")
                response = {"ok": True, "result": result}
            except Exception as error:
                traceback.print_exc(file=sys.stderr)
                response = {"ok": False, "error": str(error)}
            output.write(json.dumps(response, ensure_ascii=False) + "\n")
            output.flush()
    finally:
        if model is not None:
            with contextlib.redirect_stdout(sys.stderr):
                model.close()

if __name__ == "__main__":
    main()
