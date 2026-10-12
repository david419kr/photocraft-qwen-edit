# Upstream update procedure

This is the repository entry point for the next update. Read `AGENTS.md`, the
Qwen README and `docs/upstream-070-integration.md` before changing code. The
complete user handoff remains `dots-upstream-update-handoff.md` (Library ID
`libfile_204a13a2d2ac8191941b60fb3c09ceac`); this procedure does not replace its
model, routing, translation, metadata or package contracts. Materialize and
read it in the environment consuming it; never assume another environment's
path exists. If unavailable, report the access failure and do not declare the
semantic merge complete without the full supplied handoff text.

1. Compare official, published, non-prerelease upstream releases with
   `docs/upstream-update-state.json` and the actual fork history. Resolve the
   release tag to its exact commit SHA, including annotated tags. Upstream is
   read-only; do not merge upstream main or the old private repository.
2. Start from current fork main on `update/{version}`. Merge the release SHA
   and review semantic conflicts as well as textual conflicts. Do not use a
   blanket ours/theirs resolution. Do not mix ComfyUI or translation-node
   updates into the integration.
3. Run **current stable Clippy across the whole workspace and all targets
   early**, with HEIF and `-D warnings`. The CI `early Clippy` job gates long
   native tests; existing platform Clippy steps remain. The Windows portable
   compiler stays Rust 1.95.0 until separately approved. Check compatibility
   on both toolchains instead of suppressing a new lint or silently pinning
   stable to an older version.
4. Run focused fork regressions (reject zero executed tests), installer and
   package tests, full native/HEIF/font variants, corpus, WASM/web size,
   format, lint, layers and scorecard checks. Preserve every existing floor.
   Record pass/fail/ignored/not-run separately for the exact candidate SHA.
5. Build Windows x64 MSVC with static CRT through the fork packager. Check
   executable version, imports, exact 18-file/six-root-entry package, licenses,
   scripts, README and SHA256. Never package model/runtime installations or
   personal settings. Adapt the candidate workflow's guarded branch/version
   for the new update; its current 0.7.0 guard is intentionally narrow.
6. Push the candidate, provide a draft PR and downloadable ZIP, and leave the
   actual CUDA/VRAM/UI acceptance checklist explicit. Automatic update runs
   stop here. Future main merge and public release require fresh user approval.
7. After approval, preserve tested source provenance through merge/tag/build.
   Use the actual publication date in Asia/Tokyo for `YYMMDD-version`. Verify
   public downloads and publish their SHA256 and provenance. Never overwrite
   an existing release/tag/asset to conceal a failed attempt. Address a draft
   by release ID when its tag lookup returns 404.

## One-time 0.7.0 exception

Release `261013-0.7.0` uses source `df715bc41646f7d434a07be774297eaa71e3f444`.
Its functional suites and Windows package passed, but Rust 1.99 Clippy failed
on `chunks_exact_to_as_chunks`. The user explicitly requested immediate merge
and publication despite that known lint failure. This exception applies only
to that release. It does **not** authorize future CI bypasses or automatic
publication. The array-chunk fix and early-Clippy job are a separate follow-up;
the published EXE and tag remain unchanged.

Scheduling is managed outside this repository by the user's parent workflow.
Do not create an additional scheduler as part of a candidate update.
