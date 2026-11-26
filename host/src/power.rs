use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, LazyLock, Mutex};
use tracing::info;

#[cfg(windows)]
use windows::Win32::System::Power::{
    SetThreadExecutionState, ES_AWAYMODE_REQUIRED, ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED,
};

#[derive(Clone, Copy, Debug)]
enum PowerState {
    /// Headless 24/7 mode: CPU + network stay up, display may sleep.
    HostAlwaysAwake,
    /// Active remote session: additionally keep the display on.
    ActiveSession,
    /// Release every requirement (shutdown).
    Disabled,
}

/// `SetThreadExecutionState` is tracked per OS thread, and Tokio migrates tasks between
/// workers. All calls therefore go through one dedicated long-lived thread so a request
/// made on one worker can never be orphaned (and leak ES_DISPLAY_REQUIRED) on another.
static POWER_TX: LazyLock<Mutex<mpsc::Sender<PowerState>>> = LazyLock::new(|| {
    let (tx, rx) = mpsc::channel::<PowerState>();
    std::thread::Builder::new()
        .name("sway-power".into())
        .spawn(move || {
            for state in rx {
                #[cfg(windows)]
                unsafe {
                    match state {
                        PowerState::HostAlwaysAwake => {
                            SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_AWAYMODE_REQUIRED);
                        }
                        PowerState::ActiveSession => {
                            SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED);
                        }
                        PowerState::Disabled => {
                            SetThreadExecutionState(ES_CONTINUOUS);
                        }
                    }
                }
                #[cfg(not(windows))]
                let _ = state;
            }
        })
        .expect("failed to spawn power thread");
    Mutex::new(tx)
});

static ACTIVE_SESSIONS: AtomicUsize = AtomicUsize::new(0);

fn send(state: PowerState) {
    if let Ok(tx) = POWER_TX.lock() {
        let _ = tx.send(state);
    }
}

pub struct PowerManager;

impl PowerManager {
    /// Keeps Windows host alive in background server mode so the laptop CPU and network
    /// connection stay active 24/7 even if the display turns off or the lid closes.
    pub fn set_host_always_awake(enabled: bool) {
        if enabled {
            info!("[PowerManager] Enabling 24/7 background host stay-awake policy");
            send(PowerState::HostAlwaysAwake);
        } else {
            info!("[PowerManager] Restoring standard Windows sleep policy");
            send(PowerState::Disabled);
        }
    }

    #[cfg(test)]
    pub fn active_sessions() -> usize {
        ACTIVE_SESSIONS.load(Ordering::SeqCst)
    }

    /// Releases all execution-state requirements (process shutdown / panic).
    pub fn shutdown() {
        send(PowerState::Disabled);
    }
}

/// RAII handle for one authenticated controller session (local Wi-Fi or cloud relay).
/// The display-keep-awake request and the `on_last_leave` cleanup (privacy curtain,
/// display mode, held input) only run when the *last* active session ends, so one
/// transport disconnecting never tears down another transport's live session.
pub struct SessionGuard {
    on_last_leave: Option<Box<dyn FnOnce() + Send>>,
}

impl SessionGuard {
    pub fn acquire(on_last_leave: impl FnOnce() + Send + 'static) -> Self {
        if ACTIVE_SESSIONS.fetch_add(1, Ordering::SeqCst) == 0 {
            info!("[PowerManager] Active session: preventing display sleep");
            send(PowerState::ActiveSession);
        }
        Self { on_last_leave: Some(Box::new(on_last_leave)) }
    }
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        if ACTIVE_SESSIONS.fetch_sub(1, Ordering::SeqCst) == 1 {
            info!("[PowerManager] Last session ended: restoring background policy");
            send(PowerState::HostAlwaysAwake);
            if let Some(cleanup) = self.on_last_leave.take() {
                cleanup();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[test]
    fn session_guard_runs_cleanup_only_on_last_drop() {
        let ran1 = Arc::new(AtomicBool::new(false));
        let ran2 = Arc::new(AtomicBool::new(false));
        let r1 = ran1.clone();
        let r2 = ran2.clone();

        let g1 = SessionGuard::acquire(move || r1.store(true, Ordering::SeqCst));
        assert_eq!(PowerManager::active_sessions(), 1);
        let g2 = SessionGuard::acquire(move || r2.store(true, Ordering::SeqCst));
        assert_eq!(PowerManager::active_sessions(), 2);

        drop(g1);
        assert_eq!(PowerManager::active_sessions(), 1);
        assert!(!ran1.load(Ordering::SeqCst));

        drop(g2);
        assert_eq!(PowerManager::active_sessions(), 0);
        assert!(ran2.load(Ordering::SeqCst));
    }
}
