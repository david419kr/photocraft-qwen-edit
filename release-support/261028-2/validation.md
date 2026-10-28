# PhotoCraft 261028-2-0.16.0 release validation

- Functional source approved by the owner: `c8cb9bc4ad38210d83bea1b45f35aec7107dfbe5`.
- Final main, Windows build and tag source: `c65be04e50df6dae32195f7b0a962aac4db42b65`.
- The only source change is `release-tag.txt`: `261028-1-0.16.0` to `261028-2-0.16.0`.
  The proposed revision-1 parser/packaging changes were reverted and never committed.
  Updater rules, package rules, existing checks and user READMEs are unchanged.
- This release does not update upstream. Integrated upstream remains stable v0.16.0,
  release ID 423446623, SHA `b1794c6e0608ccfc887213dcf58398977c5cb75e`.

## Successful checks

[Main CI 40715203865](https://github.com/david419kr/photocraft-qwen-edit/actions/runs/40715203865)
and [SonarCloud 40715203854](https://github.com/david419kr/photocraft-qwen-edit/actions/runs/40715203854)
succeeded on the final SHA. Ubuntu: 11,763 passed / 94 skipped, plus panic hunt;
all nine gate stages passed. Windows unit: 9,292 passed / 76 skipped, one nextest
leaky annotation. Windows integration: 2,394 passed / 19 skipped, two leaky
annotations. The workflow reports success; annotations are retained rather than
represented as absent. FreeBSD remains manual-only and was not run. Fuzz followed
the existing skipped policy; no checks or floors were weakened.

[Candidate build 40721829712](https://github.com/david419kr/photocraft-qwen-edit/actions/runs/40721829712)
succeeded on that same SHA. This existing candidate workflow also repeated focused
fork regressions: 31 nonempty groups, 650 aggregate passes and three ignored
selections (groups can overlap). Stable and Rust 1.95 Clippy succeeded. Windows
PowerShell 5.1 replacement/checksum/rollback tests: three passed. The owner objected
to the duplicated regression work; no further CI or executable build was started.

## Artifact and identity

Original Windows artifact: [12832900372](https://github.com/david419kr/photocraft-qwen-edit/actions/runs/40721829712/artifacts/12832900372).
Toolchain: Rust 1.95.0; target: `x86_64-pc-windows-msvc`; static CRT flag:
`-C target-feature=+crt-static`; manifest AVIF encode/decode enabled.

ZIP: `photocraft-qwen-edit-261028-2-0.16.0-windows-x64.zip`.
SHA256: `186a045371bdc9fe16b8ee34573217e0ad5686068cddcaf080d7d0fa6adb484a`.
EXE SHA256: `4a665b34df7bd52c3e65594936cd616f3dda7e2f36fa5f9f5c686af06b315b5d`.

The build packager checked actual EXE --version against release-tag.txt, AMD64 PE,
absence of dynamic CRT imports, exact 18-file package and license contents, and
ZIP/EXE hashes. The publication workflow downloads this original artifact without
rebuilding or renaming it, checks the internal README/portable marker identity,
runs --version again, and compiles the unchanged production updater comparison
against the actual build identity. Equal installed/latest identities must offer
no update; the previous 261028-0.16.0 installation must select the exact new asset.

[Release](https://github.com/david419kr/photocraft-qwen-edit/releases/tag/261028-2-0.16.0)
and [durable provenance](https://github.com/david419kr/photocraft-qwen-edit/releases/download/261028-2-0.16.0/release-provenance.json)
record the publication controller/run and independent artifact/updater verification.
The publisher re-downloads every public asset and verifies hashes and package
contents, then preserves its result as the publication-verification Actions artifact.
Existing releases, tags and assets are preserved, including the historical corrected
261028-0.16.0 assets. The original failed CI 40710260628 remains historical evidence.

No user PC, real CUDA/VRAM inference, or interactive hardware UI acceptance was
performed by this cloud release task. Historical local results in docs/comfyui.md
are separate evidence.
