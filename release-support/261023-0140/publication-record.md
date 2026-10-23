# Approved 0.14.0 publication

User approval: 2026-10-23T15:59:34Z, “잘 되네 0.14 머지하고 릴리즈 해줘”. The user reports the candidate works; no detailed CUDA/VRAM/UI scenarios are inferred. This approval supersedes the earlier candidate-only restriction.

- PR: https://github.com/david419kr/photocraft-qwen-edit/pull/12
- Candidate/binary source: ba3c57ca29ca100c321146c5be039bb10c11c992
- Main merge/tag source: 9c0d6c3c8f39652c0356369a5cfe0956887dd59a
- Common exact tree: 09b323a35f7e7ef4ccc8ae066f81309c1d7c88cc
- Merge parents: 65d3270f61905b7a480ddeafceec4f1697499ad0 and ba3c57ca29ca100c321146c5be039bb10c11c992.
- Upstream release 421334654 / v0.14.0 / ff4738810134d7f7e68b49f2e4f231b686282ff3.
- Embedded/tag identity retained: 261023-0.14.0. Publication does not rebuild or rename a binary merely because the Japan calendar date changed.
- Original Windows artifact: 12476920666, run 39893380962.
- ZIP: photocraft-qwen-edit-261023-0.14.0-windows-x64.zip
- ZIP SHA256: 0527becd1ddaf618aa1aa358fc9635bec0257fea4f5b0943d50747308b1a0028
- EXE SHA256: 06eef5a4cb19e1582b9f3a15c2fc62574500345bc3571904f9a0e1ca0b60118d

The exact candidate's seven workflows passed; 26 mandatory nonzero focused suites passed 542 tests (0 failed/ignored). Main has the same complete tree, including workflows/commands/platform selection, so those source-specific results remain the publication evidence. Automatic ready-for-review/main CI runs are retained and not made an additional gate. No manual duplicate CI, cancellation or deletion is requested.

The helper retains the successful 261022-2 publication path: Windows verify the original artifact/version/static CRT/18-file package, enforce source-specific CI and tree equivalence, create a unique tag and draft, upload unchanged ZIP/sidecar/provenance, verify asset metadata, publish, download public bytes, verify all hashes/package and latest updater metadata, and preserve previous-release assets.

The public release-provenance.json, publication verification Actions artifact and PR #12 completion record contain final release ID/URL, publication timestamp and public-byte verification results. No upstream write, user-PC operation, ComfyUI/model upgrade or credential/repository-permission change.
