use image::codecs::jpeg::JpegEncoder;
use image::ColorType;
use tracing::info;
use xcap::Monitor;

#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub index: usize,
    pub name: String,
    pub width: i32,
    pub height: i32,
    pub is_primary: bool,
}

#[cfg(windows)]
fn draw_real_cursor(img: &mut image::RgbaImage, mon_x: i32, mon_y: i32, mon_w: u32, mon_h: u32) {
    use std::sync::atomic::{AtomicIsize, AtomicI32, Ordering};
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject,
        BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HDC,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        DrawIconEx, GetCursorInfo, GetIconInfo, GetSystemMetrics, CURSORINFO, CURSOR_SHOWING, DI_NORMAL,
        SM_CXCURSOR, SM_CYCURSOR,
    };

    // Cache the last cursor handle + position to skip redundant redraws when cursor hasn't changed.
    // This eliminates visible flicker during typing (rapid I-beam ↔ arrow shape transitions).
    static LAST_CURSOR_HANDLE: AtomicIsize = AtomicIsize::new(0);
    static LAST_CURSOR_X: AtomicI32 = AtomicI32::new(i32::MIN);
    static LAST_CURSOR_Y: AtomicI32 = AtomicI32::new(i32::MIN);

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

        // Fast-path: if cursor handle AND position are identical to last frame, skip full redraw.
        // Only the handle matters for shape flicker; position check avoids missing movement.
        let cur_handle = ci.hCursor.0 as isize;
        let prev_handle = LAST_CURSOR_HANDLE.load(Ordering::Relaxed);
        let prev_x = LAST_CURSOR_X.load(Ordering::Relaxed);
        let prev_y = LAST_CURSOR_Y.load(Ordering::Relaxed);
        let _same_cursor = cur_handle == prev_handle
            && ci.ptScreenPos.x == prev_x
            && ci.ptScreenPos.y == prev_y;

        // Update cache for next frame
        LAST_CURSOR_HANDLE.store(cur_handle, Ordering::Relaxed);
        LAST_CURSOR_X.store(ci.ptScreenPos.x, Ordering::Relaxed);
        LAST_CURSOR_Y.store(ci.ptScreenPos.y, Ordering::Relaxed);

        // Use a thread-local cursor bitmap cache to avoid expensive GDI DrawIconEx calls
        // when the cursor shape (handle) hasn't changed between frames. This eliminates
        // visible flicker during keyboard input where the cursor rapidly switches shapes.
        use std::cell::RefCell;
        struct CachedCursor {
            handle: isize,
            width: i32,
            height: i32,
            hotspot_x: i32,
            hotspot_y: i32,
            rgba_data: Vec<u8>, // Pre-rendered BGRA pixel data
        }
        thread_local! {
            static CURSOR_CACHE: RefCell<Option<CachedCursor>> = RefCell::new(None);
        }

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

        let cur_x = ci.ptScreenPos.x;
        let cur_y = ci.ptScreenPos.y;

        let local_x = cur_x - mon_x;
        let local_y = cur_y - mon_y;
        if local_x < -64 || local_x >= mon_w as i32 + 64 || local_y < -64 || local_y >= mon_h as i32 + 64 {
            return;
        }

        let img_w = img.width() as f64;
        let img_h = img.height() as f64;
        let scale_x = img_w / (mon_w as f64).max(1.0);
        let scale_y = img_h / (mon_h as f64).max(1.0);

        let target_cur_x = ((local_x as f64) * scale_x).round() as i32;
        let target_cur_y = ((local_y as f64) * scale_y).round() as i32;

        let cur_w = GetSystemMetrics(SM_CXCURSOR).max(32);
        let cur_h = GetSystemMetrics(SM_CYCURSOR).max(32);
        let scaled_cur_w = ((cur_w as f64) * scale_x).round().clamp(16.0, 128.0) as i32;
        let scaled_cur_h = ((cur_h as f64) * scale_y).round().clamp(16.0, 128.0) as i32;

        // Check if we can reuse the cached cursor bitmap (same handle = same shape)
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
            // Cursor shape changed — re-render via GDI and cache the result
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
                    ).is_ok();

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

        // Composite the cached cursor bitmap onto the frame
        CURSOR_CACHE.with(|cache| {
            let cache_ref = cache.borrow();
            if let Some(ref cached) = *cache_ref {
                let scaled_hotspot_x = ((cached.hotspot_x as f64) * scale_x).round() as i32;
                let scaled_hotspot_y = ((cached.hotspot_y as f64) * scale_y).round() as i32;
                let draw_origin_x = target_cur_x - scaled_hotspot_x;
                let draw_origin_y = target_cur_y - scaled_hotspot_y;

                for cy in 0..cached.height {
                    for cx in 0..cached.width {
                        let idx = ((cy * cached.width + cx) * 4) as usize;
                        let b = cached.rgba_data[idx];
                        let g = cached.rgba_data[idx + 1];
                        let r = cached.rgba_data[idx + 2];
                        let a = cached.rgba_data[idx + 3];

                        if a == 0 && r == 0 && g == 0 && b == 0 {
                            continue;
                        }

                        let px = draw_origin_x + cx;
                        let py = draw_origin_y + cy;

                        if px >= 0 && px < img.width() as i32 && py >= 0 && py < img.height() as i32 {
                            let alpha = if a > 0 { a } else { 255 } as f32 / 255.0;
                            let dest = img.get_pixel(px as u32, py as u32);
                            let out_r = (r as f32 * alpha + dest[0] as f32 * (1.0 - alpha)) as u8;
                            let out_g = (g as f32 * alpha + dest[1] as f32 * (1.0 - alpha)) as u8;
                            let out_b = (b as f32 * alpha + dest[2] as f32 * (1.0 - alpha)) as u8;
                            img.put_pixel(px as u32, py as u32, image::Rgba([out_r, out_g, out_b, 255]));
                        }
                    }
                }
            }
        });
    }
}

#[cfg(not(windows))]
fn draw_real_cursor(_img: &mut image::RgbaImage, _mon_x: i32, _mon_y: i32, _mon_w: u32, _mon_h: u32) {}

pub struct ScreenCapture {
    pub active_monitor: usize,
    pub monitors: Vec<MonitorInfo>,
    cached_monitors: Vec<Monitor>,
    rgb_buffer: Vec<u8>,
    last_frame_time: std::time::Instant,
    last_monitor_refresh: std::time::Instant,
}

impl ScreenCapture {
    pub fn new() -> Self {
        #[cfg(windows)]
        {
            extern "system" {
                fn OpenInputDesktop(dwFlags: u32, fInherit: i32, dwDesiredAccess: u32) -> isize;
                fn SetThreadDesktop(hDesktop: isize) -> i32;
                fn CloseDesktop(hDesktop: isize) -> i32;
            }
            unsafe {
                let h_desk = OpenInputDesktop(0, 0, 0x01FF);
                if h_desk != 0 {
                    let _ = SetThreadDesktop(h_desk);
                    let _ = CloseDesktop(h_desk);
                }
            }
        }

        let monitors = Self::detect_monitors();
        let cached_monitors = Monitor::all().unwrap_or_default();
        let mut active_index = 0;

        // Auto-detect which display actually has active desktop content (ignoring alpha channel)
        for (idx, m) in cached_monitors.iter().enumerate() {
            if let Ok(img) = m.capture_image() {
                let has_content = img.chunks_exact(4).any(|c| c[0] > 15 || c[1] > 15 || c[2] > 15);
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
            rgb_buffer: Vec::with_capacity(1920 * 1080 * 3),
            last_frame_time: std::time::Instant::now(),
            last_monitor_refresh: std::time::Instant::now(),
        }
    }

    #[allow(dead_code)]
    pub fn set_active_monitor(&mut self, index: usize) -> bool {
        if index < self.monitors.len() {
            self.active_monitor = index;
            info!("Switched active capture monitor to: #{}", index);
            true
        } else {
            false
        }
    }

    pub fn detect_monitors() -> Vec<MonitorInfo> {
        if let Ok(all) = Monitor::all() {
            if !all.is_empty() {
                return all
                    .into_iter()
                    .enumerate()
                    .map(|(i, m)| {
                        let name = m.name().unwrap_or_else(|_| format!("Display #{}", i));
                        let width = m.width().unwrap_or(1920) as i32;
                        let height = m.height().unwrap_or(1080) as i32;
                        let is_primary = m.is_primary().unwrap_or(false);
                        MonitorInfo {
                            index: i,
                            name,
                            width,
                            height,
                            is_primary,
                        }
                    })
                    .collect();
            }
        }

        vec![MonitorInfo {
            index: 0,
            name: "Primary Display".to_string(),
            width: 1920,
            height: 1080,
            is_primary: true,
        }]
    }

    pub fn capture_frame_jpeg(&mut self, quality: u8) -> Option<Vec<u8>> {
        #[cfg(windows)]
        crate::input::InputSimulator::ensure_input_desktop();

        if self.cached_monitors.is_empty() {
            if let Ok(all) = Monitor::all() {
                self.cached_monitors = all;
                self.last_monitor_refresh = std::time::Instant::now();
            }
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
                // If capture failed (e.g. resolution change, desktop switch, or DXGI reset),
                // refresh monitors if at least 250ms have passed
                if self.last_monitor_refresh.elapsed() >= std::time::Duration::from_millis(250) {
                    if let Ok(all) = Monitor::all() {
                        self.cached_monitors = all;
                        self.last_monitor_refresh = std::time::Instant::now();
                        self.cached_monitors.get(monitor_idx).and_then(|m| m.capture_image().ok())
                    } else {
                        None
                    }
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

        let (mon_x, mon_y, mon_w, mon_h) = if let Some(m) = self.cached_monitors.get(monitor_idx) {
            (
                m.x().unwrap_or(0),
                m.y().unwrap_or(0),
                m.width().unwrap_or(width).max(1),
                m.height().unwrap_or(height).max(1),
            )
        } else {
            (0, 0, width.max(1), height.max(1))
        };

        // Adaptive resolution scaling:
        // Turbo / mobile data (quality <= 55) uses 1280 (720p) for ultra-low latency & small JPEG packets (~35 KB).
        // High fidelity uses full 1080p / 1920.
        // Uses FilterType::Nearest for instant sub-millisecond scaling instead of slow CPU Triangle convolution.
        let max_dim = match quality {
            0..=55 => 1280,
            56..=75 => 1440,
            _ => 1920,
        };
        let mut target_image = if width > max_dim || height > max_dim {
            let (target_w, target_h) = if width >= height {
                (max_dim, (height * max_dim) / width)
            } else {
                ((width * max_dim) / height, max_dim)
            };
            image::imageops::resize(
                &rgba_image,
                target_w,
                target_h,
                image::imageops::FilterType::Nearest,
            )
        } else {
            rgba_image
        };

        // Render real hardware mouse cursor overlay directly on the frame
        draw_real_cursor(&mut target_image, mon_x, mon_y, mon_w, mon_h);

        // Fast zero-allocation RGBA to RGB conversion into reusable pre-allocated buffer
        let raw_rgba = target_image.as_raw();
        let pixel_count = (target_image.width() * target_image.height()) as usize;
        let rgb_len = pixel_count * 3;
        if self.rgb_buffer.len() < rgb_len {
            self.rgb_buffer.resize(rgb_len, 0);
        }

        let dst = &mut self.rgb_buffer[..rgb_len];
        let src = raw_rgba;
        let mut s = 0;
        let mut d = 0;
        while d < rgb_len {
            dst[d] = src[s];
            dst[d + 1] = src[s + 1];
            dst[d + 2] = src[s + 2];
            s += 4;
            d += 3;
        }

        self.last_frame_time = std::time::Instant::now();

        let mut jpeg_bytes = Vec::with_capacity(32 * 1024);
        let mut encoder = JpegEncoder::new_with_quality(&mut jpeg_bytes, quality);
        if encoder
            .encode(
                dst,
                target_image.width(),
                target_image.height(),
                ColorType::Rgb8.into(),
            )
            .is_ok()
        {
            Some(jpeg_bytes)
        } else {
            None
        }
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
            let hbm = CreateCompatibleBitmap(hdc_screen, w, h);
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

            // Convert BGRA to RGBA
            for chunk in raw_pixels.chunks_exact_mut(4) {
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
        if let Ok(monitors) = Monitor::all() {
            for (i, m) in monitors.iter().enumerate() {
                println!("Monitor {}: x={}, y={}, w={}, h={}", 
                    i, 
                    m.x().unwrap_or(0), 
                    m.y().unwrap_or(0), 
                    m.width().unwrap_or(0), 
                    m.height().unwrap_or(0)
                );
            }
        }
        let frame = capture.capture_frame_jpeg(50);
        assert!(frame.is_some(), "Capture failed to produce a frame");
        let bytes = frame.unwrap();
        println!("Captured JPEG size: {} bytes", bytes.len());
        assert!(bytes.len() > 1000);
        assert_eq!(bytes[0], 0xFF);
        assert_eq!(bytes[1], 0xD8);
        assert_eq!(bytes[2], 0xFF);
    }
}
