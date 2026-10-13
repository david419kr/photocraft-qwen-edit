"""Exercise the actual PowerShell replacement helper with native Rust fixtures.

Runs on Windows (powershell.exe) and Linux (pwsh); Windows also tests EXE locking.
Set PHOTOCRAFT_TEST_PWSH when PowerShell is installed outside PATH.
"""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import unittest


ROOT = Path(__file__).resolve().parents[2]
HELPER = ROOT / "apps/photocraft/src/updater.ps1"
PWSH = os.environ.get("PHOTOCRAFT_TEST_PWSH") or shutil.which("pwsh") or shutil.which("powershell.exe")


@unittest.skipUnless(PWSH and shutil.which("rustc"), "PowerShell and rustc required")
class HelperTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.fixtures = tempfile.TemporaryDirectory(prefix="photocraft-update-fixtures-")
        cls.binaries = {}
        for version in ["old", "new"]:
            root = Path(cls.fixtures.name)
            source = root / f"{version}.rs"
            source.write_text('fn main() { if std::env::args().nth(1).as_deref() == Some("--wait") { '
                              'std::thread::sleep(std::time::Duration::from_secs(30)); return; } let args: Vec<_> = std::env::args().skip(1).collect(); '
                              f'let text = format!("{version}\\n{{}}", args.join("\\n")); '
                              'std::fs::write(std::env::var("UPDATE_TEST_LOG").unwrap(), text).unwrap(); }')
            binary = root / f"{version}.exe"
            subprocess.run(["rustc", str(source), "-o", str(binary)], check=True, capture_output=True)
            cls.binaries[version] = binary.read_bytes()

    @classmethod
    def tearDownClass(cls):
        cls.fixtures.cleanup()

    def scenario(self, mode):
        with tempfile.TemporaryDirectory(prefix="PhotoCraft space '한글-") as temporary:
            root = Path(temporary)
            target = root / "photocraft.exe"
            target.write_bytes(self.binaries["old"])
            target.chmod(0o755)
            data = root / "PhotoCraftData"
            data.mkdir()
            settings = data / "preferences.json"
            settings.write_text('{"automaticUpdateCheck":false}')
            stage = root / ".photocraft-update-test"
            stage.mkdir()
            staged = stage / "photocraft.exe"
            contents = b"not an executable" if mode == "restart-failure" else self.binaries["new"]
            staged.write_bytes(contents)
            staged.chmod(0o755)
            log = root / "restarted.txt"
            arguments = ["space value", 'quote"value', "한글", "backslash\\", ""]
            old = subprocess.Popen([str(target), "--wait"])
            manifest = stage / "update.json"
            manifest.write_text(json.dumps({"pid":old.pid, "target":str(target), "staged":str(staged),
                "directory":str(stage), "cwd":str(root), "arguments":arguments,
                "sha256":"0" * 64 if mode == "checksum-failure" else hashlib.sha256(contents).hexdigest()}, ensure_ascii=False), encoding="utf-8")
            env = dict(os.environ, UPDATE_TEST_LOG=str(log))
            try:
                helper = subprocess.Popen([PWSH, "-NoProfile", "-NonInteractive", "-File", str(HELPER), "-Manifest", str(manifest), "-Silent"], env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
                deadline = time.monotonic() + 15
                while not (stage / "ready").exists() and helper.poll() is None and time.monotonic() < deadline:
                    time.sleep(0.05)
                self.assertTrue((stage / "ready").exists(), "helper must signal readiness before app quits")
                self.assertEqual(target.read_bytes(), self.binaries["old"], "running app must remain untouched")
                old.terminate()
                old.wait(timeout=5)
                out, err = helper.communicate(timeout=20)
                self.assertEqual(helper.returncode, 0, (out, err))
                if mode == "success":
                    self.assertNotIn(b"PhotoCraft update failed:", out, (out, err))
                elif mode == "checksum-failure":
                    self.assertIn(b"Staged executable checksum changed.", out, (out, err))
                else:
                    self.assertIn(b"PhotoCraft update failed:", out, (out, err))
                    self.assertNotIn(b"Staged executable checksum changed.", out, (out, err))
                    self.assertIn(b'"Start"', out, (out, err))
                deadline = time.monotonic() + 5
                while not log.exists() and time.monotonic() < deadline:
                    time.sleep(0.05)
                expected = "new" if mode == "success" else "old"
                self.assertEqual(log.read_text(encoding="utf-8"), expected + "\n" + "\n".join(arguments), (out, err))
                self.assertEqual(target.read_bytes(), self.binaries[expected])
                self.assertEqual(settings.read_text(), '{"automaticUpdateCheck":false}')
                self.assertFalse(stage.exists(), "ZIP, helper, manifest and backup must be removed")
            finally:
                if old.poll() is None:
                    old.terminate()
                    old.wait(timeout=5)
                if 'helper' in locals() and helper.poll() is None:
                    helper.kill()
                    helper.communicate(timeout=5)

    def test_success_waits_for_exit_restarts_with_arguments_and_cleans_staging(self):
        self.scenario("success")

    def test_checksum_failure_restarts_original_and_cleans_staging(self):
        self.scenario("checksum-failure")

    def test_restart_failure_rolls_back_and_cleans_staging(self):
        self.scenario("restart-failure")


if __name__ == "__main__":
    unittest.main()
