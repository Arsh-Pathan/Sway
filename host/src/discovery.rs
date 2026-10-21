use tokio::net::UdpSocket;
use tracing::{info, warn};

pub struct DiscoveryService;

impl DiscoveryService {
    pub fn start(device_id: String, token: String, local_port: u16) {
        tokio::spawn(async move {
            let socket = match UdpSocket::bind("0.0.0.0:8088").await {
                Ok(s) => {
                    info!("UDP Auto-Discovery service running on port 8088");
                    s
                }
                Err(e) => {
                    warn!("UDP discovery socket bind error: {}", e);
                    return;
                }
            };

            let mut buf = [0u8; 1024];
            loop {
                match socket.recv_from(&mut buf).await {
                    Ok((len, peer_addr)) => {
                        let msg = String::from_utf8_lossy(&buf[..len]);
                        if msg.contains("SWAY_DISCOVER") {
                            info!("Received UDP discovery ping from phone at: {}", peer_addr);
                            let computer_name = std::env::var("COMPUTERNAME")
                                .unwrap_or_else(|_| "Windows Laptop".to_string());

                            let host_ip = match std::net::UdpSocket::bind("0.0.0.0:0") {
                                Ok(s) => match s.connect(peer_addr) {
                                    Ok(_) => s.local_addr().ok().map(|a| a.ip().to_string()),
                                    Err(_) => None,
                                },
                                Err(_) => None,
                            };

                            let response = serde_json::json!({
                                "type": "sway_beacon",
                                "name": format!("Sway PC ({})", computer_name),
                                "deviceId": device_id,
                                "token": token,
                                "port": local_port,
                                "ip": host_ip,
                            });

                            let raw = response.to_string();
                            let _ = socket.send_to(raw.as_bytes(), peer_addr).await;
                        }
                    }
                    Err(e) => {
                        warn!("UDP discovery recv error: {}", e);
                        break;
                    }
                }
            }
        });
    }
}
