# Upstream 0.13.0 candidate integration

Status: preparation and validation only. Main merge, Git tags and public release
are not authorized. The draft PR and matching Actions runs record exact final
source SHA, check results and Windows artifact hashes.

## Provenance and admission

- Official upstream release ID `421188825`, tag `v0.13.0`, draft=false,
  prerelease=false; Git remote resolves the lightweight tag to
  `ce2a1e337a2fa68bf56c793a70c09477deccf63e`.
- Verified fork main base: `65d3270f61905b7a480ddeafceec4f1697499ad0`.
- Branch `update/0.13.0` merges that exact release with a two-parent commit.
  Upstream main and the old private fork are not integration sources.
- At admission no open PR, 0.13.0 branch or active Actions run existed.
  Latest published fork release was `261022-2-0.12.0`, ID `420636944`,
  built from the main merge of color-persistence PR #10. Older 0.12.0
  candidate-state documents are historical; that version was published.
- Clippy cleanup PR #2 and the early full Clippy gate are already incorporated.
  The 0.7.0 lint exception does not apply. FreeBSD remains manual only and
  non-required; PR #7 is not restarted. Ubuntu is retained.
- Work takes place only in the saved cloud environment. Repository handoffs
  supply the contracts per the task; no `.agents/skills` or private sibling
  craftrules checkout is available. No user PC is accessed.

## Integration decisions

- Retain the fork README/distribution identity, AVIF decoder/encoder and
  licenses while combining upstream DICOM/IFF/XCF and Cineon support.
- Combine upstream indexed BMP support with the fork's indexed PNG XMP,
  Metadata None and alpha-LSB cleaning paths. Metadata None still strips
  ICC/DPI/EXIF/XMP without mutating the source document.
- Combine translation additions by key, preserving Korean and fork AI/updater
  labels. Retain both AI and upstream panel modules and control tests.
- Combine upstream numeric field wheel/precision behavior with explicit
  integer sampling-step controls. Retain mock AI/update snapshots and Windows
  primary-backend protection alongside upstream texture-filtering options.
- Retain color-persistence PR #10: pixel/mask pairs, exact CMYK selections,
  finite HDR/alpha, exit flush, swaps/resets and malformed-preset rejection.
  Incorporate the upstream brush-owner persistence correction without dropping
  the color flush.
- Preserve Qwen generation/edit/Fill/removal, original layers and Undo,
  cancellation/document routing, model/TE/CFG/Turbo schedule, HyMT isolation,
  residency/two-marker full-free, updater and unchanged installation pins.
- Adapt focused test invocations to upstream's consolidated `it` binaries;
  their filters must execute nonzero tests. No tests or floors are removed.

## Validation and Windows package

Compare Rust 1.95.0 and latest stable (1.99.1 at preparation) full workspace,
all-target HEIF Clippy early. Run focused fork gates, native/HEIF/font/corpus,
wasm/web-size and upstream CI, docs and packaging checks at the final source.
Record failures and fixes rather than accepting a previous version's result.
Local Python checks initially passed: installer 14; scripts 8 passed and 3
platform-dependent skips. Final exact-source CI remains authoritative.

The candidate workflow builds Rust 1.95.0 Windows x64 MSVC with static CRT,
AVIF and identity `261023-0.13.0`. The existing packager checks PE imports,
version, exact 18 files/six root entries, licenses and SHA256. Build provenance
records upstream release ID/tag/SHA, fork base and candidate SHA. Artifacts
expire after 14 days. Writing `release-tag.txt` creates no Git tag or release.

## Not run: actual local acceptance

Cloud mock/CPU tests do not establish CUDA, VRAM or interactive UI behavior.
On the exact candidate, separately validate Generate/Edit/Fill/Remove/reroll,
layer preservation, Undo/cancel/document routing, model/TE/CFG, translation
residency and full-free, Korean UI, color persistence (including CMYK/masks),
AVIF/history save/reopen, Metadata All/None and updater behavior. Use a separate
folder and the existing ComfyUI/models. This cloud task does not run those tests.
