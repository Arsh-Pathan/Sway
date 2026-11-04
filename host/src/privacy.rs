use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::Arc;
use tracing::info;

#[cfg(windows)]
use windows::core::w;
#[cfg(windows)]
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
#[cfg(windows)]
use windows::Win32::Graphics::Gdi::{
    CreateSolidBrush, DeleteObject, FillRect, SetBkMode, SetStretchBltMode,
    StretchDIBits, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HALFTONE, SRCCOPY, TRANSPARENT,
};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    GetSystemMetrics, PostMessageW, PostQuitMessage, RegisterClassExW, SetWindowDisplayAffinity,
    ShowWindow, TranslateMessage, MSG, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
    SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SW_SHOW, WINDOW_DISPLAY_AFFINITY, WM_CLOSE,
    WM_DESTROY, WM_ERASEBKGND, WM_KEYDOWN, WM_NCHITTEST, WM_PAINT, WM_SETCURSOR,
    WNDCLASSEXW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_LAYERED,
    WS_POPUP, WS_VISIBLE, SetLayeredWindowAttributes, LWA_ALPHA,
    SetWindowsHookExW, UnhookWindowsHookEx, CallNextHookEx, WH_KEYBOARD_LL, WH_MOUSE_LL, HHOOK, KBDLLHOOKSTRUCT, MSLLHOOKSTRUCT,
};

// WDA_EXCLUDEFROMCAPTURE = 0x00000011 (Windows 10 2004+)
const WDA_EXCLUDEFROMCAPTURE: u32 = 0x00000011;

#[cfg(windows)]
static CURTAIN_MONITORS: std::sync::RwLock<Vec<(i32, i32, i32, i32)>> = std::sync::RwLock::new(Vec::new());

#[derive(Default)]
struct PinEntryState {
    entered_pin: String,
    expected_pin: String,
    expected_token: String,
    last_error: bool,
    curtain_hwnd: isize,
}

static PIN_STATE: std::sync::Mutex<PinEntryState> = std::sync::Mutex::new(PinEntryState {
    entered_pin: String::new(),
    expected_pin: String::new(),
    expected_token: String::new(),
    last_error: false,
    curtain_hwnd: 0,
});

#[cfg(windows)]
static HOOK_HANDLE: AtomicIsize = AtomicIsize::new(0);
#[cfg(windows)]
static MOUSE_HOOK_HANDLE: AtomicIsize = AtomicIsize::new(0);

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

pub struct PrivacyCurtain {
    is_active: Arc<AtomicBool>,
    hwnd_val: Arc<AtomicIsize>,
    pin: Arc<std::sync::RwLock<String>>,
    token: Arc<std::sync::RwLock<String>>,
}

impl PrivacyCurtain {
    pub fn new(pin: String, token: String) -> Self {
        Self {
            is_active: Arc::new(AtomicBool::new(false)),
            hwnd_val: Arc::new(AtomicIsize::new(0)),
            pin: Arc::new(std::sync::RwLock::new(pin)),
            token: Arc::new(std::sync::RwLock::new(token)),
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

                let cur_pin = self.pin.read().map(|p| p.clone()).unwrap_or_else(|_| "1234".to_string());
                let cur_token = self.token.read().map(|t| t.clone()).unwrap_or_default();

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

                        // Initialize PIN State
                        if let Ok(mut state) = PIN_STATE.lock() {
                            state.entered_pin.clear();
                            state.expected_pin = cur_pin;
                            state.expected_token = cur_token;
                            state.last_error = false;
                            state.curtain_hwnd = 0;
                        }

                        // Hide hardware mouse cursor globally on physical laptop display
                        set_system_cursor_hidden(true);

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

                        // Enumerate all active physical monitors so emblem & text center on each display
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
                            if let Ok(mut state) = PIN_STATE.lock() {
                                state.curtain_hwnd = h.0 as isize;
                            }

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

                            // Install Low-Level Keyboard and Mouse Hooks to lock physical controls completely
                            let hinst = windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap_or_default();
                            let khook: Result<HHOOK, _> = SetWindowsHookExW(
                                WH_KEYBOARD_LL,
                                Some(ll_keyboard_hook),
                                hinst,
                                0,
                            );
                            if let Ok(khk) = khook {
                                HOOK_HANDLE.store(khk.0 as isize, Ordering::SeqCst);
                            }

                            let mhook: Result<HHOOK, _> = SetWindowsHookExW(
                                WH_MOUSE_LL,
                                Some(ll_mouse_hook),
                                hinst,
                                0,
                            );
                            if let Ok(mhk) = mhook {
                                MOUSE_HOOK_HANDLE.store(mhk.0 as isize, Ordering::SeqCst);
                            }

                            let mut msg = MSG::default();
                            while is_active.load(Ordering::SeqCst) && GetMessageW(&mut msg, None, 0, 0).as_bool() {
                                if msg.message == WM_CLOSE || msg.message == WM_DESTROY {
                                    break;
                                }
                                let _ = TranslateMessage(&msg);
                                DispatchMessageW(&msg);
                            }

                            // Remove Low-Level Hooks
                            let khk_val = HOOK_HANDLE.swap(0, Ordering::SeqCst);
                            if khk_val != 0 {
                                let _ = UnhookWindowsHookEx(HHOOK(khk_val as *mut _));
                            }
                            let mhk_val = MOUSE_HOOK_HANDLE.swap(0, Ordering::SeqCst);
                            if mhk_val != 0 {
                                let _ = UnhookWindowsHookEx(HHOOK(mhk_val as *mut _));
                            }

                            let _ = DestroyWindow(h);
                            let _ = DeleteObject(black_brush);
                        }

                        // Restore hardware cursor visibility across system
                        set_system_cursor_hidden(false);

                        if h_desk != 0 {
                            let _ = CloseDesktop(h_desk);
                        }
                        is_active.store(false, Ordering::SeqCst);
                        hwnd_store.store(0, Ordering::SeqCst);
                    }
                    info!("Privacy Curtain window closed & restored physical display");
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
                    let _ = windows::Win32::UI::WindowsAndMessaging::SetSystemCursor(
                        cur,
                        windows::Win32::UI::WindowsAndMessaging::SYSTEM_CURSOR_ID(id),
                    );
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
unsafe extern "system" fn ll_keyboard_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let kbd = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        // If LLKHF_INJECTED (0x10) is set, keystroke was generated by remote SendInput (from phone)
        // Pass it through immediately so remote typing from phone works untouched!
        if (kbd.flags.0 & 0x10) != 0 {
            return CallNextHookEx(None, code, wparam, lparam);
        }

        // Physical laptop keyboard: BLOCK ALL INPUT completely!
        // No local keyboard controls work on the PC while Privacy Curtain is active.
        return LRESULT(1);
    }

    CallNextHookEx(None, code, wparam, lparam)
}

#[cfg(windows)]
unsafe extern "system" fn ll_mouse_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let ms = &*(lparam.0 as *const MSLLHOOKSTRUCT);
        // If LLMHF_INJECTED (0x01) is set, mouse event was generated by remote SendInput (from phone)
        // Pass it through immediately so remote touch and mouse control work untouched!
        if (ms.flags & 0x01) != 0 {
            return CallNextHookEx(None, code, wparam, lparam);
        }

        // Physical mouse/trackpad interaction on the laptop: BLOCK ALL INPUT completely!
        return LRESULT(1);
    }

    CallNextHookEx(None, code, wparam, lparam)
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
        WM_KEYDOWN => LRESULT(0),
        WM_PAINT => {
            let mut ps = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
            let hdc = windows::Win32::Graphics::Gdi::BeginPaint(hwnd, &mut ps);
            if !hdc.is_invalid() {
                let mut rect = RECT::default();
                let _ = windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rect);

                // Pitch OLED Black Background (Monochrome Theme)
                let bg_brush = CreateSolidBrush(COLORREF(0x00000000));
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
        privacy.set_enabled(false);
        assert_ne!(hwnd, 0, "Curtain HWND should not be 0!");
    }
}
