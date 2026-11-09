# PhotoCraft Local Qwen AI {{TAG}}

Windows x64 portable release based on upstream PhotoCraft 0.21.0.

- Adds direct local Remove for partial selections, available independently of ComfyUI. The optional external erase backend must be installed separately by the user; model files are not bundled. The result preserves source alpha and is placed on a new layer, with Undo/Redo support.
- Preserves native CMYK and Lab color values outside the selected erase region.
- Fixes the missing External erase preference translations across 27 locale catalogs.
- Retains the Quick Mask fixes already merged into main: correct Selected Areas masking and remembered Quick Mask mode.

[External backend setup and requirements](https://github.com/david419kr/photocraft-qwen-edit/blob/85fe92d71fbe03cdf1c7990c7b29fb5332cb63bc/docs/external-cutout-backends.md)

## Validation and provenance

Published only after the final PR CI passes, including Ubuntu and Windows checks, and the separate Windows candidate build, lint, and focused regressions pass. All 36 focused regression groups must execute at least one passing test. Main's successful provenance-only CI verifies that the merged application tree is the tested PR tree and skips duplicate work.

The original candidate ZIP is promoted without rebuilding or repacking. The release tag points to the merged main commit. The build tree differs from the feature/merge tree only in the candidate workflow and release-tag.txt; application sources are identical. Exact commits, CI runs, original artifact identity, checksums and verification results are recorded in release-provenance.json.

The Windows executable's AMD64 architecture, static CRT imports, version, updater identity and exact portable package contents are verified, as are public downloaded bytes and checksums after publication. Actual GPU/UI interaction, external model inference and hardware acceptance were not tested by this release pipeline. Automated mocks are not hardware acceptance.

Analysis limits: the SonarCloud wrapper run 42644759977 succeeded, but job 129255021686 skipped the actual SonarCloud Scan and all coverage-generation steps after its token check. No Sonar scan or coverage result is claimed. Manual fuzz/performance checks and FreeBSD validation are not part of this release validation. No Nix workflow was triggered for this PR.

Download the Windows x64 ZIP and its SHA-256 sidecar. Existing releases and their assets are preserved.
