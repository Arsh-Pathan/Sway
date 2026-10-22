use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::Arc;
use tracing::info;

#[cfg(windows)]
use windows::core::w;
#[cfg(windows)]
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
#[cfg(windows)]
use windows::Win32::Graphics::Gdi::{
    CreateFontW, CreatePen, CreateSolidBrush, DeleteObject, DrawTextW, FillRect,
    LineTo, MoveToEx, Polygon, RoundRect, SelectObject, SetBkMode, SetTextColor,
    DT_CENTER, DT_NOPREFIX, FW_NORMAL, PS_NULL, PS_SOLID, TRANSPARENT,
};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    GetSystemMetrics, PostMessageW, PostQuitMessage, RegisterClassExW, SetWindowDisplayAffinity,
    ShowWindow, TranslateMessage, MSG, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
    SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SW_SHOW, WINDOW_DISPLAY_AFFINITY, WM_CLOSE,
    WM_DESTROY, WM_ERASEBKGND, WM_KEYDOWN, WM_NCHITTEST, WM_PAINT, WNDCLASSEXW, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_POPUP, WS_VISIBLE,
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
                        let black_brush = CreateSolidBrush(COLORREF(0x00000000));
                        let wc = WNDCLASSEXW {
                            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                            lpfnWndProc: Some(curtain_wnd_proc),
                            hInstance: windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap_or_default().into(),
                            lpszClassName: class_name,
                            hbrBackground: black_brush,
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
                            let _ = DeleteObject(black_brush);
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
        WM_NCHITTEST => {
            // Return HTTRANSPARENT (-1) so mouse clicks and touches pass directly through to desktop windows!
            LRESULT(-1)
        }
        WM_ERASEBKGND => {
            // Suppress default background erasing to prevent any screen flicker
            LRESULT(1)
        }
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

                // Pure pitch-black plain OLED background
                let bg_brush = CreateSolidBrush(COLORREF(0x00000000)); // True Black
                FillRect(hdc, &rect, bg_brush);
                let _ = DeleteObject(bg_brush);

                SetBkMode(hdc, TRANSPARENT);

                let cx = (rect.left + rect.right) / 2;
                let cy = (rect.top + rect.bottom) / 2;

                // Scale the vector emblem cleanly based on screen dimensions
                let icon_dim = (((rect.bottom - rect.top) as f32) * 0.22).clamp(160.0, 240.0);
                let s = icon_dim / 124.0;
                let ox = cx - ((124.0 * s) / 2.0).round() as i32;
                let oy = cy - ((124.0 * s) / 2.0).round() as i32;

                let rx = |val: f32| ox + (val * s).round() as i32;
                let ry = |val: f32| oy + (val * s).round() as i32;

                let null_pen = CreatePen(PS_NULL, 0, COLORREF(0));
                let old_pen = SelectObject(hdc, null_pen);

                // 1. Outer Squircle Container
                let container_brush = CreateSolidBrush(COLORREF(0x001B1714)); // Deep charcoal RGB(20, 23, 27)
                let border_pen = CreatePen(PS_SOLID, 1, COLORREF(0x00332D28)); // Subtle border RGB(40, 45, 51)
                SelectObject(hdc, border_pen);
                let old_brush = SelectObject(hdc, container_brush);

                let r_squircle = (28.0 * s).round() as i32;
                let _ = RoundRect(hdc, rx(0.0), ry(0.0), rx(124.0), ry(124.0), r_squircle * 2, r_squircle * 2);

                let _ = DeleteObject(border_pen);
                SelectObject(hdc, null_pen);

                // 2. Laptop Bezel (Silver White)
                let bezel_brush = CreateSolidBrush(COLORREF(0x00F0EAE7)); // RGB(231, 234, 240)
                SelectObject(hdc, bezel_brush);
                let r_bezel = (6.0 * s).round().max(2.0) as i32;
                let _ = RoundRect(hdc, rx(23.0), ry(22.0), rx(101.0), ry(79.0), r_bezel * 2, r_bezel * 2);

                // 3. Laptop Screen Glass (Deep Contrast OLED)
                let screen_brush = CreateSolidBrush(COLORREF(0x000E0A08)); // Deep Dark RGB(8, 10, 14)
                SelectObject(hdc, screen_brush);
                let r_screen = (2.0 * s).round().max(1.0) as i32;
                let _ = RoundRect(hdc, rx(28.0), ry(27.0), rx(96.0), ry(74.0), r_screen * 2, r_screen * 2);

                // 4. Laptop Base (Silver White Trapezoid)
                let base_brush = CreateSolidBrush(COLORREF(0x00E8DEDE)); // RGB(222, 222, 232)
                SelectObject(hdc, base_brush);
                let base_pts = [
                    POINT { x: rx(18.0), y: ry(83.0) },
                    POINT { x: rx(106.0), y: ry(83.0) },
                    POINT { x: rx(114.0), y: ry(94.0) },
                    POINT { x: rx(10.0), y: ry(94.0) },
                ];
                let _ = Polygon(hdc, &base_pts);

                // 5. Trackpad / Base Opening Notch
                let notch_pen = CreatePen(PS_SOLID, (2.0 * s).round().max(2.0) as i32, COLORREF(0x00665852)); // RGB(82, 88, 102)
                SelectObject(hdc, notch_pen);
                let _ = MoveToEx(hdc, rx(52.0), ry(90.0), None);
                let _ = LineTo(hdc, rx(72.0), ry(90.0));

                // 6. Ultra-subtle, minimal bottom hint
                let hint_font = CreateFontW(
                    14, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 1, 0, 0, 0, 0,
                    w!("Segoe UI"),
                );
                let old_font = SelectObject(hdc, hint_font);
                SetTextColor(hdc, COLORREF(0x004A4540)); // Low-contrast faint slate

                let mut hint_rect = RECT {
                    left: rect.left,
                    top: rect.bottom - 48,
                    right: rect.right,
                    bottom: rect.bottom - 16,
                };
                let mut hint_text: Vec<u16> = "Press ESC on laptop to exit\0".encode_utf16().collect();
                DrawTextW(hdc, &mut hint_text, &mut hint_rect, DT_CENTER | DT_NOPREFIX);

                // Clean up GDI resources
                SelectObject(hdc, old_font);
                SelectObject(hdc, old_brush);
                SelectObject(hdc, old_pen);

                let _ = DeleteObject(container_brush);
                let _ = DeleteObject(bezel_brush);
                let _ = DeleteObject(screen_brush);
                let _ = DeleteObject(base_brush);
                let _ = DeleteObject(notch_pen);
                let _ = DeleteObject(null_pen);
                let _ = DeleteObject(hint_font);

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
