use std::sync::atomic::{AtomicU8, Ordering};
use tracing::{debug, info, warn};

#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN,
    MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE,
    MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEINPUT, MOUSE_EVENT_FLAGS,
    VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_SHIFT,
};

use crate::protocol::InputPacket;

pub struct InputSimulator {
    active_modifiers: AtomicU8,
}

impl InputSimulator {
    pub fn new() -> Self {
        Self {
            active_modifiers: AtomicU8::new(0),
        }
    }

    /// Dispatches an input packet to the native OS input subsystem.
    pub fn dispatch(&self, packet: InputPacket) {
        match packet {
            InputPacket::MouseMove { dx, dy } => self.mouse_move(dx, dy),
            InputPacket::MouseDown { button } => self.mouse_down(button),
            InputPacket::MouseUp { button } => self.mouse_up(button),
            InputPacket::MouseScroll { delta_y, delta_x } => self.mouse_scroll(delta_y, delta_x),
            InputPacket::KeyDown { vkey, modifiers } => self.key_down(vkey, modifiers),
            InputPacket::KeyUp { vkey } => self.key_up(vkey),
            InputPacket::TextInput { text } => self.text_input(&text),
            InputPacket::SwitchMonitor { monitor_index } => {
                info!("Monitor switch requested to display: #{}", monitor_index);
            }
            InputPacket::SetQuality { mode } => {
                info!("Stream quality change requested: mode #{}", mode);
            }
            InputPacket::TouchDirect { norm_x, norm_y, action } => {
                self.touch_direct(norm_x, norm_y, action);
            }
            InputPacket::SetPrivacyCurtain { enabled: _ } => {}
            InputPacket::ChangeResolution { mode } => {
                Self::set_display_resolution(mode);
            }
            InputPacket::ChangeOrientation { vertical } => {
                Self::set_display_orientation(vertical);
            }
            InputPacket::ClipboardSync { text } => {
                Self::set_clipboard_text(&text);
            }
            InputPacket::SystemCommand { action } => {
                Self::execute_system_command(action);
            }
        }
    }

    #[cfg(windows)]
    fn send_mouse_input(&self, flags: MOUSE_EVENT_FLAGS, dx: i32, dy: i32, mouse_data: u32) {
        unsafe {
            let input = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx,
                        dy,
                        mouseData: mouse_data,
                        dwFlags: flags,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };

            let sent = SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
            if sent == 0 {
                debug!("SendInput mouse failed");
            }
        }
    }

    #[cfg(windows)]
    fn send_key_input(&self, vkey: u16, flags: KEYBD_EVENT_FLAGS, scan: u16) {
        unsafe {
            let input = INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(vkey),
                        wScan: scan,
                        dwFlags: flags,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };

            let sent = SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
            if sent == 0 {
                debug!("SendInput keyboard failed");
            }
        }
    }

    #[cfg(not(windows))]
    fn send_mouse_input(&self, _flags: u32, _dx: i32, _dy: i32, _mouse_data: u32) {}

    #[cfg(not(windows))]
    fn send_key_input(&self, _vkey: u16, _flags: u32, _scan: u16) {}

    pub fn mouse_move(&self, dx: i16, dy: i16) {
        #[cfg(windows)]
        self.send_mouse_input(MOUSEEVENTF_MOVE, dx as i32, dy as i32, 0);
    }

    pub fn mouse_down(&self, button: u8) {
        #[cfg(windows)]
        {
            let flags = match button {
                1 => MOUSEEVENTF_LEFTDOWN,
                2 => MOUSEEVENTF_RIGHTDOWN,
                3 => MOUSEEVENTF_MIDDLEDOWN,
                _ => return,
            };
            self.send_mouse_input(flags, 0, 0, 0);
        }
    }

    pub fn mouse_up(&self, button: u8) {
        #[cfg(windows)]
        {
            let flags = match button {
                1 => MOUSEEVENTF_LEFTUP,
                2 => MOUSEEVENTF_RIGHTUP,
                3 => MOUSEEVENTF_MIDDLEUP,
                _ => return,
            };
            self.send_mouse_input(flags, 0, 0, 0);
        }
    }

    pub fn mouse_scroll(&self, delta_y: i16, delta_x: i16) {
        #[cfg(windows)]
        {
            if delta_y != 0 {
                // Vertical wheel (WHEEL_DELTA is 120 per click in Windows)
                self.send_mouse_input(MOUSEEVENTF_WHEEL, 0, 0, (delta_y as i32) as u32);
            }
            if delta_x != 0 {
                // Horizontal wheel
                self.send_mouse_input(MOUSEEVENTF_HWHEEL, 0, 0, (delta_x as i32) as u32);
            }
        }
    }

    pub fn key_down(&self, vkey: u16, modifiers: u8) {
        #[cfg(windows)]
        {
            if modifiers != 0 {
                self.active_modifiers.store(modifiers, Ordering::Relaxed);
                if modifiers & 0x01 != 0 {
                    self.send_key_input(VK_CONTROL.0, KEYBD_EVENT_FLAGS(0), 0);
                }
                if modifiers & 0x02 != 0 {
                    self.send_key_input(VK_SHIFT.0, KEYBD_EVENT_FLAGS(0), 0);
                }
                if modifiers & 0x04 != 0 {
                    self.send_key_input(VK_MENU.0, KEYBD_EVENT_FLAGS(0), 0);
                }
                if modifiers & 0x08 != 0 {
                    self.send_key_input(VK_LWIN.0, KEYBD_EVENT_FLAGS(0), 0);
                }
            }

            self.send_key_input(vkey, KEYBD_EVENT_FLAGS(0), 0);
        }
    }

    pub fn key_up(&self, vkey: u16) {
        #[cfg(windows)]
        {
            self.send_key_input(vkey, KEYEVENTF_KEYUP, 0);

            // Automatically release any transient modifier keys from key chords
            let prev_mods = self.active_modifiers.swap(0, Ordering::Relaxed);
            if prev_mods & 0x01 != 0 {
                self.send_key_input(VK_CONTROL.0, KEYEVENTF_KEYUP, 0);
            }
            if prev_mods & 0x02 != 0 {
                self.send_key_input(VK_SHIFT.0, KEYEVENTF_KEYUP, 0);
            }
            if prev_mods & 0x04 != 0 {
                self.send_key_input(VK_MENU.0, KEYEVENTF_KEYUP, 0);
            }
            if prev_mods & 0x08 != 0 {
                self.send_key_input(VK_LWIN.0, KEYEVENTF_KEYUP, 0);
            }
        }
    }

    pub fn text_input(&self, text: &str) {
        #[cfg(windows)]
        {
            for ch in text.encode_utf16() {
                // Down
                self.send_key_input(0, KEYEVENTF_UNICODE, ch);
                // Up
                self.send_key_input(0, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP, ch);
            }
        }
    }

    pub fn touch_direct(&self, norm_x: u16, norm_y: u16, action: u8) {
        #[cfg(windows)]
        {
            use windows::Win32::UI::WindowsAndMessaging::{
                GetSystemMetrics, SetCursorPos, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
                SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
            };
            let vx = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
            let vy = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
            let vw = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
            let vh = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };

            let target_x = vx + (norm_x as i32 * vw) / 65535;
            let target_y = vy + (norm_y as i32 * vh) / 65535;

            unsafe {
                let _ = SetCursorPos(target_x, target_y);
            }

            match action {
                0 => self.send_mouse_input(MOUSEEVENTF_LEFTDOWN, 0, 0, 0),
                1 => self.send_mouse_input(MOUSEEVENTF_MOVE, 0, 0, 0),
                2 => self.send_mouse_input(MOUSEEVENTF_LEFTUP, 0, 0, 0),
                _ => {}
            }
        }
    }

    pub fn set_display_resolution(mode: u8) {
        #[cfg(windows)]
        {
            use windows::Win32::Graphics::Gdi::{
                ChangeDisplaySettingsW, DEVMODEW, DM_PELSHEIGHT, DM_PELSWIDTH,
            };
            let (w, h) = match mode {
                1 => (1280, 720),
                2 => (1920, 1080),
                _ => (0, 0),
            };

            unsafe {
                if w == 0 && h == 0 {
                    let _ = ChangeDisplaySettingsW(None, Default::default());
                    info!("Restored default host display resolution");
                } else {
                    let dm = DEVMODEW {
                        dmSize: std::mem::size_of::<DEVMODEW>() as u16,
                        dmFields: DM_PELSWIDTH | DM_PELSHEIGHT,
                        dmPelsWidth: w,
                        dmPelsHeight: h,
                        ..Default::default()
                    };
                    let res = ChangeDisplaySettingsW(Some(&dm), Default::default());
                    info!("Set display resolution to {}x{}: res={:?}", w, h, res);
                }
            }
        }
    }

    pub fn set_display_orientation(vertical: bool) {
        #[cfg(windows)]
        {
            use windows::Win32::Graphics::Gdi::{
                ChangeDisplaySettingsW, DEVMODEW, DM_DISPLAYORIENTATION, DM_PELSHEIGHT,
                DM_PELSWIDTH, DMDO_90,
            };
            unsafe {
                if !vertical {
                    let _ = ChangeDisplaySettingsW(None, Default::default());
                    info!("Restored default landscape orientation");
                } else {
                    let mut dm = DEVMODEW {
                        dmSize: std::mem::size_of::<DEVMODEW>() as u16,
                        dmFields: DM_DISPLAYORIENTATION | DM_PELSWIDTH | DM_PELSHEIGHT,
                        dmPelsWidth: 1080,
                        dmPelsHeight: 1920,
                        ..Default::default()
                    };
                    dm.Anonymous1.Anonymous2.dmDisplayOrientation = DMDO_90;
                    let res = ChangeDisplaySettingsW(Some(&dm), Default::default());
                    info!("Set display orientation to vertical portrait: res={:?}", res);
                }
            }
        }
    }

    pub fn set_clipboard_text(text: &str) {
        #[cfg(windows)]
        {
            extern "system" {
                fn OpenClipboard(hWndNewOwner: isize) -> i32;
                fn CloseClipboard() -> i32;
                fn EmptyClipboard() -> i32;
                fn SetClipboardData(uFormat: u32, hMem: isize) -> isize;
                fn GlobalAlloc(uFlags: u32, dwBytes: usize) -> isize;
                fn GlobalLock(hMem: isize) -> *mut u8;
                fn GlobalUnlock(hMem: isize) -> i32;
            }

            let utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            let byte_count = utf16.len() * std::mem::size_of::<u16>();

            let mut opened = false;
            for _ in 0..10 {
                if unsafe { OpenClipboard(0) } != 0 {
                    opened = true;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }

            if opened {
                unsafe {
                    let _ = EmptyClipboard();
                    let h_mem = GlobalAlloc(0x0002 /* GMEM_MOVEABLE */, byte_count);
                    if h_mem != 0 {
                        let ptr = GlobalLock(h_mem);
                        if !ptr.is_null() {
                            std::ptr::copy_nonoverlapping(utf16.as_ptr() as *const u8, ptr, byte_count);
                            let _ = GlobalUnlock(h_mem);
                            let _ = SetClipboardData(13 /* CF_UNICODETEXT */, h_mem);
                            info!("Synced clipboard text ({} chars) to Windows clipboard", text.chars().count());
                        }
                    }
                    let _ = CloseClipboard();
                }
            } else {
                warn!("Failed to open Windows clipboard after 10 attempts");
            }
        }
    }

    pub fn execute_system_command(action: u8) {
        #[cfg(windows)]
        {
            match action {
                1 => {
                    // Lock Workstation (Win+L)
                    extern "system" {
                        fn LockWorkStation() -> i32;
                    }
                    unsafe {
                        let _ = LockWorkStation();
                    }
                    info!("Executed remote LockWorkStation");
                }
                2 => {
                    // Show Desktop (Win+D)
                    use windows::Win32::UI::Input::KeyboardAndMouse::{KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VK_LWIN};
                    let sim = InputSimulator::new();
                    sim.send_key_input(VK_LWIN.0, KEYBD_EVENT_FLAGS(0), 0);
                    sim.send_key_input(0x44, KEYBD_EVENT_FLAGS(0), 0); // 'D'
                    sim.send_key_input(0x44, KEYEVENTF_KEYUP, 0);
                    sim.send_key_input(VK_LWIN.0, KEYEVENTF_KEYUP, 0);
                    info!("Executed remote Show Desktop (Win+D)");
                }
                3 => {
                    // Open Task Manager (Ctrl+Shift+Esc)
                    use windows::Win32::UI::Input::KeyboardAndMouse::{KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VK_CONTROL, VK_SHIFT};
                    let sim = InputSimulator::new();
                    sim.send_key_input(VK_CONTROL.0, KEYBD_EVENT_FLAGS(0), 0);
                    sim.send_key_input(VK_SHIFT.0, KEYBD_EVENT_FLAGS(0), 0);
                    sim.send_key_input(0x1B, KEYBD_EVENT_FLAGS(0), 0); // VK_ESCAPE
                    sim.send_key_input(0x1B, KEYEVENTF_KEYUP, 0);
                    sim.send_key_input(VK_SHIFT.0, KEYEVENTF_KEYUP, 0);
                    sim.send_key_input(VK_CONTROL.0, KEYEVENTF_KEYUP, 0);
                    info!("Executed remote Task Manager shortcut");
                }
                4 => {
                    // Sleep / Suspend PC
                    use windows::Win32::System::Power::SetSuspendState;
                    unsafe {
                        let _ = SetSuspendState(false, false, false);
                    }
                    info!("Executed remote Sleep / Suspend");
                }
                5 => {
                    // Turn Off Display
                    use windows::Win32::Foundation::{LPARAM, WPARAM};
                    use windows::Win32::UI::WindowsAndMessaging::{SendMessageW, HWND_BROADCAST, WM_SYSCOMMAND};
                    unsafe {
                        let _ = SendMessageW(HWND_BROADCAST, WM_SYSCOMMAND, WPARAM(0xF170), LPARAM(2)); // SC_MONITORPOWER, 2 = off
                    }
                    info!("Executed remote Turn Off Display");
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_clipboard() {
        InputSimulator::set_clipboard_text("Test Sway Clipboard Text");
    }
}
