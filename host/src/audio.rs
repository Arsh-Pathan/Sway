use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{info, warn};

#[cfg(windows)]
use windows::core::HRESULT;
#[cfg(windows)]
use windows::Win32::Media::Audio::{
    eConsole, eRender, IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator,
    AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, WAVEFORMATEX,
};
#[cfg(windows)]
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
};

pub const AUDIO_MAGIC: &[u8; 4] = b"SWAU";

/// Output format sent to phones: always 16-bit signed little-endian stereo PCM.
/// This halves WAN bandwidth versus float32 and plays on every Android device.
const OUT_CHANNELS: u16 = 2;
const OUT_BITS: u16 = 16;

#[cfg(windows)]
const AUDCLNT_E_DEVICE_INVALIDATED: HRESULT = HRESULT(0x8889_0004u32 as i32);

#[cfg(windows)]
struct ComGuard;
#[cfg(windows)]
impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

#[cfg(windows)]
struct MixFormat(*mut WAVEFORMATEX);
#[cfg(windows)]
impl Drop for MixFormat {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CoTaskMemFree(Some(self.0 as *const _)) };
        }
    }
}

pub struct AudioCapture;

impl AudioCapture {
    pub fn start_loopback(
        running: Arc<AtomicBool>,
        tx: tokio::sync::mpsc::Sender<Vec<u8>>,
    ) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            #[cfg(windows)]
            unsafe {
                if CoInitializeEx(None, COINIT_MULTITHREADED).is_err() {
                    warn!("Failed to initialize COM for WASAPI audio capture");
                    return;
                }
                // Declared before any COM interface so it is dropped AFTER all of them release.
                let _com = ComGuard;

                while running.load(Ordering::Relaxed) {
                    let retry = capture_session(&running, &tx);
                    if !retry {
                        break;
                    }
                    // Default device changed / unplugged: wait briefly and re-open the new endpoint.
                    std::thread::sleep(std::time::Duration::from_millis(500));
                }
                info!("WASAPI Loopback audio capture stopped");
            }
            #[cfg(not(windows))]
            {
                let _ = (running, tx);
            }
        })
    }
}

/// Reads one sample at `ptr` as f32 in [-1, 1].
#[cfg(windows)]
#[inline]
fn read_sample(b: &[u8], is_float: bool, bytes_per_sample: usize) -> f32 {
    if is_float {
        match bytes_per_sample {
            4 => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            8 => f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]) as f32,
            _ => 0.0,
        }
    } else {
        match bytes_per_sample {
            1 => (b[0] as f32 - 128.0) / 128.0,
            2 => i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0,
            3 => {
                let v = ((b[2] as i32) << 24 | (b[1] as i32) << 16 | (b[0] as i32) << 8) >> 8;
                v as f32 / 8_388_608.0
            }
            4 => i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2_147_483_648.0,
            _ => 0.0,
        }
    }
}

/// Converts interleaved device frames to 16-bit stereo, downmixing surround layouts.
#[cfg(windows)]
fn to_stereo_i16(data: &[u8], frames: usize, channels: usize, block: usize, is_float: bool) -> Vec<u8> {
    let bps = (block / channels.max(1)).max(1);
    let mut out = Vec::with_capacity(frames * 4);
    for f in 0..frames {
        let base = f * block;
        if base + block > data.len() {
            break;
        }
        let s = |c: usize| -> f32 {
            let off = base + c * bps;
            if c < channels && off + bps <= data.len() {
                read_sample(&data[off..off + bps], is_float, bps)
            } else {
                0.0
            }
        };
        let (l, r) = match channels {
            1 => (s(0), s(0)),
            2 => (s(0), s(1)),
            4 => (s(0) + 0.707 * s(2), s(1) + 0.707 * s(3)),
            c if c >= 6 => (
                s(0) + 0.707 * s(2) + 0.707 * s(4),
                s(1) + 0.707 * s(2) + 0.707 * s(5),
            ),
            _ => (s(0), s(1)),
        };
        out.extend_from_slice(&((l.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
        out.extend_from_slice(&((r.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    out
}

/// Runs one capture session on the current default render endpoint.
/// Returns `true` if the caller should re-open the endpoint (device invalidated / init failure).
#[cfg(windows)]
unsafe fn capture_session(running: &Arc<AtomicBool>, tx: &tokio::sync::mpsc::Sender<Vec<u8>>) -> bool {
    let enumerator: IMMDeviceEnumerator = match CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) {
        Ok(e) => e,
        Err(err) => {
            warn!("Failed to create MMDeviceEnumerator: {:?}", err);
            return false;
        }
    };
    let device = match enumerator.GetDefaultAudioEndpoint(eRender, eConsole) {
        Ok(d) => d,
        Err(err) => {
            warn!("No default audio playback device found: {:?}", err);
            return true;
        }
    };
    let client: IAudioClient = match device.Activate(CLSCTX_ALL, None) {
        Ok(c) => c,
        Err(err) => {
            warn!("Failed to activate IAudioClient: {:?}", err);
            return true;
        }
    };
    let mix = match client.GetMixFormat() {
        Ok(f) if !f.is_null() => MixFormat(f),
        Ok(_) => return false,
        Err(err) => {
            warn!("Failed to get audio mix format: {:?}", err);
            return true;
        }
    };
    let pwfx = mix.0;

    let tag = (*pwfx).wFormatTag;
    let channels = ((*pwfx).nChannels as usize).max(1);
    let rate = (*pwfx).nSamplesPerSec;
    let bits = (*pwfx).wBitsPerSample;
    let block = ((*pwfx).nBlockAlign as usize).max(1);
    let is_float = match tag {
        3 => true, // WAVE_FORMAT_IEEE_FLOAT
        0xFFFE => {
            // WAVE_FORMAT_EXTENSIBLE: SubFormat GUID Data1 lives at byte offset 24.
            let data1 = std::ptr::read_unaligned((pwfx as *const u8).add(24) as *const u32);
            data1 == 3
        }
        _ => false,
    };
    info!(
        "WASAPI Loopback format: {}ch @ {}Hz, {} bits, float={} -> sending {}ch {}-bit PCM",
        channels, rate, bits, is_float, OUT_CHANNELS, OUT_BITS
    );

    // 20ms buffer = 200,000 in 100-ns units
    if let Err(e) = client.Initialize(AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, 200_000, 0, pwfx, None) {
        warn!("Failed to initialize WASAPI loopback client: {:?}", e);
        return true;
    }

    let capture: IAudioCaptureClient = match client.GetService() {
        Ok(c) => c,
        Err(err) => {
            warn!("Failed to get IAudioCaptureClient: {:?}", err);
            return true;
        }
    };
    if let Err(e) = client.Start() {
        warn!("Failed to start IAudioClient: {:?}", e);
        return true;
    }
    info!("WASAPI Loopback audio capture started");

    let mut silent_run = 0u32;
    let mut retry = false;
    while running.load(Ordering::Relaxed) {
        let mut p_data: *mut u8 = std::ptr::null_mut();
        let mut frames: u32 = 0;
        let mut flags: u32 = 0;

        match capture.GetBuffer(&mut p_data, &mut frames, &mut flags, None, None) {
            Ok(()) => {
                if frames == 0 {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                    continue;
                }
                let silent = (flags & 0x02) != 0 || p_data.is_null(); // AUDCLNT_BUFFERFLAGS_SILENT
                let frames_n = frames as usize;

                let pcm = if silent {
                    None
                } else {
                    let raw = std::slice::from_raw_parts(p_data, frames_n * block);
                    Some(to_stereo_i16(raw, frames_n, channels, block, is_float))
                };
                // Always release exactly what GetBuffer returned.
                let _ = capture.ReleaseBuffer(frames);

                // After a few silent buffers stop sending zeros; resume instantly on real audio.
                if silent {
                    silent_run += 1;
                    if silent_run > 3 {
                        continue;
                    }
                } else {
                    silent_run = 0;
                }

                let body = pcm.unwrap_or_else(|| vec![0u8; frames_n * 4]);
                let mut packet = Vec::with_capacity(12 + body.len());
                packet.extend_from_slice(AUDIO_MAGIC);
                packet.extend_from_slice(&OUT_CHANNELS.to_le_bytes());
                packet.extend_from_slice(&rate.to_le_bytes());
                packet.extend_from_slice(&OUT_BITS.to_le_bytes());
                packet.extend_from_slice(&body);
                let _ = tx.try_send(packet);
            }
            Err(e) => {
                if e.code() == AUDCLNT_E_DEVICE_INVALIDATED {
                    warn!("Audio device invalidated; re-opening default endpoint");
                    retry = true;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
    }

    let _ = client.Stop();
    retry && running.load(Ordering::Relaxed)
}
