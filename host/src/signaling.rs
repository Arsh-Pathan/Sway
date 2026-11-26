use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use qrcode::render::unicode;
use qrcode::QrCode;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::{error, info, warn};

use crate::input::InputSimulator;
use crate::power::SessionGuard;
use crate::privacy::PrivacyCurtain;
use crate::protocol::InputPacket;
use crate::session::{
    host_info_message, new_session_guard, spawn_capture_worker, spawn_writer, SessionCtx, StreamState,
};

/// A half-open NAT/Wi-Fi drop produces no FIN. The relay pings every 25s, so 55s of silence = dead.
const READ_TIMEOUT: Duration = Duration::from_secs(55);

#[derive(Debug, Serialize, Deserialize)]
pub struct SignalingEnvelope {
    #[serde(rename = "type")]
    pub msg_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(rename = "deviceId", skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub struct HostSignaling {
    pub server_url: String,
    pub device_id: String,
    pub token: String,
    pub simulator: Arc<InputSimulator>,
    pub privacy: Arc<PrivacyCurtain>,
}

/// Extracts `status` from either `{"status":"x"}` or a bare `"x"` payload.
fn status_of(payload: &Option<serde_json::Value>) -> String {
    match payload {
        Some(serde_json::Value::Object(o)) => o.get("status").and_then(|s| s.as_str()).unwrap_or("").to_string(),
        Some(serde_json::Value::String(s)) => s.clone(),
        _ => String::new(),
    }
}

/// Normalizes any accepted server URL form to `ws(s)://host[:port]/ws`.
pub fn build_ws_url(server_url: &str) -> String {
    let trimmed = server_url.trim().trim_end_matches('/');
    let trimmed = trimmed.strip_suffix("/ws").unwrap_or(trimmed);
    let base = if let Some(rest) = trimmed.strip_prefix("http://") {
        format!("ws://{}", rest)
    } else if let Some(rest) = trimmed.strip_prefix("https://") {
        format!("wss://{}", rest)
    } else if trimmed.starts_with("ws://") || trimmed.starts_with("wss://") {
        trimmed.to_string()
    } else {
        format!("ws://{}", trimmed)
    };
    format!("{}/ws", base)
}

/// Session resources that exist only while a viewer is attached through the relay.
struct CloudSession {
    guard: Option<SessionGuard>,
    capture: Option<tokio::task::JoinHandle<()>>,
}

impl HostSignaling {
    pub fn new(
        server_url: String,
        device_id: String,
        token: String,
        privacy: Arc<PrivacyCurtain>,
    ) -> Self {
        Self {
            server_url,
            device_id,
            token,
            simulator: Arc::new(InputSimulator::new()),
            privacy,
        }
    }

    /// Prints a scannable QR code directly to the console for pairing.
    pub fn print_pairing_qr(&self) {
        let reachable_server = if self.server_url.contains("127.0.0.1") || self.server_url.contains("localhost") {
            // Find actual LAN IP so scanning with phone camera directly works over Wi-Fi
            match std::net::UdpSocket::bind("0.0.0.0:0") {
                Ok(s) => match s.connect("8.8.8.8:80") {
                    Ok(_) => s.local_addr().ok().map(|a| format!("{}:8089", a.ip())).unwrap_or_else(|| self.server_url.clone()),
                    Err(_) => self.server_url.clone(),
                },
                Err(_) => self.server_url.clone(),
            }
        } else {
            self.server_url.clone()
        };

        let pairing_payload = serde_json::json!({
            "server": reachable_server,
            "deviceId": self.device_id,
            "token": self.token
        });

        let json_str = pairing_payload.to_string();
        if let Ok(code) = QrCode::new(json_str.as_bytes()) {
            let image = code
                .render::<unicode::Dense1x2>()
                .dark_color(unicode::Dense1x2::Light)
                .light_color(unicode::Dense1x2::Dark)
                .build();

            println!("\n=======================================================");
            println!("  SWAY PAIRING QR CODE - SCAN FROM YOUR PHONE CAMERA   ");
            println!("=======================================================\n");
            println!("{}", image);
            println!("Device ID : {}", self.device_id);
            println!("Token     : {}", self.token);
            println!("Signaling : {}", self.server_url);
            println!("=======================================================\n");
        }
    }

    /// Continuously maintains connection to the signaling server with exponential backoff.
    pub async fn run_with_retry(&self) {
        let mut retry_secs = 2u64;
        loop {
            let started = std::time::Instant::now();
            match self.run().await {
                Ok(_) => {
                    info!("Signaling session closed. Reconnecting in 2s...");
                    // A long-lived session that ended resets the backoff; a flapping one does not.
                    if started.elapsed() > Duration::from_secs(30) {
                        retry_secs = 2;
                    }
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    drop(e);
                    warn!("Signaling connection error ({}). Retrying in {}s...", err_msg, retry_secs);
                    tokio::time::sleep(Duration::from_secs(retry_secs)).await;
                    retry_secs = (retry_secs * 2).min(30);
                }
            }
        }
    }

    /// Connects to the signaling server and maintains the session.
    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ws_url = build_ws_url(&self.server_url);
        info!("Connecting to signaling server at: {}", ws_url);

        let (ws_stream, _) = if let Some(rest) = ws_url.strip_prefix("ws://") {
            let addr = rest.trim_end_matches("/ws");
            if !addr.contains('/') {
                let tcp = tokio::net::TcpStream::connect(addr).await?;
                let _ = tcp.set_nodelay(true);
                // Keep the send/recv buffers small: a large send buffer hides stale video frames inside the OS.
                let sock = socket2::SockRef::from(&tcp);
                let _ = sock.set_send_buffer_size(128 * 1024);
                let _ = sock.set_recv_buffer_size(256 * 1024);
                tokio_tungstenite::client_async(&ws_url, tokio_tungstenite::MaybeTlsStream::Plain(tcp)).await?
            } else {
                connect_async(&ws_url).await?
            }
        } else {
            connect_async(&ws_url).await?
        };
        info!("Connected to signaling WebSocket!");

        let (write, mut read) = ws_stream.split();
        let (ctrl_tx, ctrl_rx) = mpsc::channel::<Message>(64);
        let (audio_tx, audio_rx) = mpsc::channel::<Vec<u8>>(16);
        let (frame_tx, frame_rx) = tokio::sync::watch::channel::<Option<Vec<u8>>>(None);
        let writer = spawn_writer(write, ctrl_rx, audio_rx, frame_rx);

        let reg_msg = SignalingEnvelope {
            msg_type: "register".to_string(),
            role: Some("host".to_string()),
            device_id: Some(self.device_id.clone()),
            token: Some(self.token.clone()),
            payload: None,
            error: None,
        };
        let _ = ctrl_tx.send(Message::Text(serde_json::to_string(&reg_msg)?)).await;
        info!("Registered host '{}' with signaling server", self.device_id);

        let state = StreamState::new(false);
        let mut session = CloudSession { guard: None, capture: None };
        let mut ctx = SessionCtx::new(
            self.simulator.clone(),
            self.privacy.clone(),
            state.clone(),
            ctrl_tx.clone(),
            audio_tx,
        );

        // Creates the guard + capture worker once per attached viewer.
        let ensure_session = |session: &mut CloudSession| {
            if session.guard.is_none() {
                info!("Viewer attached through relay");
                state.stream_enabled.store(true, Ordering::SeqCst);
                session.guard = Some(new_session_guard(self.privacy.clone(), self.simulator.clone()));
            }
            if session.capture.is_none() {
                session.capture = Some(spawn_capture_worker(state.clone(), frame_tx.clone()));
            }
        };

        let mut result: Result<(), Box<dyn std::error::Error + Send + Sync>> = Ok(());
        let mut idle_tick = tokio::time::interval(Duration::from_secs(15));
        idle_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut last_rx = tokio::time::Instant::now();
        loop {
            let msg = tokio::select! {
                m = read.next() => match m {
                    Some(m) => {
                        last_rx = tokio::time::Instant::now();
                        m
                    }
                    None => break,
                },
                _ = idle_tick.tick() => {
                    if last_rx.elapsed() >= READ_TIMEOUT {
                        warn!("Signaling connection timed out (no traffic for {:?})", READ_TIMEOUT);
                        break;
                    }
                    continue;
                }
            };
            match msg {
                Ok(Message::Text(text)) => {
                    if let Ok(env) = serde_json::from_str::<SignalingEnvelope>(&text) {
                        match env.msg_type.as_str() {
                            "status" => match status_of(&env.payload).as_str() {
                                "client_joined" => {
                                    info!("Mobile client paired and connected!");
                                    ensure_session(&mut session);
                                    let _ = ctrl_tx.try_send(host_info_message(&self.simulator));
                                }
                                "client_left" => {
                                    info!("Last mobile client disconnected. Restoring host screen state...");
                                    state.stream_enabled.store(false, Ordering::SeqCst);
                                    session.guard = None; // cleanup runs if no other session is active
                                }
                                _ => {}
                            },
                            "error" => warn!("Signaling error: {:?}", env.error),
                            _ => {}
                        }
                    }
                }
                Ok(Message::Binary(bin)) => {
                    if let Ok(packet) = InputPacket::parse(&bin) {
                        // Older relays never announce the viewer; first valid input implies one.
                        ensure_session(&mut session);
                        ctx.handle(packet);
                    }
                }
                Ok(Message::Ping(p)) => {
                    let _ = ctrl_tx.try_send(Message::Pong(p));
                }
                Ok(Message::Close(_)) => {
                    info!("Signaling connection closed by server");
                    break;
                }
                Err(e) => {
                    error!("WebSocket stream error: {}", e);
                    result = Err(Box::new(e));
                    break;
                }
                _ => {}
            }
        }

        // Only this connection's own session is torn down. If a local Wi-Fi session is active,
        // SessionGuard keeps the privacy curtain / display state intact.
        state.conn_alive.store(false, Ordering::SeqCst);
        drop(ctx);
        session.guard = None;
        writer.abort();
        if let Some(h) = session.capture.take() {
            let _ = tokio::time::timeout(Duration::from_secs(2), h).await;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws_url_normalization() {
        assert_eq!(build_ws_url("51.79.249.143:8080"), "ws://51.79.249.143:8080/ws");
        assert_eq!(build_ws_url("51.79.249.143:8080/"), "ws://51.79.249.143:8080/ws");
        assert_eq!(build_ws_url("ws://h:1/ws"), "ws://h:1/ws");
        assert_eq!(build_ws_url("http://h"), "ws://h/ws");
        assert_eq!(build_ws_url("https://h/"), "wss://h/ws");
        assert_eq!(build_ws_url(" wss://h/ws/ "), "wss://h/ws");
    }

    #[test]
    fn status_parsing_handles_object_and_string() {
        assert_eq!(status_of(&Some(serde_json::json!({"status":"client_joined"}))), "client_joined");
        assert_eq!(status_of(&Some(serde_json::json!("paired"))), "paired");
        assert_eq!(status_of(&None), "");
    }
}
