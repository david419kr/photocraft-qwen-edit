# Fork automatic updates

The Windows x64 desktop app checks the public stable releases of
`david419kr/photocraft-qwen-edit` once at startup. It never checks upstream
`storytold/photocraft`, downloads a prerelease, or updates without consent.
The latest release must have a newer `YYMMDD-major.minor.patch` tag, a
`photocraft-qwen-edit-<tag>-windows-x64.zip` asset, and a GitHub SHA-256 digest.
Same-day versions compare numerically (`0.8.10` is newer than `0.8.2`).

The update prompt has Yes / No and **Don't show again**. Help → About PhotoCraft
has **Automatically check for updates** and **Check for updates now**.
Both checkboxes control the single persisted `general.automaticUpdateCheck`
preference, with opposite labels. Checking Don't show again disables future
startup checks and explains where to check manually. Manual checking and
installation remain available even when automatic checking is disabled.
No dismisses this offer; it does not disable the next startup check by itself.

Before installing, save or close any modified documents and finish pending saves.
Download and verification run in the background. The app verifies the archive's
size and SHA-256, reads only the root `photocraft.exe` entry, checks its Windows x64
PE header, and stages it in a private temporary directory beside the installed
EXE. It does not extract installers, scripts, settings, or other ZIP entries.
The archive is deleted immediately after verified extraction.

A Windows PowerShell helper must signal readiness before the app closes. It
waits for the old process to exit, verifies the staged EXE again, backs up the
original, moves the new EXE into place and restarts it with the same working
directory and arguments. The backup, helper, manifest and staging directory are
deleted after success. Download failures and successful rollback also remove
temporary files. If rollback cannot restore the original, it retains the original
backup and reports its location rather than deleting the only recoverable EXE.
Write permission in the installation directory is required; the updater does
not request administrator elevation. Network / helper errors leave the running
app or original executable usable.

About shows the fork release identity and repository first, with the upstream
engine version separately. `PHOTOCRAFT_RELEASE_TAG` embeds the packaging tag;
source builds default to `release-tag.txt`. Update that file for a new source
baseline. The Qwen build workflow and Korean portable build script set the tag
from the Tokyo build date and workspace version before Cargo runs. Packaging
uses the embedded tag to avoid a mismatch at midnight. Release tags must be
unique; publishing a different binary under an existing tag is not an update.

Automation uses `help.update.check`, `help.update.install`,
`help.update.dismiss`, and `prefs.set general.automaticUpdateCheck`.
`ui.inspect.update` reports check/download state, offered tag, support and errors.
Executable replacement is Windows x64 only; the browser build has no native
update service.

Focused checks:

```sh
cargo test --locked -p photocraft --bin photocraft updater::tests
cargo test --locked -p photocraft-ui-egui --lib updates::tests
cargo test --locked -p photocraft-engine --lib automatic_update_check
python -m unittest scripts.tests.test_auto_updater -v
```

The helper tests need PowerShell and rustc. They execute the real helper against
native Rust fixture executables, covering exit waiting, Unicode paths, argument
quoting, successful replacement, checksum failure, rollback after restart
failure, preservation of settings, and cleanup. The Windows candidate workflow
runs them with Windows PowerShell 5.1, including Windows EXE file locking.
