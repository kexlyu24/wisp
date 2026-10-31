//! Volume of the music player itself (its Windows audio sessions, as in the Volume Mixer),
//! not the system output level. Used from the UI thread.
use windows::core::{Interface, PWSTR};
use windows::Win32::Foundation::{CloseHandle, BOOL};
use windows::Win32::Media::Audio::{
    eConsole, eRender, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};

pub struct Volume {
    app: String,
    sessions: Vec<ISimpleAudioVolume>,
}

impl Volume {
    pub fn none() -> Self {
        Self { app: String::new(), sessions: Vec::new() }
    }
    pub fn app(&self) -> &str {
        &self.app
    }
    /// Finds the audio sessions on the default output device whose process name contains `app`
    /// (e.g. "spotify" → Spotify.exe, "applemusic" → AppleMusic.exe).
    pub fn refresh(&mut self, app: &str) {
        self.app = app.to_string();
        self.sessions.clear();
        if app.is_empty() {
            return;
        }
        unsafe {
            let Ok(devices) = CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL) else { return };
            let Ok(device) = devices.GetDefaultAudioEndpoint(eRender, eConsole) else { return };
            let Ok(manager) = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else { return };
            let Ok(list) = manager.GetSessionEnumerator() else { return };
            for i in 0..list.GetCount().unwrap_or(0) {
                let Ok(control) = list.GetSession(i) else { continue };
                let Ok(control2) = control.cast::<IAudioSessionControl2>() else { continue };
                let Ok(pid) = control2.GetProcessId() else { continue };
                if pid != 0 && process_name(pid).to_lowercase().contains(app) {
                    if let Ok(volume) = control.cast::<ISimpleAudioVolume>() {
                        self.sessions.push(volume);
                    }
                }
            }
        }
    }
    /// (level 0..1, muted) of the player's first session; None when it has no audio session yet.
    pub fn get(&self) -> Option<(f32, bool)> {
        let session = self.sessions.first()?;
        unsafe { Some((session.GetMasterVolume().ok()?, session.GetMute().ok()?.as_bool())) }
    }
    pub fn set_level(&self, level: f32) {
        for session in &self.sessions {
            unsafe {
                let _ = session.SetMasterVolume(level.clamp(0., 1.), std::ptr::null());
                if level > 0. {
                    let _ = session.SetMute(BOOL::from(false), std::ptr::null());
                }
            }
        }
    }
    pub fn set_muted(&self, muted: bool) {
        for session in &self.sessions {
            unsafe {
                let _ = session.SetMute(BOOL::from(muted), std::ptr::null());
            }
        }
    }
}

/// Executable file name of a process, or empty if it cannot be queried.
fn process_name(pid: u32) -> String {
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, BOOL::from(false), pid) else { return String::new() };
        let mut buf = [0u16; 520];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
        let _ = CloseHandle(handle);
        if !ok {
            return String::new();
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit('\\').next().unwrap_or_default().to_string()
    }
}
