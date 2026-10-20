use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::sync::Arc;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::protocol::Message;
use tracing::{error, info, warn};

use crate::capture::ScreenCapture;
use crate::input::InputSimulator;
use crate::power::PowerManager;
use crate::privacy::PrivacyCurtain;
use crate::protocol::InputPacket;

pub struct LocalServer;

impl LocalServer {
    pub fn start(
        port: u16,
        simulator: Arc<InputSimulator>,
        privacy: Arc<PrivacyCurtain>,
    ) {
        tokio::spawn(async move {
            let addr = format!("0.0.0.0:{}", port);
            let listener = match TcpListener::bind(&addr).await {
                Ok(l) => {
                    info!("Direct Local Wi-Fi / LAN server listening on: ws://{}/ws", addr);
                    l
                }
                Err(e) => {
                    warn!("Failed to bind local server on {}: {}", addr, e);
                    return;
                }
            };

            while let Ok((mut stream, peer_addr)) = listener.accept().await {
                let _ = stream.set_nodelay(true);
                let sim = simulator.clone();
                let priv_curtain = privacy.clone();

                tokio::spawn(async move {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};

                    let mut peek_buf = [0u8; 1024];
                    let n = match tokio::time::timeout(std::time::Duration::from_secs(5), stream.peek(&mut peek_buf)).await {
                        Ok(Ok(n)) if n > 0 => n,
                        _ => return,
                    };
                    let req_head = String::from_utf8_lossy(&peek_buf[..n]);

                    // If this is a web browser request, serve Desktop Control Center GUI
                    if !req_head.to_lowercase().contains("upgrade: websocket") {
                        let mut buf = vec![0u8; 4096];
                        let n = stream.read(&mut buf).await.unwrap_or(0);
                        let req = String::from_utf8_lossy(&buf[..n]);

                        // CSRF / Cross-Origin Security Check for API endpoints
                        if req.contains("POST /api/") {
                            let lower = req.to_lowercase();
                            let is_cross_site = lower.contains("sec-fetch-site: cross-site");
                            let mut foreign_origin = false;
                            for line in lower.lines() {
                                if line.starts_with("origin:") || line.starts_with("referer:") {
                                    let v = line.trim();
                                    if !v.contains("localhost") && !v.contains("127.0.0.1") && !v.contains("192.168.") && !v.contains("10.") && !v.contains("172.") {
                                        foreign_origin = true;
                                        break;
                                    }
                                }
                            }

                            if is_cross_site || foreign_origin {
                                let forbidden = "HTTP/1.1 403 Forbidden\r\nContent-Type: text/plain\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n403 Forbidden: Cross-origin request rejected.";
                                let _ = stream.write_all(forbidden.as_bytes()).await;
                                return;
                            }
                        }

                        let is_get_apk = req.contains("GET /sway.apk") || req.contains("GET /app.apk");
                        let is_head_apk = req.contains("HEAD /sway.apk") || req.contains("HEAD /app.apk");

                        if is_get_apk || is_head_apk {
                            let apk_paths = [
                                "android/app/build/outputs/apk/debug/app-debug.apk",
                                "../android/app/build/outputs/apk/debug/app-debug.apk",
                                "app-debug.apk",
                                "sway.apk",
                            ];
                            for p in &apk_paths {
                                if let Ok(bytes) = tokio::fs::read(p).await {
                                    let header = format!(
                                        "HTTP/1.1 200 OK\r\nContent-Type: application/vnd.android.package-archive\r\nContent-Disposition: attachment; filename=\"Sway-Remote.apk\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                        bytes.len()
                                    );
                                    let _ = stream.write_all(header.as_bytes()).await;
                                    if is_get_apk {
                                        let _ = stream.write_all(&bytes).await;
                                    }
                                    return;
                                }
                            }
                            let not_found = "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nAPK not built yet. Run './gradlew assembleDebug' in android folder.";
                            let _ = stream.write_all(not_found.as_bytes()).await;
                            return;
                        } else if req.contains("POST /api/privacy") {
                            let enable = req.contains("\"enable\":true") || req.contains("\"enable\": true");
                            priv_curtain.set_enabled(enable);
                            let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 16\r\nConnection: close\r\n\r\n{\"success\":true}";
                            let _ = stream.write_all(resp.as_bytes()).await;
                            return;
                        } else if req.contains("POST /api/resolution") {
                            if req.contains("\"mode\":1") || req.contains("\"mode\": 1") {
                                InputSimulator::set_display_resolution(1);
                            } else {
                                InputSimulator::set_display_resolution(0);
                            }
                            let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 16\r\nConnection: close\r\n\r\n{\"success\":true}";
                            let _ = stream.write_all(resp.as_bytes()).await;
                            return;
                        } else if req.contains("POST /api/orientation") {
                            let vertical = req.contains("\"vertical\":true") || req.contains("\"vertical\": true");
                            InputSimulator::set_display_orientation(vertical);
                            let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 16\r\nConnection: close\r\n\r\n{\"success\":true}";
                            let _ = stream.write_all(resp.as_bytes()).await;
                            return;
                        } else if req.contains("POST /api/system") {
                            if req.contains("\"action\":1") || req.contains("\"action\": 1") {
                                InputSimulator::execute_system_command(1);
                            } else if req.contains("\"action\":2") || req.contains("\"action\": 2") {
                                InputSimulator::execute_system_command(2);
                            } else if req.contains("\"action\":3") || req.contains("\"action\": 3") {
                                InputSimulator::execute_system_command(3);
                            } else if req.contains("\"action\":4") || req.contains("\"action\": 4") {
                                InputSimulator::execute_system_command(4);
                            } else if req.contains("\"action\":5") || req.contains("\"action\": 5") {
                                InputSimulator::execute_system_command(5);
                            }
                            let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 16\r\nConnection: close\r\n\r\n{\"success\":true}";
                            let _ = stream.write_all(resp.as_bytes()).await;
                            return;
                        } else if req.contains("POST /api/clipboard") {
                            let full_req = req.to_string();
                            let sep = if full_req.contains("\r\n\r\n") {
                                Some("\r\n\r\n")
                            } else if full_req.contains("\n\n") {
                                Some("\n\n")
                            } else {
                                None
                            };

                            let body_str = if let Some(s) = sep {
                                let parts: Vec<&str> = full_req.splitn(2, s).collect();
                                if parts.len() > 1 && !parts[1].trim().is_empty() {
                                    parts[1].trim().to_string()
                                } else {
                                    // Body might be coming in next chunk
                                    let mut body_buf = vec![0u8; 2048];
                                    if let Ok(Ok(bn)) = tokio::time::timeout(std::time::Duration::from_millis(50), stream.read(&mut body_buf)).await {
                                        String::from_utf8_lossy(&body_buf[..bn]).trim().to_string()
                                    } else {
                                        String::new()
                                    }
                                }
                            } else {
                                String::new()
                            };

                            if !body_str.is_empty() {
                                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&body_str) {
                                    if let Some(text) = parsed.get("text").and_then(|t| t.as_str()) {
                                        InputSimulator::set_clipboard_text(text);
                                    }
                                }
                            }
                            let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 16\r\nConnection: close\r\n\r\n{\"success\":true}";
                            let _ = stream.write_all(resp.as_bytes()).await;
                            return;
                        }

                        let html = include_str!("dashboard.html");
                        let resp = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: SAMEORIGIN\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            html.len(),
                            html
                        );
                        let _ = stream.write_all(resp.as_bytes()).await;
                        return;
                    }

                    // WebSocket Connection
                    info!("Direct local Wi-Fi client connected from: {}", peer_addr);

                    let ws_stream = match accept_async(stream).await {
                        Ok(ws) => ws,
                        Err(e) => {
                            warn!("Local WebSocket handshake error from {}: {}", peer_addr, e);
                            return;
                        }
                    };

                    info!("Local direct WebSocket session established with {}", peer_addr);
                    PowerManager::set_keep_awake(true);
                    let (mut write, mut read) = ws_stream.split();
                    let (frame_tx, mut frame_rx) = mpsc::channel::<Message>(1);
                    let (ctrl_tx, mut ctrl_rx) = mpsc::channel::<Message>(32);

                    let writer_handle = tokio::spawn(async move {
                        loop {
                            tokio::select! {
                                biased;
                                Some(ctrl) = ctrl_rx.recv() => {
                                    if let Err(e) = write.send(ctrl).await {
                                        error!("Local WS write error: {}", e);
                                        break;
                                    }
                                }
                                Some(frame) = frame_rx.recv() => {
                                    if let Err(e) = write.send(frame).await {
                                        error!("Local WS frame write error: {}", e);
                                        break;
                                    }
                                }
                                else => break,
                            }
                        }
                    });

                    let is_streaming = Arc::new(AtomicBool::new(true));
                    let active_monitor = Arc::new(AtomicUsize::new(0));
                    let stream_quality = Arc::new(AtomicU8::new(45)); // Default to Fast 60 FPS on direct local connection

                    // Direct capture thread at ultra-fluid up to 60 FPS
                    let stream_flag = is_streaming.clone();
                    let monitor_flag = active_monitor.clone();
                    let quality_flag = stream_quality.clone();
                    let ft_tx = frame_tx.clone();

                    let capture_handle = tokio::task::spawn_blocking(move || {
                        let mut capture = ScreenCapture::new();
                        info!("Started direct local screen streaming loop (up to 60 FPS)");

                        while stream_flag.load(Ordering::Relaxed) {
                            let start = std::time::Instant::now();
                            capture.active_monitor = monitor_flag.load(Ordering::Relaxed);
                            let q = quality_flag.load(Ordering::Relaxed);

                            let target_interval = match q {
                                40..=45 => std::time::Duration::from_millis(16), // ~60 FPS Turbo Mode
                                70..=100 => std::time::Duration::from_millis(33), // ~30 FPS Fidelity Mode
                                _ => std::time::Duration::from_millis(20),        // ~50 FPS Balanced Mode
                            };

                            if let Some(frame) = capture.capture_frame_jpeg(q) {
                                match ft_tx.try_send(Message::Binary(frame)) {
                                    Ok(_) => {}
                                    Err(mpsc::error::TrySendError::Full(_)) => {}
                                    Err(mpsc::error::TrySendError::Closed(_)) => break,
                                }
                            }

                            let elapsed = start.elapsed();
                            if elapsed < target_interval {
                                std::thread::sleep(target_interval - elapsed);
                            }
                        }
                        info!("Direct local screen capture stopped");
                    });

                    while let Some(msg) = read.next().await {
                        match msg {
                            Ok(Message::Binary(bin)) => {
                                if let Ok(packet) = InputPacket::parse(&bin) {
                                    match &packet {
                                        InputPacket::SwitchMonitor { monitor_index } => {
                                            active_monitor.store(*monitor_index as usize, Ordering::SeqCst);
                                        }
                                        InputPacket::SetQuality { mode } => {
                                            let q = match mode {
                                                0 => 40,
                                                1 => 50,
                                                _ => 70,
                                            };
                                            stream_quality.store(q, Ordering::SeqCst);
                                        }
                                        InputPacket::SetPrivacyCurtain { enabled } => {
                                            priv_curtain.set_enabled(*enabled);
                                        }
                                        InputPacket::ChangeResolution { mode } => {
                                            InputSimulator::set_display_resolution(*mode);
                                        }
                                        InputPacket::ChangeOrientation { vertical } => {
                                            InputSimulator::set_display_orientation(*vertical);
                                        }
                                        _ => {}
                                    }
                                    sim.dispatch(packet);
                                }
                            }
                            Ok(Message::Ping(p)) => {
                                let _ = ctrl_tx.send(Message::Pong(p)).await;
                            }
                            Ok(Message::Text(t)) => {
                                if t.contains("\"ping\"") {
                                    let pong = t.replace("\"ping\"", "\"pong\"");
                                    let _ = ctrl_tx.send(Message::Text(pong)).await;
                                }
                            }
                            Ok(Message::Close(_)) => {
                                info!("Local client disconnected");
                                break;
                            }
                            Err(e) => {
                                warn!("Local client stream error: {}", e);
                                break;
                            }
                            _ => {}
                        }
                    }

                    is_streaming.store(false, Ordering::SeqCst);
                    priv_curtain.set_enabled(false);
                    InputSimulator::set_display_resolution(0);
                    InputSimulator::set_display_orientation(false);
                    PowerManager::set_keep_awake(false);
                    writer_handle.abort();
                    let _ = capture_handle.await;
                    info!("Local direct session cleaned up, restored screen settings");
                });
            }
        });
    }
}
