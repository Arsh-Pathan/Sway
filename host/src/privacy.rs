use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::Arc;
use tracing::info;

#[cfg(windows)]
use windows::core::w;
#[cfg(windows)]
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
#[cfg(windows)]
use windows::Win32::Graphics::Gdi::{
    CreateFontW, CreateSolidBrush, DeleteObject, DrawTextW, FillRect,
    SelectObject, SetBkMode, SetTextColor, DT_CENTER, DT_NOPREFIX,
    DT_WORDBREAK, FW_BOLD, FW_NORMAL, TRANSPARENT,
};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    GetSystemMetrics, PostMessageW, PostQuitMessage, RegisterClassExW, SetWindowDisplayAffinity,
    ShowWindow, TranslateMessage, MSG, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
    SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SW_SHOW, WINDOW_DISPLAY_AFFINITY, WM_CLOSE,
    WM_DESTROY, WM_KEYDOWN, WM_PAINT, WNDCLASSEXW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
    WS_VISIBLE,
};

// WDA_EXCLUDEFROMCAPTURE = 0x00000011 (Windows 10 2004+)
const WDA_EXCLUDEFROMCAPTURE: u32 = 0x00000011;

pub struct PrivacyCurtain {
    is_active: Arc<AtomicBool>,
    hwnd_val: Arc<AtomicIsize>,
}

impl PrivacyCurtain {
    pub fn new() -> Self {
        Self {
            is_active: Arc::new(AtomicBool::new(false)),
            hwnd_val: Arc::new(AtomicIsize::new(0)),
        }
    }

    pub fn set_enabled(&self, enable: bool) {
        #[cfg(windows)]
        {
            if enable {
                if self.is_active.swap(true, Ordering::SeqCst) {
                    return; // already active
                }
                info!("Enabling Privacy Curtain: hiding physical monitor while streaming");
                let is_active = self.is_active.clone();
                let hwnd_store = self.hwnd_val.clone();

                std::thread::spawn(move || {
                    unsafe {
                        extern "system" {
                            fn OpenInputDesktop(dwFlags: u32, fInherit: i32, dwDesiredAccess: u32) -> isize;
                            fn SetThreadDesktop(hDesktop: isize) -> i32;
                            fn CloseDesktop(hDesktop: isize) -> i32;
                        }
                        let h_desk = OpenInputDesktop(0, 0, 0x01FF);
                        if h_desk != 0 {
                            let _ = SetThreadDesktop(h_desk);
                        }

                        let class_name = w!("SwayPrivacyCurtainClass");
                        let wc = WNDCLASSEXW {
                            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                            lpfnWndProc: Some(curtain_wnd_proc),
                            hInstance: windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap_or_default().into(),
                            lpszClassName: class_name,
                            ..Default::default()
                        };
                        let _ = RegisterClassExW(&wc);

                        let vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
                        let vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
                        let vw = GetSystemMetrics(SM_CXVIRTUALSCREEN);
                        let vh = GetSystemMetrics(SM_CYVIRTUALSCREEN);

                        let hwnd = CreateWindowExW(
                            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                            class_name,
                            w!("Sway Remote Privacy Curtain"),
                            WS_POPUP | WS_VISIBLE,
                            vx,
                            vy,
                            vw,
                            vh,
                            None,
                            None,
                            None,
                            None,
                        );

                        if let Ok(h) = hwnd {
                            hwnd_store.store(h.0 as isize, Ordering::SeqCst);

                            // Exclude this window from DXGI / Desktop Duplication capture!
                            // Physical monitor shows black curtain, but Sway captures desktop behind it!
                            let _ = SetWindowDisplayAffinity(h, WINDOW_DISPLAY_AFFINITY(WDA_EXCLUDEFROMCAPTURE));
                            let _ = ShowWindow(h, SW_SHOW);

                            // Force window to topmost over all taskbars and windows across virtual screen
                            let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowPos(
                                h,
                                windows::Win32::UI::WindowsAndMessaging::HWND_TOPMOST,
                                vx,
                                vy,
                                vw,
                                vh,
                                windows::Win32::UI::WindowsAndMessaging::SWP_SHOWWINDOW,
                            );

                            let mut msg = MSG::default();
                            while is_active.load(Ordering::SeqCst) && GetMessageW(&mut msg, None, 0, 0).as_bool() {
                                if msg.message == WM_CLOSE || msg.message == WM_DESTROY {
                                    break;
                                }
                                let _ = TranslateMessage(&msg);
                                DispatchMessageW(&msg);
                            }

                            let _ = DestroyWindow(h);
                        }
                        if h_desk != 0 {
                            let _ = CloseDesktop(h_desk);
                        }
                        is_active.store(false, Ordering::SeqCst);
                        hwnd_store.store(0, Ordering::SeqCst);
                    }
                    info!("Privacy Curtain window closed");
                });
            } else {
                if !self.is_active.swap(false, Ordering::SeqCst) {
                    return;
                }
                info!("Disabling Privacy Curtain: restoring physical monitor");
                let hwnd = self.hwnd_val.swap(0, Ordering::SeqCst);
                if hwnd != 0 {
                    unsafe {
                        let _ = PostMessageW(HWND(hwnd as *mut _), WM_CLOSE, WPARAM(0), LPARAM(0));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_curtain_window_creation() {
        let privacy = PrivacyCurtain::new();
        privacy.set_enabled(true);
        std::thread::sleep(std::time::Duration::from_millis(500));
        let hwnd = privacy.hwnd_val.load(Ordering::SeqCst);
        println!("Curtain HWND: {}", hwnd);
        privacy.set_enabled(false);
        assert_ne!(hwnd, 0, "Curtain HWND should not be 0!");
    }
}

#[cfg(windows)]
unsafe extern "system" fn curtain_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_KEYDOWN => {
            // If user presses Escape on host laptop keyboard, immediately dismiss curtain
            if wparam.0 == 0x1B { // VK_ESCAPE
                let _ = PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
                return LRESULT(0);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_PAINT => {
            let mut ps = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
            let hdc = windows::Win32::Graphics::Gdi::BeginPaint(hwnd, &mut ps);
            if !hdc.is_invalid() {
                let mut rect = RECT::default();
                let _ = windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rect);

                // Deep sleek OLED pitch-black background
                let bg_brush = CreateSolidBrush(COLORREF(0x000F0A0A)); // RGB(10, 10, 15)
                FillRect(hdc, &rect, bg_brush);
                let _ = DeleteObject(bg_brush);

                SetBkMode(hdc, TRANSPARENT);

                // Title: Sway Remote Active
                let title_font = CreateFontW(
                    38, 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0, 1, 0, 0, 0, 0,
                    w!("Segoe UI"),
                );
                let old_font = SelectObject(hdc, title_font);
                SetTextColor(hdc, COLORREF(0x00FFAA63)); // Vibrant indigo/cyan accent

                let mut title_rect = RECT {
                    left: rect.left,
                    top: rect.bottom / 2 - 120,
                    right: rect.right,
                    bottom: rect.bottom / 2 - 40,
                };
                let mut title_text: Vec<u16> = "SWAY REMOTE CONTROL ACTIVE\0".encode_utf16().collect();
                DrawTextW(hdc, &mut title_text, &mut title_rect, DT_CENTER | DT_NOPREFIX);

                // Subtitle
                let sub_font = CreateFontW(
                    22, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 1, 0, 0, 0, 0,
                    w!("Segoe UI"),
                );
                SelectObject(hdc, sub_font);
                SetTextColor(hdc, COLORREF(0x00E0E0E0)); // Light Gray

                let mut sub_rect = RECT {
                    left: rect.left + 50,
                    top: rect.bottom / 2 - 30,
                    right: rect.right - 50,
                    bottom: rect.bottom / 2 + 30,
                };
                let mut sub_text: Vec<u16> =
                    "This display is currently being controlled remotely by your mobile device.\0"
                        .encode_utf16()
                        .collect();
                DrawTextW(hdc, &mut sub_text, &mut sub_rect, DT_CENTER | DT_WORDBREAK | DT_NOPREFIX);

                // Hint
                SetTextColor(hdc, COLORREF(0x00888888)); // Dim Gray
                let mut hint_rect = RECT {
                    left: rect.left,
                    top: rect.bottom / 2 + 50,
                    right: rect.right,
                    bottom: rect.bottom / 2 + 100,
                };
                let mut hint_text: Vec<u16> =
                    "Desktop streaming is active  *  Press ESC on laptop or disconnect mobile to restore\0"
                        .encode_utf16()
                        .collect();
                DrawTextW(hdc, &mut hint_text, &mut hint_rect, DT_CENTER | DT_NOPREFIX);

                SelectObject(hdc, old_font);
                let _ = DeleteObject(title_font);
                let _ = DeleteObject(sub_font);

                let _ = windows::Win32::Graphics::Gdi::EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
