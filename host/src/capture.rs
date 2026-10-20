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

const CURSOR_WIDTH: usize = 14;
const CURSOR_HEIGHT: usize = 20;

// 0 = Transparent, 1 = Black border, 2 = White body
#[rustfmt::skip]
const CURSOR_BITMAP: [u8; CURSOR_WIDTH * CURSOR_HEIGHT] = [
    1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    1, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    1, 2, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    1, 2, 2, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    1, 2, 2, 2, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0,
    1, 2, 2, 2, 2, 2, 1, 0, 0, 0, 0, 0, 0, 0,
    1, 2, 2, 2, 2, 2, 2, 1, 0, 0, 0, 0, 0, 0,
    1, 2, 2, 2, 2, 2, 2, 2, 1, 0, 0, 0, 0, 0,
    1, 2, 2, 2, 2, 2, 2, 2, 2, 1, 0, 0, 0, 0,
    1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 0, 0, 0,
    1, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 0, 0,
    1, 2, 2, 1, 2, 2, 1, 0, 0, 0, 0, 0, 0, 0,
    1, 2, 1, 0, 1, 2, 2, 1, 0, 0, 0, 0, 0, 0,
    1, 1, 0, 0, 1, 2, 2, 1, 0, 0, 0, 0, 0, 0,
    1, 0, 0, 0, 0, 1, 2, 2, 1, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 1, 2, 2, 1, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 1, 2, 2, 1, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

#[cfg(windows)]
fn get_cursor_pos() -> Option<(i32, i32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let mut pt = POINT::default();
    if unsafe { GetCursorPos(&mut pt) }.is_ok() {
        Some((pt.x, pt.y))
    } else {
        None
    }
}

#[cfg(not(windows))]
fn get_cursor_pos() -> Option<(i32, i32)> {
    None
}

fn draw_cursor(img: &mut image::RgbaImage, cur_x: i32, cur_y: i32) {
    let img_w = img.width() as i32;
    let img_h = img.height() as i32;

    for y in 0..CURSOR_HEIGHT {
        for x in 0..CURSOR_WIDTH {
            let val = CURSOR_BITMAP[y * CURSOR_WIDTH + x];
            if val == 0 {
                continue;
            }
            let px = cur_x + x as i32;
            let py = cur_y + y as i32;
            if px >= 0 && px < img_w && py >= 0 && py < img_h {
                let color = if val == 1 {
                    image::Rgba([0, 0, 0, 255])
                } else {
                    image::Rgba([255, 255, 255, 255])
                };
                img.put_pixel(px as u32, py as u32, color);
            }
        }
    }
}

pub struct ScreenCapture {
    pub active_monitor: usize,
    pub monitors: Vec<MonitorInfo>,
    rgb_buffer: Vec<u8>,
    last_cursor_x: i32,
    last_cursor_y: i32,
    last_checksum: u64,
    last_frame_time: std::time::Instant,
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
        let mut active_index = 0;

        // Auto-detect which display actually has active desktop content (ignoring alpha channel)
        if let Ok(all) = Monitor::all() {
            for (idx, m) in all.iter().enumerate() {
                if let Ok(img) = m.capture_image() {
                    let has_content = img.chunks_exact(4).any(|c| c[0] > 15 || c[1] > 15 || c[2] > 15);
                    if has_content {
                        active_index = idx;
                        info!("Auto-detected active desktop stream on Monitor #{} ({:?})", idx, m.name());
                        break;
                    }
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
            rgb_buffer: Vec::with_capacity(854 * 480 * 3),
            last_cursor_x: -1,
            last_cursor_y: -1,
            last_checksum: 0,
            last_frame_time: std::time::Instant::now(),
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
        let all_monitors = Monitor::all().ok()?;
        if all_monitors.is_empty() {
            return None;
        }

        let monitor = if self.active_monitor < all_monitors.len() {
            &all_monitors[self.active_monitor]
        } else {
            all_monitors
                .iter()
                .find(|m| m.is_primary().unwrap_or(false))
                .unwrap_or(&all_monitors[0])
        };

        let rgba_image = match monitor.capture_image() {
            Ok(img) => img,
            Err(_) => return None,
        };
        let selected_monitor = monitor;
        let width = rgba_image.width();
        let height = rgba_image.height();

        // Downscale to 854 max width for sub-1ms resize & ultra-lightweight 10-25KB frames
        let max_dim = 854;
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

        // Render hardware mouse cursor overlay directly on the frame
        let mon_x = selected_monitor.x().unwrap_or(0);
        let mon_y = selected_monitor.y().unwrap_or(0);
        let mon_w = selected_monitor.width().unwrap_or(width).max(1);
        let mon_h = selected_monitor.height().unwrap_or(height).max(1);

        let cur_pos = get_cursor_pos().unwrap_or((0, 0));
        let cursor_changed = cur_pos.0 != self.last_cursor_x || cur_pos.1 != self.last_cursor_y;
        self.last_cursor_x = cur_pos.0;
        self.last_cursor_y = cur_pos.1;

        if let Some((cx, cy)) = Some(cur_pos) {
            let local_x = cx - mon_x;
            let local_y = cy - mon_y;
            if local_x >= 0 && local_x < mon_w as i32 && local_y >= 0 && local_y < mon_h as i32 {
                let cur_x = ((local_x as f64 * target_image.width() as f64) / mon_w as f64).round() as i32;
                let cur_y = ((local_y as f64 * target_image.height() as f64) / mon_h as f64).round() as i32;
                draw_cursor(&mut target_image, cur_x, cur_y);
            }
        }

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

        // Fast lightweight 64-sample checksum to detect static screen
        let mut checksum: u64 = 0;
        let step = (rgb_len / 64).max(1);
        for i in (0..rgb_len).step_by(step) {
            checksum = checksum.wrapping_add(dst[i] as u64).rotate_left(3);
        }

        let is_identical = checksum == self.last_checksum && !cursor_changed;
        self.last_checksum = checksum;

        // If screen is unchanged and cursor hasn't moved, throttle to ~5 FPS to save battery & airtime
        if is_identical && self.last_frame_time.elapsed().as_millis() < 200 {
            return None;
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
