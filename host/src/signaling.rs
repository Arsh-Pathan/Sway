use std::sync::Arc;
use futures_util::{SinkExt, StreamExt};
use qrcode::render::unicode;
use qrcode::QrCode;
use serde::{Deserialize, Serialize};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::{error, info, warn};

use crate::input::InputSimulator;
use crate::power::PowerManager;
use crate::protocol::InputPacket;

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
}

impl HostSignaling {
    pub fn new(server_url: String, device_id: String, token: String) -> Self {
        Self {
            server_url,
            device_id,
            token,
            simulator: Arc::new(InputSimulator::new()),
        }
    }

    /// Prints a scannable QR code directly to the console for pairing.
    pub fn print_pairing_qr(&self) {
        let pairing_payload = serde_json::json!({
            "server": self.server_url,
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

    /// Connects to the signaling server and maintains the session.
    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let ws_url = if self.server_url.starts_with("http://") {
            format!("{}/ws", self.server_url.replace("http://", "ws://"))
        } else if self.server_url.starts_with("https://") {
            format!("{}/ws", self.server_url.replace("https://", "wss://"))
        } else if !self.server_url.starts_with("ws://") && !self.server_url.starts_with("wss://") {
            format!("ws://{}/ws", self.server_url)
        } else {
            format!("{}/ws", self.server_url)
        };

        info!("Connecting to signaling server at: {}", ws_url);
        let (ws_stream, _) = connect_async(&ws_url).await?;
        info!("Connected to signaling WebSocket!");

        let (mut write, mut read) = ws_stream.split();

        // Send registration message
        let reg_msg = SignalingEnvelope {
            msg_type: "register".to_string(),
            role: Some("host".to_string()),
            device_id: Some(self.device_id.clone()),
            token: Some(self.token.clone()),
            payload: None,
            error: None,
        };

        let raw = serde_json::to_string(&reg_msg)?;
        write.send(Message::Text(raw)).await?;
        info!("Registered host '{}' with signaling server", self.device_id);

        let simulator = self.simulator.clone();

        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Ok(env) = serde_json::from_str::<SignalingEnvelope>(&text) {
                        match env.msg_type.as_str() {
                            "status" => {
                                if let Some(p) = env.payload {
                                    if p.to_string().contains("client_joined") {
                                        info!("Mobile client paired and connected!");
                                        PowerManager::set_keep_awake(true);
                                    }
                                }
                            }
                            "offer" => {
                                info!("Received WebRTC offer from mobile client");
                            }
                            "answer" => {
                                info!("Received WebRTC answer from mobile client");
                            }
                            "candidate" => {
                                info!("Received remote ICE candidate");
                            }
                            "error" => {
                                warn!("Signaling error: {:?}", env.error);
                            }
                            _ => {}
                        }
                    }
                }
                Ok(Message::Binary(bin)) => {
                    // Direct input packet forwarded over WebSocket or DataChannel
                    if let Ok(packet) = InputPacket::parse(&bin) {
                        simulator.dispatch(packet);
                    }
                }
                Ok(Message::Ping(p)) => {
                    let _ = write.send(Message::Pong(p)).await;
                }
                Ok(Message::Close(_)) => {
                    info!("Signaling connection closed by server");
                    PowerManager::set_keep_awake(false);
                    break;
                }
                Err(e) => {
                    error!("WebSocket stream error: {}", e);
                    PowerManager::set_keep_awake(false);
                    break;
                }
                _ => {}
            }
        }

        Ok(())
    }
}
