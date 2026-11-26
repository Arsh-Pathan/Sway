#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod capture;
mod discovery;
mod input;
mod local_server;
mod power;
mod privacy;
mod protocol;
mod session;
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
use tracing::{info, warn};
use uuid::Uuid;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct HostConfig {
    pub signaling_url: String,
    pub device_id: String,
    pub token: String,
    pub local_port: u16,
    #[serde(default)]
    pub pin: Option<String>,
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
            match std::fs::read_to_string(&config_path).map_err(|e| e.to_string()).and_then(|c| Self::parse(&c).map_err(|e| e.to_string())) {
                Ok(c) => c,
                Err(e) => {
                    // Never silently roll a new identity on every boot: keep the bad file and persist a fresh one.
                    warn!("config.json unreadable ({}); backing up and creating a new one", e);
                    let _ = std::fs::copy(&config_path, config_dir.join("config.json.bad"));
                    let def = Self::default_config();
                    if let Ok(content) = serde_json::to_string_pretty(&def) {
                        let _ = std::fs::write(&config_path, content);
                    }
                    def
                }
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
        if let Ok(p) = env::var("SWAY_PIN") {
            config.pin = Some(p);
        }
        if config.pin.is_none() {
            config.pin = Some("1234".to_string());
        }
        if let Ok(p) = env::var("SWAY_LOCAL_PORT") {
            if let Ok(port) = p.parse::<u16>() {
                config.local_port = port;
            }
        }

        config
    }

    /// Parses config JSON, tolerating a UTF-8 BOM (Windows PowerShell 5.1 writes one).
    fn parse(content: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(content.trim_start_matches('\u{feff}'))
    }

    fn default_config() -> Self {
        Self {
            signaling_url: "51.79.249.143:8080".to_string(),
            device_id: format!("sway-laptop-{}", &Uuid::new_v4().simple().to_string()[..8]),
            token: Uuid::new_v4().simple().to_string()[..16].to_string(),
            local_port: 8089,
            pin: Some("1234".to_string()),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    unsafe {
        // If launched from an interactive terminal (CMD / PowerShell), attach to its console for logging.
        // If launched by Windows Autostart or desktop shortcut, AttachConsole returns 0 and NO console window is created.
        extern "system" {
            fn AttachConsole(dwProcessId: u32) -> i32;
        }
        let _ = AttachConsole(0xFFFFFFFF);

        use windows::Win32::UI::HiDpi::{
            SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
            DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE,
        };
        // Enable Per-Monitor DPI Awareness V2 so Windows never virtualizes coordinates
        // regardless of display scaling (100%, 125%, 150%, 175%, 200%).
        if SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).is_err() {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE);
        }
    }

    tracing_subscriber::fmt::init();

    // Emergency panic safety hook: always give the user their screen, mouse, keyboard and power policy back
    let orig_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        PrivacyCurtain::emergency_restore();
        InputSimulator::restore_display_if_changed();
        PowerManager::shutdown();
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

    // Prevent Windows laptop from sleeping when idle or display turns off
    PowerManager::set_host_always_awake(true);

    // Detect monitors (cheap; no capture buffers are allocated here)
    info!("Detected {} display(s)", ScreenCapture::detect_monitors().len());

    // Initialize privacy curtain manager with laptop PIN and token
    let pin = config.pin.clone().unwrap_or_else(|| "1234".to_string());
    let privacy = Arc::new(PrivacyCurtain::new(pin, token.clone()));

    // Initialize signaling manager
    let host = Arc::new(HostSignaling::new(signaling_url, device_id.clone(), token.clone(), privacy.clone()));

    // Render pairing QR code to terminal
    host.print_pairing_qr();

    // Start direct local Wi-Fi / LAN WebSocket & Web Control Center on port 8089
    LocalServer::start(
        local_port,
        device_id.clone(),
        token.clone(),
        host.simulator.clone(),
        privacy.clone(),
    );

    // Start UDP auto-discovery service on port 8088 (never broadcasts the token)
    DiscoveryService::start(device_id.clone(), local_port);

    // Spawn signaling background worker with automatic reconnection
    let host_signaling = host.clone();
    let signaling_handle = tokio::spawn(async move {
        host_signaling.run_with_retry().await;
    });

    // Determine whether we are attached to an interactive console or running as a silent 24/7 background service
    #[cfg(windows)]
    extern "system" {
        fn GetConsoleWindow() -> isize;
    }
    #[cfg(windows)]
    let has_console = unsafe { GetConsoleWindow() != 0 };
    #[cfg(not(windows))]
    let has_console = true;

    if has_console {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                info!("Received shutdown signal (Ctrl+C). Restoring display settings and power policies...");
            }
            Err(_) => {
                std::future::pending::<()>().await;
            }
        }
    } else {
        info!("Running as headless 24/7 background service. Keeping daemon alive permanently.");
        std::future::pending::<()>().await;
    }

    signaling_handle.abort();
    privacy.set_enabled(false);
    privacy.shutdown();
    PrivacyCurtain::emergency_restore();
    host.simulator.release_all();
    InputSimulator::restore_display_if_changed();
    PowerManager::shutdown();

    info!("Sway Host Daemon shut down cleanly.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_parses_with_utf8_bom() {
        let json = "\u{feff}{\"signaling_url\":\"1.2.3.4:8080\",\"device_id\":\"dev\",\"token\":\"tok\",\"local_port\":8089}";
        let cfg = HostConfig::parse(json).expect("BOM config must parse");
        assert_eq!(cfg.device_id, "dev");
        assert_eq!(cfg.token, "tok");
    }

    #[test]
    fn default_config_has_strong_token() {
        let cfg = HostConfig::default_config();
        assert_eq!(cfg.token.len(), 16);
        assert!(cfg.device_id.starts_with("sway-laptop-"));
    }
}
