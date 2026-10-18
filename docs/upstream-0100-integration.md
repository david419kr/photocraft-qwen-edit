# Upstream 0.10.0 candidate integration

Status: candidate validation in progress. Main merge, tags and public publication are not authorized.

## Source identity

- Fork base: `d211a1280e3fd396a8de55a83ffea13ef09b2c8d` (main).
- Upstream release: `v0.10.0`, ID `416221086`, published `2026-10-18T04:01:42Z`, not draft or prerelease.
- Resolved lightweight tag: `8407d9ccb125af52944fca9d228383459f94aaef`.
- Merge base / previously integrated release: `c699e3cda69f4852b43c0b191ab0091aaeff9d6a` (v0.9.0).
- Candidate: `update/0.10.0`; exact head and run identities are recorded in the draft PR and subsequent validation record.
- At start: no open fork PR, no existing 0.10.0 branch and no running/queued Actions. Existing completed release is `261015-0.9.0`, ID `413311718`.

## Semantic resolutions

The merge brings the release tag, not upstream main. The full Library handoff was materialized and read in this cloud environment.

- Retain the fork CLI identity using upstream's broken-pipe-safe stdout writer. Keep ComfyUI session setup together with Linux print-portal setup and the new presentation cap.
- Move all three fork update shell commands into the new engine-owned shell command table. Keep their UI implementations.
- Keep both AVIF and Cineon open extensions and AVIF read/write capabilities. Retain XMP prompt history and Metadata None handling while integrating Lab TIFF and decode-once paths.
- Combine preference schema exceptions for alternate shortcuts and ComfyUI model settings. Retain fork tool-selection regression tests alongside expanded upstream temporary-tool tests.
- Preserve all fork translation entries and incorporate upstream additions; remove duplicate catalog keys without dropping AI/update messages.
- Preserve latest stable whole-workspace/all-targets HEIF Clippy before native suites, including the new upstream variant jobs. Keep Rust 1.95 portable and focused compatibility gates, without lint exemptions.
- README.md and README.ko.md remain byte-identical to fork main. Tool inventory stays in docs/native-tools.md; removed scheduled workflows remain unscheduled.
- Qwen routing/model sampling, HyMT workers/full-free protocol and installer pins are unchanged. No ComfyUI/model upgrades.

## Validation

Pending exact-head CI, focused nonzero regressions and Windows artifact verification. This document must not be read as a green result until final evidence is attached.

Actual NVIDIA CUDA inference, VRAM residency/free timing, interactive Korean UI, document cancellation/Undo and update-install acceptance remain local GPU checks; cloud tests do not replace them. Existing installations, models and settings are untouched.
