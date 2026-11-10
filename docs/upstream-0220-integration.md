# Upstream 0.22.0 integration candidate

## Provenance and scope

- Upstream: `storytold/photocraft`, stable release ID `439097058`, tag `v0.22.0`, exact commit `5000240c6e5dc30a024d20f97a620c78eca10d1e`.
- Fork base: `e5902663906c7099b46586664c1632a5c6d2ea54`, verified current main after PR #26 and release `261110-0.21.0` (ID `439380230`). No open PR or existing 0.22.0 branch was found before preparation.
- Branch: `update/0.22.0`. The release history diverges from v0.21.0; merge base is `2ff8f419cbef9c231a5647eebda43040f6dfa3fb`. Integrate with a two-parent merge, never a presumed fast-forward.
- Candidate version: `261110-0.22.0`, using the existing Asia/Tokyo build-date contract. Confirm the date again before each executable build.
- Main merge, tags and public release are not authorized. Upstream is read-only. No user-computer access, ComfyUI/model upgrade or FreeBSD dispatch.

## Semantic resolutions

Combine upstream tool, filter, text, RAW Develop and rendering changes with the fork contracts. The full supplied update handoff and current `docs/comfyui.md` were read; current code/tests and the latest documented policies supersede historical green-annotation and residency descriptions.

- Keep the dated fork release transport and portable installation/rollback path. Do not enable upstream's separate installer alongside it. The existing manual-check failure test already covers upstream's equivalent fix.
- Filter previews receive both the fork AI harmonization source and upstream foreground/background tool colors. Preserve async cancellation, frozen source state and preview identity checks. Keep both suites of preview tests.
- Keep selected-tool and native CMYK color persistence alongside upstream toolbar-group memory. Retain Brush for fresh profiles.
- Preserve AVIF quality while incorporating upstream JPEG chroma subsampling options. Keep metadata-clean export, XMP prompt history and non-destructive source state.
- Merge translation catalogs by context/source key, preserving fork-modified wording and upstream additions. Supply Georgian coverage for fork features. Preserve attribution for both translation and vertical-text glyphs. Excluded trademark assets remain absent.
- Qwen engine/backend/model/TE/CFG/Turbo sigma, local selection erase, installer and packaging implementations remain fork contracts. Quick Mask and local erase are explicitly included in the nonzero-test regression harness.
- Rust 1.95's `nonminimal_bool` lint exposed an upstream NEF validity expression; use its equivalent `is_none_or` form without suppressing the lint or changing error handling.
- Preserve all PR CI, Ubuntu, Windows, corpus, web, HEIF/font variants and regression gates. Only adapt the candidate workflow's branch/provenance guards. FreeBSD remains manual-only.

## Validation record

Initial cloud diagnostics, not final source acceptance:

- CI-reuse policy: 19 passed.
- Installer suite: 14 passed.
- Package/updater Python suite: 14 discovered, 11 passed, 3 Windows-only tests skipped; Windows workflow retains these checks.
- Initial concurrent compiler attempts were terminated by SIGKILL during large engine/UI crates under the cloud memory limit; these are incomplete checks, not passes. Retry with bounded concurrency and retain hosted gates.
- Rust toolchains installed: 1.95.0 and current stable 1.99.1. Full locked workspace/all-target HEIF Clippy is required on both.
- Dependency reconciliation restored the fork's `zopfli` dependency; no model or ComfyUI pins changed.

The draft PR and Actions records identify the final candidate SHA, measured gates and artifact links. No earlier-version result certifies this candidate. Windows x64 MSVC portable packaging uses Rust 1.95.0 and static CRT; its existing verifier checks PE imports, embedded version, exact package allowlist/licenses and SHA256. Artifact availability alone is not acceptance.

Actual CUDA inference, VRAM residency, interactive UI and local GPU acceptance are not performed in the cloud. Follow the checklist in `docs/upstream-070-integration.md` on the exact downloaded candidate, including Quick Mask, native-color local-selection erase and Undo/Redo. No claim is made that historical GPU verification covers this source.

## Recovery validation, 2026-11-10

- The exact two-parent merge tree is `ec235abd1f3dbfbd259cecadcfdeed3857582dbb`; remote merge commit `61970b10724410554bc912cf24430b0eca63c50b` preserves the base and upstream parents above. The connected GitHub transfer recovered and its complete tree was matched to the locally reviewed merge; partial object transfers did not create branches or PRs.
- Full locked workspace/all-target HEIF Clippy passed on both Rust 1.95.0 and stable 1.99.1 after narrow canvas complexity and dash-test range fixes. No lint suppression or floor reduction was used.
- Removed an exact duplicated block of nine tool-color persistence tests introduced during merge resolution; each unique test remains. Added primary/secondary/fullscreen AI-bar coverage for the extracted helper.
- Completed remaining Korean, Japanese, Simplified Chinese and Traditional Chinese catalog gaps (2513/2513 keys in each), and refreshed the generated preference scorecard (210 preferences).
- Formatting, layering, panel-command coverage, scorecard freshness, catalog structure/placeholders and the Python suites passed locally. Native runtime regressions are being resumed from cache; interrupted compilation is not a test pass.
- Initial local WASM attempt could not execute because the Rust target standard library was missing. Hosted WASM remains required. Final PR and candidate Actions must certify the exact remote candidate SHA; no prior-release result is substituted.

### Full-gate findings and corrections

The first hosted candidate `10802f82f264ab9a64eb99293def7ed644ed2ee2` produced a verified Windows ZIP, but was not accepted: the Ubuntu gate executed 17,265 tests (17,240 passed, 25 failed, 112 skipped). The focused fork suite also stopped on a selected-tool fixture. These failure records and the original artifact remain attached to PR #28.

- Fresh profiles intentionally start on Brush. Upstream group-history, shortcut and flyout fixtures assumed Move. Fixtures now explicitly establish the history under test or assert the fork's Brush fallback, preserving the original behavior checks and adding setup assertions. No production tool behavior changed.
- The upstream CLI metadata-none test expected retained ICC/DPI, alpha identifier bits and layered XMP. Assertions and CLI help now follow the existing fork privacy contract instead; all/default metadata checks remain. Metadata removal behavior is unchanged.
- Restored the missing Star entry in the checked 69-tool inventory.
- Completed missing source and dynamically generated labels across all 33 complete catalogs, preserved placeholders/plural forms, removed retired display-name rows and reconciled contextual label collisions. No catalog completeness flag or test gate was lowered.
- The UI translation source scanner compared literal backslash-n with a catalog runtime newline. It now decodes supported source-string escapes before lookup, with a regression proving newline/tab/quote/backslash handling and a negative raw-key assertion; the source-key set and coverage threshold are unchanged.

Final hosted validation and replacement Windows artifact must use the corrective candidate SHA recorded in PR #28. The first candidate's ZIP/hash is retained only as historical evidence, not as final acceptance.

## Combined PR 27 candidate

At the user’s request, integrate PR #27 (`feat/external-upscalers`) at exact head
`a8812d986bbbd02d4068b791f456252d5a62aeec` into this candidate, not into main.
Preserve the completed upstream 0.22.0 integration and its fixes, including native
metadata privacy. Catalog conflicts are resolved by context/source key, preserving
existing 0.22 wording and adding the offline backend labels. Windows binaries must
be rebuilt and full PR CI and fork regressions rerun for the combined candidate.
The external runtime and model weights remain separately installed and are not
redistributed in the portable ZIP. Actual CUDA/VRAM/model/UI validation is not
claimed by cloud checks. No main merge, public tag or release is authorized here.
