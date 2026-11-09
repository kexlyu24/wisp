//! User settings, changed from the tray menu and kept in `%APPDATA%\Wisp\settings.txt`.
//! "Start with Windows" is not stored there: it is the `Wisp` value under the current user's
//! `Run` key, so it stays true to what Windows will actually do.
use crate::wide;
use std::ffi::c_void;
use std::ptr::null_mut;
type Handle = *mut c_void;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Settings {
    /// Show Windows notifications in the notch.
    pub notifications: bool,
    /// Open the panel when the cursor reaches the notch (otherwise only on click).
    pub open_on_hover: bool,
    /// Show the sung lyric line in the collapsed notch while lyrics are on.
    pub collapsed_lyrics: bool,
    /// 14:25 rather than 2:25.
    pub clock_24h: bool,
    /// Skip animations when Windows' "Animation effects" is off. Off by default: many PCs turn that
    /// off for speed, and Wisp's springs are the point.
    pub follow_windows_animations: bool,
    /// Shrink the closed notch to a sliver while it would cover another window's content.
    pub minimal_when_covering: bool,
    /// Pomodoro lengths in minutes: focus, short break, long break.
    pub pomo_minutes: [u32; 3],
    /// Background style: 0 = Frost (blurred translucent), 1 = Aura (ambient tint), 2 = Slate (charcoal), 3 = OLED (solid black), 4 = Image (custom image).
    pub bg_style: u32,
    /// Background opacity percentage: 50..=100 (default 80).
    pub bg_opacity: u32,
    /// Background blur level: 0 = None, 1..=30 = blur radius in pixels (default 16).
    pub bg_blur: u32,
    /// Background image framing position: 0 = Center, 1 = Top, 2 = Bottom.
    pub bg_pos: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            notifications: true,
            open_on_hover: true,
            collapsed_lyrics: true,
            clock_24h: true,
            follow_windows_animations: false,
            minimal_when_covering: true,
            pomo_minutes: [25, 5, 15],
            bg_style: 3,
            bg_opacity: 100,
            bg_blur: 16,
            bg_pos: 0,
        }
    }
}

const KEYS: [&str; 6] = [
    "notifications",
    "open_on_hover",
    "collapsed_lyrics",
    "clock_24h",
    "follow_windows_animations",
    "minimal_when_covering",
];

impl Settings {
    fn fields(&mut self) -> [&mut bool; 6] {
        [
            &mut self.notifications,
            &mut self.open_on_hover,
            &mut self.collapsed_lyrics,
            &mut self.clock_24h,
            &mut self.follow_windows_animations,
            &mut self.minimal_when_covering,
        ]
    }
    /// `key=0|1` lines; unknown keys and bad lines are ignored, missing keys keep their default.
    pub fn parse(src: &str) -> Self {
        let mut settings = Self::default();
        for line in src.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            if let Some(i) = ["focus_minutes", "short_minutes", "long_minutes"]
                .iter()
                .position(|k| *k == key.trim())
            {
                if let Ok(m) = value.trim().parse::<u32>() {
                    settings.pomo_minutes[i] = m;
                }
                continue;
            }
            if key.trim() == "player" {
                // Obsolete manual player setting ignored safely on migration
                continue;
            }
            if key.trim() == "bg_style" {
                if let Ok(v) = value.trim().parse::<u32>() {
                    if v <= 4 {
                        settings.bg_style = v;
                    }
                }
                continue;
            }
            if key.trim() == "bg_opacity" {
                if let Ok(v) = value.trim().parse::<u32>() {
                    if (50..=100).contains(&v) {
                        settings.bg_opacity = v;
                    }
                }
                continue;
            }
            if key.trim() == "bg_blur" {
                if let Ok(v) = value.trim().parse::<u32>() {
                    if v <= 30 {
                        settings.bg_blur = v;
                    }
                }
                continue;
            }
            if key.trim() == "bg_pos" {
                if let Ok(v) = value.trim().parse::<u32>() {
                    if v <= 2 {
                        settings.bg_pos = v;
                    }
                }
                continue;
            }
            if let Some(i) = KEYS.iter().position(|k| *k == key.trim()) {
                match value.trim() {
                    "1" => *settings.fields()[i] = true,
                    "0" => *settings.fields()[i] = false,
                    _ => {}
                }
            }
        }
        settings
    }
    pub fn format(mut self) -> String {
        let values: Vec<bool> = self.fields().iter().map(|v| **v).collect();
        KEYS.iter()
            .zip(values)
            .map(|(k, v)| format!("{k}={}\n", v as u8))
            .collect::<String>()
            + &format!(
                "focus_minutes={}\nshort_minutes={}\nlong_minutes={}\nbg_style={}\nbg_opacity={}\nbg_blur={}\nbg_pos={}\n",
                self.pomo_minutes[0],
                self.pomo_minutes[1],
                self.pomo_minutes[2],
                self.bg_style,
                self.bg_opacity,
                self.bg_blur,
                self.bg_pos
            )
    }
    pub fn load() -> Self {
        crate::tasks::data_path("settings.txt")
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|src| Self::parse(&src))
            .unwrap_or_default()
    }
    pub fn save(self) {
        crate::tasks::write_data("settings.txt", &self.format());
    }
}

#[link(name = "advapi32")]
extern "system" {
    fn RegGetValueW(
        key: Handle,
        subkey: *const u16,
        value: *const u16,
        flags: u32,
        kind: *mut u32,
        data: *mut c_void,
        size: *mut u32,
    ) -> i32;
    fn RegSetKeyValueW(
        key: Handle,
        subkey: *const u16,
        value: *const u16,
        kind: u32,
        data: *const c_void,
        size: u32,
    ) -> i32;
    fn RegDeleteKeyValueW(key: Handle, subkey: *const u16, value: *const u16) -> i32;
}
const RUN: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
fn current_user() -> Handle {
    (0x80000001u32 as i32) as isize as Handle
}

/// The command line Windows runs at sign-in: this executable, quoted.
fn launch_command() -> Option<String> {
    Some(format!("\"{}\"", std::env::current_exe().ok()?.display()))
}

/// True when the Run value exists and points at this executable.
pub fn starts_with_windows() -> bool {
    let mut buffer = [0u16; 1024];
    let mut size = (buffer.len() * 2) as u32;
    let found = unsafe {
        RegGetValueW(
            current_user(),
            wide(RUN).as_ptr(),
            wide("Wisp").as_ptr(),
            0x2,
            null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut size,
        ) == 0
    };
    if !found {
        return false;
    }
    let len = (size as usize / 2).saturating_sub(1).min(buffer.len());
    let value = String::from_utf16_lossy(&buffer[..len]);
    launch_command().is_some_and(|cmd| value.eq_ignore_ascii_case(&cmd))
}

pub fn set_start_with_windows(on: bool) {
    unsafe {
        if on {
            let Some(cmd) = launch_command() else { return };
            let data = wide(&cmd);
            RegSetKeyValueW(
                current_user(),
                wide(RUN).as_ptr(),
                wide("Wisp").as_ptr(),
                1,
                data.as_ptr().cast(),
                (data.len() * 2) as u32,
            );
        } else {
            RegDeleteKeyValueW(current_user(), wide(RUN).as_ptr(), wide("Wisp").as_ptr());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_defaults() {
        let custom = Settings {
            notifications: false,
            open_on_hover: true,
            collapsed_lyrics: false,
            clock_24h: false,
            follow_windows_animations: true,
            minimal_when_covering: false,
            pomo_minutes: [50, 10, 20],
            bg_style: 1,
            bg_opacity: 70,
            bg_blur: 20,
            bg_pos: 1,
        };
        assert_eq!(Settings::parse(&custom.format()), custom);
        assert_eq!(Settings::parse(""), Settings::default());
        let partial = Settings::parse(
            "open_on_hover=0\nbogus=1\nclock_24h=maybe\nbg_style=2\nbg_opacity=90\nbg_blur=12\nbg_pos=2\n",
        );
        assert!(!partial.open_on_hover && partial.notifications && partial.clock_24h);
        assert!(!partial.follow_windows_animations);
        assert_eq!(partial.bg_style, 2);
        assert_eq!(partial.bg_opacity, 90);
        assert_eq!(partial.bg_blur, 12);
        assert_eq!(partial.bg_pos, 2);
        let img_style = Settings::parse("bg_style=4\n");
        assert_eq!(img_style.bg_style, 4);
        // Verify obsolete player setting from earlier versions is ignored safely
        let migrated = Settings::parse("player=spotifast\nnotifications=0\n");
        assert!(!migrated.notifications);
        assert!(!custom.format().contains("player="));
    }

    #[test]
    fn settings_all_seven_toggle_defaults_and_parse_isolation() {
        let def = Settings::default();
        assert!(def.notifications);
        assert!(def.open_on_hover);
        assert!(def.collapsed_lyrics);
        assert!(def.clock_24h);
        assert!(!def.follow_windows_animations);
        assert!(def.minimal_when_covering);

        for (key, inverted_line) in [
            ("notifications", "notifications=0\n"),
            ("open_on_hover", "open_on_hover=0\n"),
            ("collapsed_lyrics", "collapsed_lyrics=0\n"),
            ("clock_24h", "clock_24h=0\n"),
            ("follow_windows_animations", "follow_windows_animations=1\n"),
            ("minimal_when_covering", "minimal_when_covering=0\n"),
        ] {
            let s = Settings::parse(inverted_line);
            match key {
                "notifications" => assert!(!s.notifications),
                "open_on_hover" => assert!(!s.open_on_hover),
                "collapsed_lyrics" => assert!(!s.collapsed_lyrics),
                "clock_24h" => assert!(!s.clock_24h),
                "follow_windows_animations" => assert!(s.follow_windows_animations),
                "minimal_when_covering" => assert!(!s.minimal_when_covering),
                _ => unreachable!(),
            }
        }
    }
}
