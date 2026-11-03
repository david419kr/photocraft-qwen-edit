# CMYK adjustment Fill regression investigation

2026-11-03, saved `photocraft-qwen-edit` cloud environment only.

## Failure and provenance

Main CI [41584005048](https://github.com/david419kr/photocraft-qwen-edit/actions/runs/41584005048)
failed `cmyk::tests::the_split_route_agrees_with_the_exact_path` at main
`6ee15434818f1229569b8f650e7316e27a8d827c` (14,090 passed, one failed).
Persisted seed: `d1757ee3cfe1063f960af4de5515d32032837a4cee17d5f0c8d4c499cbc5329d`.
The split route returned K=0; the exact route returned K=59/255.

Replaying that seed failed identically on both unmodified fork main and exact
upstream v0.20.1 `327c4cc82e737a3e0af7c147a477126d77525495` in a detached local
worktree. Both runs failed before generating a new case (`successes: 0`). This is
an upstream defect, not a production compose change introduced by the fork merge.
Before this fix, the fork's only difference under `crates/compose` from that tag
was an additional clipped-text regression test.

Reproduction command, with the committed seed file at its current module path:

```sh
cargo +stable test --locked -p photocraft-compose --lib \
  cmyk::tests::the_split_route_agrees_with_the_exact_path -- --exact --nocapture
```

## Cause and fix

The ordinary RGB adjustment path already applies the special eight blend modes'
Fill inside the blend and uses Opacity times mask for coverage. The exact CMYK
path and clipped RGB adjustment path instead multiplied Fill into coverage.
For the minimized Posterize(2)/LinearDodge case, source brightness becomes 1;
backdrop brightness approximately 0.565 plus Fill 0.467 saturates to 1 (K=0).
Fading a full-strength blend instead leaves the erroneous K=59/255.

Share the adjustment coverage/Fill decision, use the existing Fill-aware blend
on CMYK channel brightness and clipped RGB adjustments, and preserve alpha and
Dissolve's noise/coverage behavior. The reference compositor independently
expresses the same Fill rule in ink space. Existing assertions, tolerances,
property case counts and filters are unchanged.

Added deterministic checks cover the minimized nested groups at 8/16/32-bit,
with and without clipping; a 360-case matrix covers all eight special modes,
Normal/Dissolve, zero/partial/full Fill and mask, partial/full Opacity, clipping,
opaque/partial/empty coverage. Both new tests failed before the production fix.
The original CI seed is retained in the current module's proptest regression file.
The four historical seeds are migrated from `cmyk_tests.txt` to `cmyk/tests.txt`:
the old path stopped being discovered when the test module moved to `cmyk/tests.rs`.

During matrix development, exact half-level U8 rounding exposed a separate
one-level disagreement between the existing ink-space and brightness-space
interpolation arithmetic (for example ColorBurn, Fill=.4, Opacity=.25, mask=1,
backdrop inks [.2,.4,.6,.8]). This change does not claim to resolve that numerical
boundary issue. The matrix uses non-boundary fractional coverage; the original
property assertions and failing CI input remain intact.

## Validation and release boundary

Check the associated fix PR for final commit-bound test and CI results.
Cloud checks passed: compose 419 tests (two existing timing tests ignored),
45 CMYK tests after restoring all five seeds, compose all-target Clippy on
Rust 1.95.0 and stable 1.99.1, layering (31 crates), and wasm (24 crates plus
HEIF/AVIF codec variants). The CPU quick performance suite measured 16 rows;
its regression comparison was skipped because the baseline machine differs. The
withdrawn `261103-0.20.1` release is now a draft (ID 432312262), as explicitly
approved by the user after this defect was reproduced. Its tag, ZIP and checksum
are preserved. The user also approved merging and publishing the corrected build
after validation. The replacement identity is `261103-2-0.20.1`, following the
existing same-day revision convention; no existing tag or asset will be overwritten.
The candidate workflow retains the exact upstream release provenance and records
main `6ee15434818f1229569b8f650e7316e27a8d827c` as this fix's fork base.
No actual GPU, CUDA/VRAM,
ComfyUI or interactive UI validation was performed; no user PC was accessed.
