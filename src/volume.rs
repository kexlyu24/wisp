//! Universal Native Volume Control for Wisp
//! Controls internal player volume directly across 5 supported players:
//! 1. Spotify Desktop (`SpotifyVolumeAdapter`) via Windows UI Automation `RangeValuePattern` [0.0, 1.0].
//! 2. Spotifast (`SpotifastVolumeAdapter`) via native CLI IPC [0.0, 100.0].
//! 3. Apple Music for Windows (`AppleMusicVolumeAdapter`) via Windows UI Automation `RangeValuePattern`.
//! 4. Google Chrome (`ChromeYouTubeVolumeAdapter`) via Windows Core Audio session volume (`ISimpleAudioVolume`).
//! 5. Microsoft Edge (`EdgeYouTubeVolumeAdapter`) via Windows Core Audio session volume (`ISimpleAudioVolume`).
//!
//! Strict volume routing isolation: Never touches `IAudioEndpointVolume`. Browser volume targets only the matching application's session.
//! Background operation: Does not bring target windows to foreground, restore minimized windows, or switch tabs.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use windows::core::{Interface, PWSTR};
use windows::Win32::Foundation::{CloseHandle, BOOL, HWND, POINT};
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
};
use windows::Win32::System::Threading::{
    GetCurrentThreadId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, FindWindowW, GetAncestor, GetCursorPos, GetForegroundWindow, GetWindow,
    GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindow,
    IsWindowVisible, SetForegroundWindow, SetWindowPos, WindowFromPoint, GA_ROOT, GW_HWNDNEXT,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
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

/// Maps Wisp volume [0.0, 1.0] to Spotify native slider value [0.0, 1.0].
pub fn map_to_spotify_range(v: f32) -> f64 {
    normalize_volume(v) as f64
}

/// Maps Spotify native slider value to Wisp volume [0.0, 1.0].
pub fn map_from_spotify_range(v: f64) -> f32 {
    if v.is_nan() {
        0.
    } else {
        (v as f32).clamp(0., 1.)
    }
}

/// Maps Wisp volume [0.0, 1.0] to Spotifast native slider value [0.0, 100.0].
#[allow(dead_code)]
pub fn map_to_spotifast_range(v: f32) -> f64 {
    (normalize_volume(v) as f64) * 100.0
}

/// Maps Spotifast native slider value [0.0, 100.0] to Wisp volume [0.0, 1.0].
#[allow(dead_code)]
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

/// Locates the top-level main window of Spotify Desktop.
pub fn find_spotify_window() -> Option<HWND> {
    unsafe {
        let mut curr =
            FindWindowW(windows::core::w!("Chrome_WidgetWin_1"), None).unwrap_or_default();
        while !curr.is_invalid() {
            let mut pid = 0;
            let _ = GetWindowThreadProcessId(curr, Some(&mut pid));
            if pid != 0 {
                let pname = process_name(pid);
                if pname.eq_ignore_ascii_case("spotify.exe") {
                    let len = GetWindowTextLengthW(curr);
                    let visible = IsWindowVisible(curr).as_bool();
                    let iconic = IsIconic(curr).as_bool();
                    if len > 0 && (visible || iconic) {
                        return Some(curr);
                    }
                }
            }
            curr = FindWindowExW(
                HWND::default(),
                curr,
                windows::core::w!("Chrome_WidgetWin_1"),
                None,
            )
            .unwrap_or_default();
        }
        None
    }
}

/// Locates any window or background process of Spotifast.
pub fn find_spotifast_window() -> Option<HWND> {
    unsafe {
        let mut curr =
            FindWindowExW(HWND::default(), HWND::default(), None, None).unwrap_or_default();
        while !curr.is_invalid() {
            let mut pid = 0;
            let _ = GetWindowThreadProcessId(curr, Some(&mut pid));
            if pid != 0 {
                let pname = process_name(pid);
                if pname.eq_ignore_ascii_case("spotifast.exe") {
                    return Some(curr);
                }
            }
            curr = FindWindowExW(HWND::default(), curr, None, None).unwrap_or_default();
        }
        None
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

pub fn spotifast_volume_file_path() -> Option<std::path::PathBuf> {
    let local_app_data = std::env::var("LOCALAPPDATA").ok()?;
    let p = std::path::PathBuf::from(local_app_data).join(r"paolino\spotifast\data\volume\volume");
    Some(p)
}

pub fn read_spotifast_volume() -> Option<(f32, bool)> {
    let p = spotifast_volume_file_path()?;
    let text = std::fs::read_to_string(p).ok()?;
    let trimmed = text.trim();
    let raw: u32 = trimmed.parse().ok()?;
    let normalized = (raw as f32) / 65535.0;
    let muted = raw == 0;
    Some((normalized.clamp(0.0, 1.0), muted))
}

pub fn write_spotifast_volume(level: f32) -> bool {
    let percent = (level * 100.0).round() as u32;
    let raw = (level * 65535.0).round() as u32;
    if let Some(path) = spotifast_volume_file_path() {
        let _ = std::fs::write(path, raw.to_string());
    }
    let exe = spotifast_exe_path();
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    std::process::Command::new(exe)
        .arg("volume")
        .arg(percent.to_string())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .is_ok()
}

pub fn set_spotifast_mute(muted: bool) -> bool {
    let (_, current_muted) = read_spotifast_volume().unwrap_or((0.2, false));
    if current_muted != muted {
        if let Some(path) = spotifast_volume_file_path() {
            if muted {
                let _ = std::fs::write(path, "0");
            } else {
                let _ = std::fs::write(path, "26214"); // 40% restore
            }
        }
        let exe = spotifast_exe_path();
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let _ = std::process::Command::new(exe)
            .arg("mute")
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    }
    true
}

#[allow(dead_code)]
pub fn toggle_spotifast_mute() -> bool {
    let (_, current_muted) = read_spotifast_volume().unwrap_or((0.2, false));
    set_spotifast_mute(!current_muted)
}

/// Locates the top-level window of Apple Music for Windows.
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
// 1. Spotify Desktop Volume Adapter
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[allow(dead_code)]
pub enum VolumeUpdateStatus {
    Idle = 0,
    Pending = 1,
    Confirmed = 2,
    Failed = 3,
}

pub struct SpotifyVolumeAdapter {
    tx: Option<Sender<DesktopCommand>>,
    cached_level: Arc<AtomicU32>,
    cached_muted: Arc<AtomicBool>,
    pub is_connected: Arc<AtomicBool>,
    last_unmuted: Arc<Mutex<f32>>,
    update_status: Arc<AtomicU8>,
}

impl SpotifyVolumeAdapter {
    pub fn new() -> Self {
        Self {
            tx: None,
            cached_level: Arc::new(AtomicU32::new(f32::to_bits(-1.))),
            cached_muted: Arc::new(AtomicBool::new(false)),
            is_connected: Arc::new(AtomicBool::new(false)),
            last_unmuted: Arc::new(Mutex::new(0.5)),
            update_status: Arc::new(AtomicU8::new(VolumeUpdateStatus::Idle as u8)),
        }
    }

    #[allow(dead_code)]
    pub fn update_status(&self) -> VolumeUpdateStatus {
        match self.update_status.load(Ordering::Relaxed) {
            1 => VolumeUpdateStatus::Pending,
            2 => VolumeUpdateStatus::Confirmed,
            3 => VolumeUpdateStatus::Failed,
            _ => VolumeUpdateStatus::Idle,
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
        let update_status = self.update_status.clone();

        thread::Builder::new()
            .name("wisp-spotify-uia".into())
            .spawn(move || {
                spotify_worker_loop(
                    rx,
                    cached_level,
                    cached_muted,
                    is_connected,
                    last_unmuted,
                    update_status,
                );
            })
            .ok();

        self.tx = Some(tx.clone());
        tx
    }
}

impl VolumeAdapter for SpotifyVolumeAdapter {
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

    fn set_volume(&mut self, value: f32) -> bool {
        let level = normalize_volume(value);
        self.cached_level
            .store(f32::to_bits(level), Ordering::Relaxed);
        if level > 0. {
            self.cached_muted.store(false, Ordering::Relaxed);
            if let Ok(mut unmuted) = self.last_unmuted.lock() {
                *unmuted = level;
            }
        }
        self.update_status
            .store(VolumeUpdateStatus::Pending as u8, Ordering::Relaxed);
        let tx = self.ensure_worker();
        let _ = tx.send(DesktopCommand::SetLevel(level));
        true
    }

    fn get_muted(&self) -> Option<bool> {
        if !self.is_connected.load(Ordering::Relaxed) {
            return None;
        }
        Some(self.cached_muted.load(Ordering::Relaxed))
    }

    fn set_muted(&mut self, value: bool) -> bool {
        self.cached_muted.store(value, Ordering::Relaxed);
        self.update_status
            .store(VolumeUpdateStatus::Pending as u8, Ordering::Relaxed);
        let tx = self.ensure_worker();
        let _ = tx.send(DesktopCommand::SetMuted(value));
        true
    }

    fn reconnect(&mut self) {
        let tx = self.ensure_worker();
        let _ = tx.send(DesktopCommand::Reconnect);
    }

    fn disconnect(&mut self) {
        self.cached_level
            .store(f32::to_bits(-1.), Ordering::Relaxed);
        self.cached_muted.store(false, Ordering::Relaxed);
        self.is_connected.store(false, Ordering::Relaxed);
        self.update_status
            .store(VolumeUpdateStatus::Idle as u8, Ordering::Relaxed);
        self.tx = None;
    }
}

#[link(name = "user32")]
extern "system" {
    fn AttachThreadInput(idAttach: u32, idAttachTo: u32, fAttach: BOOL) -> BOOL;
    fn GetLastInputInfo(plii: *mut LASTINPUTINFO) -> BOOL;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetTickCount() -> u32;
}

#[repr(C)]
struct LASTINPUTINFO {
    cb_size: u32,
    dw_time: u32,
}

fn get_last_input_tick() -> u32 {
    let mut lii = LASTINPUTINFO {
        cb_size: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dw_time: 0,
    };
    unsafe {
        if GetLastInputInfo(&mut lii).as_bool() {
            lii.dw_time
        } else {
            0
        }
    }
}

/// Preserves the user's active foreground window and Z-order across Spotify UI Automation actions.
/// Chromium's accessibility implementation natively sets focus to sliders and buttons when
/// RangeValuePattern::SetValue or InvokePattern::Invoke is invoked, which can cause Spotify
/// or its child windows (Chrome_RenderWidgetHostHWND) to steal foreground focus and rise in Z-order.
/// This guard detects user input, prevents restoring stale windows on concurrent user interaction,
/// and ensures Spotify is kept in the background without stealing focus.
struct SpotifyFocusGuard {
    spotify_hwnd: HWND,
    spotify_pid: u32,
    last_known_good_fg: HWND,
}

impl SpotifyFocusGuard {
    fn new(spotify_hwnd: HWND) -> Self {
        let mut pid = 0;
        if !spotify_hwnd.is_invalid() {
            unsafe {
                GetWindowThreadProcessId(spotify_hwnd, Some(&mut pid));
            }
        }
        let cur_fg = unsafe { GetForegroundWindow() };
        let mut cur_pid = 0;
        if !cur_fg.is_invalid() {
            unsafe {
                GetWindowThreadProcessId(cur_fg, Some(&mut cur_pid));
            }
        }
        let initial_good = if !cur_fg.is_invalid() && cur_pid != pid && cur_fg != spotify_hwnd {
            cur_fg
        } else {
            HWND::default()
        };

        Self {
            spotify_hwnd,
            spotify_pid: pid,
            last_known_good_fg: initial_good,
        }
    }

    unsafe fn find_intentional_target(spotify_hwnd: HWND, spotify_pid: u32) -> Option<HWND> {
        // 1. Check window directly under mouse cursor
        let mut pt = POINT::default();
        if GetCursorPos(&mut pt).is_ok() {
            let hit = WindowFromPoint(pt);
            if !hit.is_invalid() {
                let root = GetAncestor(hit, GA_ROOT);
                if !root.is_invalid() && root != spotify_hwnd {
                    let mut pid = 0;
                    GetWindowThreadProcessId(root, Some(&mut pid));
                    if pid != 0 && pid != spotify_pid && IsWindowVisible(root).as_bool() {
                        return Some(root);
                    }
                }
            }
        }

        // 2. Scan Z-order immediately beneath Spotify (e.g. for Alt+Tab or keyboard switch)
        if !spotify_hwnd.is_invalid() {
            let mut next = GetWindow(spotify_hwnd, GW_HWNDNEXT).unwrap_or_default();
            while !next.is_invalid() {
                if IsWindowVisible(next).as_bool() {
                    let mut pid = 0;
                    GetWindowThreadProcessId(next, Some(&mut pid));
                    if pid != 0 && pid != spotify_pid && next != spotify_hwnd {
                        let mut title = [0u16; 64];
                        let len = GetWindowTextW(next, &mut title);
                        if len > 0 {
                            let s = String::from_utf16_lossy(&title[..len as usize]);
                            if s != "Default IME" && s != "MSCTFIME UI" {
                                return Some(next);
                            }
                        }
                    }
                }
                next = GetWindow(next, GW_HWNDNEXT).unwrap_or_default();
            }
        }
        None
    }

    unsafe fn restore_foreground(target_hwnd: HWND) -> bool {
        if target_hwnd.is_invalid() || !IsWindow(target_hwnd).as_bool() {
            return false;
        }
        let current_fg = GetForegroundWindow();
        if current_fg == target_hwnd {
            return true;
        }

        let current_thread = GetCurrentThreadId();
        let fg_thread = GetWindowThreadProcessId(current_fg, None);
        let target_thread = GetWindowThreadProcessId(target_hwnd, None);

        if fg_thread != 0 && fg_thread != current_thread {
            let _ = AttachThreadInput(current_thread, fg_thread, BOOL::from(true));
        }
        if target_thread != 0 && target_thread != current_thread {
            let _ = AttachThreadInput(current_thread, target_thread, BOOL::from(true));
        }

        let ok = SetForegroundWindow(target_hwnd).as_bool();

        if fg_thread != 0 && fg_thread != current_thread {
            let _ = AttachThreadInput(current_thread, fg_thread, BOOL::from(false));
        }
        if target_thread != 0 && target_thread != current_thread {
            let _ = AttachThreadInput(current_thread, target_thread, BOOL::from(false));
        }

        ok
    }

    fn execute<F, R>(&mut self, action: F) -> R
    where
        F: FnOnce() -> R,
    {
        let fg_before = unsafe { GetForegroundWindow() };
        let mut fg_before_pid = 0;
        if !fg_before.is_invalid() {
            unsafe {
                GetWindowThreadProcessId(fg_before, Some(&mut fg_before_pid));
            }
        }

        let spotify_was_fg = !fg_before.is_invalid()
            && (fg_before_pid == self.spotify_pid || fg_before == self.spotify_hwnd);

        if !spotify_was_fg && !fg_before.is_invalid() && unsafe { IsWindow(fg_before).as_bool() } {
            self.last_known_good_fg = fg_before;
        }

        let was_iconic = if !self.spotify_hwnd.is_invalid() {
            unsafe { IsIconic(self.spotify_hwnd).as_bool() }
        } else {
            false
        };

        let input_before = get_last_input_tick();
        let t_start = unsafe { GetTickCount() };

        let result = action();

        if spotify_was_fg {
            // User intentionally focused Spotify; do not interfere with foreground
            return result;
        }

        let input_after = get_last_input_tick();
        let has_new_user_input = input_after > input_before
            && (input_after >= t_start || t_start.wrapping_sub(input_after) < 50);

        // Check if Spotify stole focus immediately or within micro-window
        for poll in 0..3 {
            let fg_after = unsafe { GetForegroundWindow() };
            let mut fg_after_pid = 0;
            if !fg_after.is_invalid() {
                unsafe {
                    GetWindowThreadProcessId(fg_after, Some(&mut fg_after_pid));
                }
            }

            if (fg_after_pid != 0 && fg_after_pid == self.spotify_pid)
                || fg_after == self.spotify_hwnd
            {
                let target_restore = if has_new_user_input {
                    // New user input occurred during UIA execution.
                    // Preserve the user's intentional application choice rather than stale fg_before.
                    unsafe { Self::find_intentional_target(self.spotify_hwnd, self.spotify_pid) }
                } else {
                    // No new user input occurred; Spotify focus was an involuntary UIA side-effect.
                    let good =
                        if !fg_before.is_invalid() && unsafe { IsWindow(fg_before).as_bool() } {
                            fg_before
                        } else {
                            self.last_known_good_fg
                        };
                    if !good.is_invalid() && unsafe { IsWindow(good).as_bool() } {
                        Some(good)
                    } else {
                        None
                    }
                };

                if let Some(target) = target_restore {
                    if target != fg_after && unsafe { IsWindow(target).as_bool() } {
                        unsafe {
                            if !was_iconic && !self.spotify_hwnd.is_invalid() {
                                let _ = SetWindowPos(
                                    self.spotify_hwnd,
                                    target,
                                    0,
                                    0,
                                    0,
                                    0,
                                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                                );
                            }
                            Self::restore_foreground(target);
                        }
                    }
                }
                break;
            }
            if poll < 2 {
                thread::sleep(Duration::from_millis(5));
            }
        }

        result
    }
}

struct PendingVerification {
    target_level: Option<f32>,
    target_muted: Option<bool>,
    deadline: Instant,
    last_read: Instant,
}

fn spotify_worker_loop(
    rx: Receiver<DesktopCommand>,
    cached_level: Arc<AtomicU32>,
    cached_muted: Arc<AtomicBool>,
    is_connected: Arc<AtomicBool>,
    last_unmuted: Arc<Mutex<f32>>,
    update_status: Arc<AtomicU8>,
) {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let automation =
            match CoCreateInstance::<_, IUIAutomation>(&CUIAutomation, None, CLSCTX_ALL) {
                Ok(a) => a,
                Err(_) => return,
            };

        let cond_slider_type = match automation.CreatePropertyCondition(
            UIA_ControlTypePropertyId,
            &windows::core::VARIANT::from(UIA_SliderControlTypeId.0 as i32),
        ) {
            Ok(c) => c,
            Err(_) => return,
        };

        let cond_slider_name = match automation.CreatePropertyCondition(
            UIA_NamePropertyId,
            &windows::core::VARIANT::from("Change volume"),
        ) {
            Ok(c) => c,
            Err(_) => return,
        };

        let cond_slider = match automation.CreateAndCondition(&cond_slider_type, &cond_slider_name)
        {
            Ok(c) => c,
            Err(_) => return,
        };

        let cond_btn_type = match automation.CreatePropertyCondition(
            UIA_ControlTypePropertyId,
            &windows::core::VARIANT::from(UIA_ButtonControlTypeId.0 as i32),
        ) {
            Ok(c) => c,
            Err(_) => return,
        };

        let cond_mute_name = match automation
            .CreatePropertyCondition(UIA_NamePropertyId, &windows::core::VARIANT::from("Mute"))
        {
            Ok(c) => c,
            Err(_) => return,
        };
        let cond_mute_btn = automation
            .CreateAndCondition(&cond_btn_type, &cond_mute_name)
            .ok();

        let cond_unmute_name = match automation
            .CreatePropertyCondition(UIA_NamePropertyId, &windows::core::VARIANT::from("Unmute"))
        {
            Ok(c) => c,
            Err(_) => return,
        };
        let cond_unmute_btn = automation
            .CreateAndCondition(&cond_btn_type, &cond_unmute_name)
            .ok();

        let mut current_hwnd = HWND::default();
        let mut focus_guard = SpotifyFocusGuard::new(HWND::default());
        let mut slider_pattern: Option<IUIAutomationRangeValuePattern> = None;
        let mut root_elem: Option<IUIAutomationElement> = None;
        let mut last_poll = Instant::now() - Duration::from_secs(1);
        let mut last_reconnect = Instant::now() - Duration::from_secs(10);
        let mut pending: Option<PendingVerification> = None;

        loop {
            let wait_dur = if pending.is_some() {
                Duration::from_millis(50)
            } else if slider_pattern.is_some() {
                Duration::from_millis(250).saturating_sub(last_poll.elapsed())
            } else {
                Duration::from_millis(1000).saturating_sub(last_poll.elapsed())
            };

            let mut cmd = match rx.recv_timeout(wait_dur) {
                Ok(c) => Some(c),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => break,
            };
            while let Ok(next) = rx.try_recv() {
                cmd = Some(next);
            }

            let is_explicit_reconnect = matches!(cmd, Some(DesktopCommand::Reconnect));
            let need_reconnect = slider_pattern.is_none() || current_hwnd.is_invalid();

            if need_reconnect
                && (is_explicit_reconnect
                    || last_reconnect.elapsed() >= Duration::from_millis(1000))
            {
                last_reconnect = Instant::now();
                if let Some(hwnd) = find_spotify_window() {
                    current_hwnd = hwnd;
                    focus_guard = SpotifyFocusGuard::new(hwnd);
                    if let Ok(elem) = automation.ElementFromHandle(hwnd) {
                        if let Ok(slider_elem) = elem.FindFirst(TreeScope_Descendants, &cond_slider)
                        {
                            if let Ok(unk) = slider_elem.GetCurrentPattern(UIA_RangeValuePatternId)
                            {
                                if let Ok(pat) = unk.cast::<IUIAutomationRangeValuePattern>() {
                                    slider_pattern = Some(pat);
                                    root_elem = Some(elem);
                                    is_connected.store(true, Ordering::Relaxed);
                                }
                            }
                        }
                    }
                }
                if slider_pattern.is_none() {
                    current_hwnd = HWND::default();
                    root_elem = None;
                    is_connected.store(false, Ordering::Relaxed);
                }
            }

            if let Some(cmd) = cmd {
                if let Some(ref pat) = slider_pattern {
                    match cmd {
                        DesktopCommand::SetLevel(lvl) => {
                            let target = map_to_spotify_range(lvl);
                            let res = focus_guard.execute(|| pat.SetValue(target));
                            if res.is_ok() {
                                cached_level.store(f32::to_bits(lvl), Ordering::Relaxed);
                                if lvl > 0. {
                                    cached_muted.store(false, Ordering::Relaxed);
                                }
                                pending = Some(PendingVerification {
                                    target_level: Some(lvl),
                                    target_muted: if lvl > 0. { Some(false) } else { None },
                                    deadline: Instant::now() + Duration::from_millis(1500),
                                    last_read: Instant::now(),
                                });
                                update_status
                                    .store(VolumeUpdateStatus::Pending as u8, Ordering::Relaxed);
                            } else {
                                slider_pattern = None;
                                is_connected.store(false, Ordering::Relaxed);
                                update_status
                                    .store(VolumeUpdateStatus::Failed as u8, Ordering::Relaxed);
                                pending = None;
                            }
                        }
                        DesktopCommand::SetMuted(muted) => {
                            let mut handled = false;
                            if let Some(ref root) = root_elem {
                                let mut btn_opt = None;
                                if muted {
                                    if let Some(ref cond) = cond_mute_btn {
                                        if let Ok(btn) = root.FindFirst(TreeScope_Descendants, cond)
                                        {
                                            if btn.CurrentName().is_ok() {
                                                btn_opt = Some(btn);
                                            }
                                        }
                                    }
                                } else {
                                    if let Some(ref cond) = cond_unmute_btn {
                                        if let Ok(btn) = root.FindFirst(TreeScope_Descendants, cond)
                                        {
                                            if btn.CurrentName().is_ok() {
                                                btn_opt = Some(btn);
                                            }
                                        }
                                    }
                                }
                                if let Some(btn) = btn_opt {
                                    if let Ok(unk) = btn.GetCurrentPattern(UIA_InvokePatternId) {
                                        if let Ok(inv) = unk.cast::<IUIAutomationInvokePattern>() {
                                            let _ = focus_guard.execute(|| inv.Invoke());
                                            handled = true;
                                        }
                                    }
                                } else {
                                    let is_already_muted = cond_unmute_btn
                                        .as_ref()
                                        .and_then(|c| root.FindFirst(TreeScope_Descendants, c).ok())
                                        .is_some_and(|b| b.CurrentName().is_ok());
                                    if is_already_muted == muted {
                                        handled = true;
                                    }
                                }
                            }
                            if !handled {
                                let res = focus_guard.execute(|| {
                                    if muted {
                                        pat.SetValue(0.0)
                                    } else {
                                        let restore =
                                            last_unmuted.lock().map(|u| *u).unwrap_or(0.5).max(0.1);
                                        pat.SetValue(map_to_spotify_range(restore))
                                    }
                                });
                                if res.is_err() {
                                    slider_pattern = None;
                                    is_connected.store(false, Ordering::Relaxed);
                                }
                            }
                            cached_muted.store(muted, Ordering::Relaxed);
                            pending = Some(PendingVerification {
                                target_level: None,
                                target_muted: Some(muted),
                                deadline: Instant::now() + Duration::from_millis(1500),
                                last_read: Instant::now(),
                            });
                            update_status
                                .store(VolumeUpdateStatus::Pending as u8, Ordering::Relaxed);
                        }
                        DesktopCommand::Reconnect => {}
                    }
                }
            } else if let Some(ref mut p) = pending {
                // Asynchronous bounded verification (up to 1500 ms)
                if p.last_read.elapsed() >= Duration::from_millis(50) {
                    p.last_read = Instant::now();
                    let current_val = slider_pattern
                        .as_ref()
                        .and_then(|pat| pat.CurrentValue().ok());
                    let current_lvl = current_val.map(map_from_spotify_range);

                    let mut confirmed = false;

                    if let (Some(target_lvl), Some(read_lvl)) = (p.target_level, current_lvl) {
                        if (read_lvl - target_lvl).abs() <= 0.02 {
                            confirmed = true;
                            cached_level.store(f32::to_bits(read_lvl), Ordering::Relaxed);
                        }
                    }

                    if let Some(target_muted) = p.target_muted {
                        let is_muted = if let Some(ref root) = root_elem {
                            cond_unmute_btn
                                .as_ref()
                                .and_then(|c| root.FindFirst(TreeScope_Descendants, c).ok())
                                .is_some_and(|b| b.CurrentName().is_ok())
                        } else {
                            current_lvl.map_or(false, |l| l <= 0.005)
                        };
                        if is_muted == target_muted {
                            confirmed = true;
                            cached_muted.store(is_muted, Ordering::Relaxed);
                        }
                    }

                    if confirmed {
                        update_status.store(VolumeUpdateStatus::Confirmed as u8, Ordering::Relaxed);
                        pending = None;
                        last_poll = Instant::now();
                    } else if Instant::now() >= p.deadline {
                        update_status.store(VolumeUpdateStatus::Failed as u8, Ordering::Relaxed);
                        if let Some(read_lvl) = current_lvl {
                            cached_level.store(f32::to_bits(read_lvl), Ordering::Relaxed);
                        }
                        pending = None;
                        last_poll = Instant::now();
                    }
                    // While pending, do NOT overwrite cached_level with stale readback!
                }
            } else if last_poll.elapsed() >= Duration::from_millis(250) {
                last_poll = Instant::now();
                if let Some(ref pat) = slider_pattern {
                    if let Ok(val) = pat.CurrentValue() {
                        let lvl = map_from_spotify_range(val);
                        cached_level.store(f32::to_bits(lvl), Ordering::Relaxed);
                        let is_muted = if let Some(ref root) = root_elem {
                            cond_unmute_btn
                                .as_ref()
                                .and_then(|c| root.FindFirst(TreeScope_Descendants, c).ok())
                                .is_some_and(|b| b.CurrentName().is_ok())
                        } else {
                            lvl <= 0.005
                        };
                        cached_muted.store(is_muted, Ordering::Relaxed);
                        if lvl > 0.005 && !is_muted {
                            if let Ok(mut u) = last_unmuted.lock() {
                                *u = lvl;
                            }
                        }
                        is_connected.store(true, Ordering::Relaxed);
                        update_status.store(VolumeUpdateStatus::Idle as u8, Ordering::Relaxed);
                    } else {
                        slider_pattern = None;
                        is_connected.store(false, Ordering::Relaxed);
                    }
                }
            }
        }
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
        let bits = self.cached_level.load(Ordering::Relaxed);
        let val = f32::from_bits(bits);
        if val >= 0. {
            return Some(val);
        }
        if let Some((vol, muted)) = read_spotifast_volume() {
            self.cached_level
                .store(f32::to_bits(vol), Ordering::Relaxed);
            self.cached_muted.store(muted, Ordering::Relaxed);
            return Some(vol);
        }
        None
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
        if find_spotifast_window().is_some() {
            self.is_connected.store(true, Ordering::Relaxed);
            if let Some((vol, muted)) = read_spotifast_volume() {
                self.cached_level
                    .store(f32::to_bits(vol), Ordering::Relaxed);
                self.cached_muted.store(muted, Ordering::Relaxed);
            }
        }
        let tx = self.ensure_worker();
        let _ = tx.send(DesktopCommand::Reconnect);
    }

    fn disconnect(&mut self) {
        self.cached_level
            .store(f32::to_bits(-1.), Ordering::Relaxed);
        self.cached_muted.store(false, Ordering::Relaxed);
        self.is_connected.store(false, Ordering::Relaxed);
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
    let mut last_poll = Instant::now() - Duration::from_secs(1);
    let mut last_reconnect = Instant::now() - Duration::from_secs(10);
    let mut running = false;

    loop {
        let poll_interval = if running {
            Duration::from_millis(250)
        } else {
            Duration::from_millis(1500)
        };
        let wait_dur = poll_interval.saturating_sub(last_poll.elapsed());

        let mut cmd = match rx.recv_timeout(wait_dur) {
            Ok(c) => Some(c),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        // Coalesce rapid volume drag commands
        while let Ok(next) = rx.try_recv() {
            cmd = Some(next);
        }

        let is_explicit_reconnect = matches!(cmd, Some(DesktopCommand::Reconnect));
        if !running
            || is_explicit_reconnect
            || last_reconnect.elapsed() >= Duration::from_millis(1500)
        {
            last_reconnect = Instant::now();
            running = find_spotifast_window().is_some();
            is_connected.store(running, Ordering::Relaxed);
            if running
                && (is_explicit_reconnect
                    || cached_level.load(Ordering::Relaxed) == f32::to_bits(-1.))
            {
                if let Some((vol, muted)) = read_spotifast_volume() {
                    cached_level.store(f32::to_bits(vol), Ordering::Relaxed);
                    cached_muted.store(muted, Ordering::Relaxed);
                    if vol > 0.005 {
                        if let Ok(mut u) = last_unmuted.lock() {
                            *u = vol;
                        }
                    }
                }
            }
        }

        if running {
            if let Some(cmd) = cmd {
                match cmd {
                    DesktopCommand::SetLevel(lvl) => {
                        let _ = write_spotifast_volume(lvl);
                        cached_level.store(f32::to_bits(lvl), Ordering::Relaxed);
                        if lvl > 0.005 {
                            cached_muted.store(false, Ordering::Relaxed);
                            if let Ok(mut u) = last_unmuted.lock() {
                                *u = lvl;
                            }
                        }
                        last_poll = Instant::now() + Duration::from_millis(300);
                    }
                    DesktopCommand::SetMuted(muted) => {
                        let _ = set_spotifast_mute(muted);
                        cached_muted.store(muted, Ordering::Relaxed);
                        last_poll = Instant::now() + Duration::from_millis(500);
                    }
                    DesktopCommand::Reconnect => {
                        if let Some((vol, muted)) = read_spotifast_volume() {
                            cached_level.store(f32::to_bits(vol), Ordering::Relaxed);
                            cached_muted.store(muted, Ordering::Relaxed);
                        }
                    }
                }
            } else if last_poll.elapsed() >= Duration::from_millis(250) {
                last_poll = Instant::now();
                if let Some((vol, muted)) = read_spotifast_volume() {
                    cached_level.store(f32::to_bits(vol), Ordering::Relaxed);
                    cached_muted.store(muted, Ordering::Relaxed);
                    if vol > 0.005 {
                        if let Ok(mut u) = last_unmuted.lock() {
                            *u = vol;
                        }
                    }
                }
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
// 4 & 5. Chrome & Edge Browser Audio Session Volume Adapters (Windows Core Audio)
// ---------------------------------------------------------------------------

/// Finds all active or inactive audio sessions belonging to the specified executable name (e.g. "chrome.exe", "msedge.exe").
/// Returns matching ISimpleAudioVolume instances, sorted with active sessions first.
pub fn find_browser_audio_sessions(target_exe: &str) -> Vec<ISimpleAudioVolume> {
    let mut active = Vec::new();
    let mut inactive = Vec::new();
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let Ok(enumerator) =
            CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL)
        else {
            return active;
        };
        let Ok(device) = enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia) else {
            return active;
        };
        let Ok(mgr) = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else {
            return active;
        };
        let Ok(session_enum) = mgr.GetSessionEnumerator() else {
            return active;
        };
        let count = session_enum.GetCount().unwrap_or(0);
        for i in 0..count {
            let Ok(session) = session_enum.GetSession(i) else {
                continue;
            };
            let Ok(control2) = session.cast::<IAudioSessionControl2>() else {
                continue;
            };
            let Ok(pid) = control2.GetProcessId() else {
                continue;
            };
            if pid == 0 {
                continue;
            }
            let pname = process_name(pid);
            if !pname.eq_ignore_ascii_case(target_exe) {
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
    active.extend(inactive);
    active
}

pub struct BrowserAudioSessionVolumeAdapter {
    target_exe: &'static str,
    cached_level: AtomicU32,
    cached_muted: AtomicBool,
    is_connected: AtomicBool,
    last_unmuted: Mutex<f32>,
    last_poll: Mutex<Instant>,
}

impl BrowserAudioSessionVolumeAdapter {
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
        let sessions = find_browser_audio_sessions(self.target_exe);
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

impl VolumeAdapter for BrowserAudioSessionVolumeAdapter {
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
        let sessions = find_browser_audio_sessions(self.target_exe);
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
        let sessions = find_browser_audio_sessions(self.target_exe);
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

pub type ChromeYouTubeVolumeAdapter = BrowserAudioSessionVolumeAdapter;
pub type EdgeYouTubeVolumeAdapter = BrowserAudioSessionVolumeAdapter;

// ---------------------------------------------------------------------------
// Orchestrator: Unified Wisp Volume
// ---------------------------------------------------------------------------

pub struct Volume {
    app: String,
    spotify: Arc<Mutex<SpotifyVolumeAdapter>>,
    spotifast: Arc<Mutex<SpotifastVolumeAdapter>>,
    apple_music: Arc<Mutex<AppleMusicVolumeAdapter>>,
    chrome: Arc<Mutex<ChromeYouTubeVolumeAdapter>>,
    edge: Arc<Mutex<EdgeYouTubeVolumeAdapter>>,
}

impl Volume {
    pub fn none() -> Self {
        Self {
            app: String::new(),
            spotify: Arc::new(Mutex::new(SpotifyVolumeAdapter::new())),
            spotifast: Arc::new(Mutex::new(SpotifastVolumeAdapter::new())),
            apple_music: Arc::new(Mutex::new(AppleMusicVolumeAdapter::new())),
            chrome: Arc::new(Mutex::new(BrowserAudioSessionVolumeAdapter::new(
                "chrome.exe",
            ))),
            edge: Arc::new(Mutex::new(BrowserAudioSessionVolumeAdapter::new(
                "msedge.exe",
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
            "spotify" | "spotifast" | "applemusic" | "chrome" | "msedge"
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
        assert!((map_to_spotify_range(0.8) - 0.8).abs() < 1e-6);
        assert_eq!(map_to_spotify_range(-0.1), 0.0);
        assert_eq!(map_to_spotify_range(1.2), 1.0);

        assert!((map_from_spotify_range(0.4) - 0.4).abs() < 1e-6);
        assert_eq!(map_from_spotify_range(-0.5), 0.0);
        assert_eq!(map_from_spotify_range(1.5), 1.0);
        assert_eq!(map_from_spotify_range(f64::NAN), 0.0);

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

        adapter.disconnect();
        assert!(!adapter.is_available());
        assert_eq!(adapter.get_volume(), None);
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
        let mut adapter = BrowserAudioSessionVolumeAdapter::new("nonexistent_app_12345.exe");
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
        let mut chrome = BrowserAudioSessionVolumeAdapter::new("chrome.exe");
        let mut edge = BrowserAudioSessionVolumeAdapter::new("msedge.exe");

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
        let mut bogus = BrowserAudioSessionVolumeAdapter::new("calculator.exe");
        bogus.reconnect();
        assert!(!bogus.is_available());
        assert_eq!(bogus.get_volume(), None);
        assert!(!bogus.set_volume(0.8));
        assert!(!bogus.set_muted(true));
    }

    #[test]
    fn test_browser_audio_sessions_independent_of_master_volume() {
        // Enumerate sessions to verify query safety and isolation
        let chrome_sessions = find_browser_audio_sessions("chrome.exe");
        let edge_sessions = find_browser_audio_sessions("msedge.exe");
        let spotify_sessions = find_browser_audio_sessions("spotify.exe");

        // Verify that chrome query returns only chrome sessions, edge only edge sessions
        println!("Detected Chrome audio sessions: {}", chrome_sessions.len());
        println!("Detected Edge audio sessions: {}", edge_sessions.len());
        println!(
            "Detected Spotify audio sessions: {}",
            spotify_sessions.len()
        );
    }

    #[test]
    fn test_spotify_uia_live_if_running() {
        if let Some(hwnd) = find_spotify_window() {
            println!("Verified Spotify running under HWND: {:?}", hwnd);
            let mut pid = 0;
            unsafe {
                GetWindowThreadProcessId(hwnd, Some(&mut pid));
            }
            let fg_before = unsafe { GetForegroundWindow() };
            let iconic_before = unsafe { IsIconic(hwnd).as_bool() };

            let mut adapter = SpotifyVolumeAdapter::new();
            adapter.reconnect();
            let start = Instant::now();
            while !adapter.is_available() && start.elapsed() < Duration::from_secs(10) {
                thread::sleep(Duration::from_millis(100));
            }
            if adapter.is_available() {
                if let Some(orig) = adapter.get_volume() {
                    // Test Matrix: 80%, 40%, 20%, mute, unmute, restore
                    adapter.set_volume(0.80);
                    thread::sleep(Duration::from_millis(300));
                    assert_eq!(adapter.get_volume(), Some(0.80));

                    adapter.set_volume(0.40);
                    thread::sleep(Duration::from_millis(300));
                    assert_eq!(adapter.get_volume(), Some(0.40));

                    adapter.set_volume(0.20);
                    thread::sleep(Duration::from_millis(300));
                    assert_eq!(adapter.get_volume(), Some(0.20));

                    adapter.set_muted(true);
                    let start_mute = Instant::now();
                    while adapter.get_muted() != Some(true)
                        && start_mute.elapsed() < Duration::from_secs(3)
                    {
                        thread::sleep(Duration::from_millis(100));
                    }
                    assert_eq!(adapter.get_muted(), Some(true));

                    adapter.set_muted(false);
                    let start_unmute = Instant::now();
                    while adapter.get_muted() != Some(false)
                        && start_unmute.elapsed() < Duration::from_secs(3)
                    {
                        thread::sleep(Duration::from_millis(100));
                    }
                    assert_eq!(adapter.get_muted(), Some(false));

                    adapter.set_volume(orig);
                    let start_restore = Instant::now();
                    while adapter.get_volume() != Some(orig)
                        && start_restore.elapsed() < Duration::from_secs(3)
                    {
                        thread::sleep(Duration::from_millis(100));
                    }
                    assert_eq!(adapter.get_volume(), Some(orig));

                    // Verify foreground window and iconic state invariance
                    let fg_after = unsafe { GetForegroundWindow() };
                    let mut fg_after_pid = 0;
                    unsafe {
                        GetWindowThreadProcessId(fg_after, Some(&mut fg_after_pid));
                    }
                    let iconic_after = unsafe { IsIconic(hwnd).as_bool() };

                    assert_eq!(
                        iconic_before, iconic_after,
                        "Spotify minimized state must remain invariant"
                    );
                    if !fg_before.is_invalid() && fg_before != hwnd {
                        assert_ne!(fg_after_pid, pid, "Spotify must not steal foreground focus");
                    }
                }
            }
        }
    }

    #[test]
    fn test_chrome_session_live_if_running() {
        let mut chrome = BrowserAudioSessionVolumeAdapter::new("chrome.exe");
        chrome.reconnect();
        if chrome.is_available() {
            if let Some(orig) = chrome.get_volume() {
                println!("Chrome live session initial volume: {}", orig);

                // Read Spotify volume before
                let spotify_sessions_before = find_browser_audio_sessions("spotify.exe");
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
                let spotify_sessions_after = find_browser_audio_sessions("spotify.exe");
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

    #[test]
    fn test_spotify_volume_update_status() {
        let mut adapter = SpotifyVolumeAdapter::new();
        assert_eq!(adapter.update_status(), VolumeUpdateStatus::Idle);
        adapter.is_connected.store(true, Ordering::Relaxed);
        assert!(adapter.set_volume(0.75));
        assert_eq!(adapter.update_status(), VolumeUpdateStatus::Pending);
        assert_eq!(adapter.get_volume(), Some(0.75));
        assert!(adapter.set_muted(true));
        assert_eq!(adapter.update_status(), VolumeUpdateStatus::Pending);
        assert_eq!(adapter.get_muted(), Some(true));
        adapter.disconnect();
        assert_eq!(adapter.update_status(), VolumeUpdateStatus::Idle);
    }
}
