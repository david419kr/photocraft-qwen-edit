# PhotoCraft Qwen fork: upstream 0.7.0 candidate

This candidate merges upstream's published `v0.7.0` commit
`79261615956ab21334e49ca1d384390272e647da` into fork main
`5a9c9a2b21e25e976cf3f19a9ad13a264bb4c9de`, using a merge commit on
`update/0.7.0`. It does not merge upstream main, publish a release or tag,
or change the ComfyUI/HyMT installation pins.

## Integration decisions

- Combine the 16 conflicting translation catalogs by key, retaining the fork's
  AI and Korean labels and upstream's new settings, theme and tool translations.
- Keep the independent Qwen fork README, portable download instructions and
  AVIF read/write capabilities. Upstream's branded homepage and platform
  downloads do not describe this distribution. Identify 0.7.0 as an unreleased
  candidate in both README languages.
- Combine attribution for the new row/column marquee glyphs and the fork's
  translation glyph. Restricted ArtCraft logo/mark files remain absent;
  `docs/brand/LICENSE-brand.txt` remains for legal notices.
- Retain both upstream's new command registrations and the Qwen AI commands.
  Regenerate the parity/scorecard documents from the combined source; never
  resolve their conflict by lowering a test floor.
- Review the automatically merged UI state, command dispatch, canvas,
  preferences, shortcut and save paths. Upstream's Save action recording still
  reaches the same metadata-preserving save implementation; export metadata
  None retains its separate removal policy.
- The AI command/backend/model/history implementations, green-annotation
  removal, two-marker full-free protocol, installer source and Windows GPU
  startup policy are unchanged from the fork base. Shared registration and UI
  changes require the regression gates below despite a clean textual merge.

## Candidate build

`.github/workflows/qwen-candidate.yml` runs only on this fork's
`update/0.7.0` branch (push or manual dispatch). It has read-only repository
permissions and no release environment, signing secrets, tag or release step.
The existing PR CI remains the full upstream workspace/corpus gate.

The Linux job runs the handoff's focused regressions with
`scripts/check-qwen-regressions.py`, which fails on zero executed tests and
stores command results and logs. It also runs fmt, Clippy, layers, wasm,
parity/scorecard checks and installer/package-validator tests.

The Windows job runs independently so its build diagnostics and artifact are
available even if a Linux gate fails. Both job results must be checked for the
same source SHA; artifact availability alone is not acceptance.

The Windows job uses Rust 1.95.0, `x86_64-pc-windows-msvc`, and
`CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS=-C target-feature=+crt-static`.
The desktop manifest enables both AVIF decoding and encoding. Optional HEIF
and craft-fonts are not enabled by this fork's candidate recipe.

`scripts/build-qwen-candidate.ps1` inspects the x64 PE headers/imports, rejects
dynamic CRT imports, runs the executable's `--version`, and invokes the
existing `scripts/package-qwen-portable.ps1`. It verifies the exact 18-file ZIP
allowlist, executable provenance and SHA256 sidecar. The ZIP has exactly the
six top-level entries specified by the handoff: `licenses/`, `scripts/`,
`install-qwen-image21.cmd`, `photocraft.exe`, `portable.txt`, and `README.txt`.
No model, runtime installation or personal settings are allowed in the ZIP.

The artifact also includes the source/base/upstream SHA, toolchain, flags,
executable and ZIP digests, PE inspection, version output and build log.
Artifacts expire after 14 days; rebuild the same source SHA if they expire.
The date in the ZIP filename uses Asia/Tokyo and does not create a release tag.

## Validation evidence

Check the PR and the workflow run for the exact candidate SHA for measured
results and outstanding gates. Local focused results are written to
`target/qwen-regressions/results.json`, with one log per command. A successful
Linux test or a synthetic package test is not a Windows executable build or
live GPU test. The 634-item parity floor includes both new upstream commands
and the fork's AI entries; no test or corpus floor was lowered.

## Windows local acceptance (after a real candidate artifact exists)

1. Match the source SHA and run `Get-FileHash -Algorithm SHA256` on the ZIP.
   Extract into a separate writable candidate folder; preserve existing
   ComfyUI, models, settings and documents.
2. Run Generate, Edit, partial Fill, blank-prompt Remove, background removal
   and reroll sequentially. Check original layer visibility, dimensions,
   feathered boundaries, cancellation, Undo/Redo and cross-document job routing.
3. Check Korean UI, Ctrl+Enter, hover-wheel number changes, model/encoder
   selection, stock Turbo CFG lock and steps. Use the unchanged ComfyUI and
   translation-node versions; do not update them as part of this test.
4. Check translation toggle/model persistence, repeat-run residency and
   model-switch full-free/two-marker ordering on actual CUDA hardware.
5. Save/reopen PNG, AVIF and native documents with prompt history; verify
   Metadata All preserves it and None removes exported history without
   changing the open document. Check AVIF alpha and export.
6. Record the tested SHA and actual GPU in the PR. Existing README hardware
   claims do not establish that this candidate was tested on those GPUs.

Main merge and public release remain separate, explicitly approved steps.

### Validation fixes after the first CI run

- The fork base already had a Unix save-filter test comparing `Vec<String>` with `&&str`, which failed to compile. Compare string contents with `iter().any()`; retain every format including AVIF.
- The unchanged legacy PNG roundtrip test contradicted the fork's Metadata None implementation and dedicated privacy regression: it expected DPI and ICC to survive. Assert removal of all metadata for None, preservation of DPI/ICC/XMP for All, and an unchanged source document. No metadata implementation changed.
- The first browser build produced 25,194,637 bytes against the unchanged 25,165,824-byte gate. Request Binaryen `--converge` through Trunk's supported `data-wasm-opt-params` attribute; repeat size optimization without dropping functionality or raising the limit. CI measured 25,150,403 bytes with convergence, below the unchanged gate (15,421 bytes of headroom).
- A local whole-workspace test attempt exhausted the 32 GiB cloud disk during linking (linker bus errors); this is an incomplete run, not a passing suite. Focused regressions and GitHub's platform jobs provide separate evidence.

- The unchanged EXIF orientation test made the same legacy None assumption. Assert that None removes EXIF while retaining upright pixels; All still normalizes EXIF and XMP to Orientation 1, and the source document stays unchanged.

- Full UI unit tests additionally found two identical Korean theme entries introduced by the automatic merge. Deduplicate by context/source without changing either translation. Complete the fork's previously missing model-setting/negative-prompt labels across all 16 complete catalogs (retain existing Korean wording), and add upstream's tested 55-tool inventory to the fork README without replacing its distribution identity. No localization coverage gate was relaxed.
