# Changelog

All notable changes to Wisp in this development checkpoint are documented in this file.

## [0.2.2] - 2026-11-25

### Added
- **High-resolution Quick Access icons**: resolves shortcuts via direct path, `SearchPathW`, App Paths registry (`HKCU`/`HKLM`), and `FOLDERID_AppsFolder` (`ms-settings:*`), extracts 96–256 px icons via `IShellItemImageFactory::GetImage` with `SHDefExtractIconW`/`SHGetFileInfoW` fallbacks (including 1-bit mask alpha recovery for legacy icons), and downscales in premultiplied RGBA with Catmull-Rom filtering (#34).
- **Windows CI & automated draft Releases**: `.github/workflows/rust.yml` runs unit tests, release builds, and the smoke test on `windows-latest`; `.github/workflows/release.yml` builds tagged releases and drafts a GitHub Release with the portable ZIP, `wisp-windows-x64.exe`, and (for stable tags) `wisp-update.json` (#31, #35).

### Improved
- **Quick Access smooth spring scrolling**: scrolling through more than 6 shortcuts now glides continuously on a spring (`reduced_motion` snaps immediately) with smoothstep top/bottom edge fades, a continuous right-edge scrollbar pill, and proper row-bound clamping (#37).
- **Quick Access drag-and-reorder**: replaced thread-local motion with per-card springs (`QuickCardMotion`) integrated into `State::advance` with `reduced_motion` support and `PACE_MOTION` frame pacing, adding live neighbor slot-shift preview, a drop-slot placeholder, subtle lift elevation, and smooth post-drop/cancel settle (#34).

### Changed
- **In-app updater (HTTPS + SHA-256)**: migrated from Ed25519 signature verification (`update-public-key.hex` and `wisp-update.json.sig`) to HTTPS + SHA-256 verification (`wisp-windows-x64.exe` and `wisp-update.json`) with official GitHub release URL pinning, manifest version/size/SHA-256 checks, and x64 PE validation. Manual checks no longer have a cooldown and are disabled only while a check or download is running (#34, #35).

### Fixed
- **Quick Access edge cases**: fixed the `< 6` items draw loop bug, pointer capture release on drag cancel (Escape, tab switch, search, collapse, `WM_MOUSELEAVE`), mouse-wheel guarding during drag, and scroll clamping after deleting items (#34).
- **8.3 short `%TEMP%` test compatibility**: fixed `test_file_data_object_carries_paths` so it passes on environments whose `%TEMP%` uses Windows 8.3 short paths (#31).

### Known limitations
- Clients running `0.2.1` (which require Ed25519 `.sig` assets) or `0.2.0` and older cannot auto-update to `0.2.2`; download the `0.2.2` ZIP once to switch to the HTTPS + SHA-256 updater.
- WhatsApp Web image drops are not supported (blob URLs / virtual-file data).
- The updater's replace and rollback paths should be exercised end to end on Windows before relying on them in production.
- Apple Music's volume adapter only detects the app; it does not change the volume yet.

## [0.2.1] - 2026-11-23

### Added
- **Text editing** in the To-do, Calendar event title and Quick Access search, name and command fields: click to place the caret, drag or Shift-click to select, Left/Right/Home/End (Shift extends), Ctrl+A/C/X/V, Backspace/Delete, and horizontal scrolling for long text. Pasted line breaks and control characters are removed, and the character limits stay the same (To-do 120, Calendar 100, Quick Access 80) (#22).
- **In-app updater** (Settings > Updates): checks the latest stable GitHub Release, shows its notes, downloads in the background with progress and cancel, and verifies an Ed25519-signed manifest, size, SHA-256 and the x64 executable before Restart & update replaces `wisp.exe`. If the new version fails to start within 15 seconds, the helper tries to restore the previous executable. Automatic checks run at most once a day at startup and can be turned off; manual checks have a 60-second cooldown (#22).
- **Browser image drops**: drag a direct HTTPS PNG, JPEG, WebP or BMP image from Chrome or Edge onto the notch and it is downloaded (no browser cookies, up to 20 MiB and 8192 px), saved as a PNG in `%APPDATA%\Wisp\web-images` and added to the shelf. Failures show an in-app alert (#22).
- **Local diagnostics**: `%APPDATA%\Wisp\wisp.log` records startup, updates, image drops and data-file errors, rotating at 2 MiB with three archives. User text, clipboard contents, full URLs and credentials are never logged, and nothing is sent anywhere (#22).

### Changed
- The README's privacy note lists update checks and update downloads as network requests.

### Known limitations
- WhatsApp Web image drops are not supported: they use blob URLs, and reading the virtual-file data they offer is not implemented.
- 0.2.0 and older have no updater, so updating to 0.2.1 means downloading the zip once.
- The updater's replace and roll-back paths have not yet been exercised end to end on Windows.
- Apple Music's volume adapter only detects the app; it does not change the volume yet.

## [0.1.9] - 2026-11-04

### Added
- **SoundCloud desktop app**: sessions whose app id contains "soundcloud" count as a desktop player, with Now Playing, transport controls and a volume slider that sets the `soundcloud.exe` level in the Windows Volume Mixer.

### Known limitations
- Apple Music's volume adapter only detects the app; it does not change the volume yet.
- SoundCloud in Chrome or Edge stays a browser session.

## [0.1.8hf] - 2026-11-02

Hotfix for 0.1.8.

### Fixed
- **Spotify volume** did almost nothing and made the notch slider flicker: driving Spotify's in-app slider through UI Automation blocks ~2 s, steals the focus, moves in 10% key steps and is ignored while minimized. Spotify now uses its own Windows audio session (Volume Mixer), which is instant and works minimized; `SpotifyVolumeAdapter`, its worker and `SpotifyFocusGuard` are removed.
- **Spotify became "always on top"** after a volume change: the focus guard re-stacked it behind the topmost notch.
- **Spotifast volume jumped up and down**: `spotifast volume` processes overtook each other during a drag, and its internal volume file was read and written. Commands now run in order (newest drag level only), the level comes from `spotifast now-playing --raw`, its files are left alone, unmute restores the previous level, and mute works.
- Audio sessions are found on every active output device, not only the default one.

### Improved
- **Calendar agenda**: an "Add an event" composer with a title, a time picker (− / + in 15-minute steps) or All day, and Cancel / Add buttons; click an event to edit it. Typed times (`9:30 Standup`) still work, an empty time is all day, and the composer and the day's events animate.

### Known limitations
- Apple Music's volume adapter only detects the app; it does not change the volume yet.

## [0.1.8] - 2026-11-02

### Added
- **Spotify Desktop Native Volume Adapter (`SpotifyVolumeAdapter`)**: Direct player volume and mute/unmute control via Windows UI Automation (`IUIAutomationRangeValuePattern`, `IUIAutomationInvokePattern`) targeting Spotify's internal volume slider.
- **Asynchronous Bounded Volume Verification**: Introduced `VolumeUpdateStatus` (`Idle`, `Pending`, `Confirmed`, `Failed`) tracking with a bounded 1500 ms stabilization window; immediate UI slider updates without waiting or bouncing back to stale readbacks.
- **Smart Focus Preservation (`SpotifyFocusGuard`)**: Integrated Win32 `GetLastInputInfo` and `GetTickCount` to distinguish UIA-induced focus events from concurrent user interactions. Implemented `find_intentional_target` using `WindowFromPoint` (mouse clicks) and `GW_HWNDNEXT` (keyboard/Alt+Tab) to respect user window switching and keep Spotify in the background.
- **Spotifast CLI IPC Volume Adapter (`SpotifastVolumeAdapter`)**: Native CLI volume control [0.0, 100.0] and mute toggling without touching Windows master volume.
- **Apple Music for Windows UIA Volume Adapter (`AppleMusicVolumeAdapter`)**: Windows UI Automation RangeValue adapter structure for Apple Music Desktop.
- **Browser Per-Application Audio Session Adapters**: Dedicated per-process Core Audio session volume control for Google Chrome (`ChromeYouTubeVolumeAdapter`) and Microsoft Edge (`EdgeYouTubeVolumeAdapter`) using Windows `ISimpleAudioVolume`.
- **Interactive Calendar & Event Scheduling**: Month-view calendar with weekday headers, date selection, month/year boundary navigation, leap year handling, and local persistence (`%APPDATA%\Wisp\calendar.txt`) with schedule creation, editing, deletion, and date indicators.
- **Standardized Volume Readback & Focus Race Test Suites**: Unit and integration test suites covering player volume normalization, range mapping, concurrent player switching, UIA live verification, and mock audio session isolation.
- **Formal PRD Documentation**: Complete Product Requirements Documents in `docs/WISP-PRD.md` and `Wisp-Product-PRD-EN.md` with verified SHA-256 byte parity.

### Improved
- **Volume Slider Responsiveness**: Decoupled optimistic UI updates from background UIA verification. Discards stale UI Automation readbacks during the 1500 ms stabilization period.
- **Focus Guard Resiliency**: Eliminated focus hijacking where Spotify or its renderer window (`Chrome_RenderWidgetHostHWND`) rises in Z-order or steals focus when setting volume. Minimized Spotify window (`IsIconic`) state remains invariant.
- **Idle Resource Consumption**: Lazy initialization of HTTP clients, reduced idle timer wakeups, and event-driven notification polling, reducing deep idle footprint to 3.64 MB Working Set / 4.23 MB Private Bytes.
- **Player Switching and Session Isolation**: Media session detection accurately maps active sessions among Spotify, Spotifast, Apple Music, and browser playback without interfering with Windows endpoint volume.

### Fixed
- **Spotify Focus Hijacking on Volume Adjustment**: Fixed Chromium UIA accessibility implementation stealing foreground focus when `RangeValuePattern::SetValue` is invoked.
- **Volume Slider Bounce-Back**: Fixed stale UIA readback overwriting requested slider values before Chromium finished applying the change.
- **Interfering with Intentional Window Switches**: Fixed focus guard incorrectly restoring previous foreground window when user clicked another window or Alt+Tabbed during an active volume operation.
- **Premature Network Worker Activation**: Prevented HTTP client and notification poller from allocating resources during application idle.

### Removed / Replaced
- **Browser Extension & Native Messaging**: Removed custom browser extension, Manifest V3 content scripts, Native Messaging host registry keys, and browser Named Pipe IPC bridge.
- **Replaced with Core Audio Session Volume**: Replaced browser IPC infrastructure with Windows Core Audio `ISimpleAudioVolume` per-application session control.

### Deferred / Known Limitations
- **Apple Music Live Verification**: `AppleMusicVolumeAdapter` is implemented and unit-tested via mock interfaces, but live verification is deferred because Apple Music for Windows is not installed in the test environment.
- **Microsoft Edge Live Playback Verification**: `EdgeYouTubeVolumeAdapter` is implemented and unit-tested for session isolation; live playback testing remains pending installation of Edge Desktop.
- **Process-Scoped Browser Volume**: Windows Core Audio `ISimpleAudioVolume` controls volume at the browser process level; individual YouTube tabs sharing an audio process are not isolated from each other.
- **Transient Foreground Transition**: Chromium's accessibility implementation can still exhibit a brief transient OS-level foreground transition during UIA calls before `SpotifyFocusGuard` restores the intentional target window.
- **Future Roadmap Features (Not in This Checkpoint)**: Notch-native Settings panel, Notification Center history, Live Activities, Quick Capture notes, Multi-Timer, Water Reminder, System HUD, Device Awareness, and Smart Automation.

### Performance
- **Deep Idle Baseline (No Active Media Playback)**:
  - Working Set: 3.64 MB average (min: 3.11 MB, max: 4.18 MB)
  - Private Bytes: 4.23 MB average (min: 4.20 MB, max: 4.25 MB)
  - Thread Count: 12.5 threads average
  - CPU Utilization: 0.028% (over 45.61 s sampling duration)
- **Active Playback Mode (Spotify Playing with Live Album Artwork & Lyrics)**:
  - Working Set: ~28.96 MB (due to GDI+ album art textures, DirectWrite layouts, and LRCLIB caching)
  - Private Bytes: 7.45 MB
  - Thread Count: 20 threads
  - CPU Utilization: 0.598% (over 30.54 s sampling duration)

### Verification
- `cargo fmt --check`: Passed with clean formatting.
- `cargo test`: 59 passed, 0 failed, 5 ignored (live network/audio tests ignored by default).
- `cargo check --release`: Passed with zero warnings.
- `cargo build --release`: Built optimized release binary `target/release/wisp.exe`.
- `cargo run --release -- --smoke-test`: Passed (exit code 0; verified layered alpha rendering, dynamic notch sizing 200x36 collapsed / 460x196 expanded, and shelf folder grouping).
- PRD Parity: `docs/WISP-PRD.md` and `Wisp-Product-PRD-EN.md` verified byte-for-byte identical (SHA-256: `EEA20A12992F0B3E16D590D4DC7097A199B4D8FF4D8F0483E9BE59CD26D9444E`).
