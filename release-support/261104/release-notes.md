PhotoCraft Local Qwen AI **261104-0.20.1**, based on upstream PhotoCraft **0.20.1**.

### PSD import limit

Native 64-bit builds now allow up to **20 GiB (21,474,836,480 bytes)** of estimated decoded PSD pixel memory instead of 2 GiB. The reported PSD estimate of 2,559,631,404 bytes fits this limit. This is an import safety budget, not a promise that every 20 GiB document will fit the available RAM or VRAM.

Wasm32 remains at 1 GiB and other 32-bit targets remain at 2 GiB. Overflow checks, lower custom limits, per-plane decoder limits and early rejection remain intact. No model or dependency upgrades are included.

### Build and validation

This release promotes the original, owner-tested Windows ZIP built on **2026-11-04 UTC** without rebuilding or changing its bytes.

- Candidate: `730192e89a5d4ca40553a7c4892437037349abe2`
- Release tag/main merge: `973eb64ebfb3073851fb9602201645e088c6470f` ([PR #21](https://github.com/david419kr/photocraft-qwen-edit/pull/21)); candidate and merge have the identical source tree.
- Upstream v0.20.1: `327c4cc82e737a3e0af7c147a477126d77525495`
- Windows x64 MSVC, Rust 1.95, static CRT, AVIF portable ZIP.
- [Candidate verification](https://github.com/david419kr/photocraft-qwen-edit/actions/runs/41842423799): 34 focused groups, 819 passed executions, 4 manual ignored; installer and script tests passed.
- [Post-merge CI](https://github.com/david419kr/photocraft-qwen-edit/actions/runs/41861188849): Ubuntu 14,097, Windows unit 11,232, Windows integration 2,768, corpus 5,893 and without-HEIF 2,017 tests passed. The first Windows integration attempt hit a GPU wait timeout in an existing brush-rendering test; the unchanged rerun passed all integration tests. Skipped/ignored cases and warnings are recorded in provenance.
- SHA256: `9b4cb555e596f40c55c42690d2c8cb3688a5ca298bdba7a1f158c0adb0837a8c`

The owner reported that this candidate works locally. The release process does not claim comprehensive GPU/VRAM, model-inference or interactive UI validation and did not access the owner's PC. Sonar's actual scan did not run because its token was unavailable; workflow success is not a completed analysis. FreeBSD remains manual-only and was not run.

Existing release tags and assets are preserved. See the attached `release-provenance.json` for exact build and validation evidence.
