use tokio::net::UdpSocket;
use tracing::{info, warn};

pub struct DiscoveryService;

impl DiscoveryService {
    /// Answers LAN `SWAY_DISCOVER` probes with the host's name, id, IP and port.
    /// The pairing token is deliberately NOT broadcast: anyone on the Wi-Fi could read it.
    /// Phones obtain the token once via QR / manual entry and then use discovery only to
    /// resolve the host's current LAN address.
    pub fn start(device_id: String, local_port: u16) {
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

            let computer_name = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Windows Laptop".to_string());

            let mut buf = [0u8; 1024];
            loop {
                match socket.recv_from(&mut buf).await {
                    Ok((len, peer_addr)) => {
                        let msg = String::from_utf8_lossy(&buf[..len]);
                        if msg.contains("SWAY_DISCOVER") {
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
                                "port": local_port,
                                "ip": host_ip,
                            });

                            let _ = socket.send_to(response.to_string().as_bytes(), peer_addr).await;
                        }
                    }
                    Err(e) => {
                        // On Windows an ICMP port-unreachable from a phone that already closed its
                        // socket surfaces here as WSAECONNRESET. It is transient: keep serving.
                        warn!("UDP discovery recv error (continuing): {}", e);
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    }
                }
            }
        });
    }
}
