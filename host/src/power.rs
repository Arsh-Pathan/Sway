use tracing::info;

#[cfg(windows)]
use windows::Win32::System::Power::{
    SetThreadExecutionState, ES_AWAYMODE_REQUIRED, ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED,
};

pub struct PowerManager;

impl PowerManager {
    /// Keeps Windows host alive in background server mode (ES_SYSTEM_REQUIRED | ES_AWAYMODE_REQUIRED)
    /// so the laptop CPU and network connection stay active 24/7 even if the display turns off or lid closes.
    pub fn set_host_always_awake(enabled: bool) {
        #[cfg(windows)]
        unsafe {
            if enabled {
                info!("[PowerManager] Enabling 24/7 background host stay-awake policy (ES_SYSTEM_REQUIRED | ES_AWAYMODE_REQUIRED)");
                SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_AWAYMODE_REQUIRED);
            } else {
                info!("[PowerManager] Restoring standard Windows sleep policy");
                SetThreadExecutionState(ES_CONTINUOUS);
            }
        }
        #[cfg(not(windows))]
        {
            let _ = enabled;
        }
    }

    /// Prevents or permits Windows laptop sleep during active streaming sessions.
    pub fn set_keep_awake(enabled: bool) {
        #[cfg(windows)]
        unsafe {
            if enabled {
                info!("[PowerManager] Active session: Preventing laptop sleep (ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED)");
                SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED);
            } else {
                info!("[PowerManager] Session idle: reverting to background stay-awake policy");
                Self::set_host_always_awake(true);
            }
        }

        #[cfg(not(windows))]
        {
            let _ = enabled;
        }
    }
}
