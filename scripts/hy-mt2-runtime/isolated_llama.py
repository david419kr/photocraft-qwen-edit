"""The node's Llama interface, backed by its private Python process rather than ComfyUI imports."""
import json
import os
import queue
import subprocess
import threading
from pathlib import Path

class Llama:
    def __init__(self, **kwargs):
        root = Path(__file__).resolve().parent.parent
        python = root / ".venv" / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
        if not python.is_file():
            raise RuntimeError("Hy-MT2 dedicated runtime missing; run install-comfyui-hymt2.ps1")
        env = os.environ.copy()
        for key in ("PYTHONPATH", "PYTHONHOME", "HIP_PATH"):
            env.pop(key, None)
        self._lock = threading.Lock()
        self._responses = queue.Queue()
        self._process = subprocess.Popen(
            [str(python), "-I", "-X", "utf8", "-u", str(root / "runtime" / "llama_worker.py")],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=None,
            text=True, encoding="utf-8", env=env,
            creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
        )
        def read_responses(stream, responses):
            try:
                while True:
                    line = stream.readline(2 << 20)
                    if not line:
                        break
                    responses.put(json.loads(line))
            except Exception as error:
                responses.put({"ok": False, "error": str(error)})
            finally:
                responses.put({"ok": False, "error": "Hy-MT2 dedicated worker exited; check ComfyUI log"})
        threading.Thread(target=read_responses, args=(self._process.stdout, self._responses), daemon=True).start()
        try:
            self._request("load", kwargs)
        except Exception:
            self.close()
            raise

    def _request(self, operation, args):
        with self._lock:
            try:
                self._process.stdin.write(json.dumps({"op": operation, "args": args}, ensure_ascii=False) + "\n")
                self._process.stdin.flush()
                response = self._responses.get(timeout=300)
                if not response.get("ok"):
                    raise RuntimeError(response.get("error", "Hy-MT2 dedicated worker failed"))
                return response.get("result")
            except (OSError, queue.Empty) as error:
                self.close()
                raise RuntimeError("Hy-MT2 dedicated worker disconnected or timed out") from error

    def create_chat_completion(self, **kwargs):
        return self._request("complete", kwargs)

    def close(self):
        process = getattr(self, "_process", None)
        if process is not None:
            self._process = None
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=3)
            if process.stdin:
                process.stdin.close()

    def __del__(self):
        try:
            self.close()
        except Exception:
            pass
