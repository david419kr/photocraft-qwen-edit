# Upstream 0.12.0 candidate integration

Status: candidate validation in [draft PR #9](https://github.com/david419kr/photocraft-qwen-edit/pull/9), not approved for main merge, tag or public release.
The draft PR and exact-head Actions records will record final validation and artifact hashes.

## Identity and admission

- Upstream official release `v0.12.0`, release ID `419907723`, published
  `2026-10-21T18:28:57Z`, draft=false and prerelease=false.
- Resolved lightweight tag SHA: `6a2d027ee2e7ca12afd248544ed2262b0fa078dd`.
- Fork main base: `312561fff855408f9ae28a6d9d8b51ef13a5550b`.
- Branch: `update/0.12.0`; exact release integrated with a two-parent merge.
- No existing 0.12.0 branch, PR, run or artifact at admission. Release SHA was
  not a main ancestor. No tag mutation or republishing was observed.
- PR #8 is merged; public `261022-0.11.1` release ID is `420260128`.
  Its publication helper is not this candidate. The two active old CI runs
  (39711635302 and 39711622821) were left untouched. PR #7 was not rerun.
- Clippy cleanup PR #2 is closed; its array-chunk fixes and early lint gate are
  already present in current main and are not reapplied.
- Only the saved photocraft-qwen-edit cloud environment is used. Original
  Library handoff was materialized and read here; no user PC was accessed.
  No local .agents/skills or sibling private craftrules checkout is available.

## Integration review

- Translation conflicts combine fork AI/updater labels and upstream History
  Brush labels, keyed by context/source. Keep Korean history-label rendering
  alongside the new source-state selector in the History panel.
- Cargo.lock retains fork reqwest and disambiguates upstream roxmltree versions.
  Fork README distribution identity and trademark exclusions are retained.
- Automatic merge review found upstream Intel startup re-enabling Windows GL.
  Keep upstream Intel Vulkan avoidance but use DX12 only on that automatic path;
  retain PRIMARY for other automatic Windows adapters and respect explicit
  backend choices. Extend the existing regression and focused GPU-startup gate.
- AI routing, green-annotation removal, layers, cancellation, document-scoped jobs,
  model/TE/CFG/Turbo sigma, translation isolation/residency/two-marker full-free,
  background-removal prompt and installer pins remain unchanged from fork main.
- Review shared command/history, UI, export and save paths; preserve AI XMP and
  Metadata None removal, AVIF, remembered tools, Korean UI and static CRT.
- No ComfyUI/model upgrade, extra product feature, lint exception, removed test
  or reduced floor. Regenerate scorecard/parity from the combined source.

## Candidate gates and package

Run Rust 1.95.0 and latest stable full-workspace/all-targets HEIF Clippy early.
The first stable invocation overlapped toolchain installation and did not run;
retry only after installation completed (stable 1.99.1). This is not a code failure.
Run focused fork regressions with nonzero execution required, installer/package
checks, and all original applicable CI including Ubuntu, Windows, native variants,
corpus, wasm/web, ARM64, docs, packaging lint and Nix. FreeBSD stays manual-only,
excluded from acceptance; manual fuzz is not claimed as executed.

Windows x64 MSVC builds with Rust 1.95.0 and static CRT. Candidate identity is
`261022-0.12.0`; writing release-tag.txt does not create a Git tag or release.
The artifact records exact source/base/upstream SHA, release ID/tag, toolchain,
PE imports, version, 18-file/six-root package contract, licenses and SHA256.
No artifact or test success is asserted until the exact source run completes.

## Remaining hardware acceptance

Cloud mock/CPU checks do not establish actual CUDA/VRAM/UI behavior. Verify
Generate/Edit/Fill/Remove/background removal/reroll, Undo/cancel/document routing,
model/TE/CFG, translation reuse and two-marker full-free, Korean UI, remembered
tools, AVIF and prompt history save/reopen, Metadata All/None, and updater on the
exact candidate in a separate folder with existing ComfyUI/models. No user-PC
operation is performed by this cloud task.
