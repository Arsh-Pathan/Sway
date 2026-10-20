use tracing::info;

#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub index: usize,
    pub name: String,
    pub width: i32,
    pub height: i32,
    pub is_primary: bool,
}

pub struct ScreenCapture {
    #[allow(dead_code)]
    pub active_monitor: usize,
    pub monitors: Vec<MonitorInfo>,
}

impl ScreenCapture {
    pub fn new() -> Self {
        let monitors = Self::detect_monitors();
        info!("Detected {} connected monitor(s)", monitors.len());
        for m in &monitors {
            info!("  Monitor #{}: {} ({}x{}) primary={}", m.index, m.name, m.width, m.height, m.is_primary);
        }

        Self {
            active_monitor: 0,
            monitors,
        }
    }

    #[allow(dead_code)]
    pub fn set_active_monitor(&mut self, index: usize) -> bool {
        if index < self.monitors.len() {
            self.active_monitor = index;
            info!("Switched active capture monitor to: #{}", index);
            true
        } else {
            false
        }
    }

    pub fn detect_monitors() -> Vec<MonitorInfo> {
        #[cfg(windows)]
        {
            use windows::Win32::UI::WindowsAndMessaging::{
                GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN,
            };

            unsafe {
                let width = GetSystemMetrics(SM_CXSCREEN);
                let height = GetSystemMetrics(SM_CYSCREEN);

                vec![MonitorInfo {
                    index: 0,
                    name: "Primary Display".to_string(),
                    width,
                    height,
                    is_primary: true,
                }]
            }
        }

        #[cfg(not(windows))]
        {
            vec![MonitorInfo {
                index: 0,
                name: "Virtual Display".to_string(),
                width: 1920,
                height: 1080,
                is_primary: true,
            }]
        }
    }
}
