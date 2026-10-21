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
use input::InputSimulator;
use local_server::LocalServer;
use power::PowerManager;
use privacy::PrivacyCurtain;
use signaling::HostSignaling;
use tracing::info;
use uuid::Uuid;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct HostConfig {
    pub signaling_url: String,
    pub device_id: String,
    pub token: String,
    pub local_port: u16,
}

impl HostConfig {
    pub fn load_or_create() -> Self {
        let config_dir = if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            std::path::PathBuf::from(local_app_data).join("Sway")
        } else {
            std::path::PathBuf::from(".")
        };
        let config_path = config_dir.join("config.json");

        let mut config = if config_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                serde_json::from_str::<HostConfig>(&content).unwrap_or_else(|_| Self::default_config())
            } else {
                Self::default_config()
            }
        } else {
            let def = Self::default_config();
            let _ = std::fs::create_dir_all(&config_dir);
            if let Ok(content) = serde_json::to_string_pretty(&def) {
                let _ = std::fs::write(&config_path, content);
            }
            def
        };

        // Environment overrides
        if let Ok(url) = env::var("SWAY_SIGNALING_URL") {
            config.signaling_url = url;
        }
        if let Ok(id) = env::var("SWAY_DEVICE_ID") {
            config.device_id = id;
        }
        if let Ok(t) = env::var("SWAY_TOKEN") {
            config.token = t;
        }
        if let Ok(p) = env::var("SWAY_LOCAL_PORT") {
            if let Ok(port) = p.parse::<u16>() {
                config.local_port = port;
            }
        }

        config
    }

    fn default_config() -> Self {
        Self {
            signaling_url: "127.0.0.1:8080".to_string(),
            device_id: format!("sway-laptop-{}", &Uuid::new_v4().to_string()[..8]),
            token: Uuid::new_v4().to_string()[..12].to_string(),
            local_port: 8089,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // Emergency panic safety hook to guarantee display restoration
    let orig_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        InputSimulator::set_display_resolution(0);
        InputSimulator::set_display_orientation(false);
        PowerManager::set_keep_awake(false);
        orig_hook(panic_info);
    }));

    info!("=======================================================");
    info!("          SWAY REMOTE HOST DAEMON (WINDOWS)            ");
    info!("=======================================================");

    // Read config from file or environment overrides
    let config = HostConfig::load_or_create();
    let signaling_url = config.signaling_url;
    let device_id = config.device_id;
    let token = config.token;
    let local_port = config.local_port;

    // Initialize screen capture and detect monitors
    let capture = ScreenCapture::new();
    info!("Initialized capture for {} display(s)", capture.monitors.len());

    // Initialize privacy curtain manager
    let privacy = Arc::new(PrivacyCurtain::new());

    // Initialize signaling manager
    let host = Arc::new(HostSignaling::new(signaling_url, device_id.clone(), token.clone(), privacy.clone()));

    // Render pairing QR code to terminal
    host.print_pairing_qr();

    // Start direct local Wi-Fi / LAN WebSocket & Web Control Center on port 8089
    LocalServer::start(local_port, host.simulator.clone(), privacy.clone());

    // Start UDP auto-discovery service on port 8088
    DiscoveryService::start(device_id.clone(), token.clone(), local_port);

    // Spawn signaling background worker with automatic reconnection
    let host_signaling = host.clone();
    let signaling_handle = tokio::spawn(async move {
        host_signaling.run_with_retry().await;
    });

    // Wait for shutdown signal
    match tokio::signal::ctrl_c().await {
        Ok(()) => {
            info!("Received shutdown signal (Ctrl+C). Restoring display settings and power policies...");
        }
        Err(e) => {
            tracing::warn!("Ctrl+C handler unavailable (background daemon mode): {}. Keeping daemon alive.", e);
            std::future::pending::<()>().await;
        }
    }

    signaling_handle.abort();
    privacy.set_enabled(false);
    InputSimulator::set_display_resolution(0);
    InputSimulator::set_display_orientation(false);
    PowerManager::set_keep_awake(false);

    info!("Sway Host Daemon shut down cleanly.");
    Ok(())
}
