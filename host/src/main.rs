mod capture;
mod discovery;
mod input;
mod local_server;
mod power;
mod privacy;
mod protocol;
mod signaling;

use std::env;
use std::sync::Arc;
use capture::ScreenCapture;
use discovery::DiscoveryService;
use local_server::LocalServer;
use power::PowerManager;
use privacy::PrivacyCurtain;
use signaling::HostSignaling;
use tracing::info;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    info!("=======================================================");
    info!("          SWAY REMOTE HOST DAEMON (WINDOWS)            ");
    info!("=======================================================");

    // Read environment or command-line args for configuration
    let signaling_url = env::var("SWAY_SIGNALING_URL")
        .unwrap_or_else(|_| "127.0.0.1:8080".to_string());

    let device_id = env::var("SWAY_DEVICE_ID")
        .unwrap_or_else(|_| format!("sway-laptop-{}", &Uuid::new_v4().to_string()[..8]));

    let token = env::var("SWAY_TOKEN")
        .unwrap_or_else(|_| Uuid::new_v4().to_string()[..12].to_string());

    let local_port = env::var("SWAY_LOCAL_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8089);

    // Initialize screen capture and detect monitors
    let capture = ScreenCapture::new();
    info!("Initialized capture for {} display(s)", capture.monitors.len());

    // Initialize privacy curtain manager
    let privacy = Arc::new(PrivacyCurtain::new());

    // Initialize signaling manager
    let host = HostSignaling::new(signaling_url, device_id.clone(), token.clone(), privacy.clone());

    // Render pairing QR code to terminal
    host.print_pairing_qr();

    // Start direct local Wi-Fi / LAN WebSocket & Web Control Center on port 8089
    LocalServer::start(local_port, host.simulator.clone(), privacy.clone());

    // Start UDP auto-discovery service on port 8088
    DiscoveryService::start(device_id.clone(), token.clone(), local_port);

    // Spawn signaling background worker
    tokio::select! {
        res = host.run() => {
            if let Err(e) = res {
                tracing::error!("Signaling runner terminated: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal (Ctrl+C). Restoring power policies and exiting...");
            privacy.set_enabled(false);
            PowerManager::set_keep_awake(false);
        }
    }

    info!("Sway Host Daemon shut down cleanly.");
    Ok(())
}
