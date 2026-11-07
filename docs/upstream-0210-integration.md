# Upstream 0.21.0 integration candidate

## Source and boundaries

Merge the exact stable upstream release `436962270`, tag `v0.21.0`, commit
`efb7ebd0c0dab1fc108ee4642aa47c22694271a8`, into fork main
`f69112e5ae550834899044c8d72eac2083c45947` on `update/0.21.0`.
The upstream release is read-only. Candidate date/version is `261107-0.21.0`
(Asia/Tokyo). Main merge, tags, public release and user-PC access are not part
of candidate preparation. No preceding-version test results certify this source.

## Integration decisions

- Merge by subsystem and translation key, retaining the fork's Qwen features
  alongside upstream's new Place, frame, guide, renderer and background-job APIs.
- Retain the central 20 GiB PSD import ceiling in adaptive RAM/scratch admission,
  streamed Source import and standalone conversion. Unlimited scratch and user
  approval cannot bypass the cap. Preserve upstream's new memory diagnostics.
- Retain shared CMYK adjustment coverage for both clipped/unclipped layers and
  the independent ink-space oracle; integrate upstream split-render regressions
  and exact zero-Fill handling rather than reverting the fork's special Fill fix.
- Preserve Metadata None cleaning (including PNG alpha-LSB), XMP prompt history,
  AVIF read/write and metadata-clean cloning in the new renderer export paths.
- Keep Qwen cancellation and harmonization alongside guide cancellation and
  dedicated Selective Color handling. Fallible frame selection is checked before
  committing AI or view fields, with a combined transaction regression.
- Combine complete language catalogs by context/source key. Preserve Korean/AI
  wording, complete newly introduced labels and Bosnian fork coverage, and retain
  all completeness and quality gates. Regenerate parity and scorecard from the
  combined source; never lower a floor.
- Qwen generation/Edit/Fill/removal, annotation handling, reference inputs,
  nondestructive layers/Undo, cross-document jobs, model/TE/CFG/Turbo sampling,
  translation isolation/full-free markers and installer pins remain contracts.
- Preserve both fork and upstream attribution, existing license notices, static
  CRT packaging and the 18-file/six-root portable allowlist. No unrelated upgrade.
- Candidate workflow guards and provenance include this exact version/base.
  Ubuntu remains enabled; FreeBSD stays manual-only and is not dispatched.

## Validation and artifact policy

Run Rust 1.95.0 and current stable full-workspace/all-target locked HEIF Clippy
before costly hosted gates. Check metadata, translations and generated reports
locally, then run final focused fork regressions and upstream CI on the exact
candidate source. The focused harness rejects zero executed tests. Record actual
pass/fail/skipped/ignored/unrun outcomes and exact workflow/source identities.

Windows x64 MSVC uses Rust 1.95.0, static CRT and AVIF. The existing packager checks
PE imports, embedded `--version`, source provenance, ZIP allowlist/licenses and
SHA256. A candidate Actions artifact is not a public release. CPU/mock tests and
hosted builds do not establish actual CUDA/VRAM/interactive UI acceptance.

## Early validation findings

- Full locked workspace/all-target HEIF Clippy passes on Rust 1.95.0 and current
  stable 1.99.1 (stable channel checked on 2026-11-07 Asia/Tokyo).
- Adapted the Qwen resize call to upstream's fallible hidden-surface budget guard;
  propagate errors before generation or document mutation. Added synchronous and
  background atomic-failure regression coverage with sparse off-canvas pixels.
- Rust 1.95 flagged upstream's finite pen-cursor range comparison; use equivalent
  inclusive-range membership, retaining its preceding finite-value check.
- Merged lockfile retains both fork and upstream dependency graphs; Cargo only
  disambiguated the new cipher/md-5 dependencies against fork digest versions.
- All 118 fork-added files remain present; installer and model/HyMT source pins
  are unchanged. Metadata/workflow, the resize adaptation/test and the strengthened
  regression harness are the only changed files within that added-file set.
- Installer Python suite: 14 passed. Packaging/provenance/updater Python suite:
  11 passed, three Windows-only tests skipped locally and retained in CI.

These working-tree diagnostics are not a final hosted CI or Windows build claim.

- Rebuilt catalog suite passed 77 tests. The dynamic preference-label gate then
  exposed three tablet backend labels absent from catalogs; translated them in
  all catalogs, and the unchanged preference-label test passed.
- Regenerated menu parity is 664/664; the floor is 664. Scorecard regeneration
  retains all source checks. Local CMYK regression suite passed 49 tests.
- Final local memory policy tests: 28 passed after adapting upstream's unlimited
  overflow expectation to the fork's existing 20 GiB ceiling. Diagnostic assertions
  remain; six cap+1 checks cover unknown/ample RAM and caller-limit combinations.
- Local PSD-budget tests: 27 passed, three failed; engine AI filter: 95 passed,
  one failed, two ignored. The failures report available memory zero. This cloud
  sandbox exposes cgroup membership `0::/` but no cgroup mount; unchanged upstream
  memory detection deliberately fails closed for unreadable limits. Do not bypass
  that behavior. Hosted CI must verify the affected small-file/save-reopen cases.
- Final post-translation catalog suite: 77 passed; layers: 31 crates, no violations.
- A complete local UI-library run reported 5,392 passed, 44 failed and 22 ignored.
  Four failures exposed generated filter labels missing from translation catalogs;
  the labels are completed across all complete languages without altering tests.
  One upstream Home-chrome test probed Move's Auto-Select control while the fork
  intentionally starts with Brush; the test now explicitly selects Move and keeps
  every assertion. The other failures require hosted font/memory conditions:
  19 missing CJK/Arabic glyphs, 18 explicit zero-memory PSD admission errors and
  two background PSD loads retaining their placeholder document after refusal.
  Targeted reruns and final hosted CI, not this incomplete local run, determine
  candidate acceptance.
