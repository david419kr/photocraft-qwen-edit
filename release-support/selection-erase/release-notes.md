# PhotoCraft Local Qwen AI {{TAG}}

Windows x64 portable release based on upstream PhotoCraft 0.22.0, combined with PR #27 external upscalers.

- Integrates the exact upstream v0.22.0 release commit.
- Includes PR #27 external upscaler support. External executables and model files are separate optional installations, not bundled.
- Preserves existing fork Qwen generation/editing/Fill, non-destructive layers, Undo/cancellation, metadata behavior, Korean support and static CRT packaging.
- Retains existing selection erase and Quick Mask Selected Areas behavior.

## Validation and provenance

Publication requires the final PR CI, including Ubuntu and both Windows test legs, and candidate lint, focused regressions and Windows packaging to pass. All 39 focused groups must execute positive test counts. No failure waiver is assumed.

The original candidate ZIP is promoted without rebuilding or repacking. The release tag points to the merged main commit. Candidate and PR source are identical; merged tree and parent identity are checked before publication. Duplicate main CI is not a release prerequisite. Exact source SHA, artifacts and checksums are recorded in release-provenance.json.

The Windows executable's architecture, static CRT imports, version, updater identity and portable contents are checked; public downloaded bytes are verified after publication. Actual CUDA/VRAM, UI, model inference and external upscaler hardware validation were not executed by this cloud pipeline. SonarCloud scan/coverage, manual fuzz/performance and FreeBSD validation are not claimed.

Download the Windows x64 ZIP and its SHA-256 sidecar. Existing releases and assets are preserved.
