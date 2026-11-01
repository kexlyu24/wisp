//! Volume of the music player as its own volume slider shows it, not the Volume Mixer and not the
//! system output level. Spotifast is driven through its `spotifast volume|mute|now-playing`
//! commands, which keep working while its window is minimized (its UI is not updated then);
//! Spotify and Apple Music through their volume slider and mute button via UI Automation.
//! Everything runs on a worker thread; the UI thread only reads the latest snapshot.
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use windows::core::{PWSTR, VARIANT};
use windows::Win32::Foundation::{CloseHandle, BOOL, HWND, LPARAM};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationInvokePattern, IUIAutomationRangeValuePattern, TreeScope_Descendants,
    UIA_ButtonControlTypeId, UIA_ControlTypePropertyId, UIA_InvokePatternId, UIA_RangeValuePatternId, UIA_SliderControlTypeId,
};
use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowTextLengthW, GetWindowThreadProcessId, IsWindowVisible};

/// `CREATE_NO_WINDOW`: the player's command-line verbs must not flash a console.
const NO_WINDOW: u32 = 0x0800_0000;
/// The UI asks every few ticks; the player is asked at most this often.
const POLL_EVERY: Duration = Duration::from_millis(900);
/// After a change, polls are held back so a reading taken before it lands can't undo the slider.
const SETTLE: Duration = Duration::from_millis(1200);
/// A player whose slider wasn't found is searched again at most this often.
const RESCAN_EVERY: Duration = Duration::from_secs(2);

enum Cmd {
    Follow(String),
    Poll,
    Set(f32),
    Mute(bool),
}

pub struct Volume {
    app: String,
    tx: Option<Sender<Cmd>>,
    /// (level 0..1, muted) as last read from or sent to the player; None when it can't be controlled.
    state: Arc<Mutex<Option<(f32, bool)>>>,
}

impl Volume {
    /// No worker: never reports a volume (smoke mode).
    pub fn none() -> Self {
        Self { app: String::new(), tx: None, state: Arc::new(Mutex::new(None)) }
    }
    pub fn start() -> Self {
        let (tx, rx) = mpsc::channel();
        let state = Arc::new(Mutex::new(None));
        let shared = state.clone();
        thread::spawn(move || worker(rx, shared));
        Self { app: String::new(), tx: Some(tx), state }
    }
    /// Follows `app` (a `media::app_key`) and asks for a fresh reading; cheap to call often.
    pub fn refresh(&mut self, app: &str) {
        if app != self.app {
            self.app = app.to_string();
            *self.state.lock().unwrap() = None;
            self.send(Cmd::Follow(app.to_string()));
        }
        self.send(Cmd::Poll);
    }
    pub fn get(&self) -> Option<(f32, bool)> {
        *self.state.lock().unwrap()
    }
    pub fn set_level(&self, level: f32) {
        self.send(Cmd::Set(level.clamp(0., 1.)));
    }
    pub fn set_muted(&self, muted: bool) {
        self.send(Cmd::Mute(muted));
    }
    fn send(&self, cmd: Cmd) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(cmd);
        }
    }
}

fn worker(rx: Receiver<Cmd>, state: Arc<Mutex<Option<(f32, bool)>>>) {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    let mut player = Player::None;
    let (mut app, mut searched) = (String::new(), Instant::now());
    let mut polled: Option<Instant> = None;
    let mut changed: Option<Instant> = None;
    while let Ok(first) = rx.recv() {
        let mut batch = vec![first];
        batch.extend(rx.try_iter());
        let mut poll = false;
        for (i, cmd) in batch.iter().enumerate() {
            match cmd {
                Cmd::Follow(followed) => {
                    app = followed.clone();
                    player = Player::find(&app);
                    searched = Instant::now();
                    polled = None;
                    changed = None;
                }
                Cmd::Poll => poll = true,
                // A drag queues many levels; only the newest one matters.
                Cmd::Set(_) if batch[i + 1..].iter().any(|c| matches!(c, Cmd::Set(_) | Cmd::Follow(_))) => {}
                Cmd::Set(level) => {
                    player.set_level(*level);
                    *state.lock().unwrap() = Some((*level, false));
                    changed = Some(Instant::now());
                }
                Cmd::Mute(muted) => {
                    player.set_muted(*muted);
                    let mut shown = state.lock().unwrap();
                    if let Some((level, _)) = *shown {
                        *shown = Some((level, *muted));
                    }
                    changed = Some(Instant::now());
                }
            }
        }
        let due = polled.map_or(true, |t| t.elapsed() >= POLL_EVERY);
        let settled = changed.map_or(true, |t| t.elapsed() >= SETTLE);
        if poll && due && settled {
            // The player's window may not have existed yet when it was first followed.
            if matches!(player, Player::None) && !app.is_empty() && searched.elapsed() >= RESCAN_EVERY {
                player = Player::find(&app);
                searched = Instant::now();
            }
            polled = Some(Instant::now());
            *state.lock().unwrap() = player.get();
        }
    }
}

enum Player {
    None,
    Spotifast(Cli),
    Ui(Ui),
}

impl Player {
    fn find(app: &str) -> Self {
        match app {
            "" => Player::None,
            "spotifast" => player_windows(app)
                .first()
                .map_or(Player::None, |&(_, pid)| Player::Spotifast(Cli { exe: process_path(pid).into(), level: 0, last_level: 0 })),
            _ => Player::Ui(Ui::new(app)),
        }
    }
    fn get(&mut self) -> Option<(f32, bool)> {
        match self {
            Player::None => None,
            Player::Spotifast(cli) => cli.get(),
            Player::Ui(ui) => ui.get(),
        }
    }
    fn set_level(&mut self, level: f32) {
        match self {
            Player::None => {}
            Player::Spotifast(cli) => cli.set_level(level),
            Player::Ui(ui) => ui.set_level(level),
        }
    }
    fn set_muted(&mut self, muted: bool) {
        match self {
            Player::None => {}
            Player::Spotifast(cli) => cli.set_muted(muted),
            Player::Ui(ui) => ui.set_muted(muted),
        }
    }
}

/// Spotifast's own command line, which talks to the running copy over its private local channel.
struct Cli {
    exe: PathBuf,
    /// Volume in percent as last seen; Spotifast reports 0 while muted.
    level: u32,
    /// The last non-zero volume, restored on unmute.
    last_level: u32,
}

impl Cli {
    fn run(&self, args: &[&str]) -> Option<String> {
        let out = Command::new(&self.exe).args(args).creation_flags(NO_WINDOW).output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    }
    fn get(&mut self) -> Option<(f32, bool)> {
        self.level = parse_now_playing_volume(&self.run(&["now-playing", "--raw"])?)?;
        if self.level > 0 {
            self.last_level = self.level;
        }
        Some(match (self.level, self.last_level) {
            (0, 0) => (0., true),
            (0, last) => (last as f32 / 100., true),
            (level, _) => (level as f32 / 100., false),
        })
    }
    fn set_level(&mut self, level: f32) {
        let percent = (level * 100.).round() as u32;
        // Setting a volume also unmutes Spotifast.
        self.run(&["volume", &percent.to_string()]);
        self.level = percent;
        if percent > 0 {
            self.last_level = percent;
        }
    }
    fn set_muted(&mut self, muted: bool) {
        if muted && self.level > 0 {
            // `mute` toggles, and Spotifast's own unmute then restores the level.
            self.run(&["mute"]);
            self.level = 0;
        } else if !muted && self.level == 0 {
            let level = if self.last_level > 0 { self.last_level } else { 50 };
            self.run(&["volume", &level.to_string()]);
            self.level = level;
        }
    }
}

/// The volume column of `now-playing --raw`: tab-separated state, title, artist, album,
/// position ms, duration ms, volume %, …
fn parse_now_playing_volume(raw: &str) -> Option<u32> {
    raw.lines().next()?.split('\t').nth(6)?.trim().parse().ok()
}

/// The player's volume slider and mute button, found through UI Automation.
struct Ui {
    app: String,
    automation: Option<IUIAutomation>,
    slider: Option<IUIAutomationRangeValuePattern>,
    mute: Option<IUIAutomationElement>,
    searched: Option<Instant>,
    /// The last non-zero level, restored on unmute when there is no mute button.
    last_level: f32,
}

impl Ui {
    fn new(app: &str) -> Self {
        let automation = unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok() };
        Self { app: app.to_string(), automation, slider: None, mute: None, searched: None, last_level: 0.5 }
    }
    /// Finds the controls when they are missing, at most every `RESCAN_EVERY`.
    fn ensure(&mut self) -> Option<&IUIAutomationRangeValuePattern> {
        if self.slider.is_none() && self.searched.map_or(true, |t| t.elapsed() >= RESCAN_EVERY) {
            self.searched = Some(Instant::now());
            if let Some(automation) = &self.automation {
                // Visible windows first; Spotify also keeps hidden helper windows.
                for (hwnd, _) in player_windows(&self.app) {
                    if let Some((slider, mute)) = unsafe { find_controls(automation, hwnd) } {
                        self.slider = Some(slider);
                        self.mute = mute;
                        break;
                    }
                }
            }
        }
        self.slider.as_ref()
    }
    /// (value, minimum, maximum); a failure means the window went away, so search again later.
    fn range(&mut self) -> Option<(f64, f64, f64)> {
        let slider = self.ensure()?;
        let read = unsafe { (slider.CurrentValue(), slider.CurrentMinimum(), slider.CurrentMaximum()) };
        match read {
            (Ok(v), Ok(min), Ok(max)) if max > min => Some((v, min, max)),
            _ => {
                self.slider = None;
                self.mute = None;
                None
            }
        }
    }
    fn muted_button(&self) -> Option<bool> {
        let name = unsafe { self.mute.as_ref()?.CurrentName().ok()? }.to_string().to_lowercase();
        Some(name.starts_with("unmute"))
    }
    fn get(&mut self) -> Option<(f32, bool)> {
        let (value, min, max) = self.range()?;
        let level = ((value - min) / (max - min)).clamp(0., 1.) as f32;
        if level > 0. {
            self.last_level = level;
        }
        let muted = self.muted_button().unwrap_or(level == 0.);
        // A muted player usually drops its slider to zero; show the level it will return to.
        Some((if muted && level == 0. { self.last_level } else { level }, muted))
    }
    fn set_level(&mut self, level: f32) {
        if level > 0. && self.muted_button() == Some(true) {
            self.press_mute();
        }
        let Some((_, min, max)) = self.range() else { return };
        if let Some(slider) = &self.slider {
            unsafe {
                let _ = slider.SetValue(min + level as f64 * (max - min));
            }
        }
        if level > 0. {
            self.last_level = level;
        }
    }
    fn set_muted(&mut self, muted: bool) {
        match self.muted_button() {
            Some(now) if now != muted => self.press_mute(),
            Some(_) => {}
            None if muted => {
                if let Some((value, min, max)) = self.range() {
                    if value > min {
                        self.last_level = ((value - min) / (max - min)) as f32;
                    }
                }
                self.set_level(0.);
            }
            None => self.set_level(self.last_level),
        }
    }
    fn press_mute(&self) {
        unsafe {
            if let Some(Ok(invoke)) = self.mute.as_ref().map(|b| b.GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId)) {
                let _ = invoke.Invoke();
            }
        }
    }
}

/// The first slider named like "Volume" in the window, and a "Mute"/"Unmute" button if there is one.
unsafe fn find_controls(automation: &IUIAutomation, hwnd: HWND) -> Option<(IUIAutomationRangeValuePattern, Option<IUIAutomationElement>)> {
    let root = automation.ElementFromHandle(hwnd).ok()?;
    let named = |kind: i32, matches: &dyn Fn(&str) -> bool| -> Option<IUIAutomationElement> {
        let condition = automation.CreatePropertyCondition(UIA_ControlTypePropertyId, &VARIANT::from(kind)).ok()?;
        let found = root.FindAll(TreeScope_Descendants, &condition).ok()?;
        (0..found.Length().ok()?).filter_map(|i| found.GetElement(i).ok()).find(|e| {
            e.CurrentName().map_or(false, |n| matches(&n.to_string().to_lowercase()))
        })
    };
    let slider = named(UIA_SliderControlTypeId.0, &|n| n.contains("volume"))?;
    let range = slider.GetCurrentPatternAs::<IUIAutomationRangeValuePattern>(UIA_RangeValuePatternId).ok()?;
    let mute = named(UIA_ButtonControlTypeId.0, &|n| n.starts_with("mute") || n.starts_with("unmute"));
    Some((range, mute))
}

/// Top-level windows with a title (minimized and hidden ones too) of processes whose executable
/// name contains `app`, visible ones first, with their process ids.
fn player_windows(app: &str) -> Vec<(HWND, u32)> {
    struct Search {
        app: String,
        found: Vec<(HWND, u32, bool)>,
    }
    unsafe extern "system" fn each(hwnd: HWND, data: LPARAM) -> BOOL {
        let search = &mut *(data.0 as *mut Search);
        if GetWindowTextLengthW(hwnd) > 0 {
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid != 0 && process_name(pid).to_lowercase().contains(&search.app) {
                search.found.push((hwnd, pid, IsWindowVisible(hwnd).as_bool()));
            }
        }
        BOOL::from(true)
    }
    let mut search = Search { app: app.to_lowercase(), found: Vec::new() };
    unsafe {
        let _ = EnumWindows(Some(each), LPARAM(&mut search as *mut Search as isize));
    }
    search.found.sort_by_key(|&(_, _, visible)| !visible);
    search.found.into_iter().map(|(hwnd, pid, _)| (hwnd, pid)).collect()
}

/// Full path of a process's executable, or empty if it cannot be queried.
fn process_path(pid: u32) -> String {
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, BOOL::from(false), pid) else { return String::new() };
        let mut buf = [0u16; 520];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
        let _ = CloseHandle(handle);
        if ok { String::from_utf16_lossy(&buf[..len as usize]) } else { String::new() }
    }
}

/// Executable file name of a process, or empty if it cannot be queried.
fn process_name(pid: u32) -> String {
    process_path(pid).rsplit('\\').next().unwrap_or_default().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_volume_column() {
        let raw = "playing\tSweet Fantasy\tMagnolia Celebration\tFantasia\t93208\t396000\t73\toff\toff\thttps://x\tno\tSpotifast\n";
        assert_eq!(parse_now_playing_volume(raw), Some(73));
        assert_eq!(parse_now_playing_volume("paused\ta\tb\tc\t0\t0\t0\toff\toff\t\tno\tSpotifast"), Some(0));
        assert_eq!(parse_now_playing_volume("stopped"), None);
        assert_eq!(parse_now_playing_volume(""), None);
    }

    /// Reads the running player's volume and writes the same value back:
    /// `cargo test live_player_volume -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_player_volume() {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        for app in ["spotifast", "spotify", "applemusic"] {
            let mut player = Player::find(app);
            let Some((level, muted)) = player.get() else {
                println!("{app}: not found");
                continue;
            };
            println!("{app}: level {level:.2} muted {muted}");
            if !muted {
                player.set_level(level);
                let again = player.get().map(|(l, _)| l);
                println!("{app}: after writing it back {again:?}");
                assert!(again.is_some_and(|l| (l - level).abs() < 0.02));
            }
        }
    }
}
