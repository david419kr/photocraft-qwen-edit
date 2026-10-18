#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Install official stable host tools where FreeBSD aarch64 lacks rustup-init."""
import hashlib
import platform
from pathlib import Path
import subprocess
import tempfile
import tomllib
from urllib.parse import urlparse


def distribution(manifest):
    target = manifest["pkg"]["rust"]["target"]["aarch64-unknown-freebsd"]
    if not target["available"]:
        raise ValueError("Stable FreeBSD aarch64 host tools unavailable")
    url, digest = target["xz_url"], target["xz_hash"]
    parsed = urlparse(url)
    if parsed.scheme != "https" or parsed.netloc != "static.rust-lang.org":
        raise ValueError("Unexpected Rust distribution origin")
    if len(digest) != 64 or any(c not in "0123456789abcdef" for c in digest):
        raise ValueError("Invalid SHA256 in Rust distribution manifest")
    return url, digest


def verify(path, digest):
    with path.open("rb") as stream:
        actual = hashlib.file_digest(stream, "sha256").hexdigest()
    if actual != digest:
        raise ValueError(f"Rust archive SHA256 mismatch: {actual} != {digest}")


def fetch(url, path):
    subprocess.run(["curl", "--fail", "--location", "--retry", "3", url, "--output", str(path)], check=True)


def main():
    if platform.system() != "FreeBSD" or platform.machine() not in ("arm64", "aarch64"):
        raise SystemExit("This bootstrap is only for native FreeBSD aarch64")
    with tempfile.TemporaryDirectory(prefix="photocraft-rust-") as temp:
        root = Path(temp)
        manifest = root / "channel-rust-stable.toml"
        fetch("https://static.rust-lang.org/dist/channel-rust-stable.toml", manifest)
        with manifest.open("rb") as stream:
            url, digest = distribution(tomllib.load(stream))
        archive = root / "rust.tar.xz"
        print(f"Installing {url}; SHA256 {digest}", flush=True)
        fetch(url, archive)
        verify(archive, digest)
        subprocess.run(["tar", "-xJf", str(archive), "-C", str(root)], check=True)
        installers = list(root.glob("rust-*/install.sh"))
        if len(installers) != 1:
            raise ValueError("Expected exactly one Rust distribution installer")
        subprocess.run(["sh", str(installers[0]), "--prefix=/usr/local", "--without=rust-docs"], check=True)


if __name__ == "__main__":
    main()
