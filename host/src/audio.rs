use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{info, warn};

#[cfg(windows)]
use windows::Win32::Media::Audio::{
    eConsole, eRender, IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator,
    AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, WAVEFORMATEX,
};
#[cfg(windows)]
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
};

pub const AUDIO_MAGIC: &[u8; 4] = b"SWAU";

pub struct AudioCapture;

impl AudioCapture {
    pub fn start_loopback(
        running: Arc<AtomicBool>,
        tx: tokio::sync::mpsc::Sender<Vec<u8>>,
    ) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            #[cfg(windows)]
            {
                unsafe {
                    let co_init = CoInitializeEx(None, COINIT_MULTITHREADED);
                    if co_init.is_err() {
                        warn!("Failed to initialize COM for WASAPI audio capture");
                        return;
                    }

                    let enumerator: Result<IMMDeviceEnumerator, _> =
                        CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL);
                    let enumerator = match enumerator {
                        Ok(e) => e,
                        Err(err) => {
                            warn!("Failed to create MMDeviceEnumerator: {:?}", err);
                            CoUninitialize();
                            return;
                        }
                    };

                    let device = match enumerator.GetDefaultAudioEndpoint(eRender, eConsole) {
                        Ok(d) => d,
                        Err(err) => {
                            warn!("No default audio playback device found: {:?}", err);
                            CoUninitialize();
                            return;
                        }
                    };

                    let client: Result<IAudioClient, _> = device.Activate(CLSCTX_ALL, None);
                    let client = match client {
                        Ok(c) => c,
                        Err(err) => {
                            warn!("Failed to activate IAudioClient: {:?}", err);
                            CoUninitialize();
                            return;
                        }
                    };

                    let pwfx: *mut WAVEFORMATEX = match client.GetMixFormat() {
                        Ok(fmt) => fmt,
                        Err(err) => {
                            warn!("Failed to get audio mix format: {:?}", err);
                            CoUninitialize();
                            return;
                        }
                    };

                    if pwfx.is_null() {
                        CoUninitialize();
                        return;
                    }

                    let channels = (*pwfx).nChannels;
                    let samples_per_sec = (*pwfx).nSamplesPerSec;
                    let bits_per_sample = (*pwfx).wBitsPerSample;

                    info!(
                        "WASAPI Loopback format: {}ch @ {}Hz, {} bits/sample",
                        channels, samples_per_sec, bits_per_sample
                    );

                    // Initialize in loopback shared mode (20ms buffer = 200,000 in 100-ns units)
                    let hresult = client.Initialize(
                        AUDCLNT_SHAREMODE_SHARED,
                        AUDCLNT_STREAMFLAGS_LOOPBACK,
                        200_000,
                        0,
                        pwfx,
                        None,
                    );

                    if let Err(e) = hresult {
                        warn!("Failed to initialize WASAPI loopback client: {:?}", e);
                        windows::Win32::System::Com::CoTaskMemFree(Some(pwfx as *const _));
                        CoUninitialize();
                        return;
                    }

                    windows::Win32::System::Com::CoTaskMemFree(Some(pwfx as *const _));

                    let capture: Result<IAudioCaptureClient, _> = client.GetService();
                    let capture = match capture {
                        Ok(c) => c,
                        Err(err) => {
                            warn!("Failed to get IAudioCaptureClient: {:?}", err);
                            CoUninitialize();
                            return;
                        }
                    };

                    if let Err(e) = client.Start() {
                        warn!("Failed to start IAudioClient: {:?}", e);
                        CoUninitialize();
                        return;
                    }

                    info!("WASAPI Loopback audio capture started");

                    while running.load(Ordering::Relaxed) {
                        let mut p_data: *mut u8 = std::ptr::null_mut();
                        let mut num_frames_read: u32 = 0;
                        let mut flags: u32 = 0;

                        let hr = capture.GetBuffer(
                            &mut p_data,
                            &mut num_frames_read,
                            &mut flags,
                            None,
                            None,
                        );

                        if hr.is_ok() && num_frames_read > 0 && !p_data.is_null() {
                            let bytes_per_frame = (channels as usize) * (bits_per_sample as usize / 8);
                            let data_len = (num_frames_read as usize) * bytes_per_frame;

                            // Build packet with SWAU header + format metadata:
                            // [0..4] = "SWAU"
                            // [4..6] = channels (u16 LE)
                            // [6..10] = sample_rate (u32 LE)
                            // [10..12] = bits_per_sample (u16 LE)
                            // [12..] = raw PCM bytes (or zeros if AUDCLNT_BUFFERFLAGS_SILENT)
                            let mut packet = Vec::with_capacity(12 + data_len);
                            packet.extend_from_slice(AUDIO_MAGIC);
                            packet.extend_from_slice(&channels.to_le_bytes());
                            packet.extend_from_slice(&samples_per_sec.to_le_bytes());
                            packet.extend_from_slice(&bits_per_sample.to_le_bytes());

                            if (flags & 0x02) != 0 {
                                // AUDCLNT_BUFFERFLAGS_SILENT
                                packet.resize(12 + data_len, 0);
                            } else {
                                let slice = std::slice::from_raw_parts(p_data, data_len);
                                packet.extend_from_slice(slice);
                            }

                            let _ = capture.ReleaseBuffer(num_frames_read);

                            // Send audio packet (non-blocking)
                            let _ = tx.try_send(packet);
                        } else {
                            std::thread::sleep(std::time::Duration::from_millis(10));
                        }
                    }

                    let _ = client.Stop();
                    CoUninitialize();
                    info!("WASAPI Loopback audio capture stopped");
                }
            }
        })
    }
}
