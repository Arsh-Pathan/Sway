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
    DT_CENTER, DT_NOPREFIX, FW_BOLD, FW_NORMAL, PS_NULL, PS_SOLID, TRANSPARENT,
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

#[cfg(windows)]
static CURTAIN_MONITORS: std::sync::RwLock<Vec<(i32, i32, i32, i32)>> = std::sync::RwLock::new(Vec::new());

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

                        // Enumerate all active physical monitors so we can duplicate the emblem & text on each display
                        let mon_rects: Vec<(i32, i32, i32, i32)> = if let Ok(all) = xcap::Monitor::all() {
                            if !all.is_empty() {
                                all.into_iter()
                                    .map(|m| {
                                        let mx = m.x().unwrap_or(0);
                                        let my = m.y().unwrap_or(0);
                                        let mw = m.width().unwrap_or(1920) as i32;
                                        let mh = m.height().unwrap_or(1080) as i32;
                                        (mx - vx, my - vy, mx - vx + mw, my - vy + mh)
                                    })
                                    .collect()
                            } else {
                                vec![(0, 0, vw, vh)]
                            }
                        } else {
                            vec![(0, 0, vw, vh)]
                        };
                        if let Ok(mut lock) = CURTAIN_MONITORS.write() {
                            *lock = mon_rects;
                        }

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

                // Pure pitch-black plain OLED background across entire virtual canvas
                let bg_brush = CreateSolidBrush(COLORREF(0x00000000)); // True Black
                FillRect(hdc, &rect, bg_brush);
                let _ = DeleteObject(bg_brush);

                SetBkMode(hdc, TRANSPARENT);

                let mon_list: Vec<(i32, i32, i32, i32)> = {
                    let lock = CURTAIN_MONITORS.read().ok();
                    let list: Vec<(i32, i32, i32, i32)> = lock.as_deref().cloned().unwrap_or_default();
                    if list.is_empty() {
                        vec![(rect.left, rect.top, rect.right, rect.bottom)]
                    } else {
                        list
                    }
                };

                let null_pen = CreatePen(PS_NULL, 0, COLORREF(0));
                let old_pen = SelectObject(hdc, null_pen);

                // GDI Brushes & Pens for the Flat Laptop Icon
                let bezel_brush = CreateSolidBrush(COLORREF(0x00FFFFFF)); // Solid Pure White
                let screen_brush = CreateSolidBrush(COLORREF(0x000E0B0A)); // Deep Dark Black Screen
                let base_brush = CreateSolidBrush(COLORREF(0x00FFFFFF));  // Solid Pure White
                let notch_pen = CreatePen(PS_SOLID, 3, COLORREF(0x004B423E)); // Solid Dark Slate

                let title_font = CreateFontW(
                    26, 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0, 1, 0, 0, 0, 0,
                    w!("Segoe UI"),
                );
                let sub_font = CreateFontW(
                    15, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 1, 0, 0, 0, 0,
                    w!("Segoe UI"),
                );
                let hint_font = CreateFontW(
                    13, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 1, 0, 0, 0, 0,
                    w!("Segoe UI"),
                );

                // Render the laptop icon and clean text centered on EACH monitor (duplicate appearance)
                for &(ml, mt, mr, mb) in &mon_list {
                    let mw = mr - ml;
                    let mh = mb - mt;
                    if mw <= 0 || mh <= 0 {
                        continue;
                    }

                    let cx = (ml + mr) / 2;
                    let cy = (mt + mb) / 2 - 25; // Centered, slightly raised for text below

                    let s = (mh as f32 * 0.17).clamp(120.0, 190.0) / 100.0;
                    let ox = cx - ((124.0 * s) / 2.0).round() as i32;
                    let oy = cy - ((100.0 * s) / 2.0).round() as i32;

                    let rx = |val: f32| ox + (val * s).round() as i32;
                    let ry = |val: f32| oy + (val * s).round() as i32;

                    // 1. Laptop Bezel (Flat Pure White)
                    SelectObject(hdc, null_pen);
                    SelectObject(hdc, bezel_brush);
                    let r_bezel = (6.0 * s).round().max(2.0) as i32;
                    let _ = RoundRect(hdc, rx(23.0), ry(22.0), rx(101.0), ry(79.0), r_bezel * 2, r_bezel * 2);

                    // 2. Laptop Screen Glass (Deep Contrast Dark)
                    SelectObject(hdc, screen_brush);
                    let r_screen = (2.0 * s).round().max(1.0) as i32;
                    let _ = RoundRect(hdc, rx(28.0), ry(27.0), rx(96.0), ry(74.0), r_screen * 2, r_screen * 2);

                    // 3. Laptop Base (Flat Pure White Trapezoid)
                    SelectObject(hdc, base_brush);
                    let base_pts = [
                        POINT { x: rx(18.0), y: ry(83.0) },
                        POINT { x: rx(106.0), y: ry(83.0) },
                        POINT { x: rx(114.0), y: ry(94.0) },
                        POINT { x: rx(10.0), y: ry(94.0) },
                    ];
                    let _ = Polygon(hdc, &base_pts);

                    // 4. Trackpad Notch
                    SelectObject(hdc, notch_pen);
                    let _ = MoveToEx(hdc, rx(52.0), ry(90.0), None);
                    let _ = LineTo(hdc, rx(72.0), ry(90.0));

                    // 5. Clean Modern Typography Beneath the Laptop Icon
                    // Title: Sway Remote
                    SelectObject(hdc, title_font);
                    SetTextColor(hdc, COLORREF(0x00FFFFFF)); // Solid Pure White
                    let mut title_rect = RECT {
                        left: ml + 20,
                        top: ry(108.0),
                        right: mr - 20,
                        bottom: ry(108.0) + 32,
                    };
                    let mut title_text: Vec<u16> = "Sway Remote\0".encode_utf16().collect();
                    DrawTextW(hdc, &mut title_text, &mut title_rect, DT_CENTER | DT_NOPREFIX);

                    // Subtitle: Display active on mobile
                    SelectObject(hdc, sub_font);
                    SetTextColor(hdc, COLORREF(0x00A0A0A0)); // Light Gray
                    let mut sub_rect = RECT {
                        left: ml + 20,
                        top: title_rect.bottom + 4,
                        right: mr - 20,
                        bottom: title_rect.bottom + 26,
                    };
                    let mut sub_text: Vec<u16> = "Display active on mobile\0".encode_utf16().collect();
                    DrawTextW(hdc, &mut sub_text, &mut sub_rect, DT_CENTER | DT_NOPREFIX);

                    // Hint: Press ESC on laptop to exit
                    SelectObject(hdc, hint_font);
                    SetTextColor(hdc, COLORREF(0x00555555)); // Subtle Dim Slate
                    let mut hint_rect = RECT {
                        left: ml + 20,
                        top: sub_rect.bottom + 8,
                        right: mr - 20,
                        bottom: sub_rect.bottom + 26,
                    };
                    let mut hint_text: Vec<u16> = "Press ESC on laptop to exit\0".encode_utf16().collect();
                    DrawTextW(hdc, &mut hint_text, &mut hint_rect, DT_CENTER | DT_NOPREFIX);
                }

                // Clean up GDI resources
                SelectObject(hdc, old_pen);
                let _ = DeleteObject(bezel_brush);
                let _ = DeleteObject(screen_brush);
                let _ = DeleteObject(base_brush);
                let _ = DeleteObject(notch_pen);
                let _ = DeleteObject(null_pen);
                let _ = DeleteObject(title_font);
                let _ = DeleteObject(sub_font);
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
