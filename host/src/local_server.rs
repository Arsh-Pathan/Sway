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
                let mut peek_buf = [0u8; 1024];
                let n = match stream.peek(&mut peek_buf).await {
                    Ok(n) => n,
                    Err(_) => continue,
                };
                let req_head = String::from_utf8_lossy(&peek_buf[..n]);

                // If this is a web browser request, serve Desktop Control Center GUI
                if !req_head.to_lowercase().contains("upgrade: websocket") {
                    let priv_curtain = privacy.clone();
                    tokio::spawn(async move {
                        use tokio::io::{AsyncReadExt, AsyncWriteExt};
                        let mut buf = vec![0u8; 2048];
                        let _ = stream.read(&mut buf).await;
                        let req = String::from_utf8_lossy(&buf);

                        if req.contains("POST /api/privacy") {
                            let enable = req.contains("\"enable\":true") || req.contains("\"enable\": true");
                            priv_curtain.set_enabled(enable);
                            let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"success\":true}";
                            let _ = stream.write_all(resp.as_bytes()).await;
                            return;
                        } else if req.contains("POST /api/resolution") {
                            if req.contains("\"mode\":1") || req.contains("\"mode\": 1") {
                                InputSimulator::set_display_resolution(1);
                            } else {
                                InputSimulator::set_display_resolution(0);
                            }
                            let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"success\":true}";
                            let _ = stream.write_all(resp.as_bytes()).await;
                            return;
                        } else if req.contains("POST /api/orientation") {
                            let vertical = req.contains("\"vertical\":true") || req.contains("\"vertical\": true");
                            InputSimulator::set_display_orientation(vertical);
                            let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"success\":true}";
                            let _ = stream.write_all(resp.as_bytes()).await;
                            return;
                        }

                        let html = include_str!("dashboard.html");
                        let resp = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            html.len(),
                            html
                        );
                        let _ = stream.write_all(resp.as_bytes()).await;
                    });
                    continue;
                }

                info!("Direct local Wi-Fi client connected from: {}", peer_addr);
                let sim = simulator.clone();
                let priv_curtain = privacy.clone();

                tokio::spawn(async move {
                    let ws_stream = match accept_async(stream).await {
                        Ok(ws) => ws,
                        Err(e) => {
                            warn!("Local WebSocket handshake error from {}: {}", peer_addr, e);
                            return;
                        }
                    };

                    info!("Local direct WebSocket session established with {}", peer_addr);
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
                    let stream_quality = Arc::new(AtomicU8::new(50));

                    // Direct capture thread at ultra-fluid 45-60 FPS
                    let stream_flag = is_streaming.clone();
                    let monitor_flag = active_monitor.clone();
                    let quality_flag = stream_quality.clone();
                    let ft_tx = frame_tx.clone();

                    let capture_handle = tokio::task::spawn_blocking(move || {
                        let mut capture = ScreenCapture::new();
                        // 22ms = ~45 FPS for silky-smooth direct local Wi-Fi streaming
                        let target_interval = std::time::Duration::from_millis(22);
                        info!("Started direct local screen streaming loop at 45 FPS");

                        while stream_flag.load(Ordering::Relaxed) {
                            let start = std::time::Instant::now();
                            capture.active_monitor = monitor_flag.load(Ordering::Relaxed);
                            let q = quality_flag.load(Ordering::Relaxed);

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
                    writer_handle.abort();
                    let _ = capture_handle.await;
                    info!("Local direct session cleaned up, restored screen settings");
                });
            }
        });
    }
}
