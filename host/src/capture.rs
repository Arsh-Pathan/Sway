use tracing::info;

#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub index: usize,
    pub name: String,
    pub width: i32,
    pub height: i32,
    pub is_primary: bool,
}

pub struct ScreenCapture {
    #[allow(dead_code)]
    pub active_monitor: usize,
    pub monitors: Vec<MonitorInfo>,
}

impl ScreenCapture {
    pub fn new() -> Self {
        let monitors = Self::detect_monitors();
        info!("Detected {} connected monitor(s)", monitors.len());
        for m in &monitors {
            info!("  Monitor #{}: {} ({}x{}) primary={}", m.index, m.name, m.width, m.height, m.is_primary);
        }

        Self {
            active_monitor: 0,
            monitors,
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
        #[cfg(windows)]
        {
            use windows::Win32::UI::WindowsAndMessaging::{
                GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN,
            };

            unsafe {
                let width = GetSystemMetrics(SM_CXSCREEN);
                let height = GetSystemMetrics(SM_CYSCREEN);

                vec![MonitorInfo {
                    index: 0,
                    name: "Primary Display".to_string(),
                    width,
                    height,
                    is_primary: true,
                }]
            }
        }

        #[cfg(not(windows))]
        {
            vec![MonitorInfo {
                index: 0,
                name: "Virtual Display".to_string(),
                width: 1920,
                height: 1080,
                is_primary: true,
            }]
        }
    }

    #[cfg(windows)]
    pub fn capture_frame_jpeg(&self, quality: u8) -> Option<Vec<u8>> {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::Graphics::Gdi::{
            BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
            GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
            DIB_RGB_COLORS, SRCCOPY,
        };
        use windows::Win32::UI::WindowsAndMessaging::{
            GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN,
        };
        use image::codecs::jpeg::JpegEncoder;
        use image::ColorType;

        unsafe {
            let width = GetSystemMetrics(SM_CXSCREEN);
            let height = GetSystemMetrics(SM_CYSCREEN);
            if width <= 0 || height <= 0 {
                return None;
            }

            let hdc_screen = GetDC(HWND(std::ptr::null_mut()));
            if hdc_screen.0.is_null() {
                return None;
            }

            let hdc_mem = CreateCompatibleDC(hdc_screen);
            let h_bitmap = CreateCompatibleBitmap(hdc_screen, width, height);
            let old_bmp = SelectObject(hdc_mem, h_bitmap);

            // Blit screen to memory DC
            let _ = BitBlt(hdc_mem, 0, 0, width, height, hdc_screen, 0, 0, SRCCOPY);

            let mut bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height, // top-down bitmap
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

            let mut bgra_buf: Vec<u8> = vec![0u8; (width * height * 4) as usize];
            GetDIBits(
                hdc_mem,
                h_bitmap,
                0,
                height as u32,
                Some(bgra_buf.as_mut_ptr() as *mut _),
                &mut bmi,
                DIB_RGB_COLORS,
            );

            // Cleanup GDI objects
            let _ = SelectObject(hdc_mem, old_bmp);
            let _ = DeleteObject(h_bitmap);
            let _ = DeleteDC(hdc_mem);
            let _ = ReleaseDC(HWND(std::ptr::null_mut()), hdc_screen);

            // Convert BGRA to RGB
            let mut rgb_buf = Vec::with_capacity((width * height * 3) as usize);
            for chunk in bgra_buf.chunks_exact(4) {
                rgb_buf.push(chunk[2]); // R
                rgb_buf.push(chunk[1]); // G
                rgb_buf.push(chunk[0]); // B
            }

            // Encode to JPEG
            let mut jpeg_bytes = Vec::new();
            let mut encoder = JpegEncoder::new_with_quality(&mut jpeg_bytes, quality);
            if encoder
                .encode(&rgb_buf, width as u32, height as u32, ColorType::Rgb8.into())
                .is_ok()
            {
                Some(jpeg_bytes)
            } else {
                None
            }
        }
    }

    #[cfg(not(windows))]
    pub fn capture_frame_jpeg(&self, _quality: u8) -> Option<Vec<u8>> {
        None
    }
}
