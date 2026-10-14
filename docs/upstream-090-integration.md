# Upstream 0.9.0 integration candidate

Upstream release ID 412944200, tag `v0.9.0`, exact commit
`c699e3cda69f4852b43c0b191ab0091aaeff9d6a`, published 2026-10-14T16:36:26Z.
Fork base: `8a97a1b43d963286cd4ce171319b46a706e39fbd`.
Merge base: `d43db3e153c76aefce9b32cef9099e458eaf17a8`.
The release contains 442 commits beyond the common ancestor. No existing update
branch, open fork PR or running Action existed at detection.

## Integration decisions under validation

- Preserve fork README presentation byte-for-byte; native tool inventory remains
  in `docs/native-tools.md` with the exact enumeration gate.
- Merge new codec registrations, indexed-PNG palette support and export preview
  refactoring with AVIF, XMP prompt history and Metadata None scrubbing.
- Keep the upstream safe slice bounds check together with None metadata filtering.
- Integrate upstream preference layout grouping with the ComfyUI endpoint/port UI.
- Preserve Korean history labels with upstream history seek semantics.
- Preserve explicit Eraser selection and remember the original tool during
  spring-loaded switching; explicit tool/menu/preset choices clear temporary state.
- Retain integer hover-wheel speed and horizontal scroll while adding upstream
  wrapped drag behavior. Retain auto-updater UI alongside rasterize prompts.
- Merge translation catalogs by context/source and retain fork-specific entries.
- Preserve model, ComfyUI/HyMT installer pins, no-cron policy, static CRT and licenses.

## Validation status

Integration is not yet accepted. Follow the draft PR for the exact candidate SHA,
full stable Clippy, Rust 1.95 checks, focused nonzero regressions, complete upstream
CI and actual Windows package verification. A local offline early Clippy attempt
could not resolve the newly pinned craft-print Git revision; this is not a pass.
The connected GitHub app and read-only fork Actions transferred exact source Git
objects because the shell GitHub credential is invalid; no permission settings changed.

Main merge, tags and public release are not authorized. Real CUDA inference,
VRAM residency and interactive hardware/UI acceptance are not run in the cloud.
Use the local checklist in `upstream-070-integration.md` on a separate candidate
folder after a verified artifact exists, preserving current settings/models.
