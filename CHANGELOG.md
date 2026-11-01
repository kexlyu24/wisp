# Changelog

All notable changes to Wisp in this development checkpoint are documented in this file.

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
