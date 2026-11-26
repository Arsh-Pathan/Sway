use libjpeg_turbo_rs::{compress, PixelFormat, Subsampling};
use std::sync::atomic::Ordering;
use tracing::info;
use xcap::Monitor;

#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub index: usize,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub is_primary: bool,
}

#[cfg(windows)]
fn draw_real_cursor(raw: &mut [u8], img_w: u32, img_h: u32, mon_x: i32, mon_y: i32, mon_w: u32, mon_h: u32) {
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject,
        BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HDC,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        DrawIconEx, GetCursorInfo, GetIconInfo, GetSystemMetrics, CURSORINFO, CURSOR_SHOWING, DI_NORMAL,
        SM_CXCURSOR, SM_CYCURSOR,
    };

    let mut ci = CURSORINFO {
        cbSize: std::mem::size_of::<CURSORINFO>() as u32,
        ..Default::default()
    };

    unsafe {
        if GetCursorInfo(&mut ci).is_err() {
            return;
        }
        if (ci.flags.0 & CURSOR_SHOWING.0) == 0 || ci.hCursor.is_invalid() {
            return;
        }

        let cur_handle = ci.hCursor.0 as isize;
        let cur_x = ci.ptScreenPos.x;
        let cur_y = ci.ptScreenPos.y;

        let local_x = cur_x - mon_x;
        let local_y = cur_y - mon_y;
        if local_x < -64 || local_x >= mon_w as i32 + 64 || local_y < -64 || local_y >= mon_h as i32 + 64 {
            return;
        }

        let scale_x = img_w as f64 / (mon_w as f64).max(1.0);
        let scale_y = img_h as f64 / (mon_h as f64).max(1.0);

        let target_cur_x = ((local_x as f64) * scale_x).round() as i32;
        let target_cur_y = ((local_y as f64) * scale_y).round() as i32;

        let cur_w = GetSystemMetrics(SM_CXCURSOR).max(32);
        let cur_h = GetSystemMetrics(SM_CYCURSOR).max(32);
        let scaled_cur_w = ((cur_w as f64) * scale_x).round().clamp(16.0, 128.0) as i32;
        let scaled_cur_h = ((cur_h as f64) * scale_y).round().clamp(16.0, 128.0) as i32;

        use std::cell::RefCell;
        struct CachedCursor {
            handle: isize,
            width: i32,
            height: i32,
            hotspot_x: i32,
            hotspot_y: i32,
            rgba_data: Vec<u8>,
        }
        thread_local! {
            static CURSOR_CACHE: RefCell<Option<CachedCursor>> = const { RefCell::new(None) };
        }

        // Check cursor cache BEFORE calling GetIconInfo so we don't allocate/delete 2 GDI bitmaps per frame.
        let cache_hit = CURSOR_CACHE.with(|cache| {
            let cache_ref = cache.borrow();
            if let Some(ref cached) = *cache_ref {
                cached.handle == cur_handle
                    && cached.width == scaled_cur_w
                    && cached.height == scaled_cur_h
            } else {
                false
            }
        });

        if !cache_hit {
            let mut ii = windows::Win32::UI::WindowsAndMessaging::ICONINFO::default();
            if GetIconInfo(ci.hCursor, &mut ii).is_err() {
                return;
            }
            let hotspot_x = ii.xHotspot as i32;
            let hotspot_y = ii.yHotspot as i32;
            if !ii.hbmMask.is_invalid() {
                let _ = DeleteObject(ii.hbmMask);
            }
            if !ii.hbmColor.is_invalid() {
                let _ = DeleteObject(ii.hbmColor);
            }

            let hdc_mem = CreateCompatibleDC(HDC::default());
            if hdc_mem.is_invalid() {
                return;
            }

            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: scaled_cur_w,
                    biHeight: -scaled_cur_h, // Top-down DIB
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };

            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            if let Ok(hbm) = CreateDIBSection(hdc_mem, &bmi, DIB_RGB_COLORS, &mut bits, None, 0) {
                if !bits.is_null() {
                    let old_bm = SelectObject(hdc_mem, hbm);
                    std::ptr::write_bytes(bits as *mut u8, 0, (scaled_cur_w * scaled_cur_h * 4) as usize);

                    let drawn = DrawIconEx(
                        hdc_mem,
                        0,
                        0,
                        ci.hCursor,
                        scaled_cur_w,
                        scaled_cur_h,
                        0,
                        None,
                        DI_NORMAL,
                    )
                    .is_ok();

                    SelectObject(hdc_mem, old_bm);

                    if drawn {
                        let data_len = (scaled_cur_w * scaled_cur_h * 4) as usize;
                        let pixel_slice = std::slice::from_raw_parts(bits as *const u8, data_len);
                        CURSOR_CACHE.with(|cache| {
                            *cache.borrow_mut() = Some(CachedCursor {
                                handle: cur_handle,
                                width: scaled_cur_w,
                                height: scaled_cur_h,
                                hotspot_x,
                                hotspot_y,
                                rgba_data: pixel_slice.to_vec(),
                            });
                        });
                    }
                }
                let _ = DeleteObject(hbm);
            }
            let _ = DeleteDC(hdc_mem);
        }

        // Composite the cached cursor bitmap onto the frame.
        // DrawIconEx into a zeroed 32-bit DIB outputs premultiplied BGRA (or a=0 for 1-bit masked cursors).
        let drew_cursor = CURSOR_CACHE.with(|cache| {
            let cache_ref = cache.borrow();
            if let Some(ref cached) = *cache_ref {
                let scaled_hotspot_x = ((cached.hotspot_x as f64) * scale_x).round() as i32;
                let scaled_hotspot_y = ((cached.hotspot_y as f64) * scale_y).round() as i32;
                let draw_origin_x = target_cur_x - scaled_hotspot_x;
                let draw_origin_y = target_cur_y - scaled_hotspot_y;
                let mut drawn_any = false;

                for cy in 0..cached.height {
                    let py = draw_origin_y + cy;
                    if py < 0 || py >= img_h as i32 {
                        continue;
                    }
                    for cx in 0..cached.width {
                        let idx = ((cy * cached.width + cx) * 4) as usize;
                        let b = cached.rgba_data[idx] as u32;
                        let g = cached.rgba_data[idx + 1] as u32;
                        let r = cached.rgba_data[idx + 2] as u32;
                        let a = cached.rgba_data[idx + 3] as u32;

                        if a == 0 && r == 0 && g == 0 && b == 0 {
                            continue;
                        }

                        let px = draw_origin_x + cx;
                        if px >= 0 && px < img_w as i32 {
                            let offset = (py as usize * img_w as usize + px as usize) * 4;
                            if offset + 4 <= raw.len() {
                                let dest = &mut raw[offset..offset + 4];
                                if a > 0 {
                                    let inv_alpha = 255 - a.min(255);
                                    dest[0] = (r + (dest[0] as u32 * inv_alpha) / 255).min(255) as u8;
                                    dest[1] = (g + (dest[1] as u32 * inv_alpha) / 255).min(255) as u8;
                                    dest[2] = (b + (dest[2] as u32 * inv_alpha) / 255).min(255) as u8;
                                } else {
                                    dest[0] = r as u8;
                                    dest[1] = g as u8;
                                    dest[2] = b as u8;
                                }
                                dest[3] = 255;
                                drawn_any = true;
                            }
                        }
                    }
                }
                drawn_any
            } else {
                false
            }
        });

        // Fallback: If system cursor is hidden/blanked (Privacy Mode), draw crisp standard arrow pointer on phone stream.
        if !drew_cursor {
            const ARROW: [[u8; 12]; 16] = [
                [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                [1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                [1, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                [1, 2, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0],
                [1, 2, 2, 2, 1, 0, 0, 0, 0, 0, 0, 0],
                [1, 2, 2, 2, 2, 1, 0, 0, 0, 0, 0, 0],
                [1, 2, 2, 2, 2, 2, 1, 0, 0, 0, 0, 0],
                [1, 2, 2, 2, 2, 2, 2, 1, 0, 0, 0, 0],
                [1, 2, 2, 2, 2, 2, 2, 2, 1, 0, 0, 0],
                [1, 2, 2, 2, 2, 2, 1, 1, 1, 1, 0, 0],
                [1, 2, 2, 1, 2, 2, 1, 0, 0, 0, 0, 0],
                [1, 2, 1, 0, 1, 2, 2, 1, 0, 0, 0, 0],
                [1, 1, 0, 0, 1, 2, 2, 1, 0, 0, 0, 0],
                [1, 0, 0, 0, 0, 1, 2, 2, 1, 0, 0, 0],
                [0, 0, 0, 0, 0, 1, 2, 2, 1, 0, 0, 0],
                [0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0],
            ];
            for (dy, row) in ARROW.iter().enumerate() {
                for (dx, &val) in row.iter().enumerate() {
                    if val == 0 {
                        continue;
                    }
                    let px = target_cur_x + dx as i32;
                    let py = target_cur_y + dy as i32;
                    if px >= 0 && px < img_w as i32 && py >= 0 && py < img_h as i32 {
                        let offset = (py as usize * img_w as usize + px as usize) * 4;
                        if offset + 4 <= raw.len() {
                            let dest = &mut raw[offset..offset + 4];
                            let c = if val == 1 { 0 } else { 255 };
                            dest[0] = c;
                            dest[1] = c;
                            dest[2] = c;
                            dest[3] = 255;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(not(windows))]
fn draw_real_cursor(_raw: &mut [u8], _img_w: u32, _img_h: u32, _mon_x: i32, _mon_y: i32, _mon_w: u32, _mon_h: u32) {}

#[cfg(windows)]
fn get_current_cursor_state() -> (isize, i32, i32) {
    use windows::Win32::UI::WindowsAndMessaging::{GetCursorInfo, CURSORINFO};
    let mut ci = CURSORINFO {
        cbSize: std::mem::size_of::<CURSORINFO>() as u32,
        ..Default::default()
    };
    unsafe {
        if GetCursorInfo(&mut ci).is_ok() {
            (ci.hCursor.0 as isize, ci.ptScreenPos.x, ci.ptScreenPos.y)
        } else {
            (0, i32::MIN, i32::MIN)
        }
    }
}

#[cfg(not(windows))]
fn get_current_cursor_state() -> (isize, i32, i32) {
    (0, 0, 0)
}

#[inline(always)]
fn compute_sample_hash(data: &[u8]) -> u64 {
    let len = data.len();
    if len < 8 {
        return 0;
    }
    // Tightened pixel-aligned stride (~16K sample points across 1080p) catches thin 1px carets while taking <0.2ms
    let step = (len / 16384).clamp(8, 64) & !3;
    let mut h: u64 = 0xcbf29ce484222325;
    let mut i = 0;
    while i + 8 <= len {
        let chunk = u64::from_le_bytes([
            data[i], data[i + 1], data[i + 2], data[i + 3],
            data[i + 4], data[i + 5], data[i + 6], data[i + 7],
        ]);
        h ^= chunk;
        h = h.wrapping_mul(0x100000001b3);
        i += step;
    }
    h
}

pub struct ScreenCapture {
    pub active_monitor: usize,
    pub monitors: Vec<MonitorInfo>,
    cached_monitors: Vec<Monitor>,
    scaled_rgba_buffer: Vec<u8>,
    x_lut: Vec<usize>,
    x_lut_target_w: u32,
    x_lut_src_w: u32,
    last_frame_time: std::time::Instant,
    last_monitor_refresh: std::time::Instant,
    last_display_gen: usize,
    last_frame_hash: u64,
    last_cursor_state: (isize, i32, i32),
    last_encoded_time: std::time::Instant,
}

impl ScreenCapture {
    fn build_monitor_info(all: &[Monitor]) -> Vec<MonitorInfo> {
        if all.is_empty() {
            return vec![MonitorInfo {
                index: 0,
                name: "Primary Display".to_string(),
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
                is_primary: true,
            }];
        }
        all.iter()
            .enumerate()
            .map(|(i, m)| MonitorInfo {
                index: i,
                name: m.name().unwrap_or_else(|_| format!("Display #{}", i)),
                x: m.x().unwrap_or(0),
                y: m.y().unwrap_or(0),
                width: m.width().unwrap_or(1920).max(1) as i32,
                height: m.height().unwrap_or(1080).max(1) as i32,
                is_primary: m.is_primary().unwrap_or(false),
            })
            .collect()
    }

    pub fn new() -> Self {
        #[cfg(windows)]
        {
            crate::input::InputSimulator::ensure_input_desktop();
            unsafe {
                use windows::Win32::UI::HiDpi::{
                    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE,
                    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
                };
                if SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).is_err() {
                    let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE);
                }
            }
        }

        // Single Monitor::all() query instead of enumerating twice on startup
        let cached_monitors = Monitor::all().unwrap_or_default();
        let monitors = Self::build_monitor_info(&cached_monitors);
        let mut active_index = 0;

        // Auto-detect which display actually has active desktop content (ignoring alpha channel)
        for (idx, m) in cached_monitors.iter().enumerate() {
            if let Ok(img) = m.capture_image() {
                let has_content = img.as_chunks::<4>().0.iter().any(|c| c[0] > 15 || c[1] > 15 || c[2] > 15);
                if has_content {
                    active_index = idx;
                    info!("Auto-detected active desktop stream on Monitor #{} ({:?})", idx, m.name());
                    break;
                }
            }
        }

        info!("Detected {} connected monitor(s)", monitors.len());
        for m in &monitors {
            info!("  Monitor #{}: {} ({}x{}) primary={}", m.index, m.name, m.width, m.height, m.is_primary);
        }

        Self {
            active_monitor: active_index,
            monitors,
            cached_monitors,
            scaled_rgba_buffer: Vec::new(), // Lazy allocation only if downsampling is needed
            x_lut: Vec::new(),
            x_lut_target_w: 0,
            x_lut_src_w: 0,
            last_frame_time: std::time::Instant::now(),
            last_monitor_refresh: std::time::Instant::now(),
            last_display_gen: crate::input::DISPLAY_GENERATION.load(Ordering::Relaxed),
            last_frame_hash: 0,
            last_cursor_state: (0, 0, 0),
            last_encoded_time: std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs(10))
                .unwrap_or_else(std::time::Instant::now),
        }
    }

    pub fn detect_monitors() -> Vec<MonitorInfo> {
        let all = Monitor::all().unwrap_or_default();
        Self::build_monitor_info(&all)
    }

    fn refresh_monitors_cache(&mut self) {
        if let Ok(all) = Monitor::all() {
            self.monitors = Self::build_monitor_info(&all);
            self.cached_monitors = all;
            self.last_monitor_refresh = std::time::Instant::now();
            self.last_frame_hash = 0;
        }
    }

    pub fn capture_frame_jpeg(&mut self, quality: u8) -> Option<Vec<u8>> {
        #[cfg(windows)]
        crate::input::InputSimulator::ensure_input_desktop();

        let cur_gen = crate::input::DISPLAY_GENERATION.load(Ordering::Relaxed);
        if self.cached_monitors.is_empty() || cur_gen != self.last_display_gen {
            self.refresh_monitors_cache();
            self.last_display_gen = cur_gen;
        }

        let monitor_idx = if self.active_monitor < self.cached_monitors.len() {
            self.active_monitor
        } else {
            0
        };

        let rgba_result = self.cached_monitors.get(monitor_idx).and_then(|m| m.capture_image().ok());

        let rgba_image = match rgba_result {
            Some(img) => img,
            None => {
                // If capture failed (e.g. resolution change, desktop switch, or DXGI reset), refresh after 250ms
                if self.last_monitor_refresh.elapsed() >= std::time::Duration::from_millis(250) {
                    self.refresh_monitors_cache();
                    self.cached_monitors.get(monitor_idx).and_then(|m| m.capture_image().ok())
                } else {
                    None
                }
                .or_else(|| {
                    #[cfg(windows)]
                    {
                        Self::capture_gdi_fallback()
                    }
                    #[cfg(not(windows))]
                    {
                        None
                    }
                })?
            }
        };

        let width = rgba_image.width();
        let height = rgba_image.height();
        if width == 0 || height == 0 {
            return None;
        }

        // Check if cached monitor bounds went stale (e.g. external Windows resolution change)
        if let Some(info) = self.monitors.get(monitor_idx) {
            if info.width != width as i32 || info.height != height as i32 {
                self.refresh_monitors_cache();
            }
        }

        // Real-Time Screen Change Detection:
        // On identical static screens, send a 4 FPS (250ms) keepalive instead of wasting 30 JPEG encodes/sec.
        // On ANY screen or cursor change, encode & stream immediately at up to 60 FPS!
        let cur_cursor = get_current_cursor_state();
        let frame_hash = compute_sample_hash(rgba_image.as_raw());
        let is_identical = frame_hash == self.last_frame_hash && cur_cursor == self.last_cursor_state;

        if is_identical && self.last_encoded_time.elapsed() < std::time::Duration::from_millis(250) {
            return None;
        }

        self.last_frame_hash = frame_hash;
        self.last_cursor_state = cur_cursor;
        self.last_encoded_time = std::time::Instant::now();

        let (mon_x, mon_y, mon_w, mon_h) = if let Some(info) = self.monitors.get(monitor_idx) {
            (info.x, info.y, (info.width as u32).max(1), (info.height as u32).max(1))
        } else {
            (0, 0, width, height)
        };

        // Adaptive resolution scaling mapped to quality ranges across LAN and WAN
        let max_dim = match quality {
            0..=36 => 854,
            37..=52 => 960,
            53..=68 => 1280,
            _ => 1920,
        };

        let (target_w, target_h) = if width > max_dim || height > max_dim {
            if width >= height {
                (max_dim, ((height * max_dim) / width).max(1))
            } else {
                (((width * max_dim) / height).max(1), max_dim)
            }
        } else {
            (width, height)
        };

        let mut target_image = rgba_image;
        self.last_frame_time = std::time::Instant::now();

        let (final_slice, final_w, final_h) = if target_w == width && target_h == height {
            let raw = target_image.as_mut();
            draw_real_cursor(raw, width, height, mon_x, mon_y, mon_w, mon_h);
            (raw as &[u8], width, height)
        } else {
            if self.x_lut_target_w != target_w || self.x_lut_src_w != width {
                self.x_lut = (0..target_w)
                    .map(|x| ((x as usize * width as usize) / target_w as usize).min(width as usize - 1))
                    .collect();
                self.x_lut_target_w = target_w;
                self.x_lut_src_w = width;
            }
            let x_lut = &self.x_lut;

            let dst_len = (target_w * target_h * 4) as usize;
            if self.scaled_rgba_buffer.len() < dst_len {
                self.scaled_rgba_buffer.resize(dst_len, 0);
            }

            // Safe, UB-free aligned [u8; 4] slice downsampler (zero raw pointer casts)
            let src_pixels = target_image.as_raw().as_chunks::<4>().0;
            let dst_pixels = self.scaled_rgba_buffer[..dst_len].as_chunks_mut::<4>().0;
            let src_w = width as usize;
            let dst_w = target_w as usize;

            for (y, dst_row) in dst_pixels.chunks_exact_mut(dst_w).enumerate() {
                let src_y = ((y * height as usize) / target_h as usize).min(height as usize - 1);
                let src_row = &src_pixels[src_y * src_w..(src_y + 1) * src_w];
                for (x, dst_px) in dst_row.iter_mut().enumerate() {
                    *dst_px = src_row[x_lut[x]];
                }
            }

            draw_real_cursor(&mut self.scaled_rgba_buffer[..dst_len], target_w, target_h, mon_x, mon_y, mon_w, mon_h);
            (&self.scaled_rgba_buffer[..dst_len], target_w, target_h)
        };

        compress(
            final_slice,
            final_w as usize,
            final_h as usize,
            PixelFormat::Rgba,
            quality,
            Subsampling::S420,
        )
        .ok()
    }

    #[cfg(windows)]
    pub fn capture_gdi_fallback() -> Option<image::RgbaImage> {
        use windows::Win32::Graphics::Gdi::*;
        use windows::Win32::UI::WindowsAndMessaging::*;

        unsafe {
            let w = GetSystemMetrics(SM_CXSCREEN);
            let h = GetSystemMetrics(SM_CYSCREEN);
            if w <= 0 || h <= 0 {
                return None;
            }

            let hdc_screen = GetDC(None);
            if hdc_screen.is_invalid() {
                return None;
            }

            let hdc_mem = CreateCompatibleDC(hdc_screen);
            if hdc_mem.is_invalid() {
                ReleaseDC(None, hdc_screen);
                return None;
            }

            let hbm = CreateCompatibleBitmap(hdc_screen, w, h);
            if hbm.is_invalid() {
                let _ = DeleteDC(hdc_mem);
                ReleaseDC(None, hdc_screen);
                return None;
            }

            let old_bm = SelectObject(hdc_mem, hbm);
            let success = BitBlt(hdc_mem, 0, 0, w, h, hdc_screen, 0, 0, SRCCOPY).is_ok();
            SelectObject(hdc_mem, old_bm);

            if !success {
                let _ = DeleteObject(hbm);
                let _ = DeleteDC(hdc_mem);
                ReleaseDC(None, hdc_screen);
                return None;
            }

            let mut bi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h, // Top-down
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };

            let mut raw_pixels = vec![0u8; (w * h * 4) as usize];
            let lines = GetDIBits(
                hdc_mem,
                hbm,
                0,
                h as u32,
                Some(raw_pixels.as_mut_ptr() as *mut _),
                &mut bi,
                DIB_RGB_COLORS,
            );

            let _ = DeleteObject(hbm);
            let _ = DeleteDC(hdc_mem);
            ReleaseDC(None, hdc_screen);

            if lines == 0 {
                return None;
            }

            for chunk in raw_pixels.as_chunks_mut::<4>().0 {
                let b = chunk[0];
                let r = chunk[2];
                chunk[0] = r;
                chunk[2] = b;
                chunk[3] = 255;
            }

            image::RgbaImage::from_raw(w as u32, h as u32, raw_pixels)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_screen() {
        let mut capture = ScreenCapture::new();
        for m in &capture.monitors {
            println!("Monitor {}: x={}, y={}, w={}, h={}", m.index, m.x, m.y, m.width, m.height);
        }
        let mut success = 0;
        for q in [30, 45, 60, 75] {
            capture.last_frame_hash = 0; // force encode across all qualities
            let t0 = std::time::Instant::now();
            if let Some(frame) = capture.capture_frame_jpeg(q) {
                let ms = t0.elapsed().as_secs_f64() * 1000.0;
                println!("Quality {}: size = {} KB, time = {:.2} ms", q, frame.len() / 1024, ms);
                if !frame.is_empty() {
                    success += 1;
                }
            }
        }
        assert!(success > 0, "Should have captured at least 1 valid JPEG frame");
    }

    #[test]
    fn test_dpi_cursor_coordinates() {
        #[cfg(windows)]
        unsafe {
            use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
            use windows::Win32::UI::WindowsAndMessaging::{GetCursorInfo, GetSystemMetrics, CURSORINFO, SM_CXSCREEN, SM_CYSCREEN};

            let before_w = GetSystemMetrics(SM_CXSCREEN);
            let before_h = GetSystemMetrics(SM_CYSCREEN);
            println!("Before DPI V2: Screen = {}x{}", before_w, before_h);

            let res = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            println!("SetProcessDpiAwarenessContext result: {:?}", res.is_ok());

            let after_w = GetSystemMetrics(SM_CXSCREEN);
            let after_h = GetSystemMetrics(SM_CYSCREEN);
            println!("After DPI V2: Screen = {}x{}", after_w, after_h);
            assert!(after_w >= 1920, "DPI awareness should report physical monitor width >= 1920");

            let mut ci = CURSORINFO {
                cbSize: std::mem::size_of::<CURSORINFO>() as u32,
                ..Default::default()
            };
            let cur_ok = GetCursorInfo(&mut ci);
            println!("Cursor pos: ({}, {}), ok: {:?}", ci.ptScreenPos.x, ci.ptScreenPos.y, cur_ok.is_ok());
        }
    }
}
