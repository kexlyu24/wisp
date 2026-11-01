use std::{
    sync::{
        mpsc::{self, Receiver},
        Mutex,
    },
    thread,
    time::Duration,
};
use windows::{Media::Control::*, Storage::Streams::DataReader};

#[derive(Default)]
pub struct Media {
    pub title: String,
    pub album: String,
    pub artist: String,
    pub playing: bool,
    pub artwork: Option<Vec<u32>>,
    /// Position in seconds at the moment of sampling, extrapolated from the session's last update.
    pub pos_secs: f64,
    pub duration: i64,
    /// Which player the session belongs to: "spotify", "applemusic" or "spotifast" (empty when idle).
    pub app: String,
}

pub const IDLE_TITLE: &str = "Not playing";
pub const IDLE_ARTIST: &str = "Play music on Spotify, Apple Music or Spotifast";

/// Supported players, matched against the session's app id (and, for volume, the process name).
pub fn app_key(id: &str) -> Option<&'static str> {
    let id = id.to_lowercase();
    if id.contains("spotifast") {
        Some("spotifast")
    } else if id.contains("spotify") {
        Some("spotify")
    } else if id.contains("applemusic") {
        Some("applemusic")
    } else if id.contains("chrome") {
        Some("chrome")
    } else if id.contains("msedge") || id.contains("microsoft.edge") || id.contains("edge.exe") {
        Some("msedge")
    } else {
        None
    }
}

fn default_media() -> Media {
    Media {
        title: IDLE_TITLE.into(),
        artist: IDLE_ARTIST.into(),
        ..Default::default()
    }
}

/// Current UTC time as a Windows DateTime (100 ns ticks since 1601).
fn filetime_now() -> i64 {
    let unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    (unix.as_nanos() / 100) as i64 + 116_444_736_000_000_000
}

/// App id of the session on screen, so the transport buttons control that same player.
static FOLLOWED: Mutex<String> = Mutex::new(String::new());

/// Helper to classify desktop players vs browser sessions.
fn is_desktop(c: &Candidate) -> bool {
    app_key(&c.app_id).map_or(false, |k| {
        k == "spotify" || k == "spotifast" || k == "applemusic"
    })
}

/// A media session as seen when choosing which one to show.
pub struct Candidate {
    pub app_id: String,
    pub playing: bool,
    /// Windows' current session: the one its media overlay shows, i.e. the one used last.
    pub current: bool,
}

/// Which session to show when several players are open.
/// Deterministic automatic selection:
/// 1. Prefer a valid media source actively playing audio.
/// 2. Prefer the most recently started/activated playback source (current && playing).
/// 3. Preserve the current selection while it remains valid and actively playing (followed && playing).
/// 4. Switch automatically when the selected source stops and another supported source becomes active.
/// 5. When idle/paused, prefer desktop players and current session over inactive browser tabs.
pub fn choose(candidates: &[Candidate], followed: &str, only: &str) -> Option<usize> {
    let ok = |c: &Candidate| app_key(&c.app_id).is_some_and(|k| only.is_empty() || k == only);
    let find = |pred: &dyn Fn(&Candidate) -> bool| candidates.iter().position(|c| ok(c) && pred(c));
    let is_followed = |c: &Candidate| !followed.is_empty() && c.app_id == followed;

    // 1 & 2: Actively playing and currently active session
    find(&|c| c.current && c.playing)
        // 3: Actively playing and previously followed session (prevents ping-pong)
        .or_else(|| find(&|c| is_followed(c) && c.playing))
        // 4: Any actively playing desktop player, then any actively playing browser
        .or_else(|| find(&|c| is_desktop(c) && c.playing))
        .or_else(|| find(&|c| c.playing))
        // Paused/idle: prefer active desktop session, then followed desktop, then any current
        .or_else(|| find(&|c| is_desktop(c) && c.current))
        .or_else(|| find(&|c| is_desktop(c) && is_followed(c)))
        .or_else(|| find(&|c| is_desktop(c)))
        .or_else(|| find(&|c| c.current))
        .or_else(|| find(&|c| is_followed(c)))
        .or_else(|| find(&|_| true))
}

pub fn get_media_session(
    manager: &GlobalSystemMediaTransportControlsSessionManager,
) -> windows::core::Result<Option<GlobalSystemMediaTransportControlsSession>> {
    let current = manager
        .GetCurrentSession()
        .ok()
        .and_then(|s| s.SourceAppUserModelId().ok())
        .map(|id| id.to_string());
    let sessions: Vec<GlobalSystemMediaTransportControlsSession> =
        manager.GetSessions()?.into_iter().collect();
    let candidates: Vec<Candidate> = sessions
        .iter()
        .map(|session| {
            let app_id = session
                .SourceAppUserModelId()
                .map(|id| id.to_string())
                .unwrap_or_default();
            let playing = session
                .GetPlaybackInfo()
                .ok()
                .and_then(|info| info.PlaybackStatus().ok())
                .map(|s| s == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing)
                .unwrap_or(false);
            Candidate {
                current: current.as_deref() == Some(app_id.as_str()),
                app_id,
                playing,
            }
        })
        .collect();
    let followed = FOLLOWED.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let Some(index) = choose(&candidates, &followed, "") else {
        FOLLOWED.lock().unwrap_or_else(|e| e.into_inner()).clear();
        return Ok(None);
    };
    *FOLLOWED.lock().unwrap_or_else(|e| e.into_inner()) = candidates[index].app_id.clone();
    Ok(sessions.into_iter().nth(index))
}

/// The session currently on screen, for the transport buttons and the scrubber.
fn followed_session(
    manager: &GlobalSystemMediaTransportControlsSessionManager,
) -> windows::core::Result<Option<GlobalSystemMediaTransportControlsSession>> {
    let followed = FOLLOWED.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if !followed.is_empty() {
        for session in manager.GetSessions()? {
            if session
                .SourceAppUserModelId()
                .map(|id| id.to_string())
                .ok()
                .as_deref()
                == Some(followed.as_str())
            {
                return Ok(Some(session));
            }
        }
    }
    get_media_session(manager)
}

/// Spotify's thumbnail pads the cover onto a canvas with its logo underneath.
/// Keep the first block of non-background rows when that block is roughly square.
fn cover_only(img: image::DynamicImage) -> image::DynamicImage {
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    if w < 16 || h < 16 {
        return img;
    }
    let bg = *rgb.get_pixel(0, h - 1);
    let differs = |x: u32, y: u32| {
        let p = rgb.get_pixel(x, y);
        (0..3)
            .map(|i| (p[i] as i32 - bg[i] as i32).abs())
            .max()
            .unwrap_or(0)
            > 28
    };
    let row_has = |y: u32| (0..w).step_by(2).any(|x| differs(x, y));
    let Some(top) = (0..h).find(|&y| row_has(y)) else {
        return img;
    };
    let bottom = (top..h).find(|&y| !row_has(y)).unwrap_or(h);
    if bottom == h && top == 0 {
        return img;
    }
    let cols: Vec<u32> = (0..w)
        .filter(|&x| (top..bottom).step_by(2).any(|y| differs(x, y)))
        .collect();
    let (left, right) = match (cols.first(), cols.last()) {
        (Some(&l), Some(&r)) => (l, r + 1),
        _ => return img,
    };
    let (cw, ch) = (right - left, bottom - top);
    if cw < w / 3 || (cw as i32 - ch as i32).abs() > (cw.max(ch) / 10) as i32 {
        return img;
    }
    img.crop_imm(left, top, cw, ch)
}

pub fn start() -> Receiver<Media> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut last = String::new();
        let mut manager: Option<GlobalSystemMediaTransportControlsSessionManager> = None;
        loop {
            // Reuse one session manager: requesting a new one every poll leaks a little memory each time.
            if manager.is_none() {
                manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
                    .and_then(|op| op.get())
                    .ok();
            }
            let result = (|| -> windows::core::Result<Media> {
                let Some(manager) = &manager else {
                    return Ok(default_media());
                };
                let session = match get_media_session(manager)? {
                    Some(s) => s,
                    None => return Ok(default_media()),
                };
                let props = session.TryGetMediaPropertiesAsync()?.get()?;
                let playing = session
                    .GetPlaybackInfo()
                    .ok()
                    .and_then(|info| info.PlaybackStatus().ok())
                    .map(|s| s == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing)
                    .unwrap_or(false);
                let (pos_secs, duration) = if let Ok(timeline) = session.GetTimelineProperties() {
                    let mut pos = timeline
                        .Position()
                        .map(|d| d.Duration as f64 / 1e7)
                        .unwrap_or(0.);
                    let dur = timeline
                        .EndTime()
                        .map(|d| d.Duration / 10_000_000)
                        .unwrap_or(0);
                    // Spotify only refreshes the timeline now and then; advance it by the time since that update.
                    if playing {
                        let updated = timeline
                            .LastUpdatedTime()
                            .map(|t| t.UniversalTime)
                            .unwrap_or(0);
                        if updated > 0 {
                            pos += ((filetime_now() - updated) as f64 / 1e7).clamp(0., 30.);
                        }
                    }
                    (if dur > 0 { pos.min(dur as f64) } else { pos }, dur)
                } else {
                    (0., 0)
                };
                let title = props.Title().map(|s| s.to_string()).unwrap_or_default();
                let album = props
                    .AlbumTitle()
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                let artist = props.Artist().map(|s| s.to_string()).unwrap_or_default();
                let (title, artist) = if title.is_empty() && artist.is_empty() {
                    (IDLE_TITLE.to_string(), IDLE_ARTIST.to_string())
                } else {
                    (title, artist)
                };
                let mut media = Media {
                    title,
                    album,
                    artist,
                    playing,
                    artwork: None,
                    pos_secs,
                    duration,
                    app: session
                        .SourceAppUserModelId()
                        .ok()
                        .and_then(|id| app_key(&id.to_string()))
                        .unwrap_or_default()
                        .to_string(),
                };
                let key = format!("{}\0{}\0{}", media.title, media.album, media.artist);
                if key == last {
                    return Ok(media);
                }
                if let Ok(reference) = props.Thumbnail() {
                    if let Ok(stream) = reference.OpenReadAsync().and_then(|op| op.get()) {
                        if let Ok(size) = stream.Size() {
                            if size > 0 && size <= 10_000_000 {
                                if let Ok(reader) = DataReader::CreateDataReader(&stream) {
                                    if reader
                                        .LoadAsync(size as u32)
                                        .and_then(|op| op.get())
                                        .is_ok()
                                    {
                                        let mut bytes = vec![0; size as usize];
                                        if reader.ReadBytes(&mut bytes).is_ok() {
                                            if let Ok(img) = image::load_from_memory(&bytes) {
                                                let img = cover_only(img);
                                                let img = img
                                                    .resize_to_fill(
                                                        136,
                                                        136,
                                                        image::imageops::FilterType::Triangle,
                                                    )
                                                    .to_rgb8();
                                                media.artwork = Some(
                                                    img.pixels()
                                                        .map(|p| {
                                                            ((p[0] as u32) << 16)
                                                                | ((p[1] as u32) << 8)
                                                                | p[2] as u32
                                                        })
                                                        .collect(),
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(media)
            })();
            let media = result.unwrap_or_else(|_| {
                manager = None;
                default_media()
            });
            let is_playing = media.playing;
            last = format!("{}\0{}\0{}", media.title, media.album, media.artist);
            if tx.send(media).is_err() {
                break;
            }
            let sleep_dur = if is_playing {
                Duration::from_millis(1000)
            } else {
                Duration::from_millis(1500)
            };
            thread::sleep(sleep_dur);
        }
    });
    rx
}

pub fn seek(seconds: i64) {
    thread::spawn(move || {
        let _ = (|| -> windows::core::Result<()> {
            let manager =
                GlobalSystemMediaTransportControlsSessionManager::RequestAsync()?.get()?;
            if let Some(session) = followed_session(&manager)? {
                session
                    .TryChangePlaybackPositionAsync(seconds.max(0) * 10_000_000)?
                    .get()?;
            }
            Ok(())
        })();
    });
}

pub fn play_pause() {
    thread::spawn(move || {
        let _ = (|| -> windows::core::Result<()> {
            let manager =
                GlobalSystemMediaTransportControlsSessionManager::RequestAsync()?.get()?;
            if let Some(session) = followed_session(&manager)? {
                session.TryTogglePlayPauseAsync()?.get()?;
            }
            Ok(())
        })();
    });
}

pub fn next() {
    thread::spawn(move || {
        let _ = (|| -> windows::core::Result<()> {
            let manager =
                GlobalSystemMediaTransportControlsSessionManager::RequestAsync()?.get()?;
            if let Some(session) = followed_session(&manager)? {
                session.TrySkipNextAsync()?.get()?;
            }
            Ok(())
        })();
    });
}

pub fn prev() {
    thread::spawn(move || {
        let _ = (|| -> windows::core::Result<()> {
            let manager =
                GlobalSystemMediaTransportControlsSessionManager::RequestAsync()?.get()?;
            if let Some(session) = followed_session(&manager)? {
                session.TrySkipPreviousAsync()?.get()?;
            }
            Ok(())
        })();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(app_id: &str, playing: bool, current: bool) -> Candidate {
        Candidate {
            app_id: app_id.into(),
            playing,
            current,
        }
    }

    #[test]
    fn test_app_key_browser_and_player_mapping() {
        assert_eq!(app_key("Spotify.exe"), Some("spotify"));
        assert_eq!(app_key("spotify"), Some("spotify"));
        assert_eq!(app_key("Spotifast.exe"), Some("spotifast"));
        assert_eq!(app_key("applemusic"), Some("applemusic"));
        assert_eq!(app_key("AppleMusic.exe"), Some("applemusic"));
        assert_eq!(app_key("chrome.exe"), Some("chrome"));
        assert_eq!(app_key("Google.Chrome"), Some("chrome"));
        assert_eq!(app_key("msedge.exe"), Some("msedge"));
        assert_eq!(app_key("Microsoft.Edge_8wekyb3d8bbwe!App"), Some("msedge"));
        assert_eq!(app_key("notepad.exe"), None);
        assert_eq!(app_key("vlc.exe"), None);
    }

    #[test]
    fn test_concurrent_spotify_and_browser_scenarios() {
        let spotify = "Spotify.exe";
        let chrome = "chrome.exe";
        let edge = "msedge.exe";

        // 1. Both Spotify and Chrome playing, Chrome is Windows' current active session
        assert_eq!(
            choose(&[c(spotify, true, false), c(chrome, true, true)], "", ""),
            Some(1)
        );

        // 2. Both playing, Spotify is current session
        assert_eq!(
            choose(&[c(spotify, true, true), c(chrome, true, false)], "", ""),
            Some(0)
        );

        // 3. Both playing, neither is current, but Spotify was followed on screen
        assert_eq!(
            choose(
                &[c(spotify, true, false), c(chrome, true, false)],
                spotify,
                ""
            ),
            Some(0)
        );

        // 4. Both playing, neither is current, but Chrome was followed on screen
        assert_eq!(
            choose(
                &[c(spotify, true, false), c(chrome, true, false)],
                chrome,
                ""
            ),
            Some(1)
        );

        // 5. Spotify paused, Chrome playing
        assert_eq!(
            choose(&[c(spotify, false, true), c(chrome, true, false)], "", ""),
            Some(1)
        );

        // 6. Chrome paused, Spotify playing
        assert_eq!(
            choose(&[c(spotify, true, false), c(chrome, false, true)], "", ""),
            Some(0)
        );

        // 7. Edge playing, Spotify playing, Edge current
        assert_eq!(
            choose(&[c(spotify, true, false), c(edge, true, true)], "", ""),
            Some(1)
        );

        // 8. User selected Spotify in tray: Chrome playing is ignored
        assert_eq!(
            choose(
                &[c(spotify, false, false), c(chrome, true, true)],
                "",
                "spotify"
            ),
            Some(0)
        );

        // 9. User selected Chrome in tray: Chrome is chosen even if paused
        assert_eq!(
            choose(
                &[c(spotify, true, true), c(chrome, false, false)],
                "",
                "chrome"
            ),
            Some(1)
        );
    }

    #[test]
    fn test_chrome_media_properties() {
        if let Ok(op) = GlobalSystemMediaTransportControlsSessionManager::RequestAsync() {
            if let Ok(manager) = op.get() {
                if let Ok(sessions) = manager.GetSessions() {
                    for s in sessions {
                        let id = s
                            .SourceAppUserModelId()
                            .map(|id| id.to_string())
                            .unwrap_or_default();
                        if id.to_lowercase().contains("chrome") {
                            println!("Testing Chrome session: {}", id);
                            if let Ok(op) = s.TryGetMediaPropertiesAsync() {
                                if let Ok(props) = op.get() {
                                    let title =
                                        props.Title().map(|s| s.to_string()).unwrap_or_default();
                                    let album = props
                                        .AlbumTitle()
                                        .map(|s| s.to_string())
                                        .unwrap_or_default();
                                    let artist =
                                        props.Artist().map(|s| s.to_string()).unwrap_or_default();
                                    println!(
                                        "Chrome: Title='{}', Artist='{}', Album='{}'",
                                        title, artist, album
                                    );
                                    if let Ok(reference) = props.Thumbnail() {
                                        println!("Thumbnail reference exists!");
                                        if let Ok(stream) =
                                            reference.OpenReadAsync().and_then(|op| op.get())
                                        {
                                            println!("Stream opened!");
                                            if let Ok(size) = stream.Size() {
                                                println!("Stream size: {}", size);
                                                if size > 0 && size <= 10_000_000 {
                                                    if let Ok(reader) =
                                                        DataReader::CreateDataReader(&stream)
                                                    {
                                                        println!("DataReader created!");
                                                        if reader
                                                            .LoadAsync(size as u32)
                                                            .and_then(|op| op.get())
                                                            .is_ok()
                                                        {
                                                            println!("Loaded bytes!");
                                                            let mut bytes = vec![0; size as usize];
                                                            if reader.ReadBytes(&mut bytes).is_ok()
                                                            {
                                                                println!(
                                                                    "Read {} bytes",
                                                                    bytes.len()
                                                                );
                                                                match image::load_from_memory(
                                                                    &bytes,
                                                                ) {
                                                                    Ok(img) => {
                                                                        use image::GenericImageView;
                                                                        println!("Loaded image! Dimensions: {:?}", img.dimensions());
                                                                        let cropped =
                                                                            cover_only(img);
                                                                        println!("Cropped! Dimensions: {:?}", cropped.dimensions());
                                                                    }
                                                                    Err(e) => {
                                                                        println!("image::load_from_memory error: {:?}", e);
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
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_live_get_media_session() {
        if let Ok(op) = GlobalSystemMediaTransportControlsSessionManager::RequestAsync() {
            if let Ok(manager) = op.get() {
                if let Ok(Some(s)) = get_media_session(&manager) {
                    let id = s
                        .SourceAppUserModelId()
                        .map(|id| id.to_string())
                        .unwrap_or_default();
                    println!("get_media_session chosen: {}", id);
                }
            }
        }
    }

    #[test]
    fn test_toggle_chrome() {
        if let Ok(op) = GlobalSystemMediaTransportControlsSessionManager::RequestAsync() {
            if let Ok(manager) = op.get() {
                if let Ok(sessions) = manager.GetSessions() {
                    for s in sessions {
                        let id = s
                            .SourceAppUserModelId()
                            .map(|id| id.to_string())
                            .unwrap_or_default();
                        if id.to_lowercase().contains("chrome") {
                            println!("Toggling chrome: {}", id);
                            let _ = s.TryTogglePlayPauseAsync().and_then(|op| op.get());
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_toggle_spotify() {
        if let Ok(op) = GlobalSystemMediaTransportControlsSessionManager::RequestAsync() {
            if let Ok(manager) = op.get() {
                if let Ok(sessions) = manager.GetSessions() {
                    for s in sessions {
                        let id = s
                            .SourceAppUserModelId()
                            .map(|id| id.to_string())
                            .unwrap_or_default();
                        if id.to_lowercase().contains("spotify") {
                            println!("Toggling Spotify session: {}", id);
                            let _ = s.TryTogglePlayPauseAsync().and_then(|op| op.get());
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_live_smtc_sessions() {
        if let Ok(op) = GlobalSystemMediaTransportControlsSessionManager::RequestAsync() {
            if let Ok(manager) = op.get() {
                if let Ok(sessions) = manager.GetSessions() {
                    for s in sessions {
                        let id = s
                            .SourceAppUserModelId()
                            .map(|id| id.to_string())
                            .unwrap_or_default();
                        let status = s
                            .GetPlaybackInfo()
                            .ok()
                            .and_then(|i| i.PlaybackStatus().ok());
                        println!("Live Session: AppId='{}', Status={:?}", id, status);
                        if let Ok(op) = s.TryGetMediaPropertiesAsync() {
                            if let Ok(props) = op.get() {
                                println!(
                                    "  Title='{}', Artist='{}'",
                                    props.Title().unwrap_or_default(),
                                    props.Artist().unwrap_or_default()
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn picks_the_right_player_when_several_are_open() {
        let spotify = "Spotify.exe";
        let spotifast = "Spotifast.exe";
        // Both playing: Windows' current session wins, whatever the list order.
        assert_eq!(
            choose(&[c(spotifast, true, false), c(spotify, true, true)], "", ""),
            Some(1)
        );
        // Neither is current: stay on the one already shown instead of flipping.
        assert_eq!(
            choose(
                &[c(spotify, true, false), c(spotifast, true, false)],
                spotifast,
                ""
            ),
            Some(1)
        );
        // A playing app beats a paused current one.
        assert_eq!(
            choose(
                &[c(spotify, false, true), c(spotifast, true, false)],
                "",
                ""
            ),
            Some(1)
        );
        // Nothing playing: the current session, then the followed one, then the first.
        assert_eq!(
            choose(
                &[c(spotify, false, false), c(spotifast, false, true)],
                "",
                ""
            ),
            Some(1)
        );
        assert_eq!(
            choose(
                &[c(spotify, false, false), c(spotifast, false, false)],
                spotifast,
                ""
            ),
            Some(1)
        );
        // Supported browsers (Chrome, Edge) are chosen when playing; unsupported apps are never chosen.
        assert_eq!(choose(&[c("chrome", true, true)], "", ""), Some(0));
        assert_eq!(choose(&[c("msedge.exe", true, true)], "", ""), Some(0));
        assert_eq!(choose(&[c("notepad", true, true)], "", ""), None);
        // A tray choice limits it to one player.
        assert_eq!(
            choose(
                &[c(spotify, true, true), c(spotifast, false, false)],
                "",
                "spotifast"
            ),
            Some(1)
        );
        assert_eq!(choose(&[c(spotify, true, true)], "", "applemusic"), None);
    }
}

#[cfg(test)]
mod leak {
    use super::*;
    #[repr(C)]
    #[derive(Default)]
    struct Counters {
        cb: u32,
        faults: u32,
        peak_ws: usize,
        ws: usize,
        qpp: usize,
        qp: usize,
        qnpp: usize,
        qnp: usize,
        pagefile: usize,
        peak_pagefile: usize,
        private: usize,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
        fn K32GetProcessMemoryInfo(p: *mut std::ffi::c_void, c: *mut Counters, cb: u32) -> i32;
    }
    fn private_mb() -> f64 {
        let mut c = Counters {
            cb: size_of::<Counters>() as u32,
            ..Default::default()
        };
        unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
        c.private as f64 / 1048576.
    }
    /// `cargo test request_async_memory -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn request_async_memory() {
        let poll = || {
            let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
                .unwrap()
                .get()
                .unwrap();
            let _ = get_media_session(&manager);
        };
        for _ in 0..50 {
            poll();
        }
        let before = private_mb();
        for _ in 0..3000 {
            poll();
        }
        let after_new = private_mb();
        let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
            .unwrap()
            .get()
            .unwrap();
        for _ in 0..3000 {
            let _ = get_media_session(&manager);
        }
        let after_reuse = private_mb();
        println!(
            "new manager each poll: {:+.1} MB per 3000; reused manager: {:+.1} MB per 3000",
            after_new - before,
            after_reuse - after_new
        );
    }
}
