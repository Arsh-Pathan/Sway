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
    Polygon, RoundRect, SelectObject, SetBkMode, SetTextColor,
    DT_CENTER, DT_NOPREFIX, DT_VCENTER, DT_SINGLELINE, FW_BOLD, FW_NORMAL, FW_SEMIBOLD,
    PS_NULL, PS_SOLID, TRANSPARENT,
};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    GetSystemMetrics, PostMessageW, PostQuitMessage, RegisterClassExW, SetWindowDisplayAffinity,
    ShowWindow, TranslateMessage, MSG, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
    SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SW_SHOW, WINDOW_DISPLAY_AFFINITY, WM_CLOSE,
    WM_DESTROY, WM_ERASEBKGND, WM_KEYDOWN, WM_NCHITTEST, WM_PAINT, WM_SETCURSOR,
    WNDCLASSEXW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_LAYERED, WS_EX_TRANSPARENT,
    WS_POPUP, WS_VISIBLE, SetLayeredWindowAttributes, LWA_ALPHA,
    SetWindowsHookExW, UnhookWindowsHookEx, CallNextHookEx, WH_KEYBOARD_LL, HHOOK, KBDLLHOOKSTRUCT,
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

    #[allow(dead_code)]
    pub fn set_pin(&self, new_pin: String) {
        if let Ok(mut lock) = self.pin.write() {
            *lock = new_pin;
        }
    }

    #[allow(dead_code)]
    pub fn is_enabled(&self) -> bool {
        self.is_active.load(Ordering::SeqCst)
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
                        let black_brush = CreateSolidBrush(COLORREF(0x000A0806));
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

                        // Create Layered + Transparent window: mouse clicks pass through completely to underlying windows!
                        let hwnd = CreateWindowExW(
                            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_TRANSPARENT,
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

                            // Install Low-Level Keyboard Hook to capture physical laptop PIN entry
                            let hinst = windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap_or_default();
                            let hook: Result<HHOOK, _> = SetWindowsHookExW(
                                WH_KEYBOARD_LL,
                                Some(ll_keyboard_hook),
                                hinst,
                                0,
                            );
                            if let Ok(hk) = hook {
                                HOOK_HANDLE.store(hk.0 as isize, Ordering::SeqCst);
                            }

                            let mut msg = MSG::default();
                            while is_active.load(Ordering::SeqCst) && GetMessageW(&mut msg, None, 0, 0).as_bool() {
                                if msg.message == WM_CLOSE || msg.message == WM_DESTROY {
                                    break;
                                }
                                let _ = TranslateMessage(&msg);
                                DispatchMessageW(&msg);
                            }

                            // Remove Low-Level Keyboard Hook
                            let hk_val = HOOK_HANDLE.swap(0, Ordering::SeqCst);
                            if hk_val != 0 {
                                let _ = UnhookWindowsHookEx(HHOOK(hk_val as *mut _));
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
    if code >= 0 && (wparam.0 == 0x0100 || wparam.0 == 0x0104) { // WM_KEYDOWN or WM_SYSKEYDOWN
        let kbd = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        // If LLKHF_INJECTED (0x10) is set, keystroke was generated by remote SendInput (from phone)
        // Pass it through immediately so remote typing works untouched!
        if (kbd.flags.0 & 0x10) != 0 {
            return CallNextHookEx(None, code, wparam, lparam);
        }

        // Physical keystroke on the laptop keyboard: process PIN
        let vk = kbd.vkCode;
        let mut should_consume = false;

        if let Ok(mut state) = PIN_STATE.lock() {
            let hwnd = HWND(state.curtain_hwnd as *mut _);

            let char_val = match vk {
                0x30..=0x39 => Some((b'0' + (vk - 0x30) as u8) as char),
                0x60..=0x69 => Some((b'0' + (vk - 0x60) as u8) as char),
                0x41..=0x5A => Some((b'a' + (vk - 0x41) as u8) as char),
                _ => None,
            };

            if let Some(ch) = char_val {
                if state.entered_pin.len() < 16 {
                    state.entered_pin.push(ch);
                    state.last_error = false;
                }

                let entered = state.entered_pin.to_lowercase();
                let exp_pin = state.expected_pin.to_lowercase();
                let exp_tok = state.expected_token.to_lowercase();

                let is_match = (!exp_pin.is_empty() && entered == exp_pin)
                    || (!exp_tok.is_empty() && entered == exp_tok);

                if is_match {
                    info!("Laptop PIN verified! Unlocking screen from laptop keyboard...");
                    let _ = PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
                } else if !hwnd.is_invalid() {
                    let _ = windows::Win32::Graphics::Gdi::InvalidateRect(hwnd, None, false);
                }
                should_consume = true;
            } else if vk == 0x0D { // VK_RETURN
                let entered = state.entered_pin.to_lowercase();
                let exp_pin = state.expected_pin.to_lowercase();
                let exp_tok = state.expected_token.to_lowercase();

                let is_match = (!exp_pin.is_empty() && entered == exp_pin)
                    || (!exp_tok.is_empty() && entered == exp_tok)
                    || (!exp_tok.is_empty() && exp_tok.starts_with(&entered) && entered.len() >= 4);

                if is_match {
                    info!("Laptop PIN verified on Enter! Unlocking screen...");
                    let _ = PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
                } else {
                    state.entered_pin.clear();
                    state.last_error = true;
                    if !hwnd.is_invalid() {
                        let _ = windows::Win32::Graphics::Gdi::InvalidateRect(hwnd, None, false);
                    }
                }
                should_consume = true;
            } else if vk == 0x08 { // VK_BACK
                state.entered_pin.pop();
                state.last_error = false;
                if !hwnd.is_invalid() {
                    let _ = windows::Win32::Graphics::Gdi::InvalidateRect(hwnd, None, false);
                }
                should_consume = true;
            } else if vk == 0x1B { // VK_ESCAPE
                state.entered_pin.clear();
                state.last_error = false;
                if !hwnd.is_invalid() {
                    let _ = windows::Win32::Graphics::Gdi::InvalidateRect(hwnd, None, false);
                }
                should_consume = true;
            }
        }

        if should_consume {
            return LRESULT(1);
        }
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
            // Return HTTRANSPARENT (-1) so mouse clicks and touches pass directly through to desktop windows!
            LRESULT(-1)
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
        WM_KEYDOWN => {
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_PAINT => {
            let mut ps = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
            let hdc = windows::Win32::Graphics::Gdi::BeginPaint(hwnd, &mut ps);
            if !hdc.is_invalid() {
                let mut rect = RECT::default();
                let _ = windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rect);

                // Deep Obsidian Dark Background
                let bg_brush = CreateSolidBrush(COLORREF(0x000C0908)); // Ultra-Deep Charcoal OLED
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

                // Fonts for Modern Lock Screen UI
                let clock_font = CreateFontW(
                    56, 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0, 1, 0, 0, 0, 0,
                    w!("Segoe UI Variable Display"),
                );
                let date_font = CreateFontW(
                    16, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 1, 0, 0, 0, 0,
                    w!("Segoe UI"),
                );
                let badge_font = CreateFontW(
                    12, 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0, 1, 0, 0, 0, 0,
                    w!("Segoe UI"),
                );
                let title_font = CreateFontW(
                    24, 0, 0, 0, FW_SEMIBOLD.0 as i32, 0, 0, 0, 1, 0, 0, 0, 0,
                    w!("Segoe UI Variable Display"),
                );
                let sub_font = CreateFontW(
                    14, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 1, 0, 0, 0, 0,
                    w!("Segoe UI"),
                );
                let pin_dot_font = CreateFontW(
                    22, 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0, 1, 0, 0, 0, 0,
                    w!("Segoe UI"),
                );
                let hint_font = CreateFontW(
                    13, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 1, 0, 0, 0, 0,
                    w!("Segoe UI"),
                );
                let pill_tag_font = CreateFontW(
                    11, 0, 0, 0, FW_SEMIBOLD.0 as i32, 0, 0, 0, 1, 0, 0, 0, 0,
                    w!("Segoe UI"),
                );

                // Brushes and Pens for Visual Components
                let badge_bg_brush = CreateSolidBrush(COLORREF(0x001B1714)); // Dark Glass Pill
                let badge_border_pen = CreatePen(PS_SOLID, 1, COLORREF(0x00332B25)); // Subtle Border
                let shield_brush = CreateSolidBrush(COLORREF(0x00FFFFFF)); // Crisp White Shield
                let shield_inner_brush = CreateSolidBrush(COLORREF(0x000C0908)); // Cutout
                let mint_brush = CreateSolidBrush(COLORREF(0x009DFF00)); // Glowing Mint Emerald

                // Slot Box Styles
                let slot_empty_brush = CreateSolidBrush(COLORREF(0x00130F0D)); // Empty Slot Glass
                let slot_empty_pen = CreatePen(PS_SOLID, 1, COLORREF(0x002D2520)); // Slot Border
                let slot_filled_brush = CreateSolidBrush(COLORREF(0x00221C17)); // Filled Slot Glow
                let slot_filled_pen = CreatePen(PS_SOLID, 2, COLORREF(0x009DFF00)); // Emerald Active Border
                let slot_active_pen = CreatePen(PS_SOLID, 2, COLORREF(0x00554A40)); // Focus Slot Border

                // Pill Tag Brush & Pen
                let pill_bg_brush = CreateSolidBrush(COLORREF(0x0017120F));
                let pill_border_pen = CreatePen(PS_SOLID, 1, COLORREF(0x0028211B));

                // Get Current Local Time for Realistic Lock Screen
                let st = windows::Win32::System::SystemInformation::GetLocalTime();
                let time_str = format!("{:02}:{:02}", st.wHour, st.wMinute);
                let days = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
                let months = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
                let day_name = days.get(st.wDayOfWeek as usize).unwrap_or(&"");
                let month_name = months.get(st.wMonth.saturating_sub(1) as usize).unwrap_or(&"");
                let date_str = format!("{}, {} {}", day_name, month_name, st.wDay);

                // Retrieve current PIN state
                let (pin_len, is_error) = if let Ok(state) = PIN_STATE.lock() {
                    (state.entered_pin.len(), state.last_error)
                } else {
                    (0, false)
                };

                // Render the unified professional lock screen centered on EACH active monitor
                for &(ml, mt, mr, mb) in &mon_list {
                    let mw = mr - ml;
                    let mh = mb - mt;
                    if mw <= 0 || mh <= 0 {
                        continue;
                    }

                    let cx = (ml + mr) / 2;
                    let cy = (mt + mb) / 2;

                    // 1. Clock Display (Top-Centered)
                    SelectObject(hdc, clock_font);
                    SetTextColor(hdc, COLORREF(0x00FFFFFF)); // Solid Pure White
                    let mut clock_rect = RECT {
                        left: ml + 20,
                        top: cy - 250,
                        right: mr - 20,
                        bottom: cy - 180,
                    };
                    let mut clock_utf16: Vec<u16> = format!("{}\0", time_str).encode_utf16().collect();
                    DrawTextW(hdc, &mut clock_utf16, &mut clock_rect, DT_CENTER | DT_NOPREFIX);

                    // 2. Date Display
                    SelectObject(hdc, date_font);
                    SetTextColor(hdc, COLORREF(0x00A09890)); // Soft Silver Warm Gray
                    let mut date_rect = RECT {
                        left: ml + 20,
                        top: cy - 185,
                        right: mr - 20,
                        bottom: cy - 160,
                    };
                    let mut date_utf16: Vec<u16> = format!("{}\0", date_str).encode_utf16().collect();
                    DrawTextW(hdc, &mut date_utf16, &mut date_rect, DT_CENTER | DT_NOPREFIX);

                    // 3. Security Status Capsule Badge
                    let badge_w = 264;
                    let badge_h = 32;
                    let badge_x = cx - badge_w / 2;
                    let badge_y = cy - 145;

                    SelectObject(hdc, badge_border_pen);
                    SelectObject(hdc, badge_bg_brush);
                    let _ = RoundRect(hdc, badge_x, badge_y, badge_x + badge_w, badge_y + badge_h, 16, 16);

                    // Glowing Emerald Indicator Dot inside Capsule
                    SelectObject(hdc, null_pen);
                    SelectObject(hdc, mint_brush);
                    let dot_x = badge_x + 16;
                    let dot_y = badge_y + 12;
                    let _ = RoundRect(hdc, dot_x, dot_y, dot_x + 8, dot_y + 8, 8, 8);

                    // Badge Text: ENCRYPTED REMOTE SESSION
                    SelectObject(hdc, badge_font);
                    SetTextColor(hdc, COLORREF(0x00E2DBD5));
                    let mut badge_txt_rect = RECT {
                        left: dot_x + 14,
                        top: badge_y,
                        right: badge_x + badge_w - 12,
                        bottom: badge_y + badge_h,
                    };
                    let mut badge_utf16: Vec<u16> = "ENCRYPTED REMOTE SESSION\0".encode_utf16().collect();
                    DrawTextW(hdc, &mut badge_utf16, &mut badge_txt_rect, DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);

                    // 4. Modern Geometric Security Shield Icon
                    let icon_cx = cx;
                    let icon_cy = cy - 65;

                    // Shield outer polygon
                    SelectObject(hdc, null_pen);
                    SelectObject(hdc, shield_brush);
                    let shield_pts = [
                        POINT { x: icon_cx - 24, y: icon_cy - 24 },
                        POINT { x: icon_cx + 24, y: icon_cy - 24 },
                        POINT { x: icon_cx + 24, y: icon_cy + 2 },
                        POINT { x: icon_cx, y: icon_cy + 24 },
                        POINT { x: icon_cx - 24, y: icon_cy + 2 },
                    ];
                    let _ = Polygon(hdc, &shield_pts);

                    // Inner cutout for sleek layered lock aesthetic
                    SelectObject(hdc, shield_inner_brush);
                    let inner_pts = [
                        POINT { x: icon_cx - 18, y: icon_cy - 18 },
                        POINT { x: icon_cx + 18, y: icon_cy - 18 },
                        POINT { x: icon_cx + 18, y: icon_cy },
                        POINT { x: icon_cx, y: icon_cy + 17 },
                        POINT { x: icon_cx - 18, y: icon_cy },
                    ];
                    let _ = Polygon(hdc, &inner_pts);

                    // Padlock Keyhole in glowing Mint
                    SelectObject(hdc, mint_brush);
                    let _ = RoundRect(hdc, icon_cx - 4, icon_cy - 8, icon_cx + 4, icon_cy, 4, 4);
                    let key_stem = [
                        POINT { x: icon_cx - 3, y: icon_cy - 2 },
                        POINT { x: icon_cx + 3, y: icon_cy - 2 },
                        POINT { x: icon_cx + 2, y: icon_cy + 6 },
                        POINT { x: icon_cx - 2, y: icon_cy + 6 },
                    ];
                    let _ = Polygon(hdc, &key_stem);

                    // 5. Title & Subtitle
                    SelectObject(hdc, title_font);
                    SetTextColor(hdc, COLORREF(0x00FFFFFF)); // Solid Pure White
                    let mut title_rect = RECT {
                        left: ml + 20,
                        top: cy - 20,
                        right: mr - 20,
                        bottom: cy + 12,
                    };
                    let mut title_text: Vec<u16> = "Windows Display Protected\0".encode_utf16().collect();
                    DrawTextW(hdc, &mut title_text, &mut title_rect, DT_CENTER | DT_NOPREFIX);

                    SelectObject(hdc, sub_font);
                    SetTextColor(hdc, COLORREF(0x008E857D)); // Soft Muted Slate
                    let mut sub_rect = RECT {
                        left: ml + 20,
                        top: cy + 14,
                        right: mr - 20,
                        bottom: cy + 38,
                    };
                    let mut sub_text: Vec<u16> = "Screen is shielded on laptop • Live controls active on phone\0".encode_utf16().collect();
                    DrawTextW(hdc, &mut sub_text, &mut sub_rect, DT_CENTER | DT_NOPREFIX);

                    // 6. Interactive PIN Slots Grid (Fluent Glass Slots)
                    let num_slots = pin_len.clamp(4, 8);
                    let slot_w = 48;
                    let slot_h = 56;
                    let slot_gap = 12;
                    let total_grid_w = (num_slots as i32) * slot_w + ((num_slots as i32) - 1) * slot_gap;
                    let start_x = cx - total_grid_w / 2;
                    let slot_y = cy + 50;

                    for i in 0..num_slots {
                        let sx = start_x + (i as i32) * (slot_w + slot_gap);

                        if i < pin_len {
                            // Filled Slot
                            SelectObject(hdc, slot_filled_pen);
                            SelectObject(hdc, slot_filled_brush);
                            let _ = RoundRect(hdc, sx, slot_y, sx + slot_w, slot_y + slot_h, 14, 14);

                            // Glowing Emerald Dot: ●
                            SelectObject(hdc, pin_dot_font);
                            SetTextColor(hdc, COLORREF(0x009DFF00)); // Sway Emerald Mint
                            let mut dot_rect = RECT {
                                left: sx,
                                top: slot_y,
                                right: sx + slot_w,
                                bottom: slot_y + slot_h,
                            };
                            let mut dot_text: Vec<u16> = "●\0".encode_utf16().collect();
                            DrawTextW(hdc, &mut dot_text, &mut dot_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
                        } else if i == pin_len {
                            // Active / Next Slot with focus highlight
                            SelectObject(hdc, slot_active_pen);
                            SelectObject(hdc, slot_empty_brush);
                            let _ = RoundRect(hdc, sx, slot_y, sx + slot_w, slot_y + slot_h, 14, 14);
                        } else {
                            // Inactive Empty Slot
                            SelectObject(hdc, slot_empty_pen);
                            SelectObject(hdc, slot_empty_brush);
                            let _ = RoundRect(hdc, sx, slot_y, sx + slot_w, slot_y + slot_h, 14, 14);
                        }
                    }

                    // 7. Error or Interaction Feedback Status
                    SelectObject(hdc, hint_font);
                    let mut feedback_rect = RECT {
                        left: ml + 20,
                        top: slot_y + slot_h + 16,
                        right: mr - 20,
                        bottom: slot_y + slot_h + 38,
                    };

                    if is_error {
                        SetTextColor(hdc, COLORREF(0x005555FF)); // Soft Coral Red
                        let mut err_text: Vec<u16> = "⚠ Incorrect PIN. Please try again.\0".encode_utf16().collect();
                        DrawTextW(hdc, &mut err_text, &mut feedback_rect, DT_CENTER | DT_NOPREFIX);
                    } else if pin_len == 0 {
                        SetTextColor(hdc, COLORREF(0x0080766E)); // Muted Guidance Gray
                        let mut hint_text: Vec<u16> = "Type laptop PIN or Sway token on keyboard to unlock\0".encode_utf16().collect();
                        DrawTextW(hdc, &mut hint_text, &mut feedback_rect, DT_CENTER | DT_NOPREFIX);
                    } else {
                        SetTextColor(hdc, COLORREF(0x009DFF00)); // Glowing Active
                        let mut hint_text: Vec<u16> = "Press Enter to verify • Backspace to erase\0".encode_utf16().collect();
                        DrawTextW(hdc, &mut hint_text, &mut feedback_rect, DT_CENTER | DT_NOPREFIX);
                    }

                    // 8. Bottom Helper Badges
                    let pill_tag_w = 140;
                    let pill_tag_h = 26;
                    let pill_tag_gap = 10;
                    let pill_tag_y = feedback_rect.bottom + 18;

                    let tags = [
                        "⌨ Keyboard Active",
                        "⏎ Enter = Unlock",
                        "ESC = Clear",
                    ];
                    let total_tags_w = (tags.len() as i32) * pill_tag_w + ((tags.len() as i32) - 1) * pill_tag_gap;
                    let start_tag_x = cx - total_tags_w / 2;

                    SelectObject(hdc, pill_tag_font);
                    SetTextColor(hdc, COLORREF(0x007A7067)); // Muted Warm Slate

                    for (idx, tag) in tags.iter().enumerate() {
                        let tx = start_tag_x + (idx as i32) * (pill_tag_w + pill_tag_gap);
                        SelectObject(hdc, pill_border_pen);
                        SelectObject(hdc, pill_bg_brush);
                        let _ = RoundRect(hdc, tx, pill_tag_y, tx + pill_tag_w, pill_tag_y + pill_tag_h, 8, 8);

                        let mut tag_rect = RECT {
                            left: tx,
                            top: pill_tag_y,
                            right: tx + pill_tag_w,
                            bottom: pill_tag_y + pill_tag_h,
                        };
                        let mut tag_utf16: Vec<u16> = format!("{}\0", tag).encode_utf16().collect();
                        DrawTextW(hdc, &mut tag_utf16, &mut tag_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
                    }

                    // 9. Configurable Footnote
                    let mut foot_rect = RECT {
                        left: ml + 20,
                        top: pill_tag_y + pill_tag_h + 14,
                        right: mr - 20,
                        bottom: pill_tag_y + pill_tag_h + 34,
                    };
                    SetTextColor(hdc, COLORREF(0x004A423C)); // Subtle Deep Slate
                    let mut foot_text: Vec<u16> = "Default unlock PIN: 1234 or your Sway pairing token\0".encode_utf16().collect();
                    DrawTextW(hdc, &mut foot_text, &mut foot_rect, DT_CENTER | DT_NOPREFIX);
                }

                // Clean up GDI resources
                SelectObject(hdc, old_pen);
                let _ = DeleteObject(null_pen);
                let _ = DeleteObject(badge_bg_brush);
                let _ = DeleteObject(badge_border_pen);
                let _ = DeleteObject(shield_brush);
                let _ = DeleteObject(shield_inner_brush);
                let _ = DeleteObject(mint_brush);
                let _ = DeleteObject(slot_empty_brush);
                let _ = DeleteObject(slot_empty_pen);
                let _ = DeleteObject(slot_filled_brush);
                let _ = DeleteObject(slot_filled_pen);
                let _ = DeleteObject(slot_active_pen);
                let _ = DeleteObject(pill_bg_brush);
                let _ = DeleteObject(pill_border_pen);
                let _ = DeleteObject(clock_font);
                let _ = DeleteObject(date_font);
                let _ = DeleteObject(badge_font);
                let _ = DeleteObject(title_font);
                let _ = DeleteObject(sub_font);
                let _ = DeleteObject(pin_dot_font);
                let _ = DeleteObject(hint_font);
                let _ = DeleteObject(pill_tag_font);

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
