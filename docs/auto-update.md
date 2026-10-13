# Fork automatic updates

The Windows x64 desktop app checks the public stable releases of
`david419kr/photocraft-qwen-edit` once at startup. It never checks upstream
`storytold/photocraft`, downloads a prerelease, or updates without consent.
The latest release must have a newer `YYMMDD-major.minor.patch` or
`YYMMDD-N-major.minor.patch` tag (N is a same-day revision, starting at 2), a
`photocraft-qwen-edit-<tag>-windows-x64.zip` asset, and a GitHub SHA-256 digest.
Dates, upstream versions and then same-version revisions compare numerically.
`261013-2-0.8.2` is newer than `261013-0.8.2`; revision 10 is newer than 2.

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
baseline. The Qwen candidate workflow embeds `release-tag.txt` before Cargo runs and
packaging verifies that the reported tag matches it exactly. The Korean local
build script still uses its Tokyo build date and workspace version. Packaging
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

## Windows helper CI regression (2026-10-13)

The initial feature commit `723c94e672878598b9a1f06592fd67e25090770b`
compiled successfully, but candidate run `38381222919` failed the replacement
success test. Diagnostic run `38383396048` at
`d4d431aa4eea16c80d6e53fbe9a255bf11f4584e` exposed the actual error:
Windows PowerShell could not resolve `Get-FileHash`. The two rollback tests had
passed through this same earlier error rather than their intended failure paths.

The helper now computes SHA256 through the .NET stream API without PowerShell
module discovery, and reports caught errors even when GUI dialogs are suppressed.
The Windows tests require the specific checksum/restart failure paths, run an
actual old EXE while waiting for exit, and retain argument, settings, binary,
rollback and staging-cleanup assertions. This gate runs before the release build.
The focused regression runner also executes the updater and update UI Rust tests
and rejects empty test selections. Final CI and portable artifact provenance are
recorded by the candidate workflow for each tested source SHA.

Full CI run `38384184579` then found the scorecard staleness gate: the new
`automatic_update_check` preference raises the audited setting count from 164
to 165. `cargo xtask scorecard` regenerated only those two count occurrences;
the 27 unread settings and all floors remain unchanged. The focused staleness
test and `cargo xtask scorecard --check` pass after regeneration.

## Same-day second release

For the approved `261013-2-0.8.2` publication, the updater accepts a numeric
same-day revision before the upstream version. Unnumbered tags mean revision 1;
numbered tags start at 2. Comparison remains date, upstream semantic version,
then revision. Tests cover forward updates, equality, downgrade rejection,
revision 10 versus 2, a newer upstream version, and malformed tags.
The committed release identity and candidate EXE must both be
`261013-2-0.8.2`; a ZIP renamed around an older embedded identity is not valid.
The user's local acceptance covered candidate `dd761a85b1023fd894ef118d5c2ccf5924530c36`;
the release follow-up changes version parsing/identity and retains main's cron removals.
