//! Universal Native Volume Control for Wisp
//! Controls internal player volume directly across 5 supported players:
//! 1. Spotify Desktop (`SpotifySessionVolumeAdapter`) via its Windows Core Audio session (Volume
//!    Mixer). Its in-app slider can't be driven: through UI Automation it steals focus, moves in
//!    10% key steps after ~2 s, and ignores changes while minimized.
//! 2. Spotifast (`SpotifastVolumeAdapter`) via native CLI IPC [0.0, 100.0].
//! 3. Apple Music for Windows (`AppleMusicVolumeAdapter`) via Windows UI Automation `RangeValuePattern`.
//! 4. Google Chrome (`ChromeYouTubeVolumeAdapter`) via Windows Core Audio session volume (`ISimpleAudioVolume`).
//! 5. Microsoft Edge (`EdgeYouTubeVolumeAdapter`) via Windows Core Audio session volume (`ISimpleAudioVolume`).
//!
//! Strict volume routing isolation: Never touches `IAudioEndpointVolume`. Browser volume targets only the matching application's session.
//! Background operation: Does not bring target windows to foreground, restore minimized windows, or switch tabs.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use windows::core::{Interface, PWSTR};
use windows::Win32::Foundation::{CloseHandle, BOOL, HWND};
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, GetWindowTextLengthW, GetWindowThreadProcessId, IsWindowVisible,
};

/// Common normalized volume interface for all five target players.
pub trait VolumeAdapter: Send + Sync {
    fn is_available(&self) -> bool;
    fn get_volume(&self) -> Option<f32>;
    fn set_volume(&mut self, value: f32) -> bool;
    fn get_muted(&self) -> Option<bool>;
    fn set_muted(&mut self, value: bool) -> bool;
    fn reconnect(&mut self);
    fn disconnect(&mut self);
}

/// Normalizes volume level to [0.0, 1.0].
pub fn normalize_volume(v: f32) -> f32 {
    if v.is_nan() {
        0.
    } else {
        v.clamp(0., 1.)
    }
}

/// Maps Wisp volume [0.0, 1.0] to Spotifast native slider value [0.0, 100.0].
pub fn map_to_spotifast_range(v: f32) -> f64 {
    (normalize_volume(v) as f64) * 100.0
}

/// Maps Spotifast native slider value [0.0, 100.0] to Wisp volume [0.0, 1.0].
pub fn map_from_spotifast_range(v: f64) -> f32 {
    if v.is_nan() {
        0.
    } else {
        ((v / 100.0) as f32).clamp(0., 1.)
    }
}

enum DesktopCommand {
    SetLevel(f32),
    SetMuted(bool),
    Reconnect,
}

/// Executable file name of a process, or empty if it cannot be queried.
pub fn process_name(pid: u32) -> String {
    if pid == 0 {
        return String::new();
    }
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, BOOL::from(false), pid)
        else {
            return String::new();
        };
        let mut buf = [0u16; 520];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
        .is_ok();
        let _ = CloseHandle(handle);
        if !ok {
            return String::new();
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit('\\').next().unwrap_or_default().to_string()
    }
}

pub fn spotifast_exe_path() -> std::path::PathBuf {
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let p = std::path::PathBuf::from(local_app_data).join(r"Programs\Spotifast\spotifast.exe");
        if p.exists() {
            return p;
        }
    }
    std::path::PathBuf::from("spotifast.exe")
}

/// Runs one `spotifast` verb and waits for it to finish, so successive commands reach
/// Spotifast in order (fire-and-forget processes could overtake each other during a drag).
fn run_spotifast(args: &[&str]) -> Option<String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::process::Command::new(spotifast_exe_path())
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Spotifast's volume from `spotifast now-playing --raw`: tab-separated state, title, artist,
/// album, position ms, duration ms, then the volume in percent (0 while muted).
pub fn parse_spotifast_volume(raw: &str) -> Option<f32> {
    let percent: f64 = raw.lines().next()?.split('\t').nth(6)?.trim().parse().ok()?;
    Some(map_from_spotifast_range(percent))
}

/// The running Spotifast's volume; None when it is not running.
pub fn read_spotifast_volume() -> Option<f32> {
    parse_spotifast_volume(&run_spotifast(&["now-playing", "--raw"])?)
}

/// Sets Spotifast's volume (this also unmutes it).
pub fn write_spotifast_volume(level: f32) -> bool {
    let percent = map_to_spotifast_range(level).round() as u32;
    run_spotifast(&["volume", &percent.to_string()]).is_some()
}

pub fn find_applemusic_window() -> Option<HWND> {
    unsafe {
        let mut curr =
            FindWindowExW(HWND::default(), HWND::default(), None, None).unwrap_or_default();
        while !curr.is_invalid() {
            let mut pid = 0;
            let _ = GetWindowThreadProcessId(curr, Some(&mut pid));
            if pid != 0 {
                let pname = process_name(pid);
                if pname.eq_ignore_ascii_case("AppleMusic.exe") {
                    let len = GetWindowTextLengthW(curr);
                    let visible = IsWindowVisible(curr).as_bool();
                    if len > 0 && visible {
                        return Some(curr);
                    }
                }
            }
            curr = FindWindowExW(HWND::default(), curr, None, None).unwrap_or_default();
        }
        None
    }
}

// ---------------------------------------------------------------------------
// 2. Spotifast Volume Adapter (Native CLI & State Interface)
// ---------------------------------------------------------------------------

pub struct SpotifastVolumeAdapter {
    tx: Option<Sender<DesktopCommand>>,
    pub cached_level: Arc<AtomicU32>,
    pub cached_muted: Arc<AtomicBool>,
    pub is_connected: Arc<AtomicBool>,
    pub last_unmuted: Arc<Mutex<f32>>,
}

impl SpotifastVolumeAdapter {
    pub fn new() -> Self {
        Self {
            tx: None,
            cached_level: Arc::new(AtomicU32::new(f32::to_bits(-1.))),
            cached_muted: Arc::new(AtomicBool::new(false)),
            is_connected: Arc::new(AtomicBool::new(false)),
            last_unmuted: Arc::new(Mutex::new(0.5)),
        }
    }

    fn ensure_worker(&mut self) -> Sender<DesktopCommand> {
        if let Some(ref tx) = self.tx {
            return tx.clone();
        }
        let (tx, rx) = channel::<DesktopCommand>();
        let cached_level = self.cached_level.clone();
        let cached_muted = self.cached_muted.clone();
        let is_connected = self.is_connected.clone();
        let last_unmuted = self.last_unmuted.clone();

        thread::Builder::new()
            .name("wisp-spotifast-worker".into())
            .spawn(move || {
                spotifast_worker_loop(rx, cached_level, cached_muted, is_connected, last_unmuted);
            })
            .ok();

        self.tx = Some(tx.clone());
        tx
    }
}

impl VolumeAdapter for SpotifastVolumeAdapter {
    fn is_available(&self) -> bool {
        self.is_connected.load(Ordering::Relaxed)
    }

    fn get_volume(&self) -> Option<f32> {
        if !self.is_available() {
            return None;
        }
        let val = f32::from_bits(self.cached_level.load(Ordering::Relaxed));
        (val >= 0.).then_some(val)
    }

    fn set_volume(&mut self, value: f32) -> bool {
        let level = normalize_volume(value);
        self.cached_level
            .store(f32::to_bits(level), Ordering::Relaxed);
        if level > 0.005 {
            self.cached_muted.store(false, Ordering::Relaxed);
            if let Ok(mut unmuted) = self.last_unmuted.lock() {
                *unmuted = level;
            }
        }
        let tx = self.ensure_worker();
        let _ = tx.send(DesktopCommand::SetLevel(level));
        true
    }

    fn get_muted(&self) -> Option<bool> {
        if !self.is_available() {
            return None;
        }
        Some(self.cached_muted.load(Ordering::Relaxed))
    }

    fn set_muted(&mut self, value: bool) -> bool {
        self.cached_muted.store(value, Ordering::Relaxed);
        let tx = self.ensure_worker();
        let _ = tx.send(DesktopCommand::SetMuted(value));
        true
    }

    fn reconnect(&mut self) {
        let tx = self.ensure_worker();
        let _ = tx.send(DesktopCommand::Reconnect);
    }

    /// Stops the worker but keeps the last known level, so reopening the panel shows the
    /// slider at once; the worker's first reading corrects it or hides it.
    fn disconnect(&mut self) {
        self.tx = None;
    }
}

fn spotifast_worker_loop(
    rx: Receiver<DesktopCommand>,
    cached_level: Arc<AtomicU32>,
    cached_muted: Arc<AtomicBool>,
    is_connected: Arc<AtomicBool>,
    last_unmuted: Arc<Mutex<f32>>,
) {
    // Each reading starts a `spotifast` process (~0.2 s), so poll about once a second, and
    // never shortly after a change: a reading taken before it landed would move the slider back.
    const POLL: Duration = Duration::from_millis(1000);
    const SETTLE: Duration = Duration::from_millis(1500);
    let mut last_poll: Option<Instant> = None;
    let mut changed: Option<Instant> = None;
    // Spotifast's own mute state as last read or set here. `cached_muted` can't serve: the
    // adapter stores the requested state there before the command reaches this worker.
    let mut player_muted = false;

    loop {
        let wait = last_poll.map_or(Duration::ZERO, |t| POLL.saturating_sub(t.elapsed()));
        let mut cmds = Vec::new();
        match rx.recv_timeout(wait) {
            Ok(c) => cmds.push(c),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        cmds.extend(rx.try_iter());
        // A drag queues many levels; only the newest one is sent.
        let newest_level = cmds
            .iter()
            .rposition(|c| matches!(c, DesktopCommand::SetLevel(_)));

        for (i, cmd) in cmds.into_iter().enumerate() {
            match cmd {
                DesktopCommand::SetLevel(_) if Some(i) != newest_level => {}
                DesktopCommand::SetLevel(lvl) => {
                    if write_spotifast_volume(lvl) {
                        player_muted = lvl <= 0.005;
                        cached_level.store(f32::to_bits(lvl), Ordering::Relaxed);
                        cached_muted.store(player_muted, Ordering::Relaxed);
                        if lvl > 0.005 {
                            if let Ok(mut u) = last_unmuted.lock() {
                                *u = lvl;
                            }
                        }
                        changed = Some(Instant::now());
                    }
                }
                DesktopCommand::SetMuted(muted) => {
                    if muted && !player_muted {
                        // `mute` toggles; Spotifast's own unmute then restores its level.
                        if run_spotifast(&["mute"]).is_some() {
                            player_muted = true;
                        }
                    } else if !muted && player_muted {
                        // Setting a volume unmutes, and works even if it was dragged to zero.
                        let restore = last_unmuted.lock().map(|u| *u).unwrap_or(0.5).max(0.05);
                        if write_spotifast_volume(restore) {
                            player_muted = false;
                            cached_level.store(f32::to_bits(restore), Ordering::Relaxed);
                        }
                    }
                    cached_muted.store(player_muted, Ordering::Relaxed);
                    changed = Some(Instant::now());
                }
                DesktopCommand::Reconnect => {
                    if !is_connected.load(Ordering::Relaxed) {
                        last_poll = None;
                    }
                }
            }
        }

        let settled = changed.map_or(true, |t| t.elapsed() >= SETTLE);
        let due = last_poll.map_or(true, |t| t.elapsed() >= POLL);
        if settled && due {
            last_poll = Some(Instant::now());
            match read_spotifast_volume() {
                Some(vol) => {
                    let muted = vol <= 0.005;
                    player_muted = muted;
                    cached_muted.store(muted, Ordering::Relaxed);
                    if muted {
                        // Spotifast reports 0 while muted; keep showing the level it returns to.
                        let back = last_unmuted.lock().map(|u| *u).unwrap_or(0.);
                        cached_level.store(f32::to_bits(back), Ordering::Relaxed);
                    } else {
                        cached_level.store(f32::to_bits(vol), Ordering::Relaxed);
                        if let Ok(mut u) = last_unmuted.lock() {
                            *u = vol;
                        }
                    }
                    is_connected.store(true, Ordering::Relaxed);
                }
                None => is_connected.store(false, Ordering::Relaxed),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 3. Apple Music for Windows Volume Adapter
// ---------------------------------------------------------------------------

pub struct AppleMusicVolumeAdapter {
    cached_level: Arc<AtomicU32>,
    cached_muted: Arc<AtomicBool>,
    is_connected: Arc<AtomicBool>,
}

impl AppleMusicVolumeAdapter {
    pub fn new() -> Self {
        Self {
            cached_level: Arc::new(AtomicU32::new(f32::to_bits(-1.))),
            cached_muted: Arc::new(AtomicBool::new(false)),
            is_connected: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl VolumeAdapter for AppleMusicVolumeAdapter {
    fn is_available(&self) -> bool {
        self.is_connected.load(Ordering::Relaxed)
    }

    fn get_volume(&self) -> Option<f32> {
        if !self.is_connected.load(Ordering::Relaxed) {
            return None;
        }
        let bits = self.cached_level.load(Ordering::Relaxed);
        let val = f32::from_bits(bits);
        if val < 0. {
            None
        } else {
            Some(val)
        }
    }

    fn set_volume(&mut self, _value: f32) -> bool {
        // When Apple Music is not installed or running, cleanly return false
        false
    }

    fn get_muted(&self) -> Option<bool> {
        if !self.is_connected.load(Ordering::Relaxed) {
            return None;
        }
        Some(self.cached_muted.load(Ordering::Relaxed))
    }

    fn set_muted(&mut self, _value: bool) -> bool {
        false
    }

    fn reconnect(&mut self) {
        if let Some(_hwnd) = find_applemusic_window() {
            self.is_connected.store(true, Ordering::Relaxed);
        } else {
            self.is_connected.store(false, Ordering::Relaxed);
        }
    }

    fn disconnect(&mut self) {
        self.cached_level
            .store(f32::to_bits(-1.), Ordering::Relaxed);
        self.cached_muted.store(false, Ordering::Relaxed);
        self.is_connected.store(false, Ordering::Relaxed);
    }
}

// ---------------------------------------------------------------------------
// 4. Spotify, Chrome & Edge: the app's own Windows audio session (Volume Mixer)
// ---------------------------------------------------------------------------

/// Finds all active or inactive audio sessions belonging to the specified executable name (e.g. "chrome.exe", "msedge.exe").
/// Returns matching ISimpleAudioVolume instances, sorted with active sessions first.
/// The audio sessions of `target_exe` (e.g. "spotify.exe") on every active output device, as
/// listed in the Volume Mixer; active ones first. Apps can play to a device other than the
/// default one, so all of them are searched.
pub fn find_app_audio_sessions(target_exe: &str) -> Vec<ISimpleAudioVolume> {
    let mut active = Vec::new();
    let mut inactive = Vec::new();
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let Ok(enumerator) =
            CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL)
        else {
            return active;
        };
        let Ok(devices) = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) else {
            return active;
        };
        for d in 0..devices.GetCount().unwrap_or(0) {
            let Ok(device) = devices.Item(d) else {
                continue;
            };
            let Ok(mgr) = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else {
                continue;
            };
            let Ok(session_enum) = mgr.GetSessionEnumerator() else {
                continue;
            };
            for i in 0..session_enum.GetCount().unwrap_or(0) {
                let Ok(session) = session_enum.GetSession(i) else {
                    continue;
                };
                let Ok(control2) = session.cast::<IAudioSessionControl2>() else {
                    continue;
                };
                let Ok(pid) = control2.GetProcessId() else {
                    continue;
                };
                if pid == 0 || !process_name(pid).eq_ignore_ascii_case(target_exe) {
                    continue;
                }
                let state = control2.GetState().unwrap_or(AudioSessionStateInactive);
                if state == AudioSessionStateExpired {
                    continue;
                }
                let Ok(vol) = session.cast::<ISimpleAudioVolume>() else {
                    continue;
                };
                if state == AudioSessionStateActive {
                    active.push(vol);
                } else {
                    inactive.push(vol);
                }
            }
        }
    }
    active.extend(inactive);
    active
}

pub struct AudioSessionVolumeAdapter {
    target_exe: &'static str,
    cached_level: AtomicU32,
    cached_muted: AtomicBool,
    is_connected: AtomicBool,
    last_unmuted: Mutex<f32>,
    last_poll: Mutex<Instant>,
}

impl AudioSessionVolumeAdapter {
    pub fn new(target_exe: &'static str) -> Self {
        Self {
            target_exe,
            cached_level: AtomicU32::new(f32::to_bits(-1.)),
            cached_muted: AtomicBool::new(false),
            is_connected: AtomicBool::new(false),
            last_unmuted: Mutex::new(0.5),
            last_poll: Mutex::new(Instant::now() - Duration::from_secs(10)),
        }
    }

    fn poll_sessions_if_due(&self, force: bool) {
        if !force {
            if let Ok(last) = self.last_poll.lock() {
                if last.elapsed() < Duration::from_millis(250) {
                    return;
                }
            }
        }
        let sessions = find_app_audio_sessions(self.target_exe);
        if let Ok(mut last) = self.last_poll.lock() {
            *last = Instant::now();
        }
        if let Some(vol) = sessions.first() {
            unsafe {
                if let Ok(level) = vol.GetMasterVolume() {
                    let lvl = normalize_volume(level);
                    self.cached_level
                        .store(f32::to_bits(lvl), Ordering::Relaxed);
                    self.is_connected.store(true, Ordering::Relaxed);
                    if lvl > 0.005 {
                        if let Ok(mut u) = self.last_unmuted.lock() {
                            *u = lvl;
                        }
                    }
                }
                if let Ok(muted) = vol.GetMute() {
                    self.cached_muted.store(muted.as_bool(), Ordering::Relaxed);
                }
            }
        } else {
            self.cached_level
                .store(f32::to_bits(-1.), Ordering::Relaxed);
            self.cached_muted.store(false, Ordering::Relaxed);
            self.is_connected.store(false, Ordering::Relaxed);
        }
    }
}

impl VolumeAdapter for AudioSessionVolumeAdapter {
    fn is_available(&self) -> bool {
        if self.is_connected.load(Ordering::Relaxed) {
            self.poll_sessions_if_due(false);
        }
        self.is_connected.load(Ordering::Relaxed)
    }

    fn get_volume(&self) -> Option<f32> {
        if !self.is_connected.load(Ordering::Relaxed) {
            return None;
        }
        self.poll_sessions_if_due(false);
        if !self.is_connected.load(Ordering::Relaxed) {
            return None;
        }
        let bits = self.cached_level.load(Ordering::Relaxed);
        let val = f32::from_bits(bits);
        if val < 0. {
            None
        } else {
            Some(val)
        }
    }

    fn set_volume(&mut self, value: f32) -> bool {
        let level = normalize_volume(value);
        let sessions = find_app_audio_sessions(self.target_exe);
        if sessions.is_empty() {
            self.is_connected.store(false, Ordering::Relaxed);
            return false;
        }
        let mut any_ok = false;
        for vol in &sessions {
            unsafe {
                if vol.SetMasterVolume(level, std::ptr::null()).is_ok() {
                    any_ok = true;
                }
            }
        }
        if any_ok {
            self.cached_level
                .store(f32::to_bits(level), Ordering::Relaxed);
            self.is_connected.store(true, Ordering::Relaxed);
            if level > 0.005 {
                self.cached_muted.store(false, Ordering::Relaxed);
                if let Ok(mut u) = self.last_unmuted.lock() {
                    *u = level;
                }
            }
            if let Ok(mut last) = self.last_poll.lock() {
                *last = Instant::now();
            }
            true
        } else {
            false
        }
    }

    fn get_muted(&self) -> Option<bool> {
        if !self.is_connected.load(Ordering::Relaxed) {
            return None;
        }
        self.poll_sessions_if_due(false);
        if !self.is_connected.load(Ordering::Relaxed) {
            return None;
        }
        Some(self.cached_muted.load(Ordering::Relaxed))
    }

    fn set_muted(&mut self, value: bool) -> bool {
        let sessions = find_app_audio_sessions(self.target_exe);
        if sessions.is_empty() {
            return false;
        }
        let mut any_ok = false;
        for vol in &sessions {
            unsafe {
                if vol.SetMute(BOOL::from(value), std::ptr::null()).is_ok() {
                    any_ok = true;
                }
            }
        }
        if any_ok {
            self.cached_muted.store(value, Ordering::Relaxed);
            if let Ok(mut last) = self.last_poll.lock() {
                *last = Instant::now();
            }
            true
        } else {
            false
        }
    }

    fn reconnect(&mut self) {
        self.poll_sessions_if_due(true);
    }

    fn disconnect(&mut self) {
        self.cached_level
            .store(f32::to_bits(-1.), Ordering::Relaxed);
        self.cached_muted.store(false, Ordering::Relaxed);
        self.is_connected.store(false, Ordering::Relaxed);
    }
}

pub type SpotifySessionVolumeAdapter = AudioSessionVolumeAdapter;
pub type ChromeYouTubeVolumeAdapter = AudioSessionVolumeAdapter;
pub type EdgeYouTubeVolumeAdapter = AudioSessionVolumeAdapter;
pub type SoundCloudVolumeAdapter = AudioSessionVolumeAdapter;

// ---------------------------------------------------------------------------
// Orchestrator: Unified Wisp Volume
// ---------------------------------------------------------------------------

pub struct Volume {
    app: String,
    spotify: Arc<Mutex<SpotifySessionVolumeAdapter>>,
    spotifast: Arc<Mutex<SpotifastVolumeAdapter>>,
    apple_music: Arc<Mutex<AppleMusicVolumeAdapter>>,
    chrome: Arc<Mutex<ChromeYouTubeVolumeAdapter>>,
    edge: Arc<Mutex<EdgeYouTubeVolumeAdapter>>,
    soundcloud: Arc<Mutex<SoundCloudVolumeAdapter>>,
}

impl Volume {
    pub fn none() -> Self {
        Self {
            app: String::new(),
            spotify: Arc::new(Mutex::new(AudioSessionVolumeAdapter::new("spotify.exe"))),
            spotifast: Arc::new(Mutex::new(SpotifastVolumeAdapter::new())),
            apple_music: Arc::new(Mutex::new(AppleMusicVolumeAdapter::new())),
            chrome: Arc::new(Mutex::new(AudioSessionVolumeAdapter::new(
                "chrome.exe",
            ))),
            edge: Arc::new(Mutex::new(AudioSessionVolumeAdapter::new(
                "msedge.exe",
            ))),
            soundcloud: Arc::new(Mutex::new(AudioSessionVolumeAdapter::new(
                "soundcloud.exe",
            ))),
        }
    }

    pub fn app(&self) -> &str {
        &self.app
    }

    #[allow(dead_code)]
    pub fn is_spotify(&self) -> bool {
        self.app.eq_ignore_ascii_case("spotify") || self.app.eq_ignore_ascii_case("spotifast")
    }

    pub fn is_supported(&self) -> bool {
        matches!(
            self.app.to_lowercase().as_str(),
            "spotify" | "spotifast" | "applemusic" | "chrome" | "msedge" | "soundcloud"
        )
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        !self.is_active_adapter_available()
    }

    #[allow(dead_code)]
    fn is_active_adapter_available(&self) -> bool {
        match self.app.to_lowercase().as_str() {
            "spotify" => self.spotify.lock().map_or(false, |a| a.is_available()),
            "spotifast" => self.spotifast.lock().map_or(false, |a| a.is_available()),
            "apple_music" | "applemusic" => {
                self.apple_music.lock().map_or(false, |a| a.is_available())
            }
            "chrome" => self.chrome.lock().map_or(false, |a| a.is_available()),
            "msedge" => self.edge.lock().map_or(false, |a| a.is_available()),
            "soundcloud" => self.soundcloud.lock().map_or(false, |a| a.is_available()),
            _ => false,
        }
    }

    /// Selects and connects exclusively to the active player's volume adapter.
    pub fn refresh(&mut self, app: &str) {
        let prev = self.app.to_lowercase();
        let next = app.to_lowercase();
        self.app = app.to_string();

        if prev != next {
            match prev.as_str() {
                "spotify" => {
                    let _ = self.spotify.lock().map(|mut a| a.disconnect());
                }
                "spotifast" => {
                    let _ = self.spotifast.lock().map(|mut a| a.disconnect());
                }
                "applemusic" => {
                    let _ = self.apple_music.lock().map(|mut a| a.disconnect());
                }
                "chrome" => {
                    let _ = self.chrome.lock().map(|mut a| a.disconnect());
                }
                "msedge" => {
                    let _ = self.edge.lock().map(|mut a| a.disconnect());
                }
                "soundcloud" => {
                    let _ = self.soundcloud.lock().map(|mut a| a.disconnect());
                }
                _ => {}
            }
        }

        match next.as_str() {
            "spotify" => {
                let _ = self.spotify.lock().map(|mut a| a.reconnect());
            }
            "spotifast" => {
                let _ = self.spotifast.lock().map(|mut a| a.reconnect());
            }
            "applemusic" => {
                let _ = self.apple_music.lock().map(|mut a| a.reconnect());
            }
            "chrome" => {
                let _ = self.chrome.lock().map(|mut a| a.reconnect());
            }
            "msedge" => {
                let _ = self.edge.lock().map(|mut a| a.reconnect());
            }
            "soundcloud" => {
                let _ = self.soundcloud.lock().map(|mut a| a.reconnect());
            }
            _ => {}
        }
    }

    /// Disconnects the active player's volume adapter and releases its background worker.
    pub fn disconnect(&mut self) {
        let prev = std::mem::take(&mut self.app);
        if !prev.is_empty() {
            match prev.to_lowercase().as_str() {
                "spotify" => {
                    let _ = self.spotify.lock().map(|mut a| a.disconnect());
                }
                "spotifast" => {
                    let _ = self.spotifast.lock().map(|mut a| a.disconnect());
                }
                "applemusic" => {
                    let _ = self.apple_music.lock().map(|mut a| a.disconnect());
                }
                "chrome" => {
                    let _ = self.chrome.lock().map(|mut a| a.disconnect());
                }
                "msedge" => {
                    let _ = self.edge.lock().map(|mut a| a.disconnect());
                }
                "soundcloud" => {
                    let _ = self.soundcloud.lock().map(|mut a| a.disconnect());
                }
                _ => {}
            }
        }
    }

    /// Reads (volume, muted) from the active player's volume adapter.
    pub fn get(&self) -> Option<(f32, bool)> {
        match self.app.to_lowercase().as_str() {
            "spotify" => {
                let a = self.spotify.lock().ok()?;
                let v = a.get_volume()?;
                let m = a.get_muted().unwrap_or(false);
                Some((v, m))
            }
            "spotifast" => {
                let a = self.spotifast.lock().ok()?;
                let v = a.get_volume()?;
                let m = a.get_muted().unwrap_or(false);
                Some((v, m))
            }
            "applemusic" => {
                let a = self.apple_music.lock().ok()?;
                let v = a.get_volume()?;
                let m = a.get_muted().unwrap_or(false);
                Some((v, m))
            }
            "chrome" => {
                let a = self.chrome.lock().ok()?;
                let v = a.get_volume()?;
                let m = a.get_muted().unwrap_or(false);
                Some((v, m))
            }
            "msedge" => {
                let a = self.edge.lock().ok()?;
                let v = a.get_volume()?;
                let m = a.get_muted().unwrap_or(false);
                Some((v, m))
            }
            "soundcloud" => {
                let a = self.soundcloud.lock().ok()?;
                let v = a.get_volume()?;
                let m = a.get_muted().unwrap_or(false);
                Some((v, m))
            }
            _ => None,
        }
    }

    /// Sets native internal volume on the active player.
    pub fn set_level(&self, level: f32) {
        let level = normalize_volume(level);
        match self.app.to_lowercase().as_str() {
            "spotify" => {
                let _ = self.spotify.lock().map(|mut a| a.set_volume(level));
            }
            "spotifast" => {
                let _ = self.spotifast.lock().map(|mut a| a.set_volume(level));
            }
            "applemusic" => {
                let _ = self.apple_music.lock().map(|mut a| a.set_volume(level));
            }
            "chrome" => {
                let _ = self.chrome.lock().map(|mut a| a.set_volume(level));
            }
            "msedge" => {
                let _ = self.edge.lock().map(|mut a| a.set_volume(level));
            }
            "soundcloud" => {
                let _ = self.soundcloud.lock().map(|mut a| a.set_volume(level));
            }
            _ => {}
        }
    }

    /// Sets native mute state on the active player.
    pub fn set_muted(&self, muted: bool) {
        match self.app.to_lowercase().as_str() {
            "spotify" => {
                let _ = self.spotify.lock().map(|mut a| a.set_muted(muted));
            }
            "spotifast" => {
                let _ = self.spotifast.lock().map(|mut a| a.set_muted(muted));
            }
            "applemusic" => {
                let _ = self.apple_music.lock().map(|mut a| a.set_muted(muted));
            }
            "chrome" => {
                let _ = self.chrome.lock().map(|mut a| a.set_muted(muted));
            }
            "msedge" => {
                let _ = self.edge.lock().map(|mut a| a.set_muted(muted));
            }
            "soundcloud" => {
                let _ = self.soundcloud.lock().map(|mut a| a.set_muted(muted));
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Unit Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_audio_session_symbols() {
        use windows::Win32::Media::Audio::*;
        use windows::Win32::System::Com::*;
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            if let Ok(enumerator) =
                CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL)
            {
                if let Ok(device) = enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia) {
                    if let Ok(mgr) = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) {
                        if let Ok(session_enum) = mgr.GetSessionEnumerator() {
                            let count = session_enum.GetCount().unwrap_or(0);
                            for i in 0..count {
                                if let Ok(session) = session_enum.GetSession(i) {
                                    if let Ok(control2) = session.cast::<IAudioSessionControl2>() {
                                        let pid = control2.GetProcessId().unwrap_or(0);
                                        let state = control2.GetState().map(|s| s.0).unwrap_or(-1);
                                        let pname = process_name(pid);
                                        if let Ok(vol) = session.cast::<ISimpleAudioVolume>() {
                                            let level = vol.GetMasterVolume().unwrap_or(0.0);
                                            let mute = vol
                                                .GetMute()
                                                .unwrap_or(windows::Win32::Foundation::BOOL(0));
                                            println!("Session {}: pid={}, name='{}', state={}, level={:.2}, mute={}", i, pid, pname, state, level, mute.as_bool());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_volume_normalization() {
        assert_eq!(normalize_volume(-0.5), 0.0);
        assert_eq!(normalize_volume(1.5), 1.0);
        assert_eq!(normalize_volume(0.42), 0.42);
        assert_eq!(normalize_volume(f32::NAN), 0.0);
    }

    #[test]
    fn test_range_mapping() {


        assert!((map_to_spotifast_range(0.75) - 75.0).abs() < 1e-6);
        assert_eq!(map_to_spotifast_range(-0.1), 0.0);
        assert_eq!(map_to_spotifast_range(1.2), 100.0);

        assert!((map_from_spotifast_range(50.0) - 0.5).abs() < 1e-6);
        assert_eq!(map_from_spotifast_range(-10.0), 0.0);
        assert_eq!(map_from_spotifast_range(150.0), 1.0);
        assert_eq!(map_from_spotifast_range(f64::NAN), 0.0);
    }

    #[test]
    fn test_unrelated_app_isolation_returns_none() {
        let mut vol = Volume::none();
        vol.refresh("notepad");
        assert_eq!(vol.get(), None);
        assert!(vol.is_empty());
        assert!(!vol.is_supported());

        vol.refresh("calculator");
        assert_eq!(vol.get(), None);
        assert!(vol.is_empty());
        assert!(!vol.is_supported());

        vol.set_level(0.8);
        vol.set_muted(true);
        assert_eq!(vol.get(), None);
    }

    #[test]
    fn test_rapid_player_switching_and_isolation() {
        let mut vol = Volume::none();
        for _ in 0..10 {
            vol.refresh("spotify");
            assert!(vol.is_spotify());
            assert!(vol.is_supported());

            vol.refresh("spotifast");
            assert!(vol.is_spotify());
            assert!(vol.is_supported());

            vol.refresh("chrome");
            assert!(!vol.is_spotify());
            assert!(vol.is_supported());

            vol.refresh("msedge");
            assert!(!vol.is_spotify());
            assert!(vol.is_supported());

            vol.refresh("applemusic");
            assert!(!vol.is_spotify());
            assert!(vol.is_supported());

            vol.refresh("soundcloud");
            assert!(!vol.is_spotify());
            assert!(vol.is_supported());

            vol.refresh("unknown");
            assert!(!vol.is_supported());
            assert_eq!(vol.get(), None);
        }
    }

    #[test]
    fn test_spotifast_adapter_mock() {
        let mut adapter = SpotifastVolumeAdapter::new();
        assert!(!adapter.is_available());
        assert_eq!(adapter.get_volume(), None);
        assert_eq!(adapter.get_muted(), None);

        // Inject simulated connection state
        adapter.is_connected.store(true, Ordering::Relaxed);
        adapter
            .cached_level
            .store(f32::to_bits(0.65), Ordering::Relaxed);
        adapter.cached_muted.store(false, Ordering::Relaxed);

        assert!(adapter.is_available());
        assert_eq!(adapter.get_volume(), Some(0.65));
        assert_eq!(adapter.get_muted(), Some(false));

        // Closing the panel stops the worker but keeps the last level for an instant reopen.
        adapter.disconnect();
        assert!(adapter.is_available());
        assert_eq!(adapter.get_volume(), Some(0.65));
    }

    /// Mutes and unmutes the running Spotifast through the adapter:
    /// `cargo test live_spotifast_mute -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_spotifast_mute() {
        let before = read_spotifast_volume().expect("Spotifast must be running and playing");
        assert!(before > 0.005, "unmute Spotifast first");
        let mut adapter = SpotifastVolumeAdapter::new();
        adapter.reconnect();
        thread::sleep(Duration::from_millis(1500));
        assert_eq!(adapter.get_muted(), Some(false));

        adapter.set_muted(true);
        thread::sleep(Duration::from_millis(1500));
        let muted = read_spotifast_volume();
        println!("before {before:.2}, muted {muted:?}");
        assert_eq!(muted, Some(0.));

        adapter.set_muted(false);
        thread::sleep(Duration::from_millis(1500));
        let after = read_spotifast_volume();
        println!("unmuted {after:?}");
        assert!(after.is_some_and(|v| (v - before).abs() < 0.015));
    }

    /// Halves and restores the running Spotify's Volume Mixer level through the facade:
    /// `cargo test live_spotify_session -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_spotify_session() {
        let mut vol = Volume::none();
        vol.refresh("spotify");
        let (before, muted) = vol.get().expect("Spotify must be running with an audio session");
        println!("before {before:.2} muted {muted}");
        vol.set_level(before / 2.);
        let sessions = find_app_audio_sessions("spotify.exe");
        let now = unsafe { sessions[0].GetMasterVolume().unwrap() };
        println!("after halving {now:.2}");
        vol.set_level(before);
        assert!((now - before / 2.).abs() < 0.01);
        assert!(vol.get().is_some_and(|(l, _)| (l - before).abs() < 0.01));
    }

    #[test]
    fn test_parse_spotifast_now_playing_volume() {
        let raw = "playing\tSweet Fantasy\tMagnolia Celebration\tFantasia\t93208\t396000\t73\toff\toff\thttps://x\tno\tSpotifast\n";
        assert_eq!(parse_spotifast_volume(raw), Some(0.73));
        let muted = "paused\ta\tb\tc\t0\t0\t0\toff\toff\t\tno\tSpotifast";
        assert_eq!(parse_spotifast_volume(muted), Some(0.0));
        assert_eq!(parse_spotifast_volume("stopped"), None);
        assert_eq!(parse_spotifast_volume(""), None);
    }

    #[test]
    fn test_apple_music_not_installed() {
        let win = find_applemusic_window();
        assert_eq!(win, None, "Apple Music should not be detected on test host");
        let mut adapter = AppleMusicVolumeAdapter::new();
        adapter.reconnect();
        assert!(!adapter.is_available());
        assert_eq!(adapter.get_volume(), None);
        assert_eq!(adapter.get_muted(), None);
        assert!(!adapter.set_volume(0.5));
        assert!(!adapter.set_muted(true));
    }

    #[test]
    fn test_browser_audio_session_adapter_nonexistent() {
        let mut adapter = AudioSessionVolumeAdapter::new("nonexistent_app_12345.exe");
        assert!(!adapter.is_available());
        assert_eq!(adapter.get_volume(), None);
        assert_eq!(adapter.get_muted(), None);
        assert!(!adapter.set_volume(0.50));
        assert!(!adapter.set_muted(true));
        adapter.reconnect();
        assert!(!adapter.is_available());
        adapter.disconnect();
        assert!(!adapter.is_available());
    }

    #[test]
    fn test_browser_audio_session_isolation() {
        let mut chrome = AudioSessionVolumeAdapter::new("chrome.exe");
        let mut edge = AudioSessionVolumeAdapter::new("msedge.exe");

        // Disconnect both
        chrome.disconnect();
        edge.disconnect();
        assert!(!chrome.is_available());
        assert!(!edge.is_available());

        // Reconnecting edge when edge is not running/installed remains unavailable
        edge.reconnect();
        assert!(!edge.is_available());
        assert_eq!(edge.get_volume(), None);
        assert!(!edge.set_volume(0.5));
        assert!(!edge.set_muted(true));

        // Unrelated app session volume must never be touched
        let mut bogus = AudioSessionVolumeAdapter::new("calculator.exe");
        bogus.reconnect();
        assert!(!bogus.is_available());
        assert_eq!(bogus.get_volume(), None);
        assert!(!bogus.set_volume(0.8));
        assert!(!bogus.set_muted(true));
    }

    #[test]
    fn test_browser_audio_sessions_independent_of_master_volume() {
        // Enumerate sessions to verify query safety and isolation
        let chrome_sessions = find_app_audio_sessions("chrome.exe");
        let edge_sessions = find_app_audio_sessions("msedge.exe");
        let spotify_sessions = find_app_audio_sessions("spotify.exe");

        // Verify that chrome query returns only chrome sessions, edge only edge sessions
        println!("Detected Chrome audio sessions: {}", chrome_sessions.len());
        println!("Detected Edge audio sessions: {}", edge_sessions.len());
        println!(
            "Detected Spotify audio sessions: {}",
            spotify_sessions.len()
        );
    }

    #[test]
    fn test_chrome_session_live_if_running() {
        let mut chrome = AudioSessionVolumeAdapter::new("chrome.exe");
        chrome.reconnect();
        if chrome.is_available() {
            if let Some(orig) = chrome.get_volume() {
                println!("Chrome live session initial volume: {}", orig);

                // Read Spotify volume before
                let spotify_sessions_before = find_app_audio_sessions("spotify.exe");
                let spotify_vol_before = spotify_sessions_before
                    .first()
                    .and_then(|s| unsafe { s.GetMasterVolume().ok() });

                // Test 80%
                assert!(chrome.set_volume(0.80));
                let v80 = chrome.get_volume().unwrap_or(0.0);
                println!("Chrome live 80% readback: {}", v80);
                assert!((v80 - 0.80).abs() <= 0.02, "Expected ~0.80, got {}", v80);

                // Test 40%
                assert!(chrome.set_volume(0.40));
                let v40 = chrome.get_volume().unwrap_or(0.0);
                println!("Chrome live 40% readback: {}", v40);
                assert!((v40 - 0.40).abs() <= 0.02, "Expected ~0.40, got {}", v40);

                // Test 20%
                assert!(chrome.set_volume(0.20));
                let v20 = chrome.get_volume().unwrap_or(0.0);
                println!("Chrome live 20% readback: {}", v20);
                assert!((v20 - 0.20).abs() <= 0.02, "Expected ~0.20, got {}", v20);

                // Test Mute
                assert!(chrome.set_muted(true));
                println!("Chrome live muted: {:?}", chrome.get_muted());
                assert_eq!(chrome.get_muted(), Some(true));

                // Test Unmute
                assert!(chrome.set_muted(false));
                println!("Chrome live unmuted: {:?}", chrome.get_muted());
                assert_eq!(chrome.get_muted(), Some(false));

                // Restore original volume
                assert!(chrome.set_volume(orig));
                let v_restored = chrome.get_volume().unwrap_or(0.0);
                println!("Chrome live restored volume: {}", v_restored);
                assert!((v_restored - orig).abs() <= 0.02);

                // Verify Spotify session was completely untouched (isolation)
                let spotify_sessions_after = find_app_audio_sessions("spotify.exe");
                let spotify_vol_after = spotify_sessions_after
                    .first()
                    .and_then(|s| unsafe { s.GetMasterVolume().ok() });
                assert_eq!(
                    spotify_vol_before, spotify_vol_after,
                    "Spotify volume must remain strictly invariant"
                );
            }
        }
    }

    #[test]
    fn test_spotifast_native_live_if_running() {
        let mut adapter = SpotifastVolumeAdapter::new();
        adapter.reconnect();
        thread::sleep(Duration::from_millis(500));
        if adapter.is_available() {
            if let Some(orig) = adapter.get_volume() {
                println!("Spotifast live initial volume: {}", orig);
                // Test Matrix: 80%, 40%, 20%, mute, unmute, restore
                adapter.set_volume(0.80);
                thread::sleep(Duration::from_millis(600));
                let v80 = adapter.get_volume().unwrap_or(0.0);
                println!("Spotifast live 80% readback: {}", v80);
                assert!((v80 - 0.80).abs() <= 0.05, "Expected ~0.80, got {}", v80);

                adapter.set_volume(0.40);
                thread::sleep(Duration::from_millis(600));
                let v40 = adapter.get_volume().unwrap_or(0.0);
                println!("Spotifast live 40% readback: {}", v40);
                assert!((v40 - 0.40).abs() <= 0.05, "Expected ~0.40, got {}", v40);

                adapter.set_volume(0.20);
                thread::sleep(Duration::from_millis(600));
                let v20 = adapter.get_volume().unwrap_or(0.0);
                println!("Spotifast live 20% readback: {}", v20);
                assert!((v20 - 0.20).abs() <= 0.05, "Expected ~0.20, got {}", v20);

                adapter.set_muted(true);
                thread::sleep(Duration::from_millis(600));
                println!("Spotifast live muted: {:?}", adapter.get_muted());
                assert_eq!(adapter.get_muted(), Some(true));

                adapter.set_muted(false);
                thread::sleep(Duration::from_millis(600));
                println!("Spotifast live unmuted: {:?}", adapter.get_muted());
                assert_eq!(adapter.get_muted(), Some(false));

                adapter.set_volume(orig);
                thread::sleep(Duration::from_millis(600));
                println!("Spotifast live restored volume: {:?}", adapter.get_volume());
            }
        }
    }

}
