# PSD import 20 GiB candidate

Requested on 2026-11-04. Branch `fix/psd-import-20gib`, based on fork main
`06c7aa3073df2be65b7bb06d7293a1e4d30b1d34` (upstream 0.20.1,
`327c4cc82e737a3e0af7c147a477126d77525495`, release ID 432155025).
This is a branch-only candidate, not a published release.

Native 64-bit PSD pixel-memory preflight now permits 20 GiB (21,474,836,480
bytes). The reported estimate, 2,559,631,404 bytes, exceeds the previous 2 GiB
ceiling and fits the new ceiling. This is a decoded pixel/workspace estimate,
not a file-size cap or a guarantee that an allocator can satisfy the import.
The actual user's PSD has not been supplied or opened in this cloud task.

Wasm32 retains 1 GiB; other 32-bit targets retain 2 GiB. Checked arithmetic,
custom lower limits, per-plane decoding limits, parser limits and rejection
before decoding remain intact. Layered TIFF's existing PSD-content preflight
uses the same allowance; unrelated format limits are unchanged.

Tests cover the exact limit, one byte above it, the reported estimate,
platform defaults, overflow and lower custom limits. Existing oversized-layer
rejection fixtures now use 30,000-square bounds so they remain above 20 GiB;
these fixtures contain tiny data and must be rejected before decoding.
No test allocates a 20 GiB pixel buffer.

The existing Qwen candidate workflow builds Windows x64/MSVC static CRT with
AVIF and portable-package/license/embedded-version verification. Its branch
allowlist and base SHA include this branch; the regression runner includes PSD
budget tests and rejects zero executed tests. Build identity is
`261104-0.20.1`; no tag, main merge or release publication is authorized.
