# SPDX-License-Identifier: MIT
import importlib.util
import hashlib
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("freebsd_rust", Path(__file__).parents[1] / "install-freebsd-rust.py")
bootstrap = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bootstrap)


class BootstrapTests(unittest.TestCase):
    def test_available_official_archive(self):
        target = {"available": True, "xz_url": "https://static.rust-lang.org/dist/rust.tar.xz", "xz_hash": "a" * 64}
        manifest = {"pkg": {"rust": {"target": {"aarch64-unknown-freebsd": target}}}}
        self.assertEqual(bootstrap.distribution(manifest), (target["xz_url"], "a" * 64))
        for key, value in [("available", False), ("xz_url", "https://example.com/rust.tar.xz"), ("xz_hash", "bad")]:
            with self.subTest(key=key):
                changed = dict(target, **{key: value})
                manifest["pkg"]["rust"]["target"]["aarch64-unknown-freebsd"] = changed
                with self.assertRaises(ValueError):
                    bootstrap.distribution(manifest)

    def test_corrupt_archive_fails_before_extraction(self):
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "archive"
            archive.write_bytes(b"verified archive")
            bootstrap.verify(archive, hashlib.sha256(archive.read_bytes()).hexdigest())
            archive.write_bytes(b"corrupted archive")
            with self.assertRaisesRegex(ValueError, "SHA256 mismatch"):
                bootstrap.verify(archive, hashlib.sha256(b"verified archive").hexdigest())


if __name__ == "__main__":
    unittest.main()
