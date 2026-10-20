use tracing::{debug, info};

#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN,
    MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE,
    MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEINPUT, MOUSE_EVENT_FLAGS,
    VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_SHIFT,
};

use crate::protocol::InputPacket;

pub struct InputSimulator;

impl InputSimulator {
    pub fn new() -> Self {
        Self
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
            // Apply modifiers if flagged
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

            self.send_key_input(vkey, KEYBD_EVENT_FLAGS(0), 0);
        }
    }

    pub fn key_up(&self, vkey: u16) {
        #[cfg(windows)]
        self.send_key_input(vkey, KEYEVENTF_KEYUP, 0);
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
}
