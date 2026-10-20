mod capture;
mod input;
mod power;
mod protocol;
mod signaling;

use std::env;
use capture::ScreenCapture;
use power::PowerManager;
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

    // Initialize screen capture and detect monitors
    let capture = ScreenCapture::new();
    info!("Initialized capture for {} display(s)", capture.monitors.len());

    // Initialize signaling manager
    let host = HostSignaling::new(signaling_url, device_id, token);

    // Render pairing QR code to terminal
    host.print_pairing_qr();

    // Spawn signaling background worker
    tokio::select! {
        res = host.run() => {
            if let Err(e) = res {
                tracing::error!("Signaling runner terminated: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal (Ctrl+C). Restoring power policies and exiting...");
            PowerManager::set_keep_awake(false);
        }
    }

    info!("Sway Host Daemon shut down cleanly.");
    Ok(())
}
