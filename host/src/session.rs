//! Shared per-connection session plumbing used by both the direct LAN server and the cloud
//! signaling client, so both transports behave identically.

use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::SinkExt;
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite::protocol::Message;
use tracing::info;

use crate::audio::AudioCapture;
use crate::capture::ScreenCapture;
use crate::input::InputSimulator;
use crate::power::SessionGuard;
use crate::privacy::PrivacyCurtain;
use crate::protocol::InputPacket;

/// Maps the 4 wire quality modes (0 TURBO, 1 DATA_SAVER, 2 BALANCED, 3 NATIVE) to a JPEG quality
/// and frame interval. `lan` links can afford a higher quality and 60 FPS native mode than WAN.
pub fn quality_params(mode: u8, lan: bool) -> (u8, Duration) {
    let (q_lan, q_wan, ms_lan, ms_wan) = match mode {
        0 => (45, 36, 16, 16),
        1 => (35, 28, 33, 33),
        2 => (68, 60, 16, 16),
        3 => (85, 78, 16, 20),
        _ => (68, 60, 16, 16),
    };
    if lan {
        (q_lan, Duration::from_millis(ms_lan))
    } else {
        (q_wan, Duration::from_millis(ms_wan))
    }
}

/// Atomics shared between a connection's reader and its capture worker.
pub struct StreamState {
    pub conn_alive: AtomicBool,
    pub stream_enabled: AtomicBool,
    pub active_monitor: AtomicUsize,
    pub quality_mode: AtomicU8,
    pub lan: bool,
}

impl StreamState {
    pub fn new(lan: bool) -> Arc<Self> {
        Arc::new(Self {
            conn_alive: AtomicBool::new(true),
            stream_enabled: AtomicBool::new(true),
            active_monitor: AtomicUsize::new(0),
            quality_mode: AtomicU8::new(0),
            lan,
        })
    }
}

/// One capture thread per connection. It idles (cheaply) while the client has video switched
/// off, instead of being torn down and re-spawned on every input packet.
pub fn spawn_capture_worker(
    state: Arc<StreamState>,
    frame_tx: watch::Sender<Option<Vec<u8>>>,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        let mut capture = ScreenCapture::new();
        capture.active_monitor = state.active_monitor.load(Ordering::Relaxed);
        let mut next_tick = Instant::now();
        info!("Started screen capture & streaming loop");
        while state.conn_alive.load(Ordering::Relaxed) && !frame_tx.is_closed() {
            if !state.stream_enabled.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(50));
                next_tick = Instant::now();
                continue;
            }
            capture.active_monitor = state.active_monitor.load(Ordering::Relaxed);
            let (q, interval) = quality_params(state.quality_mode.load(Ordering::Relaxed), state.lan);
            let wait_ms = interval.as_millis().clamp(1, 20) as u32;

            if let Some(frame) = capture.capture_frame_jpeg(q, wait_ms) {
                // Overwrite any unconsumed stale frame so the writer always sends the latest frame.
                frame_tx.send_replace(Some(frame));
                if frame_tx.is_closed() {
                    break;
                }
            }
            next_tick += interval;
            let now = Instant::now();
            if now < next_tick {
                std::thread::sleep(next_tick - now);
            } else {
                next_tick = now;
            }
        }
        info!("Screen capture loop stopped");
    })
}

/// Handle to a running WASAPI loopback thread. Each start gets its own stop token, so a quick
/// off/on toggle can never leave two threads sharing (and fighting over) one flag.
pub struct AudioHandle {
    stop: Arc<AtomicBool>,
}

impl AudioHandle {
    pub fn start(tx: mpsc::Sender<Vec<u8>>) -> Self {
        let stop = Arc::new(AtomicBool::new(true));
        let _ = AudioCapture::start_loopback(stop.clone(), tx);
        Self { stop }
    }
}

impl Drop for AudioHandle {
    fn drop(&mut self) {
        self.stop.store(false, Ordering::SeqCst);
    }
}

/// Spawns the single task that owns the socket's write half. Control traffic has priority over
/// audio, which has priority over video. A keepalive ping every 20s holds NAT mappings open.
pub fn spawn_writer<W>(
    mut write: W,
    mut ctrl_rx: mpsc::Receiver<Message>,
    mut audio_rx: mpsc::Receiver<Vec<u8>>,
    mut frame_rx: watch::Receiver<Option<Vec<u8>>>,
) -> tokio::task::JoinHandle<()>
where
    W: futures_util::Sink<Message> + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut keepalive = tokio::time::interval_at(
            tokio::time::Instant::now() + Duration::from_secs(20),
            Duration::from_secs(20),
        );
        loop {
            tokio::select! {
                biased;
                Some(ctrl) = ctrl_rx.recv() => {
                    if write.send(ctrl).await.is_err() { break; }
                }
                Some(audio) = audio_rx.recv() => {
                    if write.send(Message::Binary(audio)).await.is_err() { break; }
                }
                res = frame_rx.changed() => {
                    if res.is_err() { break; }
                    let frame_opt = { frame_rx.borrow_and_update().clone() };
                    if let Some(frame) = frame_opt {
                        if write.send(Message::Binary(frame)).await.is_err() { break; }
                    }
                }
                _ = keepalive.tick() => {
                    if write.send(Message::Ping(Vec::new())).await.is_err() { break; }
                }
                else => break,
            }
        }
    })
}

/// Acquires a session guard whose last-leave cleanup restores the host's physical state.
pub fn new_session_guard(privacy: Arc<PrivacyCurtain>, sim: Arc<InputSimulator>) -> SessionGuard {
    SessionGuard::acquire(move || {
        privacy.set_enabled(false);
        InputSimulator::restore_display_if_changed();
        sim.release_all();
    })
}

pub fn host_info_message(sim: &InputSimulator) -> Message {
    Message::Text(serde_json::json!({ "type": "host_info", "monitors": sim.monitor_count() }).to_string())
}

/// Executes decoded input packets for one controller connection.
pub struct SessionCtx {
    pub sim: Arc<InputSimulator>,
    pub privacy: Arc<PrivacyCurtain>,
    pub state: Arc<StreamState>,
    pub ctrl_tx: mpsc::Sender<Message>,
    pub audio_tx: mpsc::Sender<Vec<u8>>,
    audio: Option<AudioHandle>,
}

impl SessionCtx {
    pub fn new(
        sim: Arc<InputSimulator>,
        privacy: Arc<PrivacyCurtain>,
        state: Arc<StreamState>,
        ctrl_tx: mpsc::Sender<Message>,
        audio_tx: mpsc::Sender<Vec<u8>>,
    ) -> Self {
        Self { sim, privacy, state, ctrl_tx, audio_tx, audio: None }
    }

    /// Slow Win32 calls (display mode, clipboard, monitor enumeration) run off the async workers.
    fn dispatch_blocking(&self, packet: InputPacket) {
        let sim = self.sim.clone();
        tokio::task::spawn_blocking(move || sim.dispatch(packet));
    }

    pub fn handle(&mut self, packet: InputPacket) {
        match packet {
            InputPacket::SwitchMonitor { monitor_index } => {
                let idx = (monitor_index as usize) % self.sim.monitor_count().max(1);
                self.state.active_monitor.store(idx, Ordering::SeqCst);
                self.dispatch_blocking(InputPacket::SwitchMonitor { monitor_index: idx as u8 });
            }
            InputPacket::SetQuality { mode } => {
                self.state.quality_mode.store(mode, Ordering::SeqCst);
                info!("Stream quality mode set to {}", mode);
            }
            InputPacket::SetStreaming { enabled } => {
                info!("Client requested screen streaming: {}", enabled);
                self.state.stream_enabled.store(enabled, Ordering::SeqCst);
            }
            InputPacket::SetPrivacyCurtain { enabled } => {
                self.privacy.set_enabled(enabled);
            }
            InputPacket::TouchDirect { norm_x, norm_y, action } => {
                // touch_direct performs the click itself; it must NOT also go through dispatch().
                let is_text = self.sim.touch_direct(norm_x, norm_y, action);
                if is_text && (action == 0 || action == 2) {
                    let _ = self.ctrl_tx.try_send(Message::Text("{\"type\":\"text_focus\"}".to_string()));
                }
            }
            InputPacket::SetAudioConfig { speaker_enabled, mic_enabled } => {
                info!("Audio config updated: speaker={}, mic={}", speaker_enabled, mic_enabled);
                if speaker_enabled {
                    if self.audio.is_none() {
                        self.audio = Some(AudioHandle::start(self.audio_tx.clone()));
                    }
                } else {
                    self.audio = None; // Drop stops the thread via its own token
                }
            }
            p @ (InputPacket::ChangeResolution { .. }
            | InputPacket::ChangeOrientation { .. }
            | InputPacket::ClipboardSync { .. }) => self.dispatch_blocking(p),
            other => self.sim.dispatch(other),
        }
    }
}
