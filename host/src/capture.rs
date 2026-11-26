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

#[derive(Default)]
struct CursorUnderlay {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    pixels: Vec<u8>,
}

impl CursorUnderlay {
    fn save(&mut self, buf: &[u8], img_w: u32, img_h: u32, x0: i32, y0: i32, w: i32, h: i32) {
        let cx0 = x0.max(0) as u32;
        let cy0 = y0.max(0) as u32;
        let cx1 = (x0 + w).clamp(0, img_w as i32) as u32;
        let cy1 = (y0 + h).clamp(0, img_h as i32) as u32;
        let cw = cx1.saturating_sub(cx0);
        let ch = cy1.saturating_sub(cy0);
        self.x = cx0;
        self.y = cy0;
        self.w = cw;
        self.h = ch;
        let row_bytes = (cw as usize) * 4;
        let total = row_bytes * (ch as usize);
        self.pixels.resize(total, 0);
        if total == 0 {
            return;
        }
        let stride = (img_w as usize) * 4;
        for row in 0..ch as usize {
            let src_off = ((cy0 as usize + row) * stride) + (cx0 as usize) * 4;
            let dst_off = row * row_bytes;
            if src_off + row_bytes <= buf.len() {
                self.pixels[dst_off..dst_off + row_bytes]
                    .copy_from_slice(&buf[src_off..src_off + row_bytes]);
            }
        }
    }

    fn restore(&mut self, buf: &mut [u8], img_w: u32) {
        if self.w == 0 || self.h == 0 || self.pixels.is_empty() {
            self.w = 0;
            self.h = 0;
            return;
        }
        let row_bytes = (self.w as usize) * 4;
        let stride = (img_w as usize) * 4;
        for row in 0..self.h as usize {
            let dst_off = ((self.y as usize + row) * stride) + (self.x as usize) * 4;
            let src_off = row * row_bytes;
            if dst_off + row_bytes <= buf.len() && src_off + row_bytes <= self.pixels.len() {
                buf[dst_off..dst_off + row_bytes]
                    .copy_from_slice(&self.pixels[src_off..src_off + row_bytes]);
            }
        }
        self.w = 0;
        self.h = 0;
    }
}

#[cfg(windows)]
fn draw_real_cursor(
    bgra: &mut [u8],
    img_w: u32,
    img_h: u32,
    mon_x: i32,
    mon_y: i32,
    mon_w: u32,
    mon_h: u32,
    mut underlay: Option<&mut CursorUnderlay>,
) {
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
            bgra_data: Vec<u8>,
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
                                bgra_data: pixel_slice.to_vec(),
                            });
                        });
                    }
                }
                let _ = DeleteObject(hbm);
            }
            let _ = DeleteDC(hdc_mem);
        }

        // Composite the cached cursor bitmap directly in native BGRA format.
        let drew_cursor = CURSOR_CACHE.with(|cache| {
            let cache_ref = cache.borrow();
            if let Some(ref cached) = *cache_ref {
                let scaled_hotspot_x = ((cached.hotspot_x as f64) * scale_x).round() as i32;
                let scaled_hotspot_y = ((cached.hotspot_y as f64) * scale_y).round() as i32;
                let draw_origin_x = target_cur_x - scaled_hotspot_x;
                let draw_origin_y = target_cur_y - scaled_hotspot_y;
                if let Some(ref mut u) = underlay {
                    u.save(bgra, img_w, img_h, draw_origin_x, draw_origin_y, cached.width, cached.height);
                }
                let mut drawn_any = false;

                for cy in 0..cached.height {
                    let py = draw_origin_y + cy;
                    if py < 0 || py >= img_h as i32 {
                        continue;
                    }
                    for cx in 0..cached.width {
                        let idx = ((cy * cached.width + cx) * 4) as usize;
                        let b = cached.bgra_data[idx] as u32;
                        let g = cached.bgra_data[idx + 1] as u32;
                        let r = cached.bgra_data[idx + 2] as u32;
                        let a = cached.bgra_data[idx + 3] as u32;

                        if a == 0 && r == 0 && g == 0 && b == 0 {
                            continue;
                        }

                        let px = draw_origin_x + cx;
                        if px >= 0 && px < img_w as i32 {
                            let offset = (py as usize * img_w as usize + px as usize) * 4;
                            if offset + 4 <= bgra.len() {
                                let dest = &mut bgra[offset..offset + 4];
                                if a > 0 {
                                    let inv_alpha = 255 - a.min(255);
                                    dest[0] = (b + (dest[0] as u32 * inv_alpha) / 255).min(255) as u8;
                                    dest[1] = (g + (dest[1] as u32 * inv_alpha) / 255).min(255) as u8;
                                    dest[2] = (r + (dest[2] as u32 * inv_alpha) / 255).min(255) as u8;
                                } else {
                                    dest[0] = b as u8;
                                    dest[1] = g as u8;
                                    dest[2] = r as u8;
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
            if let Some(ref mut u) = underlay {
                u.save(bgra, img_w, img_h, target_cur_x, target_cur_y, 12, 16);
            }
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
                        if offset + 4 <= bgra.len() {
                            let dest = &mut bgra[offset..offset + 4];
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
fn draw_real_cursor(
    _bgra: &mut [u8],
    _img_w: u32,
    _img_h: u32,
    _mon_x: i32,
    _mon_y: i32,
    _mon_w: u32,
    _mon_h: u32,
    _underlay: Option<&mut CursorUnderlay>,
) {
}

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

#[cfg(windows)]
enum DxgiCaptureResult {
    NewFrame,
    Unchanged,
    Lost,
}

#[cfg(windows)]
struct DxgiCapturer {
    monitor_idx: usize,
    width: u32,
    height: u32,
    context: windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext,
    duplication: windows::Win32::Graphics::Dxgi::IDXGIOutputDuplication,
    staging: windows::Win32::Graphics::Direct3D11::ID3D11Texture2D,
}

#[cfg(windows)]
impl DxgiCapturer {
    fn new(monitor_idx: usize, mon_x: i32, mon_y: i32, mon_w: u32, mon_h: u32) -> Option<Self> {
        use windows::core::Interface;
        use windows::Win32::Foundation::{HMODULE, POINT};
        use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN;
        use windows::Win32::Graphics::Direct3D11::{
            D3D11CreateDevice, ID3D11Texture2D, D3D11_CPU_ACCESS_READ,
            D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_CREATE_DEVICE_SINGLETHREADED,
            D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
        };
        use windows::Win32::Graphics::Dxgi::Common::{
            DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_B8G8R8A8_UNORM_SRGB, DXGI_SAMPLE_DESC,
        };
        use windows::Win32::Graphics::Dxgi::{
            CreateDXGIFactory1, IDXGIAdapter, IDXGIFactory1, IDXGIOutput1,
        };
        use windows::Win32::Graphics::Gdi::{MonitorFromPoint, MONITOR_DEFAULTTONEAREST};

        unsafe {
            let target_hmon = MonitorFromPoint(
                POINT {
                    x: mon_x + (mon_w as i32) / 2,
                    y: mon_y + (mon_h as i32) / 2,
                },
                MONITOR_DEFAULTTONEAREST,
            );
            let factory: IDXGIFactory1 = CreateDXGIFactory1().ok()?;
            let mut adapter_idx = 0u32;

            while let Ok(adapter1) = factory.EnumAdapters1(adapter_idx) {
                adapter_idx += 1;
                let mut output_idx = 0u32;
                while let Ok(output) = adapter1.EnumOutputs(output_idx) {
                    output_idx += 1;
                    let Ok(out_desc) = output.GetDesc() else { continue };
                    if !target_hmon.is_invalid() && out_desc.Monitor != target_hmon {
                        continue;
                    }

                    let adapter: IDXGIAdapter = adapter1.cast().ok()?;
                    let mut device_opt = None;
                    let mut context_opt = None;
                    let flags = D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_SINGLETHREADED;
                    if D3D11CreateDevice(
                        &adapter,
                        D3D_DRIVER_TYPE_UNKNOWN,
                        HMODULE::default(),
                        flags,
                        None,
                        D3D11_SDK_VERSION,
                        Some(&mut device_opt),
                        None,
                        Some(&mut context_opt),
                    )
                    .is_err()
                    {
                        continue;
                    }
                    let (Some(device), Some(context)) = (device_opt, context_opt) else {
                        continue;
                    };

                    let output1: IDXGIOutput1 = match output.cast() {
                        Ok(o) => o,
                        Err(_) => continue,
                    };
                    let duplication = match output1.DuplicateOutput(&device) {
                        Ok(d) => d,
                        Err(_) => continue,
                    };

                    let dup_desc = duplication.GetDesc();
                    let w = dup_desc.ModeDesc.Width;
                    let h = dup_desc.ModeDesc.Height;
                    let fmt = dup_desc.ModeDesc.Format;
                    // Fall back cleanly if HDR (10/16-bit) or rotated mode mismatches logical dims.
                    if (fmt != DXGI_FORMAT_B8G8R8A8_UNORM && fmt != DXGI_FORMAT_B8G8R8A8_UNORM_SRGB)
                        || w == 0
                        || h == 0
                        || w != mon_w
                        || h != mon_h
                    {
                        continue;
                    }

                    let staging_desc = D3D11_TEXTURE2D_DESC {
                        Width: w,
                        Height: h,
                        MipLevels: 1,
                        ArraySize: 1,
                        Format: fmt,
                        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                        Usage: D3D11_USAGE_STAGING,
                        BindFlags: 0,
                        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                        MiscFlags: 0,
                    };
                    let mut staging_opt: Option<ID3D11Texture2D> = None;
                    if device
                        .CreateTexture2D(&staging_desc, None, Some(&mut staging_opt))
                        .is_err()
                    {
                        continue;
                    }
                    let Some(staging) = staging_opt else { continue };

                    info!(
                        "Initialized persistent DXGI Desktop Duplication on Monitor #{} ({}x{})",
                        monitor_idx, w, h
                    );
                    return Some(Self {
                        monitor_idx,
                        width: w,
                        height: h,
                        context,
                        duplication,
                        staging,
                    });
                }
            }
            None
        }
    }

    fn capture(&mut self, wait_ms: u32, dst_bgra: &mut Vec<u8>) -> DxgiCaptureResult {
        use windows::core::{HRESULT, Interface};
        use windows::Win32::Graphics::Direct3D11::{
            ID3D11Resource, ID3D11Texture2D, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ,
        };
        use windows::Win32::Graphics::Dxgi::{DXGI_OUTDUPL_FRAME_INFO, IDXGIResource};

        const DXGI_ERROR_WAIT_TIMEOUT: HRESULT = HRESULT(0x887A_0027u32 as i32);

        let expected = (self.width as usize) * (self.height as usize) * 4;
        let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut resource_opt: Option<IDXGIResource> = None;

        unsafe {
            match self
                .duplication
                .AcquireNextFrame(wait_ms, &mut frame_info, &mut resource_opt)
            {
                Err(e) => {
                    if e.code() == DXGI_ERROR_WAIT_TIMEOUT {
                        return DxgiCaptureResult::Unchanged;
                    }
                    let _ = self.duplication.ReleaseFrame();
                    return DxgiCaptureResult::Lost;
                }
                Ok(()) => {
                    if frame_info.LastPresentTime == 0 {
                        let _ = self.duplication.ReleaseFrame();
                        return DxgiCaptureResult::Unchanged;
                    }

                    let Some(res) = resource_opt else {
                        let _ = self.duplication.ReleaseFrame();
                        return DxgiCaptureResult::Unchanged;
                    };
                    let Ok(src_tex) = res.cast::<ID3D11Texture2D>() else {
                        let _ = self.duplication.ReleaseFrame();
                        return DxgiCaptureResult::Lost;
                    };
                    let (Ok(src_res), Ok(dst_res)) =
                        (src_tex.cast::<ID3D11Resource>(), self.staging.cast::<ID3D11Resource>())
                    else {
                        let _ = self.duplication.ReleaseFrame();
                        return DxgiCaptureResult::Lost;
                    };

                    self.context.CopyResource(&dst_res, &src_res);
                    let _ = self.duplication.ReleaseFrame();

                    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                    if self
                        .context
                        .Map(&dst_res, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                        .is_err()
                    {
                        return DxgiCaptureResult::Lost;
                    }

                    dst_bgra.resize(expected, 0);
                    let row_bytes = (self.width as usize) * 4;
                    let src_pitch = mapped.RowPitch as usize;
                    let src_ptr = mapped.pData as *const u8;

                    if src_pitch == row_bytes {
                        let src_slice = std::slice::from_raw_parts(src_ptr, expected);
                        dst_bgra.copy_from_slice(src_slice);
                    } else if src_pitch >= row_bytes {
                        for row in 0..self.height as usize {
                            let src_row = std::slice::from_raw_parts(
                                src_ptr.add(row * src_pitch),
                                row_bytes,
                            );
                            let dst_off = row * row_bytes;
                            dst_bgra[dst_off..dst_off + row_bytes].copy_from_slice(src_row);
                        }
                    }

                    self.context.Unmap(&dst_res, 0);
                    DxgiCaptureResult::NewFrame
                }
            }
        }
    }
}

#[cfg(windows)]
struct GdiCapturer {
    monitor_idx: usize,
    mon_x: i32,
    mon_y: i32,
    width: u32,
    height: u32,
    hdc_screen: windows::Win32::Graphics::Gdi::HDC,
    hdc_mem: windows::Win32::Graphics::Gdi::HDC,
    hbm: windows::Win32::Graphics::Gdi::HBITMAP,
    old_bm: windows::Win32::Graphics::Gdi::HGDIOBJ,
    bits_ptr: *mut u8,
}

#[cfg(windows)]
unsafe impl Send for GdiCapturer {}

#[cfg(windows)]
impl GdiCapturer {
    fn new(monitor_idx: usize, mon_x: i32, mon_y: i32, width: u32, height: u32) -> Option<Self> {
        use windows::Win32::Graphics::Gdi::{
            CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC,
            SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
        };

        if width == 0 || height == 0 {
            return None;
        }
        unsafe {
            let hdc_screen = GetDC(None);
            if hdc_screen.is_invalid() {
                return None;
            }
            let hdc_mem = CreateCompatibleDC(hdc_screen);
            if hdc_mem.is_invalid() {
                ReleaseDC(None, hdc_screen);
                return None;
            }
            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width as i32,
                    biHeight: -(height as i32), // Top-down BGRA
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let hbm = match CreateDIBSection(hdc_mem, &bmi, DIB_RGB_COLORS, &mut bits, None, 0) {
                Ok(h) if !h.is_invalid() && !bits.is_null() => h,
                Ok(h) => {
                    if !h.is_invalid() {
                        let _ = DeleteObject(h);
                    }
                    let _ = DeleteDC(hdc_mem);
                    ReleaseDC(None, hdc_screen);
                    return None;
                }
                Err(_) => {
                    let _ = DeleteDC(hdc_mem);
                    ReleaseDC(None, hdc_screen);
                    return None;
                }
            };
            let old_bm = SelectObject(hdc_mem, hbm);
            Some(Self {
                monitor_idx,
                mon_x,
                mon_y,
                width,
                height,
                hdc_screen,
                hdc_mem,
                hbm,
                old_bm,
                bits_ptr: bits as *mut u8,
            })
        }
    }

    fn capture(&self, dst_bgra: &mut Vec<u8>) -> bool {
        use windows::Win32::Graphics::Gdi::{BitBlt, GdiFlush, SRCCOPY};
        let len = (self.width as usize) * (self.height as usize) * 4;
        unsafe {
            if BitBlt(
                self.hdc_mem,
                0,
                0,
                self.width as i32,
                self.height as i32,
                self.hdc_screen,
                self.mon_x,
                self.mon_y,
                SRCCOPY,
            )
            .is_err()
            {
                return false;
            }
            let _ = GdiFlush();
            dst_bgra.resize(len, 0);
            let src_slice = std::slice::from_raw_parts(self.bits_ptr, len);
            dst_bgra.copy_from_slice(src_slice);
            true
        }
    }
}

#[cfg(windows)]
impl Drop for GdiCapturer {
    fn drop(&mut self) {
        use windows::Win32::Graphics::Gdi::{DeleteDC, DeleteObject, ReleaseDC, SelectObject};
        unsafe {
            let _ = SelectObject(self.hdc_mem, self.old_bm);
            let _ = DeleteObject(self.hbm);
            let _ = DeleteDC(self.hdc_mem);
            ReleaseDC(None, self.hdc_screen);
        }
    }
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
    desktop_bgra: Vec<u8>,
    desktop_w: u32,
    desktop_h: u32,
    has_desktop_frame: bool,
    scaled_bgra_buffer: Vec<u8>,
    cursor_underlay: CursorUnderlay,
    x_lut: Vec<usize>,
    x_lut_target_w: u32,
    x_lut_src_w: u32,
    #[cfg(windows)]
    dxgi: Option<DxgiCapturer>,
    #[cfg(windows)]
    dxgi_retry_after: std::time::Instant,
    #[cfg(windows)]
    gdi: Option<GdiCapturer>,
    last_frame_time: std::time::Instant,
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

        let cached_monitors = Monitor::all().unwrap_or_default();
        let monitors = Self::build_monitor_info(&cached_monitors);
        let mut active_index = 0;

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
            desktop_bgra: Vec::new(),
            desktop_w: 0,
            desktop_h: 0,
            has_desktop_frame: false,
            scaled_bgra_buffer: Vec::new(),
            cursor_underlay: CursorUnderlay::default(),
            x_lut: Vec::new(),
            x_lut_target_w: 0,
            x_lut_src_w: 0,
            #[cfg(windows)]
            dxgi: None,
            #[cfg(windows)]
            dxgi_retry_after: std::time::Instant::now(),
            #[cfg(windows)]
            gdi: None,
            last_frame_time: std::time::Instant::now(),
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
            self.has_desktop_frame = false;
            self.last_frame_hash = 0;
            #[cfg(windows)]
            {
                self.dxgi = None;
                self.gdi = None;
            }
        }
    }

    pub fn capture_frame_jpeg(
        &mut self,
        quality: u8,
        viewport_max_dim: u16,
        force_encode: bool,
    ) -> Option<Vec<u8>> {
        #[cfg(windows)]
        crate::input::InputSimulator::ensure_input_desktop();

        let cur_gen = crate::input::DISPLAY_GENERATION.load(Ordering::Relaxed);
        if self.monitors.is_empty() || cur_gen != self.last_display_gen {
            self.refresh_monitors_cache();
            self.last_display_gen = cur_gen;
        }

        let monitor_idx = if self.active_monitor < self.monitors.len() {
            self.active_monitor
        } else {
            0
        };

        let (mon_x, mon_y, mon_w, mon_h) = if let Some(info) = self.monitors.get(monitor_idx) {
            (info.x, info.y, (info.width as u32).max(1), (info.height as u32).max(1))
        } else {
            (0, 0, 1920, 1080)
        };

        let mut desktop_changed = false;

        #[cfg(windows)]
        {
            if let Some(ref d) = self.dxgi {
                if d.monitor_idx != monitor_idx || d.width != mon_w || d.height != mon_h {
                    self.dxgi = None;
                    self.has_desktop_frame = false;
                }
            }
            if self.dxgi.is_none() && std::time::Instant::now() >= self.dxgi_retry_after {
                self.dxgi = DxgiCapturer::new(monitor_idx, mon_x, mon_y, mon_w, mon_h);
                if self.dxgi.is_none() {
                    self.dxgi_retry_after = std::time::Instant::now() + std::time::Duration::from_secs(2);
                }
            }

            if let Some(ref mut dxgi) = self.dxgi {
                // Non-blocking poll once a base desktop image is cached so cursor-only moves encode in ~2ms.
                let poll_ms = if self.has_desktop_frame { 0 } else { 16 };
                match dxgi.capture(poll_ms, &mut self.desktop_bgra) {
                    DxgiCaptureResult::NewFrame => {
                        self.desktop_w = dxgi.width;
                        self.desktop_h = dxgi.height;
                        self.has_desktop_frame = true;
                        desktop_changed = true;
                    }
                    DxgiCaptureResult::Unchanged if self.has_desktop_frame => {}
                    DxgiCaptureResult::Unchanged => {
                        // Seed initial frame once via persistent GDI so idle screens appear immediately.
                        if self.gdi.as_ref().is_none_or(|g| g.monitor_idx != monitor_idx || g.width != mon_w || g.height != mon_h) {
                            self.gdi = GdiCapturer::new(monitor_idx, mon_x, mon_y, mon_w, mon_h);
                        }
                        if let Some(ref gdi) = self.gdi {
                            if gdi.capture(&mut self.desktop_bgra) {
                                self.desktop_w = gdi.width;
                                self.desktop_h = gdi.height;
                                self.has_desktop_frame = true;
                                desktop_changed = true;
                            }
                        }
                    }
                    DxgiCaptureResult::Lost => {
                        self.dxgi = None;
                        self.has_desktop_frame = false;
                        self.dxgi_retry_after = std::time::Instant::now() + std::time::Duration::from_secs(2);
                    }
                }
            }

            if self.dxgi.is_none() {
                if self.gdi.as_ref().is_none_or(|g| g.monitor_idx != monitor_idx || g.width != mon_w || g.height != mon_h) {
                    self.gdi = GdiCapturer::new(monitor_idx, mon_x, mon_y, mon_w, mon_h);
                }
                if let Some(ref gdi) = self.gdi {
                    if gdi.capture(&mut self.desktop_bgra) {
                        self.desktop_w = gdi.width;
                        self.desktop_h = gdi.height;
                        self.has_desktop_frame = true;
                        desktop_changed = true;
                    }
                }
            }
        }

        if !self.has_desktop_frame || self.desktop_w == 0 || self.desktop_h == 0 {
            return None;
        }

        let width = self.desktop_w;
        let height = self.desktop_h;
        let expected_len = (width as usize) * (height as usize) * 4;
        if self.desktop_bgra.len() < expected_len {
            return None;
        }

        let cur_cursor = get_current_cursor_state();
        let frame_hash = if desktop_changed {
            compute_sample_hash(&self.desktop_bgra[..expected_len])
        } else {
            self.last_frame_hash
        };
        let is_identical = frame_hash == self.last_frame_hash && cur_cursor == self.last_cursor_state;

        if !force_encode && is_identical && self.last_encoded_time.elapsed() < std::time::Duration::from_millis(250) {
            return None;
        }

        self.last_frame_hash = frame_hash;
        self.last_cursor_state = cur_cursor;
        self.last_encoded_time = std::time::Instant::now();
        self.last_frame_time = self.last_encoded_time;

        // Adaptive resolution scaling capped by client viewport tier across LAN and WAN
        let quality_max_dim: u32 = match quality {
            0..=36 => 854,
            37..=52 => 960,
            53..=68 => 1280,
            _ => 1920,
        };
        let max_dim = quality_max_dim.min((viewport_max_dim.max(480)) as u32);

        let (target_w, target_h) = if width > max_dim || height > max_dim {
            if width >= height {
                (max_dim, ((height * max_dim) / width).max(1))
            } else {
                (((width * max_dim) / height).max(1), max_dim)
            }
        } else {
            (width, height)
        };

        if target_w == width && target_h == height {
            // Save cursor bounding box before compositing so self.desktop_bgra stays clean for cursor-only updates.
            draw_real_cursor(
                &mut self.desktop_bgra[..expected_len],
                width,
                height,
                mon_x,
                mon_y,
                mon_w,
                mon_h,
                Some(&mut self.cursor_underlay),
            );
            let encoded = compress(
                &self.desktop_bgra[..expected_len],
                width as usize,
                height as usize,
                PixelFormat::Bgra,
                quality,
                Subsampling::S420,
            )
            .ok();
            self.cursor_underlay.restore(&mut self.desktop_bgra[..expected_len], width);
            encoded
        } else {
            if self.x_lut_target_w != target_w || self.x_lut_src_w != width {
                self.x_lut = (0..target_w)
                    .map(|x| ((x as usize * width as usize) / target_w as usize).min(width as usize - 1))
                    .collect();
                self.x_lut_target_w = target_w;
                self.x_lut_src_w = width;
            }
            let x_lut = &self.x_lut;
            let max_x = x_lut.iter().copied().max().unwrap_or(0);

            let dst_len = (target_w * target_h * 4) as usize;
            if self.scaled_bgra_buffer.len() < dst_len {
                self.scaled_bgra_buffer.resize(dst_len, 0);
            }

            let src_pixels = self.desktop_bgra[..expected_len].as_chunks::<4>().0;
            let dst_pixels = self.scaled_bgra_buffer[..dst_len].as_chunks_mut::<4>().0;
            let src_w = width as usize;
            let dst_w = target_w as usize;

            for (y, dst_row) in dst_pixels.chunks_exact_mut(dst_w).enumerate() {
                let src_y = ((y * height as usize) / target_h as usize).min(height as usize - 1);
                let src_row = &src_pixels[src_y * src_w..(src_y + 1) * src_w];
                if max_x < src_row.len() {
                    for (&lx, dst_px) in x_lut.iter().zip(dst_row.iter_mut()) {
                        *dst_px = src_row[lx];
                    }
                }
            }

            draw_real_cursor(
                &mut self.scaled_bgra_buffer[..dst_len],
                target_w,
                target_h,
                mon_x,
                mon_y,
                mon_w,
                mon_h,
                None,
            );
            compress(
                &self.scaled_bgra_buffer[..dst_len],
                target_w as usize,
                target_h as usize,
                PixelFormat::Bgra,
                quality,
                Subsampling::S420,
            )
            .ok()
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
            let t0 = std::time::Instant::now();
            if let Some(frame) = capture.capture_frame_jpeg(q, 1920, true) {
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
