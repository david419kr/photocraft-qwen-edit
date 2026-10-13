# PhotoCraft Qwen fork: upstream 0.8.2 candidate

Release ID **410949848**, published 2026-10-12T21:22:47Z, resolves directly to
`d43db3e153c76aefce9b32cef9099e458eaf17a8` (`v0.8.2`, not upstream main).
Fork base: `b9d8ea92e0e668f010ca1935853a243f52a38ac9`.
Common ancestor: `79261615956ab21334e49ca1d384390272e647da` (v0.7.0).
The release adds 422 upstream commits. At start no update/0.8.2 branch or PR
existed and no fork workflow was in progress. Existing draft PRs #2 and #3
remain untouched. The candidate SHA and exact-run results are recorded in the
candidate PR and CI provenance; no main merge, tag or release is authorized.

## Integration decisions

- Preserve both READMEs byte-for-byte from fork main, including the user's
  removal of Native editing tools. Move upstream's now-67-tool inventory to
  `docs/native-tools.md`; the same count and exact `Tool::ALL` comparison runs
  under a renamed nonzero focused test filter.
- Reapply the still-needed array-chunk Clippy fixes from PR #2, plus the early
  workspace/all-targets/HEIF stable Clippy gate. Native tests depend on this
  gate; Windows portable and focused regressions retain Rust 1.95.0.
- Integrate upstream's cancellable/progress export entry point with the
  existing Metadata None scrub at their shared export implementation. Indexed
  PNG All retains both XMP prompt history and upstream's new DPI; None retains
  neither and still scrubs palette transparency's alpha LSB. Add a regression
  comparing foreground/background export and All/None policies.
- Preserve AVIF decoder/encoder and format registration alongside new PCX,
  FITS and Krita import paths. Combine command/module/dependency registrations.
- Keep Qwen job, per-document cache cleanup and upstream subject-cache cleanup.
  Validate new panel-docking updates before applying any AI UI mutations.
- Adapt integer sampling fields to upstream's FieldOpts unit-expression and
  scrub-label API, retaining the fork's hover-wheel behavior and its test.
- Combine translation catalogs by context/source, retaining fork-only AI
  labels and Korean text. Keep both upstream and fork license attributions.
- Raise the existing 634 parity floor to the measured 635; regenerate derived documents from
  the combined source without lowering any test, corpus or web-size gate.
- ComfyUI/HyMT installer pins, models/TE/CFG/sigma, green-annotation removal,
  two-marker full-free ordering, Windows static CRT, restricted-brand asset
  exclusions and the canonical portable packager remain preserved.

Preliminary local checks: Rust 1.99 full workspace/all-targets/HEIF Clippy passed;
installer 14/14, package validator 5/5, and dependency layering passed.

## Validation and portable provenance

Use the PR's exact head SHA and matching runs. The candidate workflow builds
Windows x64/MSVC static CRT with Rust 1.95.0, invokes
`scripts/package-qwen-portable.ps1`, and verifies PE architecture/imports,
version, 18-file/six-root allowlist, executable provenance and ZIP SHA256.
Artifact availability alone is not acceptance; all workflow results must pass.

Local CUDA inference, residency/VRAM and interactive UI are **not run** in the
cloud. Follow the Windows local acceptance checklist in
`upstream-070-integration.md` against the delivered candidate SHA, in a separate
folder with existing 0.7.0 settings/models/ComfyUI preserved. Check Generate,
Edit, Fill, Remove, reroll, cancellation, Undo/Redo, tab routing, Korean UI,
translation/model switching and All/None metadata plus AVIF roundtrips.

## CI repairs

- Initial packaging run 38339889173 failed ShellCheck SC2015 in upstream
  `packaging/linux/AppRun`. Replace the ambiguous and/or condition with an
  explicit equivalent `if`; retain external-integrator detection and its tests.
- The preference-label gate found upstream Hungarian/Turkish missing
  `Enable animations`; add real translations without weakening completeness.
- Korean Qwen model settings and prompt bar were rendered and visually checked
  offscreen with the mock backend and CPU canvas; this is not CUDA validation.
