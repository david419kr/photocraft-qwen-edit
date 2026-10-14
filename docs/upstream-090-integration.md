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

## Resumed validation

The existing shell GH_TOKEN subsequently passed gh auth status, repository
permission lookup, Git remote read and an ordinary candidate push. No account,
credential helper or permission settings were changed. The earlier Actions token
workflow restriction remains a separate limitation. Draft PR #6 now tracks the
candidate. Rust 1.95 dependency resolution repaired the merged Cargo.lock and
the generated scorecard reports 26 unread settings out of 167.

The first full CI at b587574e caught a String/&str mismatch in the merged AVIF/ICO
preview proxy selector. Convert the owned format to a string slice for matching.
The numeric field keeps speed and wrapped-drag settings together without adding
a lint exemption. Native, wasm and web checks must rerun on the repaired SHA.

At candidate `3009a8f390aa7d4c1c15dd4c5c7a4e6233548fab`, CI run
38613834074 passed stable early Clippy, wasm32 checking and corpus tests. Both
native test jobs reached the UI suite and failed the same three translation
coverage tests: the new Serbian catalog lacked fork AI/update controls. Add
actual Serbian translations without relaxing coverage. Candidate run 38613825458
built Windows artifact 11920147545, but Rust 1.95 Clippy stopped regressions on
CR3's nested match condition; express the same preview limit as a match guard.
The web build produced 25,205,976 bytes, exceeding the unchanged 25,165,824-byte
gate. Extend size optimization to serde/serde_core's serialization code, retaining
pixel processing optimization and all formats. Gate acceptance requires a new
measurement. Restrict the native-only display chunk constant to native builds
instead of suppressing the wasm warning. The previous artifact is not final.
