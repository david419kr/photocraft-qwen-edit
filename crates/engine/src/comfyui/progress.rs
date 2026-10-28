//! Best-effort, job-scoped live progress. History remains the completion/error authority.
use super::*;
use std::net::TcpStream;
use tungstenite::{Message, WebSocket, protocol::WebSocketConfig, stream::MaybeTlsStream};

const MAX_MESSAGE: usize = 8 << 20;

#[derive(Default)]
pub(super) struct Live {
    socket: Option<WebSocket<MaybeTlsStream<TcpStream>>>,
}

impl Live {
    pub fn connect(base: &Url, job: &str) -> Self {
        Self { socket: Self::open(base, job) }
    }

    fn open(base: &Url, job: &str) -> Option<WebSocket<MaybeTlsStream<TcpStream>>> {
        let mut url = base.join("ws").ok()?;
        url.query_pairs_mut().append_pair("clientId", job);
        let scheme = if base.scheme() == "https" { "wss" } else { "ws" };
        url.set_scheme(scheme).ok()?;
        // No redirect/proxy path: the validated endpoint is loopback-only.
        let stream = url
            .socket_addrs(|| None)
            .ok()?
            .into_iter()
            .filter(|a| a.ip().is_loopback())
            .take(4)
            .find_map(|address| TcpStream::connect_timeout(&address, Duration::from_secs(2)).ok())?;
        stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
        stream.set_write_timeout(Some(Duration::from_secs(2))).ok()?;
        let config = WebSocketConfig::default()
            .read_buffer_size(16 << 10)
            .write_buffer_size(0)
            .max_write_buffer_size(MAX_MESSAGE + 1024)
            .max_message_size(Some(MAX_MESSAGE))
            .max_frame_size(Some(MAX_MESSAGE));
        let (mut socket, _) = tungstenite::client_tls_with_config(url.as_str(), stream, Some(config), None).ok()?;
        tcp(socket.get_mut())?.set_nonblocking(true).ok()?;
        Some(socket)
    }

    pub fn poll(&mut self, job: &str, ctx: &JobCtx) -> Result<()> {
        ctx.check()?;
        let Some(socket) = self.socket.as_mut() else {
            ctx.progress(0.2, "Generating image; live progress unavailable");
            return Ok(());
        };
        // Bound draining when previews/other events arrive faster than sampling updates.
        for _ in 0..64 {
            ctx.check()?;
            match socket.read() {
                Ok(Message::Text(text)) if text.len() <= MAX_JSON_BYTES as usize => {
                    if let Ok(event) = serde_json::from_str::<Value>(&text) {
                        apply(&event, job, ctx);
                    }
                }
                Ok(Message::Close(_)) => {
                    self.socket = None;
                    break;
                }
                Ok(_) => {}
                Err(tungstenite::Error::Io(error)) if matches!(error.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => break,
                Err(_) => {
                    self.socket = None;
                    ctx.progress(0.2, "Generating image; live progress disconnected");
                    break;
                }
            }
        }
        Ok(())
    }
}

fn tcp(stream: &mut MaybeTlsStream<TcpStream>) -> Option<&mut TcpStream> {
    match stream {
        MaybeTlsStream::Plain(s) => Some(s),
        MaybeTlsStream::Rustls(s) => Some(&mut s.sock),
        _ => None,
    }
}

fn update(node: &str, state: &Value, ctx: &JobCtx) {
    let (lo, hi, label) = match node {
        "3" => (0.2, 0.85, "Generating image"),
        "8" => (0.85, 0.9, "Decoding image"),
        _ => return,
    };
    let Some(max) = state.get("max").and_then(Value::as_f64).filter(|v| v.is_finite() && *v > 0.0) else { return };
    let Some(value) = state.get("value").and_then(Value::as_f64).filter(|v| v.is_finite() && *v >= 0.0) else { return };
    let ratio = (value / max).clamp(0.0, 1.0) as f32;
    ctx.progress(lo + (hi - lo) * ratio, &format!("{label} · {value:.0}/{max:.0} ({:.0}%)", ratio * 100.0));
}

fn apply(event: &Value, job: &str, ctx: &JobCtx) {
    let Some(data) = event.get("data").filter(|d| d.get("prompt_id").and_then(Value::as_str) == Some(job)) else { return };
    match event.get("type").and_then(Value::as_str) {
        Some("progress") => {
            if let Some(node) = data.get("node").or_else(|| data.get("node_id")).and_then(Value::as_str) {
                update(node, data, ctx);
            }
        }
        Some("progress_state") => {
            if let Some(nodes) = data.get("nodes").and_then(Value::as_object) {
                for node in ["8", "3"] {
                    if let Some(state) = nodes.get(node).filter(|s| s.get("prompt_id").is_none_or(|id| id == job)) {
                        update(node, state, ctx);
                        break;
                    }
                }
            }
        }
        Some("executing") => {
            let (fraction, message) = match data.get("node").and_then(Value::as_str) {
                Some("37") => (0.2, "Loading image model"),
                Some("38") => (0.2, "Loading text encoder"),
                Some("39") => (0.2, "Loading image encoder"),
                Some("64") => (0.2, "Encoding prompt and reference images"),
                Some("3") => (0.2, "Generating image"),
                Some("8") => (0.85, "Decoding image"),
                _ => return,
            };
            ctx.progress(fraction, message);
        }
        Some("execution_cached") if data.get("nodes").and_then(Value::as_array).is_some_and(|nodes| nodes.iter().any(|n| n == "3")) => {
            ctx.progress(0.85, "Using cached generation");
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests;
