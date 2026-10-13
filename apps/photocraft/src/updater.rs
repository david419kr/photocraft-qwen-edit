//! Public fork releases only. Download/verification run off the UI thread.
//! A Windows helper waits for exit, replaces only the EXE, rolls back on failure,
//! restarts with the original arguments and removes its staging directory.
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use photocraft_ui_egui::updates::{Action, Event, Release, Service};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const REPO: &str = photocraft_engine::build_info::FORK_REPOSITORY;
const MAX_ARCHIVE: u64 = 256 * 1024 * 1024;
const API_LIMIT: u64 = 1024 * 1024;

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
    digest: Option<String>,
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}

fn client() -> Result<reqwest::blocking::Client, String> {
    let mut builder = reqwest::blocking::Client::builder().user_agent("Photocraft-dev").timeout(Duration::from_secs(120));
    // Honour an explicitly configured CA bundle (for managed HTTPS proxies),
    // while retaining the public roots and normal certificate/hostname checks.
    if let Some(path) = std::env::var_os("SSL_CERT_FILE") {
        let pem = bounded(std::fs::File::open(path).map_err(|e| format!("Cannot read the configured TLS CA bundle: {e}"))?, API_LIMIT)?;
        for certificate in reqwest::Certificate::from_pem_bundle(&pem).map_err(|e| format!("Invalid TLS CA bundle: {e}"))? {
            builder = builder.add_root_certificate(certificate);
        }
    }
    builder.build().map_err(|e| e.to_string())
}

fn bounded(mut reader: impl Read, limit: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    reader.by_ref().take(limit + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("Update response exceeds the size limit.".into());
    }
    Ok(bytes)
}

fn release_key(tag: &str) -> Option<(u32, u32, u32, u32)> {
    let (day, version) = tag.split_once('-')?;
    if day.len() != 6 || !day.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let mut numbers = version.split('.');
    let key = (day.parse().ok()?, numbers.next()?.parse().ok()?, numbers.next()?.parse().ok()?, numbers.next()?.parse().ok()?);
    numbers.next().is_none().then_some(key)
}

fn digest(text: &str) -> Option<String> {
    let hash = text.strip_prefix("sha256:")?;
    (hash.len() == 64 && hash.bytes().all(|c| c.is_ascii_hexdigit())).then(|| hash.to_ascii_lowercase())
}

fn trusted_asset_url(url: &str, tag: &str, name: &str) -> bool {
    url == format!("https://github.com/{REPO}/releases/download/{tag}/{name}")
}

fn select(release: ApiRelease, current: &str) -> Result<Option<Release>, String> {
    if release.draft || release.prerelease {
        return Ok(None);
    }
    let latest = release_key(&release.tag_name).ok_or("Unsupported fork release tag.")?;
    let installed = release_key(current).ok_or("The installed fork release tag is unknown.")?;
    if latest <= installed {
        return Ok(None);
    }
    let name = format!("photocraft-qwen-edit-{}-windows-x64.zip", release.tag_name);
    let mut assets = release.assets.into_iter().filter(|a| a.name == name);
    let asset = assets.next().ok_or("The release has no Windows x64 portable ZIP.")?;
    if assets.next().is_some() || asset.size == 0 || asset.size > MAX_ARCHIVE || !trusted_asset_url(&asset.browser_download_url, &release.tag_name, &name) {
        return Err("Invalid Windows update asset.".into());
    }
    let sha256 = asset.digest.as_deref().and_then(digest).ok_or("The release ZIP has no verified SHA-256 digest.")?;
    Ok(Some(Release { tag: release.tag_name, asset_name: name, url: asset.browser_download_url, size: asset.size, sha256 }))
}

fn check() -> Result<Option<Release>, String> {
    let response = client()?.get(format!("https://api.github.com/repos/{REPO}/releases/latest")).send().map_err(|e| e.to_string())?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let bytes = bounded(response.error_for_status().map_err(|e| e.to_string())?, API_LIMIT)?;
    let release = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    select(release, photocraft_engine::build_info::release_tag())
}

fn verify_pe(bytes: &[u8]) -> Result<(), String> {
    if bytes.get(..2) != Some(b"MZ") {
        return Err("The update is not a Windows executable.".into());
    }
    let offset: [u8; 4] = bytes.get(0x3c..0x40).and_then(|v| v.try_into().ok()).ok_or("Truncated Windows executable.")?;
    let offset = u32::from_le_bytes(offset) as usize;
    let end = offset.checked_add(6).ok_or("Invalid Windows executable.")?;
    let header = bytes.get(offset..end).ok_or("Invalid Windows executable header.")?;
    if header.get(..4) != Some(b"PE\0\0") || header.get(4..6) != Some(&[0x64, 0x86]) {
        return Err("The update is not a Windows x64 executable.".into());
    }
    Ok(())
}

fn extract(archive: &Path, destination: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    if zip.len() > 512 || zip.file_names().filter(|name| *name == "photocraft.exe").count() != 1 {
        return Err("The ZIP must contain exactly one root photocraft.exe.".into());
    }
    let mut exe = zip.by_name("photocraft.exe").map_err(|e| e.to_string())?;
    if exe.is_dir() || exe.size() == 0 || exe.size() > MAX_ARCHIVE || exe.unix_mode().is_some_and(|mode| mode & 0o170000 == 0o120000) {
        return Err("Invalid executable entry in the update ZIP.".into());
    }
    let bytes = bounded(&mut exe, MAX_ARCHIVE)?;
    verify_pe(&bytes)?;
    let mut output = std::fs::OpenOptions::new().write(true).create_new(true).open(destination).map_err(|e| e.to_string())?;
    output.write_all(&bytes).and_then(|_| output.sync_all()).map_err(|e| e.to_string())
}

fn download(release: &Release, executable: &Path) -> Result<tempfile::TempDir, String> {
    if !trusted_asset_url(&release.url, &release.tag, &release.asset_name) || release.size == 0 || release.size > MAX_ARCHIVE {
        return Err("Untrusted update download.".into());
    }
    let response = client()?.get(&release.url).send().and_then(|r| r.error_for_status()).map_err(|e| e.to_string())?;
    stage(response, release, executable)
}

fn stage(response: impl Read, release: &Release, executable: &Path) -> Result<tempfile::TempDir, String> {
    if release.size == 0 || release.size > MAX_ARCHIVE {
        return Err("Invalid update size.".into());
    }
    let dir = tempfile::Builder::new()
        .prefix(".photocraft-update-")
        .tempdir_in(executable.parent().ok_or("No installation directory.")?)
        .map_err(|e| e.to_string())?;
    let archive = dir.path().join("release.zip");
    let bytes = bounded(response, release.size)?;
    if bytes.len() as u64 != release.size || format!("{:x}", Sha256::digest(&bytes)) != release.sha256 {
        return Err("The update ZIP failed SHA-256 or size verification.".into());
    }
    std::fs::write(&archive, bytes).map_err(|e| e.to_string())?;
    extract(&archive, &dir.path().join("photocraft.exe"))?;
    // The ZIP is no longer needed. All remaining files belong to this TempDir.
    std::fs::remove_file(archive).map_err(|e| e.to_string())?;
    Ok(dir)
}

#[cfg(target_os = "windows")]
fn restart(dir: tempfile::TempDir) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let staged = dir.path().join("photocraft.exe");
    let manifest = serde_json::json!({
        "pid":std::process::id(), "target":executable, "staged":staged,
        "sha256":format!("{:x}", Sha256::digest(std::fs::read(&staged).map_err(|e| e.to_string())?)),
        "directory":dir.path(), "cwd":std::env::current_dir().map_err(|e| e.to_string())?,
        "arguments":std::env::args_os().skip(1).map(|a| a.into_string().map_err(|_| "A restart argument is not Unicode.")).collect::<Result<Vec<_>, _>>()?
    });
    let manifest_path = dir.path().join("update.json");
    std::fs::write(&manifest_path, serde_json::to_vec(&manifest).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let script = dir.path().join("apply.ps1");
    std::fs::write(&script, include_str!("updater.ps1")).map_err(|e| e.to_string())?;
    // Use the OS copy of PowerShell, never a PATH-supplied executable.
    let powershell =
        std::path::PathBuf::from(std::env::var_os("SystemRoot").ok_or("SystemRoot is unavailable.")?).join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let mut helper = std::process::Command::new(powershell)
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "RemoteSigned", "-File"])
        .arg(script)
        .arg("-Manifest")
        .arg(manifest_path)
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|e| e.to_string())?;
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !dir.path().join("ready").is_file() {
        if let Some(status) = helper.try_wait().map_err(|e| e.to_string())? {
            return Err(format!("The update helper could not start ({status})."));
        }
        if std::time::Instant::now() >= deadline {
            let _ = helper.kill();
            let _ = helper.wait();
            return Err("The update helper did not become ready.".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    // The helper now owns cleanup, including its script and manifest.
    let _ = dir.keep();
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn restart(_dir: tempfile::TempDir) -> Result<(), String> {
    Err("Executable updates are available only on Windows x64.".into())
}

pub fn service(ctx: egui::Context) -> Service {
    let (tx, events) = mpsc::channel();
    let pending = Arc::new(Mutex::new(None));
    Service {
        supported: cfg!(all(target_os = "windows", target_arch = "x86_64")),
        events,
        action: Box::new(move |action| {
            match action {
                Action::Cancel => {
                    pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
                }
                Action::Restart => {
                    let dir = pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take().ok_or("No verified update is ready.")?;
                    restart(dir)?;
                }
                Action::Check | Action::Download(_) => {
                    let tx = tx.clone();
                    let ctx = ctx.clone();
                    let pending = pending.clone();
                    std::thread::Builder::new()
                        .name("photocraft-update".into())
                        .spawn(move || {
                            let event = match action {
                                Action::Check => Event::Checked(check()),
                                Action::Download(release) => {
                                    let result = std::env::current_exe().map_err(|e| e.to_string()).and_then(|exe| download(&release, &exe));
                                    match result {
                                        Ok(dir) => {
                                            *pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(dir);
                                            Event::Prepared
                                        }
                                        Err(error) => Event::Failed(error),
                                    }
                                }
                                _ => return,
                            };
                            let _ = tx.send(event);
                            ctx.request_repaint();
                        })
                        .map_err(|e| e.to_string())?;
                }
            }
            Ok(())
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str) -> ApiRelease {
        let name = format!("photocraft-qwen-edit-{tag}-windows-x64.zip");
        ApiRelease {
            tag_name: tag.into(),
            draft: false,
            prerelease: false,
            assets: vec![Asset {
                browser_download_url: format!("https://github.com/{REPO}/releases/download/{tag}/{name}"),
                name,
                size: 100,
                digest: Some(format!("sha256:{}", "a".repeat(64))),
            }],
        }
    }

    #[test]
    fn fork_tags_compare_dates_and_numeric_versions_and_reject_unknown_tags() {
        assert!(select(release("261014-0.8.2"), "261013-0.8.2").unwrap().is_some());
        assert!(select(release("261013-0.8.10"), "261013-0.8.2").unwrap().is_some());
        assert!(select(release("261013-0.8.2"), "261013-0.8.2").unwrap().is_none());
        assert!(select(release("261012-0.8.2"), "261013-0.8.2").unwrap().is_none());
        assert!(release_key("../../bad").is_none());
        assert!(release_key("261013-0.8.2-rc.1").is_none());
        let mut pre = release("261014-0.8.2");
        pre.prerelease = true;
        assert!(select(pre, "261013-0.8.2").unwrap().is_none());
    }

    #[test]
    fn untrusted_assets_and_missing_checksums_are_rejected() {
        let mut bad = release("261014-0.8.2");
        bad.assets[0].browser_download_url = "https://evil.example/update.zip".into();
        assert!(select(bad, "261013-0.8.2").is_err());
        let mut bad = release("261014-0.8.2");
        bad.assets[0].digest = None;
        assert!(select(bad, "261013-0.8.2").is_err());
        assert!(bounded(&b"1234"[..], 3).is_err());
        assert!(verify_pe(b"not an executable").is_err());
    }

    #[test]
    fn staging_cleans_failed_downloads_and_keeps_only_the_verified_exe() {
        let installation = tempfile::tempdir().unwrap();
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let mut pe = vec![0; 128];
        pe[..2].copy_from_slice(b"MZ");
        pe[0x3c..0x40].copy_from_slice(&64u32.to_le_bytes());
        pe[64..70].copy_from_slice(b"PE\0\0\x64\x86");
        writer.start_file("photocraft.exe", zip::write::SimpleFileOptions::default()).unwrap();
        writer.write_all(&pe).unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        let release = Release {
            tag: "261014-0.8.2".into(),
            asset_name: String::new(),
            url: String::new(),
            size: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(&bytes)),
        };
        let exe = installation.path().join("photocraft.exe");
        std::fs::write(&exe, b"original").unwrap();
        let mut corrupt = bytes.clone();
        corrupt[0] ^= 1;
        assert!(stage(corrupt.as_slice(), &release, &exe).is_err());
        assert_eq!(std::fs::read_dir(installation.path()).unwrap().count(), 1);
        let dir = stage(bytes.as_slice(), &release, &exe).unwrap();
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        assert_eq!(std::fs::read(dir.path().join("photocraft.exe")).unwrap(), pe);
        assert_eq!(std::fs::read(&exe).unwrap(), b"original");
        drop(dir);
        assert_eq!(std::fs::read_dir(installation.path()).unwrap().count(), 1);
    }

    #[test]
    fn zip_extracts_only_root_executable_and_never_installer_or_traversal_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.zip");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let mut pe = vec![0; 128];
        pe[..2].copy_from_slice(b"MZ");
        pe[0x3c..0x40].copy_from_slice(&64u32.to_le_bytes());
        pe[64..70].copy_from_slice(b"PE\0\0\x64\x86");
        for (name, bytes) in [("photocraft.exe", pe.as_slice()), ("../outside.exe", b"bad".as_slice()), ("scripts/install.ps1", b"bad".as_slice())] {
            writer.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
        let output = dir.path().join("new.exe");
        extract(&path, &output).unwrap();
        assert_eq!(std::fs::read(output).unwrap(), pe);
        assert!(!dir.path().join("scripts").exists());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    }
}
