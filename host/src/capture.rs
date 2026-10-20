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

pub struct ScreenCapture {
    pub active_monitor: usize,
    pub monitors: Vec<MonitorInfo>,
}

impl ScreenCapture {
    pub fn new() -> Self {
        #[cfg(windows)]
        {
            extern "system" {
                fn OpenInputDesktop(dwFlags: u32, fInherit: i32, dwDesiredAccess: u32) -> isize;
                fn SetThreadDesktop(hDesktop: isize) -> i32;
            }
            unsafe {
                let h_desk = OpenInputDesktop(0, 0, 0x01FF);
                if h_desk != 0 {
                    let _ = SetThreadDesktop(h_desk);
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
        }
    }

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

    pub fn capture_frame_jpeg(&self, quality: u8) -> Option<Vec<u8>> {
        #[cfg(windows)]
        {
            extern "system" {
                fn OpenInputDesktop(dwFlags: u32, fInherit: i32, dwDesiredAccess: u32) -> isize;
                fn SetThreadDesktop(hDesktop: isize) -> i32;
            }
            unsafe {
                let h_desk = OpenInputDesktop(0, 0, 0x01FF);
                if h_desk != 0 {
                    let _ = SetThreadDesktop(h_desk);
                }
            }
        }

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
            Ok(img) => {
                let has_rgb = img.chunks_exact(4).any(|c| c[0] > 15 || c[1] > 15 || c[2] > 15);
                if !has_rgb && all_monitors.len() > 1 {
                    let alt_idx = (self.active_monitor + 1) % all_monitors.len();
                    if let Ok(alt_img) = all_monitors[alt_idx].capture_image() {
                        if alt_img.chunks_exact(4).any(|c| c[0] > 15 || c[1] > 15 || c[2] > 15) {
                            alt_img
                        } else {
                            img
                        }
                    } else {
                        img
                    }
                } else {
                    img
                }
            }
            Err(_) => return None,
        };
        let width = rgba_image.width();
        let height = rgba_image.height();

        // Downscale to 1280 max width for 30+ FPS low-latency streaming
        let target_image = if width > 1280 {
            let target_w = 1280;
            let target_h = (height * 1280) / width;
            image::imageops::resize(
                &rgba_image,
                target_w,
                target_h,
                image::imageops::FilterType::Nearest,
            )
        } else {
            rgba_image
        };

        let rgb_image = image::DynamicImage::ImageRgba8(target_image).into_rgb8();

        let mut jpeg_bytes = Vec::new();
        let mut encoder = JpegEncoder::new_with_quality(&mut jpeg_bytes, quality);
        if encoder
            .encode(
                rgb_image.as_raw(),
                rgb_image.width(),
                rgb_image.height(),
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
        let capture = ScreenCapture::new();
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
