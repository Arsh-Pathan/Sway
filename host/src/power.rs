use tracing::info;

#[cfg(windows)]
use windows::Win32::System::Power::{
    SetThreadExecutionState, ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED,
};

pub struct PowerManager;

impl PowerManager {
    /// Prevents or permits Windows laptop sleep during active sessions.
    pub fn set_keep_awake(enabled: bool) {
        #[cfg(windows)]
        unsafe {
            if enabled {
                info!("[PowerManager] Preventing laptop sleep (ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED)");
                SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED);
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
}
