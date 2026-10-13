#!/usr/bin/env python3
"""Run the handoff's focused gates; preserve logs and reject empty test filters."""
import json
import re
import subprocess
from pathlib import Path


def main():
    root = Path(__file__).resolve().parents[1]
    output = root / "target" / "qwen-regressions"
    output.mkdir(parents=True, exist_ok=True)
    cases = [
        ("engine-ai", ["-p", "photocraft-engine", "--lib", "ai_"]),
        ("engine-comfyui", ["-p", "photocraft-engine", "--lib", "comfyui"]),
        ("engine-prefs", ["-p", "photocraft-engine", "--lib", "prefs"]),
        ("ui-ai", ["-p", "photocraft-ui-egui", "--lib", "ai_"]),
        ("ui-widgets", ["-p", "photocraft-ui-egui", "--lib", "widgets"]),
        ("ui-history", ["-p", "photocraft-ui-egui", "--lib", "history"]),
        ("ui-catalogs", ["-p", "photocraft-ui-egui", "--lib", "i18n::tests::"]),
        ("ui-preference-labels", ["-p", "photocraft-ui-egui", "--lib", "preference_labels_are_translated"]),
        ("ui-documented-tools", ["-p", "photocraft-ui-egui", "--lib", "documented_tool_list_matches_tool_all"]),
        ("avif", ["-p", "photocraft-codecs", "--features", "avif,avif-decode", "--test", "avif"]),
        ("png-metadata", ["-p", "photocraft-codecs", "--test", "png_clean_metadata"]),
        ("io-metadata", ["-p", "photocraft-io", "--test", "metadata_clean"]),
        ("io-xmp", ["-p", "photocraft-io", "--test", "xmp_export"]),
        ("io-flat", ["-p", "photocraft-io", "--test", "flat"]),
        ("panic-hunt", ["-p", "photocraft-engine", "--test", "panic_hunt", "--", "--ignored"]),
    ]
    source_sha = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    results = []
    for name, args in cases:
        command = ["cargo", "test", "--locked", *args]
        print("Running", name, flush=True)
        with (output / (name + ".log")).open("w", encoding="utf-8") as log:
            completed = subprocess.run(command, cwd=root, stdout=log, stderr=subprocess.STDOUT)
        text = (output / (name + ".log")).read_text(encoding="utf-8", errors="replace")
        counts = re.findall(r"test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored", text)
        passed = sum(int(p) for p, _, _ in counts)
        failed = sum(int(f) for _, f, _ in counts)
        ignored = sum(int(i) for _, _, i in counts)
        ok = completed.returncode == 0 and passed > 0 and failed == 0
        results.append(dict(name=name, command=command, exit_code=completed.returncode,
                            passed=passed, failed=failed, ignored=ignored, source_sha=source_sha, ok=ok))
        (output / "results.json").write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8")
        print(f"{name}: {passed} passed, {failed} failed, {ignored} ignored, exit {completed.returncode}", flush=True)
        if not ok:
            print(text[-12000:])
            raise SystemExit(f"{name} failed or selected no executed tests; remaining cases were not run")


if __name__ == "__main__":
    main()
