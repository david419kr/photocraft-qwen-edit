import importlib.util
import tempfile
import unittest
import zipfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("portable", Path(__file__).resolve().parents[1] / "verify-qwen-portable.py")
portable = importlib.util.module_from_spec(spec)
spec.loader.exec_module(portable)


class PortableTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.exe = self.root / "photocraft.exe"
        self.exe.write_bytes(b"synthetic binary, not a Windows build")
        self.archive = self.root / "candidate.zip"

    def package(self, extra=None, missing=None, binary=None, omit_notice_license=False):
        files = {name: b"test" for name in portable.EXPECTED}
        files["licenses/NOTICE.txt"] = b"See licenses/artcraft-brand-terms.txt"
        if not omit_notice_license:
            for source in portable.NOTICE_LICENSES:
                files["licenses/NOTICE.txt"] += b"\n" + (Path(__file__).resolve().parents[2] / source).read_bytes()
        files["photocraft.exe"] = binary or self.exe.read_bytes()
        if extra:
            files[extra] = b"must not ship"
        if missing:
            del files[missing]
        with zipfile.ZipFile(self.archive, "w") as bundle:
            for name, data in files.items():
                bundle.writestr(name, data)
        Path(str(self.archive) + ".sha256").write_text(f"{portable.digest(self.archive)}  {self.archive.name}\n")

    def test_exact_portable_contract(self):
        self.package()
        self.assertEqual(portable.verify(self.archive, self.exe), 18)

    def test_model_or_private_settings_are_rejected(self):
        for extra in ("model.safetensors", "PhotoCraftData/settings.json", "../outside", "ComfyUI/main.py"):
            with self.subTest(extra=extra):
                self.package(extra=extra)
                with self.assertRaisesRegex(ValueError, "file set"):
                    portable.verify(self.archive, self.exe)

    def test_missing_license_is_rejected(self):
        self.package(missing="licenses/oxideav-MIT.txt")
        with self.assertRaisesRegex(ValueError, "file set"):
            portable.verify(self.archive, self.exe)

    def test_missing_embedded_material_license_is_rejected(self):
        self.package(omit_notice_license=True)
        with self.assertRaisesRegex(ValueError, "omits full embedded-material license"):
            portable.verify(self.archive, self.exe)

    def test_wrong_executable_is_rejected(self):
        self.package(binary=b"stale executable")
        with self.assertRaisesRegex(ValueError, "executable differs"):
            portable.verify(self.archive, self.exe)

    def test_wrong_digest_is_rejected(self):
        self.package()
        Path(str(self.archive) + ".sha256").write_text("0" * 64 + "  candidate.zip\n")
        with self.assertRaisesRegex(ValueError, "SHA256"):
            portable.verify(self.archive, self.exe)


if __name__ == "__main__":
    unittest.main()
