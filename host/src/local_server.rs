use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use subtle::ConstantTimeEq;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::protocol::Message;
use tracing::{info, warn};
use uuid::Uuid;

use crate::input::InputSimulator;
use crate::privacy::PrivacyCurtain;
use crate::protocol::InputPacket;
use crate::session::{
    host_info_message, new_session_guard, spawn_capture_worker, spawn_writer, SessionCtx, StreamState,
};

const MAX_HEAD: usize = 32 * 1024;
const MAX_BODY: usize = 64 * 1024;
const READ_TIMEOUT: Duration = Duration::from_secs(55);

struct ServerConfig {
    device_id: String,
    token: String,
    /// Random per-run secret handed only to pages served to the loopback interface.
    nonce: String,
}

impl ServerConfig {
    fn token_ok(&self, provided: &str) -> bool {
        if provided.is_empty() {
            return false;
        }
        let nonce_ok: bool = provided.as_bytes().ct_eq(self.nonce.as_bytes()).into();
        let token_ok: bool = !self.token.is_empty() && provided.as_bytes().ct_eq(self.token.as_bytes()).into();
        nonce_ok || token_ok
    }
}

pub struct LocalServer;

impl LocalServer {
    pub fn start(
        port: u16,
        device_id: String,
        token: String,
        simulator: Arc<InputSimulator>,
        privacy: Arc<PrivacyCurtain>,
    ) {
        tokio::spawn(async move {
            let addr = format!("0.0.0.0:{}", port);
            let listener = match TcpListener::bind(&addr).await {
                Ok(l) => {
                    info!("Direct local Wi-Fi / LAN server listening on: ws://{}/ws", addr);
                    l
                }
                Err(e) => {
                    warn!("Failed to bind local server on {}: {}", addr, e);
                    return;
                }
            };

            let cfg = Arc::new(ServerConfig { device_id, token, nonce: Uuid::new_v4().simple().to_string() });

            loop {
                let (stream, peer) = match listener.accept().await {
                    Ok(v) => v,
                    Err(e) => {
                        warn!("Local server accept error: {}", e);
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        continue;
                    }
                };
                let _ = stream.set_nodelay(true);
                let cfg = cfg.clone();
                let sim = simulator.clone();
                let privacy = privacy.clone();
                tokio::spawn(async move {
                    handle_connection(stream, peer, cfg, sim, privacy).await;
                });
            }
        });
    }
}

struct Request {
    method: String,
    path: String,
    headers: std::collections::HashMap<String, String>,
}

impl Request {
    fn header(&self, name: &str) -> &str {
        self.headers.get(name).map(|s| s.as_str()).unwrap_or("")
    }
}

fn parse_head(head: &[u8]) -> Option<Request> {
    let text = String::from_utf8_lossy(head);
    let mut lines = text.split("\r\n");
    let mut first = lines.next()?.split_whitespace();
    let method = first.next()?.to_ascii_uppercase();
    let path = first.next()?.to_string();
    let mut headers = std::collections::HashMap::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }
    Some(Request { method, path, headers })
}

/// Peeks (without consuming) until the full request head has arrived.
async fn peek_head(stream: &TcpStream) -> Option<(Vec<u8>, usize)> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut buf = vec![0u8; MAX_HEAD];
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let n = tokio::time::timeout(remaining, stream.peek(&mut buf)).await.ok()?.ok()?;
        if n == 0 {
            return None;
        }
        if let Some(pos) = buf[..n].windows(4).position(|w| w == b"\r\n\r\n") {
            return Some((buf[..pos + 4].to_vec(), pos + 4));
        }
        if n >= buf.len() || tokio::time::Instant::now() >= deadline {
            return None;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn respond(stream: &mut TcpStream, status: &str, content_type: &str, body: &str) {
    let resp = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nX-Content-Type-Options: nosniff\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status,
        content_type,
        body.len(),
        body
    );
    let _ = stream.write_all(resp.as_bytes()).await;
}

async fn respond_json(stream: &mut TcpStream, ok: bool) {
    let (status, body) = if ok { ("200 OK", "{\"success\":true}") } else { ("400 Bad Request", "{\"success\":false}") };
    respond(stream, status, "application/json", body).await;
}

/// True when an `Origin`/`Referer` host is loopback, localhost or a private LAN address.
fn origin_allowed(origin: &str) -> bool {
    let rest = origin.split("://").nth(1).unwrap_or(origin);
    let host_port = rest.split('/').next().unwrap_or("");
    let host = if let Some(stripped) = host_port.strip_prefix('[') {
        stripped.split(']').next().unwrap_or("")
    } else {
        host_port.rsplit_once(':').map(|(h, _)| h).unwrap_or(host_port)
    };
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(v4)) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        Ok(IpAddr::V6(v6)) => v6.is_loopback(),
        Err(_) => false,
    }
}

fn apk_candidates() -> Vec<std::path::PathBuf> {
    let mut v = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            v.push(dir.join("sway.apk"));
            v.push(dir.join("app-debug.apk"));
        }
    }
    for p in [
        "sway.apk",
        "app-debug.apk",
        "src/sway.apk",
        "android/app/build/outputs/apk/debug/app-debug.apk",
        "../android/app/build/outputs/apk/debug/app-debug.apk",
    ] {
        v.push(std::path::PathBuf::from(p));
    }
    v
}

async fn serve_apk(stream: &mut TcpStream, head_only: bool) {
    for p in apk_candidates() {
        if let Ok(mut file) = tokio::fs::File::open(&p).await {
            let len = match file.metadata().await {
                Ok(m) => m.len(),
                Err(_) => continue,
            };
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/vnd.android.package-archive\r\nContent-Disposition: attachment; filename=\"Sway-Remote.apk\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                len
            );
            let _ = stream.write_all(header.as_bytes()).await;
            if !head_only {
                // Streamed with an 8 KB buffer, not loaded into RAM.
                let _ = tokio::io::copy(&mut file, stream).await;
            }
            return;
        }
    }
    respond(
        stream,
        "404 Not Found",
        "text/plain",
        "APK not built yet. Run './gradlew assembleDebug' in the android folder.",
    )
    .await;
}

async fn handle_connection(
    mut stream: TcpStream,
    peer: SocketAddr,
    cfg: Arc<ServerConfig>,
    sim: Arc<InputSimulator>,
    privacy: Arc<PrivacyCurtain>,
) {
    let (head, head_len) = match peek_head(&stream).await {
        Some(v) => v,
        None => return,
    };
    let req = match parse_head(&head) {
        Some(r) => r,
        None => return,
    };

    if req.header("upgrade").eq_ignore_ascii_case("websocket") {
        handle_ws(stream, peer, cfg, sim, privacy).await;
        return;
    }

    // Consume the head (it was only peeked) and then the declared body.
    let mut sink = vec![0u8; head_len];
    if stream.read_exact(&mut sink).await.is_err() {
        return;
    }
    let body_len: usize = req.header("content-length").parse().unwrap_or(0);
    if body_len > MAX_BODY {
        respond(&mut stream, "413 Payload Too Large", "text/plain", "too large").await;
        return;
    }
    let mut body = vec![0u8; body_len];
    if body_len > 0 {
        match tokio::time::timeout(Duration::from_secs(5), stream.read_exact(&mut body)).await {
            Ok(Ok(_)) => {}
            _ => return,
        }
    }

    let path = req.path.split('?').next().unwrap_or("/");

    match (req.method.as_str(), path) {
        ("GET", "/sway.apk") | ("GET", "/app.apk") => serve_apk(&mut stream, false).await,
        ("HEAD", "/sway.apk") | ("HEAD", "/app.apk") => serve_apk(&mut stream, true).await,
        ("GET", "/") | ("GET", "/index.html") => {
            let boot = serde_json::json!({
                "nonce": if peer.ip().is_loopback() { cfg.nonce.as_str() } else { "" },
                "deviceId": cfg.device_id,
            });
            let html = include_str!("dashboard.html")
                .replacen("<head>", &format!("<head><script>window.SWAY_BOOT={};</script>", boot), 1);
            respond(&mut stream, "200 OK", "text/html; charset=utf-8", &html).await;
        }
        ("POST", p) if p.starts_with("/api/") => {
            // Cross-site and foreign-origin requests are rejected outright.
            let origin = req.header("origin");
            let referer = req.header("referer");
            if req.header("sec-fetch-site").eq_ignore_ascii_case("cross-site")
                || (!origin.is_empty() && !origin_allowed(origin))
                || (!referer.is_empty() && !origin_allowed(referer))
            {
                respond(&mut stream, "403 Forbidden", "text/plain", "403 Forbidden: cross-origin request rejected").await;
                return;
            }
            if !cfg.token_ok(req.header("x-sway-token")) {
                respond(&mut stream, "401 Unauthorized", "text/plain", "401 Unauthorized: missing or invalid token").await;
                return;
            }
            let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
            let ok = handle_api(p, &json, sim, privacy).await;
            respond_json(&mut stream, ok).await;
        }
        _ => respond(&mut stream, "404 Not Found", "text/plain", "Not found").await,
    }
}

async fn handle_api(
    path: &str,
    json: &serde_json::Value,
    sim: Arc<InputSimulator>,
    privacy: Arc<PrivacyCurtain>,
) -> bool {
    match path {
        "/api/privacy" => match json.get("enable").and_then(|v| v.as_bool()) {
            Some(enable) => {
                privacy.set_enabled(enable);
                true
            }
            None => false,
        },
        "/api/resolution" => match json.get("mode").and_then(|v| v.as_u64()) {
            Some(mode) => {
                let m = if mode == 1 { 1 } else { 0 };
                let _ = tokio::task::spawn_blocking(move || InputSimulator::set_display_resolution(m)).await;
                true
            }
            None => false,
        },
        "/api/orientation" => match json.get("vertical").and_then(|v| v.as_bool()) {
            Some(vertical) => {
                let _ = tokio::task::spawn_blocking(move || sim.change_display_orientation(vertical)).await;
                true
            }
            None => false,
        },
        "/api/system" => match json.get("action").and_then(|v| v.as_u64()) {
            Some(a) if (1..=5).contains(&a) => {
                InputSimulator::execute_system_command(a as u8);
                true
            }
            _ => false,
        },
        "/api/clipboard" => match json.get("text").and_then(|v| v.as_str()) {
            Some(text) => {
                let text = text.to_string();
                let _ = tokio::task::spawn_blocking(move || InputSimulator::set_clipboard_text(&text)).await;
                true
            }
            None => false,
        },
        _ => false,
    }
}

#[derive(PartialEq, Clone, Copy)]
enum Role {
    Controller,
    Preview,
}

fn authenticate(text: &str, cfg: &ServerConfig, peer: SocketAddr) -> Option<Role> {
    let val: serde_json::Value = serde_json::from_str(text).ok()?;
    if val.get("type").and_then(|v| v.as_str()) != Some("join") {
        return None;
    }
    let token = val.get("token").and_then(|v| v.as_str()).unwrap_or("");
    let device = val.get("deviceId").and_then(|v| v.as_str()).unwrap_or("");
    let role = val.get("role").and_then(|v| v.as_str()).unwrap_or("");

    if role == "preview" {
        // Read-only live preview: only the loopback dashboard holding the per-run nonce.
        let nonce_ok: bool = !token.is_empty() && token.as_bytes().ct_eq(cfg.nonce.as_bytes()).into();
        return if nonce_ok && peer.ip().is_loopback() { Some(Role::Preview) } else { None };
    }

    let token_ok: bool = !cfg.token.is_empty() && token.as_bytes().ct_eq(cfg.token.as_bytes()).into();
    let device_ok: bool = !cfg.device_id.is_empty() && device.as_bytes().ct_eq(cfg.device_id.as_bytes()).into();
    if token_ok && device_ok {
        Some(Role::Controller)
    } else {
        None
    }
}

async fn handle_ws(
    stream: TcpStream,
    peer: SocketAddr,
    cfg: Arc<ServerConfig>,
    sim: Arc<InputSimulator>,
    privacy: Arc<PrivacyCurtain>,
) {
    let _ = socket2::SockRef::from(&stream).set_send_buffer_size(256 * 1024);
    let ws = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            warn!("Local WebSocket handshake error from {}: {}", peer, e);
            return;
        }
    };
    let (write, mut read) = ws.split();
    let (frame_tx, frame_rx) = tokio::sync::watch::channel::<Option<Vec<u8>>>(None);
    let (ctrl_tx, ctrl_rx) = mpsc::channel::<Message>(64);
    let (audio_tx, audio_rx) = mpsc::channel::<Vec<u8>>(16);
    let writer = spawn_writer(write, ctrl_rx, audio_rx, frame_rx);

    // The very first frame must be a valid `join`; everything else is rejected.
    let role = match tokio::time::timeout(Duration::from_secs(5), read.next()).await {
        Ok(Some(Ok(Message::Text(t)))) => authenticate(&t, &cfg, peer),
        _ => None,
    };
    let role = match role {
        Some(r) => r,
        None => {
            warn!("Local client at {} failed authentication. Rejecting.", peer);
            let _ = ctrl_tx
                .send(Message::Text(
                    serde_json::json!({"type":"error","error":"Authentication failed: invalid token"}).to_string(),
                ))
                .await;
            tokio::time::sleep(Duration::from_millis(100)).await;
            writer.abort();
            return;
        }
    };

    info!("Local client at {} authenticated", peer);
    let _ = ctrl_tx
        .send(Message::Text(serde_json::json!({"type":"status","payload":{"status":"paired"}}).to_string()))
        .await;

    let state = StreamState::new(true);
    let guard = if role == Role::Controller {
        let _ = ctrl_tx.send(host_info_message(&sim)).await;
        Some(new_session_guard(privacy.clone(), sim.clone()))
    } else {
        None
    };
    let capture = spawn_capture_worker(state.clone(), frame_tx);
    let mut ctx = SessionCtx::new(sim.clone(), privacy.clone(), state.clone(), ctrl_tx.clone(), audio_tx);

    loop {
        let msg = match tokio::time::timeout(READ_TIMEOUT, read.next()).await {
            Ok(Some(m)) => m,
            Ok(None) => break,
            Err(_) => {
                warn!("Local client {} timed out", peer);
                break;
            }
        };
        match msg {
            Ok(Message::Binary(bin)) => {
                if bin.len() == 1 && bin[0] == 0xAA {
                    continue; // frame acknowledgment heartbeat
                }
                if role == Role::Controller {
                    if let Ok(packet) = InputPacket::parse(&bin) {
                        ctx.handle(packet);
                    }
                }
            }
            Ok(Message::Text(t)) => {
                if let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&t) {
                    if v.get("type").and_then(|x| x.as_str()) == Some("ping") {
                        v["type"] = serde_json::Value::String("pong".into());
                        let _ = ctrl_tx.try_send(Message::Text(v.to_string()));
                    }
                }
            }
            Ok(Message::Ping(p)) => {
                let _ = ctrl_tx.try_send(Message::Pong(p));
            }
            Ok(Message::Close(_)) => {
                info!("Local client {} disconnected", peer);
                break;
            }
            Err(e) => {
                warn!("Local client stream error: {}", e);
                break;
            }
            _ => {}
        }
    }

    state.conn_alive.store(false, std::sync::atomic::Ordering::SeqCst);
    drop(ctx);
    drop(guard); // restores privacy/display/input only if this was the last active session
    writer.abort();
    let _ = tokio::time::timeout(Duration::from_secs(2), capture).await;
    info!("Local session for {} cleaned up", peer);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_filter_is_strict() {
        assert!(origin_allowed("http://localhost:8089"));
        assert!(origin_allowed("http://127.0.0.1:8089"));
        assert!(origin_allowed("http://192.168.11.93:8089"));
        assert!(origin_allowed("http://10.0.0.5"));
        assert!(origin_allowed("http://172.20.1.1:8089/x"));
        // Substring tricks that used to bypass the old check
        assert!(!origin_allowed("https://10.evil.com"));
        assert!(!origin_allowed("https://evil.com/?192.168."));
        assert!(!origin_allowed("https://win10.example.org"));
        assert!(!origin_allowed("http://8.8.8.8"));
        assert!(!origin_allowed("http://172.67.1.1")); // public, not RFC1918
    }

    #[test]
    fn join_requires_valid_token_and_device() {
        let cfg = ServerConfig { device_id: "dev".into(), token: "tok".into(), nonce: "nn".into() };
        let lan: SocketAddr = "192.168.1.9:5000".parse().unwrap();
        let lo: SocketAddr = "127.0.0.1:5000".parse().unwrap();
        assert!(authenticate(r#"{"type":"join","deviceId":"dev","token":"tok"}"#, &cfg, lan) == Some(Role::Controller));
        // The old bypasses: empty token / empty device / wrong type / not JSON
        assert!(authenticate(r#"{"type":"join"}"#, &cfg, lan).is_none());
        assert!(authenticate(r#"{"type":"join","deviceId":"dev","token":""}"#, &cfg, lan).is_none());
        assert!(authenticate(r#"{"type":"join","deviceId":"","token":"tok"}"#, &cfg, lan).is_none());
        assert!(authenticate(r#"{"type":"ping"}"#, &cfg, lan).is_none());
        assert!(authenticate("garbage", &cfg, lan).is_none());
        // Preview needs loopback + nonce
        assert!(authenticate(r#"{"type":"join","role":"preview","token":"nn"}"#, &cfg, lo) == Some(Role::Preview));
        assert!(authenticate(r#"{"type":"join","role":"preview","token":"nn"}"#, &cfg, lan).is_none());
        assert!(authenticate(r#"{"type":"join","role":"preview","token":"tok"}"#, &cfg, lo).is_none());
    }

    #[test]
    fn head_parsing() {
        let r = parse_head(b"POST /api/privacy HTTP/1.1\r\nHost: x\r\nContent-Length: 5\r\nX-Sway-Token: abc\r\n\r\n").unwrap();
        assert_eq!(r.method, "POST");
        assert_eq!(r.path, "/api/privacy");
        assert_eq!(r.header("content-length"), "5");
        assert_eq!(r.header("x-sway-token"), "abc");
    }
}
