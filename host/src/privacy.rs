use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Arc, Mutex};
use tracing::info;

#[cfg(windows)]
use windows::core::w;
#[cfg(windows)]
use windows::Win32::Foundation::{BOOL, COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
#[cfg(windows)]
use windows::Win32::Graphics::Gdi::{
    FillRect, GetStockObject, InvalidateRect, SetBkMode, SetStretchBltMode, StretchDIBits, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, BLACK_BRUSH, DIB_RGB_COLORS, HALFTONE, HBRUSH, SRCCOPY, TRANSPARENT,
};
#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::BlockInput;
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyCursor, DestroyWindow, DispatchMessageW, GetMessageW,
    GetSystemMetrics, PostMessageW, PostQuitMessage, RegisterClassExW, SetWindowDisplayAffinity,
    ShowWindow, TranslateMessage, MSG, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
    SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SW_SHOW, WINDOW_DISPLAY_AFFINITY, WM_CLOSE,
    WM_DESTROY, WM_DISPLAYCHANGE, WM_ERASEBKGND, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    WM_NCHITTEST, WM_PAINT, WM_SETCURSOR,
    WNDCLASSEXW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_LAYERED,
    WS_POPUP, WS_VISIBLE, SetLayeredWindowAttributes, LWA_ALPHA,
};

// WDA_EXCLUDEFROMCAPTURE = 0x00000011 (Windows 10 2004+)
const WDA_EXCLUDEFROMCAPTURE: u32 = 0x00000011;

#[cfg(windows)]
static CURTAIN_MONITORS: std::sync::RwLock<Vec<(i32, i32, i32, i32)>> = std::sync::RwLock::new(Vec::new());

#[cfg(windows)]
static CURTAIN_IMAGE: std::sync::LazyLock<Option<(u32, u32, Vec<u8>)>> = std::sync::LazyLock::new(|| {
    let bytes = include_bytes!("../assets/curtain.png");
    if let Ok(img) = image::load_from_memory(bytes) {
        let rgba = img.to_rgba8();
        let (w, h) = (rgba.width(), rgba.height());
        // Convert to 32-bit BGRA (little-endian B, G, R, A) for high-speed GDI StretchDIBits
        let mut bgra = vec![0u8; (w * h * 4) as usize];
        let raw = rgba.as_raw();
        for (src, dst) in raw.as_chunks::<4>().0.iter().zip(bgra.as_chunks_mut::<4>().0.iter_mut()) {
            dst[0] = src[2]; // B
            dst[1] = src[1]; // G
            dst[2] = src[0]; // R
            dst[3] = 255;    // A
        }
        Some((w, h, bgra))
    } else {
        None
    }
});

/// Virtual-screen bounds `(x, y, w, h)` covering every monitor.
#[cfg(windows)]
fn virtual_bounds() -> (i32, i32, i32, i32) {
    unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    }
}

/// Per-monitor rectangles relative to the curtain window so the artwork centres on each display.
#[cfg(windows)]
fn monitor_rects(vx: i32, vy: i32, vw: i32, vh: i32) -> Vec<(i32, i32, i32, i32)> {
    if let Ok(all) = xcap::Monitor::all() {
        if !all.is_empty() {
            return all
                .into_iter()
                .map(|m| {
                    let mx = m.x().unwrap_or(0);
                    let my = m.y().unwrap_or(0);
                    let mw = m.width().unwrap_or(1920) as i32;
                    let mh = m.height().unwrap_or(1080) as i32;
                    (mx - vx, my - vy, mx - vx + mw, my - vy + mh)
                })
                .collect();
        }
    }
    vec![(0, 0, vw, vh)]
}

pub struct PrivacyCurtain {
    is_active: Arc<AtomicBool>,
    hwnd_val: Arc<AtomicIsize>,
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl PrivacyCurtain {
    pub fn new(_pin: String, _token: String) -> Self {
        Self {
            is_active: Arc::new(AtomicBool::new(false)),
            hwnd_val: Arc::new(AtomicIsize::new(0)),
            thread: Mutex::new(None),
        }
    }

    /// Synchronously gives the user their keyboard, mouse and cursor back. Safe to call from a
    /// panic hook or at shutdown, from any thread, any number of times.
    pub fn emergency_restore() {
        #[cfg(windows)]
        unsafe {
            let _ = BlockInput(BOOL(0));
        }
        #[cfg(windows)]
        set_system_cursor_hidden(false);
    }

    /// Disables the curtain (if active) and waits for its window thread to exit.
    pub fn shutdown(&self) {
        self.set_enabled(false);
        let handle = self.thread.lock().ok().and_then(|mut g| g.take());
        if let Some(h) = handle {
            let _ = h.join();
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

                let handle = std::thread::spawn(move || {
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

                        // Block physical keyboard and mouse inputs on the host PC
                        let _ = BlockInput(BOOL(1));

                        // Hide hardware mouse cursor globally on physical laptop display
                        set_system_cursor_hidden(true);

                        // Stock objects are owned by the OS: never freed, never dangling in the class.
                        let black_brush = HBRUSH(GetStockObject(BLACK_BRUSH).0);
                        let class_name = w!("SwayPrivacyCurtainClass");
                        let wc = WNDCLASSEXW {
                            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                            lpfnWndProc: Some(curtain_wnd_proc),
                            hInstance: windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap_or_default().into(),
                            lpszClassName: class_name,
                            hbrBackground: black_brush,
                            ..Default::default()
                        };
                        let _ = RegisterClassExW(&wc);

                        let (vx, vy, vw, vh) = virtual_bounds();
                        if let Ok(mut lock) = CURTAIN_MONITORS.write() {
                            *lock = monitor_rects(vx, vy, vw, vh);
                        }

                        // Create Layered topmost curtain covering all displays (blocks local click-through)
                        let hwnd = CreateWindowExW(
                            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
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

                            // Layered attributes (100% opaque black surface)
                            let _ = SetLayeredWindowAttributes(h, COLORREF(0), 255, LWA_ALPHA);

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

                        // Restore physical inputs and hardware cursor visibility
                        let _ = BlockInput(BOOL(0));
                        set_system_cursor_hidden(false);

                        if h_desk != 0 {
                            let _ = CloseDesktop(h_desk);
                        }
                        is_active.store(false, Ordering::SeqCst);
                        hwnd_store.store(0, Ordering::SeqCst);
                    }
                    info!("Privacy Curtain window closed & restored physical display");
                });
                if let Ok(mut g) = self.thread.lock() {
                    *g = Some(handle);
                }
            } else {
                if !self.is_active.swap(false, Ordering::SeqCst) {
                    return;
                }
                info!("Disabling Privacy Curtain: restoring physical monitor");
                // Give input and cursor back immediately; the window thread finishes asynchronously.
                Self::emergency_restore();
                let hwnd = self.hwnd_val.swap(0, Ordering::SeqCst);
                if hwnd != 0 {
                    unsafe {
                        let _ = PostMessageW(HWND(hwnd as *mut _), WM_CLOSE, WPARAM(0), LPARAM(0));
                    }
                }
            }
        }
        #[cfg(not(windows))]
        {
            let _ = enable;
        }
    }
}

#[cfg(windows)]
fn set_system_cursor_hidden(hidden: bool) {
    unsafe {
        if hidden {
            let and_mask = [0xFFu8; 128];
            let xor_mask = [0x00u8; 128];
            let hinst = windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap_or_default();
            const CURSOR_IDS: [u32; 13] = [
                32512, 32513, 32514, 32515, 32516, 32642, 32643, 32644, 32645, 32646, 32648, 32649, 32650,
            ];
            for &id in &CURSOR_IDS {
                if let Ok(cur) = windows::Win32::UI::WindowsAndMessaging::CreateCursor(
                    hinst,
                    0,
                    0,
                    32,
                    32,
                    and_mask.as_ptr() as *const _,
                    xor_mask.as_ptr() as *const _,
                ) {
                    // On success the system takes ownership of `cur`; on failure we must free it.
                    if windows::Win32::UI::WindowsAndMessaging::SetSystemCursor(
                        cur,
                        windows::Win32::UI::WindowsAndMessaging::SYSTEM_CURSOR_ID(id),
                    )
                    .is_err()
                    {
                        let _ = DestroyCursor(cur);
                    }
                }
            }
        } else {
            use windows::Win32::UI::WindowsAndMessaging::{
                SystemParametersInfoW, SPI_SETCURSORS, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
            };
            let _ = SystemParametersInfoW(SPI_SETCURSORS, 0, None, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0));
        }
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
            // Return HTCLIENT (1) so physical clicks/touches on the laptop are absorbed by the curtain
            LRESULT(1)
        }
        WM_SETCURSOR => {
            // Ensure mouse cursor is completely hidden over the privacy curtain window
            windows::Win32::UI::WindowsAndMessaging::SetCursor(None);
            LRESULT(1)
        }
        WM_ERASEBKGND => {
            // Suppress default background erasing to prevent any screen flicker
            LRESULT(1)
        }
        WM_DISPLAYCHANGE => {
            // Resolution / rotation changed under us: re-cover the whole virtual screen so no part of
            // the physical display is ever exposed.
            let (vx, vy, vw, vh) = virtual_bounds();
            if let Ok(mut lock) = CURTAIN_MONITORS.write() {
                *lock = monitor_rects(vx, vy, vw, vh);
            }
            let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowPos(
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::HWND_TOPMOST,
                vx,
                vy,
                vw,
                vh,
                windows::Win32::UI::WindowsAndMessaging::SWP_SHOWWINDOW,
            );
            let _ = InvalidateRect(hwnd, None, BOOL(0));
            LRESULT(0)
        }
        WM_KEYDOWN | WM_KEYUP | WM_SYSKEYDOWN | WM_SYSKEYUP => {
            // Absorb any keystrokes sent to curtain window
            LRESULT(0)
        }
        WM_PAINT => {
            let mut ps = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
            let hdc = windows::Win32::Graphics::Gdi::BeginPaint(hwnd, &mut ps);
            if !hdc.is_invalid() {
                let mut rect = RECT::default();
                let _ = windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rect);

                // Pitch OLED Black Background (Monochrome Theme) using the OS-owned stock brush
                FillRect(hdc, &rect, HBRUSH(GetStockObject(BLACK_BRUSH).0));

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

                // Pixel-perfect rendering of the user's high-definition privacy screen image
                if let Some((img_w, img_h, ref bgra)) = *CURTAIN_IMAGE {
                    let bmi = BITMAPINFO {
                        bmiHeader: BITMAPINFOHEADER {
                            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                            biWidth: img_w as i32,
                            biHeight: -(img_h as i32), // negative for top-down DIB
                            biPlanes: 1,
                            biBitCount: 32,
                            biCompression: BI_RGB.0,
                            biSizeImage: 0,
                            biXPelsPerMeter: 0,
                            biYPelsPerMeter: 0,
                            biClrUsed: 0,
                            biClrImportant: 0,
                        },
                        bmiColors: [windows::Win32::Graphics::Gdi::RGBQUAD::default()],
                    };

                    for &(ml, mt, mr, mb) in &mon_list {
                        let mw = mr - ml;
                        let mh = mb - mt;
                        if mw <= 0 || mh <= 0 {
                            continue;
                        }

                        // Fit 16:9 aspect ratio cleanly into monitor
                        let (draw_w, draw_h) = if (mw as i64 * img_h as i64) > (mh as i64 * img_w as i64) {
                            ((mh * img_w as i32) / img_h as i32, mh)
                        } else {
                            (mw, (mw * img_h as i32) / img_w as i32)
                        };
                        let draw_x = ml + (mw - draw_w) / 2;
                        let draw_y = mt + (mh - draw_h) / 2;

                        SetStretchBltMode(hdc, HALFTONE);
                        let _ = StretchDIBits(
                            hdc,
                            draw_x,
                            draw_y,
                            draw_w,
                            draw_h,
                            0,
                            0,
                            img_w as i32,
                            img_h as i32,
                            Some(bgra.as_ptr() as *const _),
                            &bmi,
                            DIB_RGB_COLORS,
                            SRCCOPY,
                        );
                    }
                }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_curtain_window_creation() {
        let privacy = PrivacyCurtain::new("1234".to_string(), "token123".to_string());
        privacy.set_enabled(true);
        std::thread::sleep(std::time::Duration::from_millis(500));
        let hwnd = privacy.hwnd_val.load(Ordering::SeqCst);
        println!("Curtain HWND: {}", hwnd);
        privacy.shutdown();
        PrivacyCurtain::emergency_restore();
        assert_ne!(hwnd, 0, "Curtain HWND should not be 0!");
    }
}
