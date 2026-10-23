# Upstream 0.14.0 candidate integration

Status: candidate validation only. Main merge, tags and public release are not authorized.
The draft PR records the final source SHA, exact-head checks and Windows artifact hash.

## Identity and supersession

- Upstream `storytold/photocraft` official release ID `421334654`, `v0.14.0`,
  published `2026-10-23T00:43:42Z`, draft=false, prerelease=false.
- Exact release commit: `ff4738810134d7f7e68b49f2e4f231b686282ff3`.
- Fork `david419kr/photocraft-qwen-edit` main base:
  `65d3270f61905b7a480ddeafceec4f1697499ad0`.
- `update/0.14.0` starts from that main and merges only the exact release SHA
  with a two-parent merge. The release is not already a main ancestor.
  API identity and tag match the supplied observation; no retag/republication observed.
- At admission no 0.14.0 branch, PR or Actions run existed. Public fork release
  remains `261022-2-0.12.0`. Upstream is read-only; the old private fork is unused.
- Supersedes draft PR #11 (`update/0.13.0`,
  `bfb3ede5a4ab894d7a098c11ada62f2e44db6f27`) under the user's latest-candidate
  policy. Its seven passing workflows and 542 focused tests remain historical
  evidence, not approval or evidence for 0.14.0. Preserve branch and artifacts;
  close the old draft with a reciprocal link once the new draft exists.
- Saved cloud environment only; no user PC. No local `.agents/skills` or private
  sibling craftrules checkout is available. Repository handoffs supply contracts.

## Integration review

Reapply reviewed 0.13 compatibility resolutions only where still required:
combine AVIF and indexed PNG XMP/Metadata None with upstream indexed BMP and
new formats; preserve independent fork distribution and licenses; combine AI
and upstream UI modules/control tests and numeric integer-step controls; use
upstream wheel handling once; preserve brush-owner and exact color exit flush.
Deduplicate catalogs by context/source while retaining Korean/fork translations.
Keep corrected PNG CrossFuse enum and nonfinite fillet-angle regression.
The application-code delta against PR #11 matches the upstream 0.13-to-0.14
release delta, with prior compatibility fixes retained, not a merge of that PR.

Review 0.14 automation/dialog/save changes together with AI/document jobs:
paused playback tokens and route markers remain protected, automation saves
cannot read linked files, native saves/autosaves and SVG limits retain upstream
fixes. Qwen Generate/Edit/Fill/green removal, layers/Undo/cancel/document
isolation, model/TE/CFG/Turbo sigma, HyMT isolation/residency/two-marker full-free,
XMP/Metadata None, AVIF/Korean/updater, static CRT and installer pins are retained.
PR #10 pixel/mask colors, exact CMYK, HDR/alpha, swaps/resets and malformed preset
validation remain covered. No ComfyUI/model upgrade, unrelated feature, removed
test, lower floor or blanket lint suppression.

## Gates and artifact

Run Rust 1.95.0 and latest stable (1.99.1) full-workspace/all-target HEIF Clippy
with warnings as errors early. Focused tests use consolidated `it` binaries
where upstream moved suites; every suite must execute nonzero tests. Run all
applicable upstream CI including Ubuntu, Windows native/HEIF/fonts, corpus,
wasm/web, ARM64, docs, packaging lint and Nix on final candidate source.
FreeBSD is manual-only/non-required since 0.10; do not rerun PR #7. Manual fuzz
is not claimed. No previous 0.7 lint exception applies.

Windows portable: Rust 1.95.0, x64 MSVC, static CRT, AVIF, embedded identity
`261023-0.14.0`. Existing verifier checks version, imports, 18 files/six root
entries, licenses and SHA256. Artifact provenance records exact release ID,
tag, upstream SHA, fork base and candidate SHA. Retention is 14 days.
The PR and Actions records report pass/fail/not-run and final download/hash.

## Actual hardware acceptance (not run)

Cloud CPU/mock checks do not establish CUDA/VRAM/UI acceptance. When separately
requested, use the exact ZIP in a separate folder with existing ComfyUI/models:
Generate/Edit/Fill/Remove/reroll; layer visibility/Undo/cancel/document routing;
model/TE/CFG/Turbo; translation reuse and full-free; Korean UI; pixel/mask and
CMYK color persistence; AVIF/history save/reopen; Metadata All/None and updater.
This task performs no user-local validation or model/runtime updates.
