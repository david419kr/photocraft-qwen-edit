#!/usr/bin/env python3
"""Verify the portable ZIP's exact file allowlist, provenance and sidecar hash."""
import argparse
import hashlib
import zipfile
from pathlib import Path


LICENSES = {
    "LICENSE-MIT.txt", "LICENSE-APACHE.txt", "NOTICE.txt", "oxideav-MIT.txt",
    "avif-encoder.txt", "OFL-Inter.txt", "OFL-JetBrainsMono.txt", "LICENSE-lucide.txt",
    "LICENSE-translations.txt", "LICENSE-app-icon.txt", "artcraft-brand-terms.txt",
}
SCRIPTS = {"install-qwen-image21.ps1", "qwen-image21-setup.py", "install-comfyui-hymt2.ps1"}
EXPECTED = {"photocraft.exe", "portable.txt", "README.txt", "install-qwen-image21.cmd"}
EXPECTED |= {"licenses/" + name for name in LICENSES} | {"scripts/" + name for name in SCRIPTS}


def digest(path):
    with Path(path).open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def verify(archive, executable):
    archive, executable = Path(archive), Path(executable)
    sidecar = Path(str(archive) + ".sha256").read_text(encoding="utf-8").strip()
    if sidecar != f"{digest(archive)}  {archive.name}":
        raise ValueError("ZIP SHA256 sidecar mismatch")
    with zipfile.ZipFile(archive) as bundle:
        names = bundle.namelist()
        if len(names) != len(set(names)) or set(names) != EXPECTED:
            raise ValueError(f"ZIP file set differs: missing={EXPECTED - set(names)}, extra={set(names) - EXPECTED}")
        if any(info.file_size == 0 for info in bundle.infolist()):
            raise ValueError("Empty file in portable ZIP")
        if bundle.testzip() is not None:
            raise ValueError("ZIP CRC error")
        with bundle.open("photocraft.exe") as binary:
            if hashlib.file_digest(binary, "sha256").hexdigest() != digest(executable):
                raise ValueError("Packaged executable differs from the built candidate")
        for name in ("README.txt", "portable.txt", "licenses/NOTICE.txt"):
            bundle.read(name).decode("utf-8")
        if b"licenses/artcraft-brand-terms.txt" not in bundle.read("licenses/NOTICE.txt"):
            raise ValueError("Packaged NOTICE points to a missing brand terms path")
    return len(EXPECTED)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--zip", type=Path, required=True)
    parser.add_argument("--executable", type=Path, required=True)
    args = parser.parse_args()
    print(f"Verified {verify(args.zip, args.executable)} files, candidate executable and ZIP SHA256")
