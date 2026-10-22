use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use tracing::{debug, info, warn};

#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN,
    MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE,
    MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_VIRTUALDESK, MOUSEEVENTF_WHEEL, MOUSEINPUT, MOUSE_EVENT_FLAGS,
    VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_SHIFT,
};

use crate::protocol::InputPacket;

pub struct InputSimulator {
    active_modifiers: AtomicU8,
    active_monitor: AtomicUsize,
    monitor_bounds: std::sync::RwLock<Vec<(i32, i32, i32, i32)>>,
}

impl InputSimulator {
    #[cfg(windows)]
    pub fn ensure_input_desktop() {
        extern "system" {
            fn OpenInputDesktop(dwFlags: u32, fInherit: i32, dwDesiredAccess: u32) -> isize;
            fn SetThreadDesktop(hDesktop: isize) -> i32;
        }
        unsafe {
            let h_desk = OpenInputDesktop(0, 0, 0x01FF);
            if h_desk != 0 {
                let _ = SetThreadDesktop(h_desk);
            }
        }
    }

    #[cfg(not(windows))]
    pub fn ensure_input_desktop() {}

    pub fn new() -> Self {
        #[cfg(windows)]
        Self::ensure_input_desktop();

        let sim = Self {
            active_modifiers: AtomicU8::new(0),
            active_monitor: AtomicUsize::new(0),
            monitor_bounds: std::sync::RwLock::new(Vec::new()),
        };
        sim.refresh_monitors();
        sim
    }

    pub fn refresh_monitors(&self) {
        #[cfg(windows)]
        Self::ensure_input_desktop();

        if let Ok(all) = xcap::Monitor::all() {
            let bounds: Vec<(i32, i32, i32, i32)> = all
                .into_iter()
                .map(|m| {
                    (
                        m.x().unwrap_or(0),
                        m.y().unwrap_or(0),
                        m.width().unwrap_or(1920).max(1) as i32,
                        m.height().unwrap_or(1080).max(1) as i32,
                    )
                })
                .collect();
            if let Ok(mut lock) = self.monitor_bounds.write() {
                *lock = bounds;
            }
        }
    }

    /// Dispatches an input packet to the native OS input subsystem.
    pub fn dispatch(&self, packet: InputPacket) {
        #[cfg(windows)]
        Self::ensure_input_desktop();

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
                self.active_monitor.store(monitor_index as usize, Ordering::SeqCst);
                self.refresh_monitors();
            }
            InputPacket::SetQuality { mode } => {
                info!("Stream quality change requested: mode #{}", mode);
            }
            InputPacket::TouchDirect { norm_x, norm_y, action } => {
                let _ = self.touch_direct(norm_x, norm_y, action);
            }
            InputPacket::SetPrivacyCurtain { enabled: _ } => {}
            InputPacket::ChangeResolution { mode } => {
                Self::set_display_resolution(mode);
                self.refresh_monitors();
            }
            InputPacket::ChangeOrientation { vertical } => {
                self.change_display_orientation(vertical);
            }
            InputPacket::ClipboardSync { text } => {
                Self::set_clipboard_text(&text);
            }
            InputPacket::SystemCommand { action } => {
                Self::execute_system_command(action);
            }
            InputPacket::SetStreaming { enabled: _ } => {}
            InputPacket::SetAudioConfig { speaker_enabled: _, mic_enabled: _ } => {}
        }
    }

    #[cfg(windows)]
    fn send_mouse_input(&self, flags: MOUSE_EVENT_FLAGS, dx: i32, dy: i32, mouse_data: u32) {
        Self::ensure_input_desktop();
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
    pub fn send_raw_key(vkey: u16, flags: KEYBD_EVENT_FLAGS, scan: u16) {
        Self::ensure_input_desktop();
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

    #[cfg(windows)]
    fn send_key_input(&self, vkey: u16, flags: KEYBD_EVENT_FLAGS, scan: u16) {
        Self::send_raw_key(vkey, flags, scan);
    }

    #[cfg(not(windows))]
    pub fn send_raw_key(_vkey: u16, _flags: u32, _scan: u16) {}

    #[cfg(not(windows))]
    fn send_mouse_input(&self, _flags: u32, _dx: i32, _dy: i32, _mouse_data: u32) {}

    #[cfg(not(windows))]
    fn send_key_input(&self, _vkey: u16, _flags: u32, _scan: u16) {}

    pub fn mouse_move(&self, dx: i16, dy: i16) {
        #[cfg(windows)]
        {
            Self::ensure_input_desktop();
            self.send_mouse_input(MOUSEEVENTF_MOVE, dx as i32, dy as i32, 0);
        }
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
                let val = (delta_y as i32) * 2;
                self.send_mouse_input(MOUSEEVENTF_WHEEL, 0, 0, val as u32);
            }
            if delta_x != 0 {
                // Horizontal wheel
                let val = (delta_x as i32) * 2;
                self.send_mouse_input(MOUSEEVENTF_HWHEEL, 0, 0, val as u32);
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

    pub fn touch_direct(&self, norm_x: u16, norm_y: u16, action: u8) -> bool {
        #[cfg(windows)]
        {
            Self::ensure_input_desktop();
            use windows::Win32::UI::WindowsAndMessaging::{
                GetCursorInfo, GetSystemMetrics, LoadCursorW, SetCursorPos,
                CURSORINFO, CURSOR_SHOWING, IDC_IBEAM, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
                SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
            };

            let cur_idx = self.active_monitor.load(Ordering::Relaxed);
            let (mon_x, mon_y, mon_w, mon_h) = {
                let bounds_empty = self.monitor_bounds.read().map(|b| b.is_empty()).unwrap_or(true);
                if bounds_empty {
                    self.refresh_monitors();
                }
                let lock = self.monitor_bounds.read().ok();
                lock.as_ref()
                    .and_then(|b| b.get(cur_idx).or_else(|| b.first()).copied())
                    .unwrap_or((0, 0, 1920, 1080))
            };

            let raw_x = mon_x + (norm_x as i32 * mon_w) / 65535;
            let raw_y = mon_y + (norm_y as i32 * mon_h) / 65535;
            let target_x = raw_x.clamp(mon_x, mon_x + mon_w - 1);
            let target_y = raw_y.clamp(mon_y, mon_y + mon_h - 1);

            let virt_x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
            let virt_y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
            let virt_w = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) }.max(1);
            let virt_h = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) }.max(1);

            let abs_x = (((target_x - virt_x) as f64 / virt_w as f64) * 65535.0).round() as i32;
            let abs_y = (((target_y - virt_y) as f64 / virt_h as f64) * 65535.0).round() as i32;
            let abs_flags = MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK;

            match action {
                0 => {
                    unsafe {
                        let _ = SetCursorPos(target_x, target_y);
                    }
                    self.send_mouse_input(abs_flags | MOUSEEVENTF_LEFTDOWN, abs_x, abs_y, 0);
                }
                1 => {
                    unsafe {
                        let _ = SetCursorPos(target_x, target_y);
                    }
                    self.send_mouse_input(
                        abs_flags | MOUSEEVENTF_MOVE,
                        abs_x,
                        abs_y,
                        0,
                    );
                }
                2 => {
                    unsafe {
                        let _ = SetCursorPos(target_x, target_y);
                    }
                    self.send_mouse_input(abs_flags | MOUSEEVENTF_MOVE | MOUSEEVENTF_LEFTUP, abs_x, abs_y, 0);
                }
                _ => {}
            }

            // Check if cursor is over a text input field (I-Beam cursor)
            let mut is_text = false;
            unsafe {
                let mut ci = CURSORINFO {
                    cbSize: std::mem::size_of::<CURSORINFO>() as u32,
                    ..Default::default()
                };
                if GetCursorInfo(&mut ci).is_ok() && ci.flags == CURSOR_SHOWING {
                    if let Ok(ibeam) = LoadCursorW(None, IDC_IBEAM) {
                        if ci.hCursor == ibeam {
                            is_text = true;
                        }
                    }
                }
            }
            is_text
        }
        #[cfg(not(windows))]
        {
            false
        }
    }

    pub fn set_display_resolution(mode: u8) {
        #[cfg(windows)]
        {
            use windows::Win32::Graphics::Gdi::{
                ChangeDisplaySettingsExW, EnumDisplaySettingsW, DEVMODEW,
                DM_PELSHEIGHT, DM_PELSWIDTH, ENUM_CURRENT_SETTINGS, CDS_UPDATEREGISTRY,
            };

            unsafe {
                if mode == 0 {
                    let _ = ChangeDisplaySettingsExW(None, None, None, Default::default(), None);
                    info!("Restored default host display resolution");
                    return;
                }

                let mut dm = DEVMODEW {
                    dmSize: std::mem::size_of::<DEVMODEW>() as u16,
                    ..Default::default()
                };

                if EnumDisplaySettingsW(None, ENUM_CURRENT_SETTINGS, &mut dm).as_bool() {
                    let (w, h) = match mode {
                        1 => (1280, 720),   // 720p Mobile
                        2 => (1920, 1080),  // 1080p Full HD
                        3 => (1600, 900),   // 900p Balanced
                        4 => (1024, 768),   // Compact 4:3
                        5 => (2560, 1440),  // 1440p QHD
                        10 => (1080, 1920), // Vertical 1080p
                        11 => (720, 1280),  // Vertical 720p
                        _ => (1920, 1080),
                    };

                    dm.dmPelsWidth = w;
                    dm.dmPelsHeight = h;
                    dm.dmFields |= DM_PELSWIDTH | DM_PELSHEIGHT;

                    let res = ChangeDisplaySettingsExW(None, Some(&dm), None, CDS_UPDATEREGISTRY, None);
                    info!("Set display resolution to {}x{}: res={:?}", w, h, res);
                }
            }
        }
    }

    pub fn change_display_orientation(&self, vertical: bool) {
        Self::set_display_orientation(vertical);
        self.refresh_monitors();
    }

    pub fn set_display_orientation(vertical: bool) {
        #[cfg(windows)]
        {
            use windows::Win32::Graphics::Gdi::{
                ChangeDisplaySettingsExW, EnumDisplaySettingsW, DEVMODEW,
                DM_DISPLAYORIENTATION, DM_PELSHEIGHT, DM_PELSWIDTH,
                DMDO_DEFAULT, DMDO_90, ENUM_CURRENT_SETTINGS, CDS_UPDATEREGISTRY,
            };

            unsafe {
                let mut dm = DEVMODEW {
                    dmSize: std::mem::size_of::<DEVMODEW>() as u16,
                    ..Default::default()
                };

                if EnumDisplaySettingsW(None, ENUM_CURRENT_SETTINGS, &mut dm).as_bool() {
                    let is_already_vertical = dm.Anonymous1.Anonymous2.dmDisplayOrientation == DMDO_90
                        || dm.dmPelsWidth < dm.dmPelsHeight;

                    if vertical && !is_already_vertical {
                        let temp = dm.dmPelsWidth;
                        dm.dmPelsWidth = dm.dmPelsHeight;
                        dm.dmPelsHeight = temp;
                        dm.Anonymous1.Anonymous2.dmDisplayOrientation = DMDO_90;
                        dm.dmFields |= DM_DISPLAYORIENTATION | DM_PELSWIDTH | DM_PELSHEIGHT;
                        let res = ChangeDisplaySettingsExW(None, Some(&dm), None, CDS_UPDATEREGISTRY, None);
                        if res.0 != 0 {
                            let _ = ChangeDisplaySettingsExW(None, Some(&dm), None, Default::default(), None);
                        }
                        info!("Rotated display to vertical portrait");
                    } else if !vertical && is_already_vertical {
                        let temp = dm.dmPelsWidth;
                        dm.dmPelsWidth = dm.dmPelsHeight;
                        dm.dmPelsHeight = temp;
                        dm.Anonymous1.Anonymous2.dmDisplayOrientation = DMDO_DEFAULT;
                        dm.dmFields |= DM_DISPLAYORIENTATION | DM_PELSWIDTH | DM_PELSHEIGHT;
                        let res = ChangeDisplaySettingsExW(None, Some(&dm), None, CDS_UPDATEREGISTRY, None);
                        if res.0 != 0 {
                            let _ = ChangeDisplaySettingsExW(None, Some(&dm), None, Default::default(), None);
                        }
                        info!("Restored display to horizontal landscape");
                    }
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
                fn GlobalFree(hMem: isize) -> isize;
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
                            if SetClipboardData(13 /* CF_UNICODETEXT */, h_mem) == 0 {
                                let _ = GlobalFree(h_mem);
                            } else {
                                info!("Synced clipboard text ({} chars) to Windows clipboard", text.chars().count());
                            }
                        } else {
                            let _ = GlobalFree(h_mem);
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
            use std::sync::atomic::AtomicBool;
            static PARTY_RUNNING: AtomicBool = AtomicBool::new(false);

            use windows::Win32::UI::Input::KeyboardAndMouse::{
                KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VK_CONTROL, VK_LWIN, VK_MENU, VK_SHIFT,
            };

            let tap = |vkey: u16| {
                Self::send_raw_key(vkey, KEYBD_EVENT_FLAGS(0), 0);
                Self::send_raw_key(vkey, KEYEVENTF_KEYUP, 0);
            };

            let combo_2 = |mod_vkey: u16, vkey: u16| {
                Self::send_raw_key(mod_vkey, KEYBD_EVENT_FLAGS(0), 0);
                Self::send_raw_key(vkey, KEYBD_EVENT_FLAGS(0), 0);
                Self::send_raw_key(vkey, KEYEVENTF_KEYUP, 0);
                Self::send_raw_key(mod_vkey, KEYEVENTF_KEYUP, 0);
            };

            let combo_3 = |mod1: u16, mod2: u16, vkey: u16| {
                Self::send_raw_key(mod1, KEYBD_EVENT_FLAGS(0), 0);
                Self::send_raw_key(mod2, KEYBD_EVENT_FLAGS(0), 0);
                Self::send_raw_key(vkey, KEYBD_EVENT_FLAGS(0), 0);
                Self::send_raw_key(vkey, KEYEVENTF_KEYUP, 0);
                Self::send_raw_key(mod2, KEYEVENTF_KEYUP, 0);
                Self::send_raw_key(mod1, KEYEVENTF_KEYUP, 0);
            };

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
                    combo_2(VK_LWIN.0, 0x44); // 'D'
                    info!("Executed remote Show Desktop (Win+D)");
                }
                3 => {
                    // Open Task Manager (Ctrl+Shift+Esc)
                    combo_3(VK_CONTROL.0, VK_SHIFT.0, 0x1B); // VK_ESCAPE
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
                6 => {
                    // Volume Up (VK_VOLUME_UP = 0xAF)
                    tap(0xAF);
                }
                7 => {
                    // Volume Down (VK_VOLUME_DOWN = 0xAE)
                    tap(0xAE);
                }
                8 => {
                    // Volume Mute (VK_VOLUME_MUTE = 0xAD)
                    tap(0xAD);
                }
                9 => {
                    // Media Play / Pause (VK_MEDIA_PLAY_PAUSE = 0xB3)
                    tap(0xB3);
                }
                10 => {
                    // Media Next Track (VK_MEDIA_NEXT_TRACK = 0xB0)
                    tap(0xB0);
                }
                11 => {
                    // Media Prev Track (VK_MEDIA_PREV_TRACK = 0xB1)
                    tap(0xB1);
                }
                12 => {
                    // Alt + Tab
                    combo_2(VK_MENU.0, 0x09); // VK_TAB
                }
                13 => {
                    // Win + Tab
                    combo_2(VK_LWIN.0, 0x09); // VK_TAB
                }
                14 => {
                    // Alt + F4 (Close Window)
                    combo_2(VK_MENU.0, 0x73); // VK_F4
                }
                15 => {
                    // Win + Shift + S (Snipping Tool)
                    combo_3(VK_LWIN.0, VK_SHIFT.0, 0x53); // 'S'
                }
                16 => {
                    // Win + E (File Explorer)
                    combo_2(VK_LWIN.0, 0x45); // 'E'
                }
                17 => {
                    // Win + R (Run Dialog)
                    combo_2(VK_LWIN.0, 0x52); // 'R'
                }
                18 => {
                    // Win + Up (Maximize)
                    combo_2(VK_LWIN.0, 0x26); // VK_UP
                }
                19 => {
                    // Win + Down (Minimize)
                    combo_2(VK_LWIN.0, 0x28); // VK_DOWN
                }
                20 => {
                    // Win + Left (Snap Left)
                    combo_2(VK_LWIN.0, 0x25); // VK_LEFT
                }
                21 => {
                    // Win + Right (Snap Right)
                    combo_2(VK_LWIN.0, 0x27); // VK_RIGHT
                }
                22 => {
                    // Ctrl + T (New Tab)
                    combo_2(VK_CONTROL.0, 0x54); // 'T'
                }
                23 => {
                    // Ctrl + W (Close Tab)
                    combo_2(VK_CONTROL.0, 0x57); // 'W'
                }
                24 => {
                    // Ctrl + Shift + T (Reopen Tab)
                    combo_3(VK_CONTROL.0, VK_SHIFT.0, 0x54); // 'T'
                }
                25 => {
                    // F5 / Refresh
                    tap(0x74); // VK_F5
                }
                26 => {
                    // Alt + Left (Browser Back)
                    combo_2(VK_MENU.0, 0x25); // VK_LEFT
                }
                27 => {
                    // Alt + Right (Browser Forward)
                    combo_2(VK_MENU.0, 0x27); // VK_RIGHT
                }
                28 => {
                    // Ctrl + A (Select All)
                    combo_2(VK_CONTROL.0, 0x41); // 'A'
                }
                29 => {
                    // Ctrl + C (Copy)
                    combo_2(VK_CONTROL.0, 0x43); // 'C'
                }
                30 => {
                    // Ctrl + V (Paste)
                    combo_2(VK_CONTROL.0, 0x56); // 'V'
                }
                31 => {
                    // Ctrl + X (Cut)
                    combo_2(VK_CONTROL.0, 0x58); // 'X'
                }
                32 => {
                    // Ctrl + Z (Undo)
                    combo_2(VK_CONTROL.0, 0x5A); // 'Z'
                }
                33 => {
                    // Ctrl + Y (Redo)
                    combo_2(VK_CONTROL.0, 0x59); // 'Y'
                }
                34 => {
                    // Win + S (Windows Search)
                    combo_2(VK_LWIN.0, 0x53); // 'S'
                }
                35 => {
                    // Win + A (Action Center / Quick Settings)
                    combo_2(VK_LWIN.0, 0x41); // 'A'
                }
                36 => {
                    // Escape
                    tap(0x1B);
                }
                37 => {
                    // Enter
                    tap(0x0D);
                }
                38 => {
                    // Backspace
                    tap(0x08);
                }
                39 => {
                    // Tab
                    tap(0x09);
                }

                // ==================== DEVELOPER TOOLS ====================
                40 => {
                    // Launch Terminal (wt.exe or powershell.exe)
                    let _ = std::process::Command::new("cmd")
                        .args(["/c", "start", "wt", "||", "start", "powershell"])
                        .spawn();
                    info!("Executed remote launch Terminal");
                }
                41 => {
                    // Launch VS Code
                    let _ = std::process::Command::new("cmd")
                        .args(["/c", "start", "code"])
                        .spawn();
                    info!("Executed remote launch VS Code");
                }
                42 => {
                    // Launch Antigravity IDE / CLI
                    let _ = std::process::Command::new("cmd")
                        .args(["/c", "start", "powershell", "-NoExit", "-Command", "agy"])
                        .spawn();
                    info!("Executed remote launch Antigravity");
                }
                43 => {
                    // Open GitHub in browser
                    let _ = std::process::Command::new("cmd")
                        .args(["/c", "start", "https://github.com"])
                        .spawn();
                    info!("Executed remote open GitHub");
                }
                44 => {
                    // Open Chrome / Default Browser
                    let _ = std::process::Command::new("cmd")
                        .args(["/c", "start", "https://google.com"])
                        .spawn();
                    info!("Executed remote launch Browser");
                }
                45 => {
                    // VS Code Command Palette: Ctrl + Shift + P
                    combo_3(VK_CONTROL.0, VK_SHIFT.0, 0x50); // 'P'
                }
                46 => {
                    // Browser / Developer Tools: F12
                    tap(0x7B); // VK_F12
                }
                47 => {
                    // VS Code Terminal Toggle: Ctrl + ` (VK_OEM_3 = 0xC0)
                    combo_2(VK_CONTROL.0, 0xC0);
                }
                48 => {
                    // Quick Git Status in Terminal
                    let _ = std::process::Command::new("cmd")
                        .args(["/c", "start", "powershell", "-NoExit", "-Command", "git status"])
                        .spawn();
                    info!("Executed remote Git Status");
                }
                49 => {
                    // Format Code Document: Shift + Alt + F
                    combo_3(VK_SHIFT.0, VK_MENU.0, 0x46); // 'F'
                }
                50 => {
                    // Find in Files: Ctrl + Shift + F
                    combo_3(VK_CONTROL.0, VK_SHIFT.0, 0x46); // 'F'
                }

                // ==================== LAPTOP PARTY & DANCE MODES ====================
                51 => {
                    // Laptop Dance Mode: Autonomous cursor choreography + rhythmic window pulses + 8-bit chiptune
                    PARTY_RUNNING.store(true, Ordering::SeqCst);
                    std::thread::spawn(|| {
                        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SetCursorPos, SM_CXSCREEN, SM_CYSCREEN};
                        extern "system" {
                            fn Beep(dwFreq: u32, dwDuration: u32) -> i32;
                        }

                        // Background retro chiptune thread
                        std::thread::spawn(|| {
                            let melody = [
                                (523, 150), (659, 150), (784, 150), (1046, 250),
                                (880, 150), (1046, 300), (784, 150), (659, 200),
                                (523, 150), (587, 150), (659, 150), (523, 300),
                            ];
                            for (freq, dur) in melody.iter().cycle().take(24) {
                                if !PARTY_RUNNING.load(Ordering::Relaxed) { break; }
                                unsafe { let _ = Beep(*freq, *dur); }
                                std::thread::sleep(std::time::Duration::from_millis(30));
                            }
                        });

                        let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
                        let screen_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
                        let center_x = screen_w / 2;
                        let center_y = screen_h / 2;
                        let radius_x = (screen_w / 4) as f64;
                        let radius_y = (screen_h / 4) as f64;

                        // Dance animation loop for ~10 seconds
                        for step in 0..400 {
                            if !PARTY_RUNNING.load(Ordering::Relaxed) { break; }

                            let t = step as f64 * 0.08;
                            // Lemniscate / Figure-8 dance trajectory
                            let x = (center_x as f64 + radius_x * t.sin()).round() as i32;
                            let y = (center_y as f64 + radius_y * (t * 2.0).sin() * 0.6).round() as i32;
                            unsafe {
                                let _ = SetCursorPos(x, y);
                            }

                            // Rhythmic window beat pulse every ~80 steps
                            if step % 80 == 40 {
                                Self::send_raw_key(VK_LWIN.0, KEYBD_EVENT_FLAGS(0), 0);
                                Self::send_raw_key(0x44, KEYBD_EVENT_FLAGS(0), 0);
                                Self::send_raw_key(0x44, KEYEVENTF_KEYUP, 0);
                                Self::send_raw_key(VK_LWIN.0, KEYEVENTF_KEYUP, 0);
                            }

                            std::thread::sleep(std::time::Duration::from_millis(25));
                        }
                        PARTY_RUNNING.store(false, Ordering::SeqCst);
                    });
                    info!("Executed remote Laptop Dance Mode");
                }
                52 => {
                    // Hacker Matrix Cyberpunk Mode
                    let _ = std::process::Command::new("cmd")
                        .args(["/c", "start", "cmd", "/k", "color 0a && title [SWAY AUTONOMOUS CYBERPUNK TERMINAL] && echo. && echo  ======================================================= && echo    SWAY AI HACKER MODE ACTIVATED -- RUNNING DIAGNOSTICS && echo  ======================================================= && echo. && tree C:\\Users\\ArshPathan\\Projects /F 2>nul || dir /s"])
                        .spawn();
                    info!("Executed remote Hacker Matrix Mode");
                }
                53 => {
                    // Ghost Prank Mode: Opens Notepad and types a friendly autonomous message
                    std::thread::spawn(|| {
                        let _ = std::process::Command::new("notepad.exe").spawn();
                        std::thread::sleep(std::time::Duration::from_millis(700));
                        let msg = "Hello Arsh! Sway autonomous AI controller is live on your laptop... Everything is running silky smooth! \u{1F389}\u{1F47B}";
                        for ch in msg.encode_utf16() {
                            Self::send_raw_key(0, KEYEVENTF_UNICODE, ch);
                            Self::send_raw_key(0, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP, ch);
                            std::thread::sleep(std::time::Duration::from_millis(30));
                        }
                    });
                    info!("Executed remote Ghost Prank Mode");
                }
                54 => {
                    // Retro 8-bit Chiptune Victory Jingle
                    std::thread::spawn(|| {
                        extern "system" {
                            fn Beep(dwFreq: u32, dwDuration: u32) -> i32;
                        }
                        let notes = [(523, 120), (659, 120), (784, 120), (1046, 280), (784, 120), (1046, 400)];
                        for (freq, dur) in notes {
                            unsafe { let _ = Beep(freq, dur); }
                            std::thread::sleep(std::time::Duration::from_millis(25));
                        }
                    });
                    info!("Executed remote 8-bit Chiptune Jingle");
                }
                55 => {
                    // Emergency Stop Party / Dance Mode
                    PARTY_RUNNING.store(false, Ordering::SeqCst);
                    combo_2(VK_LWIN.0, 0x44); // Restore clean desktop
                    info!("Stopped remote Party / Dance Mode");
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
