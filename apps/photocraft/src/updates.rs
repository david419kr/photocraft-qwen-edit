//! Native HTTPS transport for release checks. Only public release metadata is requested.

use photocraft_engine::updates_cmds::{CheckFn, CheckResult, Release};
use std::time::Duration;

const LATEST: &str = "https://api.github.com/repos/david419kr/photocraft-qwen-edit/releases/latest";
const MAX_RESPONSE: u64 = 1024 * 1024;

pub fn service() -> CheckFn {
    Box::new(|| {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("update check".into())
            .spawn(move || {
                let result = fetch(LATEST, Duration::from_secs(15));
                if let Err(e) = &result {
                    log::warn!("Update check: {e}");
                }
                let _ = tx.send(result);
            })
            .map_err(|e| format!("Could not start update check: {e}"))?;
        Ok(rx)
    })
}

fn fetch(url: &str, timeout: Duration) -> CheckResult {
    // Respect the OS trust store (including managed-network certificates), while still
    // validating the certificate chain and server name.
    let agent = ureq::Agent::config_builder()
        .tls_config(ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build())
        .timeout_global(Some(timeout))
        .max_redirects(0)
        .build()
        .new_agent();
    let mut response = match agent
        .get(url)
        .header("User-Agent", "PhotoCraft-update-check")
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2026-03-10")
        .call()
    {
        Ok(response) => response,
        // The repository may not have published a stable release yet.
        Err(ureq::Error::StatusCode(404)) => return Ok(None),
        Err(ureq::Error::StatusCode(403 | 429)) => return Err("GitHub is limiting update checks. Please try again later.".into()),
        Err(e) => return Err(format!("Could not contact the release server: {e}")),
    };
    if response.status() != 200 {
        return Err(format!("Unexpected release server status: {}", response.status()));
    }
    let bytes = response.body_mut().with_config().limit(MAX_RESPONSE).read_to_vec().map_err(|e| format!("Could not read release response: {e}"))?;
    parse_fork_release(&bytes, photocraft_engine::build_info::release_tag())
}

// The upstream engine API remains available, but native fork builds must only
// offer our dated portable releases, using the executable updater's ordering.
fn parse_fork_release(bytes: &[u8], current: &str) -> CheckResult {
    #[derive(serde::Deserialize)]
    struct Metadata {
        tag_name: String,
        draft: bool,
        prerelease: bool,
    }
    let release: Metadata = serde_json::from_slice(bytes).map_err(|e| format!("Invalid release response: {e}"))?;
    if release.draft || release.prerelease {
        return Ok(None);
    }
    let latest = crate::updater::release_key(&release.tag_name).ok_or("Invalid fork release tag")?;
    let installed = crate::updater::release_key(current).ok_or("Invalid installed fork tag")?;
    if latest <= installed {
        return Ok(None);
    }
    Ok(Some(Release {
        version: release.tag_name.clone(),
        url: format!("https://github.com/{}/releases/tag/{}", photocraft_engine::build_info::FORK_REPOSITORY, release.tag_name),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    fn server(status: &str, body: &str, extra: &str) -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/latest", listener.local_addr().unwrap());
        let response = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n{body}", body.len());
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut request = Vec::new();
            let mut chunk = [0; 1024];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = stream.read(&mut chunk).unwrap();
                if n == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..n]);
            }
            let _ = stream.write_all(response.as_bytes());
            String::from_utf8(request).unwrap()
        });
        (url, worker)
    }

    #[test]
    fn fetches_release_metadata_with_generic_headers() {
        let (url, worker) = server("200 OK", r#"{"tag_name":"991231-99.0.0","draft":false,"prerelease":false}"#, "");
        let result = fetch(&url, Duration::from_secs(5)).unwrap().unwrap();
        assert_eq!(result.version, "991231-99.0.0");
        assert_eq!(result.url, "https://github.com/david419kr/photocraft-qwen-edit/releases/tag/991231-99.0.0");
        let request = worker.join().unwrap().to_ascii_lowercase();
        assert!(request.contains("user-agent: photocraft-update-check\r\n"));
        assert!(request.contains("accept: application/vnd.github+json"));
        assert!(!request.contains("authorization:") && !request.contains("cookie:"));
    }

    #[test]
    fn fork_release_check_preserves_order_and_rejects_upstream_tags() {
        let response = |tag: &str| serde_json::to_vec(&serde_json::json!({"tag_name":tag,"draft":false,"prerelease":false})).unwrap();
        assert_eq!(parse_fork_release(&response("261024-0.15.1"), "261026-0.16.0"), Ok(None));
        assert_eq!(parse_fork_release(&response("261026-0.16.0"), "261026-0.16.0"), Ok(None));
        assert!(parse_fork_release(&response("261026-2-0.16.0"), "261026-0.16.0").unwrap().is_some());
        assert!(parse_fork_release(&response("v99.0.0"), "261026-0.16.0").is_err());
        assert!(parse_fork_release(&response("../evil"), "261026-0.16.0").is_err());
    }

    #[test]
    fn handles_missing_release_rate_limits_and_bad_responses() {
        for (status, body, extra, expected_none) in [
            ("404 Not Found", "", "", true),
            ("403 Forbidden", "", "", false),
            ("429 Too Many Requests", "", "", false),
            ("500 Internal Server Error", "", "", false),
            ("200 OK", "invalid JSON", "", false),
            ("302 Found", "", "Location: https://untrusted.example/\r\n", false),
        ] {
            let (url, worker) = server(status, body, extra);
            let result = fetch(&url, Duration::from_secs(5));
            if expected_none {
                assert_eq!(result, Ok(None));
            } else {
                assert!(result.is_err(), "{status}");
            }
            worker.join().unwrap();
        }
    }

    #[test]
    fn caps_response_size_and_times_out() {
        let (url, worker) = server("200 OK", &" ".repeat(MAX_RESPONSE as usize + 1), "");
        assert!(fetch(&url, Duration::from_secs(5)).is_err());
        worker.join().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/latest", listener.local_addr().unwrap());
        let start = std::time::Instant::now();
        assert!(fetch(&url, Duration::from_millis(100)).is_err());
        assert!(start.elapsed() < Duration::from_secs(2));
    }
}
