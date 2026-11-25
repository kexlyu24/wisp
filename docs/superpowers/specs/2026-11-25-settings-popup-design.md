# Standalone Settings Popup Window — Design Spec

**Date:** 2026-11-25  
**Target release:** Wisp `v0.2.2` (branch `feat/settings-popup-window` → PR into `dev`)  
**Status:** Draft — pending owner approval

---

## 1. Intent & Why

Today, Settings lives inside the notch as `TAB_SETTINGS` (`440×328`, the 8th scrollable tab) with three stacked views (`settings_subpage`: Main, Appearance drilldown, and Updates drilldown). Moving Settings out of the notch into a standalone floating popup window solves three problems at once:

1. **Cleaner notch navigation:** Scrolling/swiping across notch pages stays focused on day-to-day ambient tools (Now Playing, Calendar, To-do, Pomodoro, Shelf, Quick Access, Notifications) instead of landing on configuration screens.
2. **Comfortable space & decoupled window:** A dedicated `620×420` macOS System Settings–style window gives every option room for a clear helper subtitle, removes nested `< Settings` drilldowns, and stays open when clicking outside so users can adjust settings and hover/test the notch at their own pace without Settings closing.
3. **Room to grow:** A left sidebar scales cleanly as Wisp adds more configuration categories in future releases without cramping the notch.

---

## 2. Goals, Non-Goals & Anti-Goals

### Goals
- Remove `TAB_SETTINGS` from the notch's scrollable page list (`TAB_COUNT`: `8 → 7`).
- Keep the Gear icon at the right edge of the notch header as a dedicated **toggle button** that opens/focuses or hides the standalone Settings popup.
- Make the system tray menu's **"Settings…"** action open and focus the same standalone Settings popup (without forcing the notch open).
- Present all existing Settings controls in a `620×420` Apple dark-mode sidebar window (**Option A**) with four flat categories: **Notch**, **General**, **Appearance**, and **Updates**.
- Keep changes immediate and auto-saved (`settings.txt`, HKCU `Run` key, updater state) while keeping `SettingsWindow` decoupled from notch expand/collapse state.
- Isolate the popup's windowing, layout, hit-testing, and painting into a dedicated Rust module (`src/settings_win.rs`) while preserving zero external UI frameworks.

### Non-Goals
- Adding new user settings or removing existing settings (scope is strictly migrating and reorganizing existing controls).
- Resizing or maximizing the Settings window (fixed `620×420` logical px, DPI-scaled).
- Adding a separate taskbar button or native Win32 chrome/caption bar.
- Changing how `settings.txt`, `%APPDATA%\Wisp\`, or `src/updater.rs` store data or verify updates.

### Anti-Goals (what would make this a failure)
- Introducing WebView2, heavy GUI crates, or noticeable idle RAM/CPU overhead when Settings is closed.
- Closing Settings unexpectedly when the user clicks the notch to test settings, or hiding the `Choose…` file picker behind the topmost notch.
- Violating Wisp's visual language (`AGENTS.md`: Apple dark mode `WHITE`/`SECONDARY`/`TERTIARY`, system `GREEN` switches, `BLUE` focus/primary actions, no card borders, no colored body text, bundled Inter / system SF Pro Display only).
- Leaving zombie windows or failing to exit cleanly when clicking **Quit Wisp** or closing from the tray.

---

## 3. Constraints

- **Toolchain & platform:** Windows 10/11 only, `stable-x86_64-pc-windows-gnu`, zero new Cargo dependencies.
- **Rendering rules (`AGENTS.md`):** Premultiplied BGRA DIB presented via `UpdateLayeredWindow`; shapes/icons via `src/render.rs` `Renderer` (dynamically loaded `gdiplus.dll`); all text via cached grayscale mask blitting (never GDI `DrawTextW` directly onto the frame DIB, which clears alpha); no per-frame surface allocations.
- **Visual language (`AGENTS.md`):** Pure dark Apple styling — window surface `#141416`, sidebar/titlebar `#0e0e10`, grouped cards `#1c1c1e`, hairline row dividers `#2c2c2e`, switch track off `#39393d` / on system `GREEN`, active sidebar item & primary CTA system `BLUE`, text `WHITE` / `SECONDARY` / `TERTIARY`. **Quit Wisp** uses neutral `WHITE` text on a dark pill (`#1c1c1e`, hover `#2c2c2e`, pressed `#36363a`) to stay strictly compliant with `AGENTS.md` (no colored text).
- **Frame pacing:** Any spring animating in Settings (switches, hover/slider) keeps the frame loop at `PACE_MOTION`; when Settings is open and idle (and not downloading an update), it must not spin the CPU (`PACE_IDLE` / redraw on input or state change).
- **Testing & CI:** Must pass `cargo test --locked`, `cargo build --release --locked`, and `target/release/wisp.exe --smoke-test` (`PASS` in `smoke-result.txt`) on `windows-latest`.

---

## 4. Approaches Considered

| Approach | Summary | Trade-offs | Decision |
|---|---|---|---|
| **Approach 1: Native Win32 + GDI+ layered window (`src/settings_win.rs`)** | Second `WispSettings` HWND on the same UI thread sharing `*mut State` via `GWLP_USERDATA`, rendered with its own DIB + `Renderer` + cached text masks. | Zero new dependencies, ~3 MB binary preserved, exact font/color/spring parity with notch, instant live updates to notch with zero IPC/locking; requires manual layout/hit-test code (~600–800 lines, reusing existing primitives). | **Selected** |
| **Approach 2: Win32 standard controls / WebView2** | Native Win32 dialog controls or embedded Edge WebView2 HTML UI. | Win32 controls break Apple dark mode; WebView2 adds 50–100 MB RAM, runtime dependency, and async JS↔Rust bridge complexity. | Rejected |

---

## 5. UX, Layout & Feature Specification (Option A)

### 5.1 Notch Header & Triggers
- **Notch pages (`TAB_COUNT = 7`):** `0` Now Playing, `1` Calendar, `2` To-do, `3` Pomodoro, `4` Shelf, `5` Quick Access, `6` Notifications.
- **Wheel / swipe tab switching:** Cycles strictly within `0..=6`.
- **Gear button:** Remains anchored at the right edge of the notch header (`x = panel_w_now() - 24. - 34.`, `y = 10.`, `w = 34.`, `h = 24.`) with dedicated header hit code `11` (`HEADER_SETTINGS`).
  - Idle color: `SECONDARY` (brightened to `WHITE` on hover, or subtle `#2c2c2e` capsule + `WHITE` icon while the Settings popup is open).
  - Click behavior: **Toggle** — if Settings popup is closed, open & focus it; if open, hide it. Does not alter `active_tab` or `expanded` state.
- **Tray menu `"Settings…"` (`tray::item::SETTINGS`):** Refreshes `autostart = settings::starts_with_windows()`, opens the Settings popup if closed, and brings it to the foreground (`SetForegroundWindow`). Does **not** open or hold the notch.

### 5.2 Window Geometry & Behavior
- **Logical dimensions:** `620 × 420` logical px (`SET_WIN_W = 620.`, `SET_WIN_H = 420.`), rounded outer corners (`r = 14.`), scaled by `State.scale` (Wisp remains System-DPI-Aware via `SetProcessDPIAware()`, sharing `State.scale` and `State.fonts` across both windows — *architect call*).
- **Window style:** Borderless `WS_POPUP` with `WS_EX_LAYERED | WS_EX_TOOLWINDOW` (no taskbar button, no native frame). Not `WS_EX_TOPMOST` so normal app windows and OS dialogs behave naturally (and the notch remains above everything as intended).
- **Notch decoupling (owner decision):** Opening Settings or switching to **Appearance** does **not** auto-expand or pin the notch open. Changes apply and save immediately (`apply_settings()` + `InvalidateRect(main_hwnd)`), and the user hovers/opens the notch themselves whenever they want to inspect visual changes.
- **Initial position:** Centered in the primary monitor work area (`SPI_GETWORKAREA`) on first open; subsequent opens in the same session preserve the last dragged position, clamped inside its monitor's work area (`MonitorFromPoint` / `GetMonitorInfoW`, falling back to `SPI_GETWORKAREA` — *architect call*).
- **Dragging:** Dragging anywhere on the `40 pt` top titlebar (excluding the right-hand `✕` close button) initiates native OS window movement (`ReleaseCapture(); SendMessageW(hwnd, WM_NCLBUTTONDOWN, 2 /* HTCAPTION */, 0)`).
- **Dismissing:** Clicking `✕` (top-right titlebar), pressing `Escape`, pressing `Alt+F4` (`WM_CLOSE`), or clicking the notch Gear icon while open hides the window (`ShowWindow(SW_HIDE)`, `open = false`). Clicking outside the window **keeps it open** so users can test notch changes live.
- **Quit Wisp:** Clicking **Quit Wisp** in the sidebar footer calls `DestroyWindow(main_hwnd)`, cleanly terminating Wisp (just like tray `Quit Wisp`).

### 5.3 Visual Structure (`620 × 420`)
- **Top Titlebar (`y = 0..40`, full width `620`, fill `#0e0e10`, 1 px bottom divider `#242426`):**
  - Left (`x = 16`): Small Wisp pill glyph (or icon) + `"Wisp Settings"` (`WHITE`, semibold/medium).
  - Right (`x = 584..608, y = 8..32`): `24×24` circular close button (`✕`, `SECONDARY`, hover fill `#2c2c2e` + `WHITE`).
- **Left Sidebar (`x = 0..168, y = 40..420`, fill `#0e0e10`, 1 px right divider `#242426`):**
  - Top navigation stack (`x = 10..158`, 4 items, height `34`, gap `4`, radius `8`):
    1. **Notch** (section `0`)
    2. **General** (section `1`)
    3. **Appearance** (section `2`)
    4. **Updates** (section `3`)
    - Active item: `BLUE` fill, `WHITE` stroke icon + label. Inactive: transparent (hover `#1c1c1e`), `WHITE`/`SECONDARY` icon + label.
  - Bottom footer (separated by 1 px hairline `#242426` at `y = 344`):
    - Version label: `concat!("Wisp ", env!("CARGO_PKG_VERSION"))` (`TERTIARY`, `y = 354`).
    - **Quit Wisp** pill button (`x = 12..156, y = 376..406`, radius `8`, fill `#1c1c1e`, hover `#2c2c2e`, pressed `#36363a`, `WHITE` centered text).
- **Right Content Pane (`x = 168..620, y = 40..420`, width `452`, padding `20` left/right → content `x = 188..600`, width `412`, fill `#141416`):**
  Each section opens with a section title (`15 pt` medium `WHITE`) and a 1-line description (`11 pt` `SECONDARY`), followed by inset grouped cards (`#1c1c1e`, radius `12`, internal `1 px` dividers `#2c2c2e` inset `14 pt` from left):

#### Section 0 — **Notch**
Subtitle: *"How the dynamic notch expands, hides, and shows live info."*  
Group header: `BEHAVIOR`
1. **Open on hover** — *"Expand the panel when the pointer reaches the top edge"* (`tray::item::OPEN_ON_HOVER`)
2. **Minimal mode** — *"Shrink to a thin sliver when covering browser tabs or content"* (`tray::item::MINIMAL`)
3. **Lyrics when closed** — *"Show the live lyric line inside the collapsed notch pill"* (`tray::item::COLLAPSED_LYRICS`)
4. **Notification popups** — *"Briefly spring open for incoming Windows notifications"* (`tray::item::NOTIFICATIONS`)

Footer caption (`TERTIARY`): *"Changes save automatically."*

#### Section 1 — **General**
Subtitle: *"Clock format, motion preference, and Windows startup."*  
Group header: `SYSTEM & DISPLAY`
1. **24-hour clock** — *"Show 14:25 instead of 2:25 PM in the collapsed notch"* (`tray::item::CLOCK_24H`)
2. **Match Windows motion** — *"Snap animations when Windows animation effects are off"* (`tray::item::FOLLOW_ANIMATIONS`)
3. **Start with Windows** — *"Launch Wisp automatically when you sign in"* (`tray::item::AUTOSTART`)

Footer caption (`TERTIARY`): *"Data is stored locally in %APPDATA%\Wisp."*

#### Section 2 — **Appearance**
Subtitle: *"Panel backdrop theme, frost blur, and opacity. Closed notch stays pure black."*
- Group `BACKGROUND STYLE` (card with 5 selectable tiles in one row): **Frost** (`0`), **Aura** (`1`), **Slate** (`2`), **OLED** (`3`), **Image** (`4`). Each tile shows a small dark swatch preview + label; active tile has `#2c2c2e` fill and subtle `BLUE` highlight indicator. Clicking **Image** when no custom image is loaded opens the image picker immediately (matching existing behavior).
- Group card below (rows separated by `#2c2c2e` dividers):
  - **Custom image** (shown when `bg_style == 4` or custom image loaded): `Choose…` / `Change…` button (`5320`), `Reset` button (`5321`, when an image is loaded), plus format hint (`PNG, JPG, BMP, WebP`) or active status.
  - **Framing** (shown when `bg_style == 4`): segmented pills `Top` (`1`), `Center` (`0`), `Bottom` (`2`).
  - **Blur radius** (shown when `bg_style != 3`, i.e., not OLED): draggable slider `0..=30 px` (`5340`) with live readout (`None` or `{n}px`). Same behavior as today: updates `settings.bg_blur` live while dragging, runs `prepare_custom_bg` / `save()` on pointer release (`WM_LBUTTONUP`) so dragging stays smooth.
  - **Opacity** (always shown): segmented pills `70%`, `80%`, `90%`, `100%`.

#### Section 3 — **Updates**
Subtitle: *"HTTPS + SHA-256 verified stable releases from GitHub."*
- Card 1:
  - Row 1: **Check automatically** — *"Check once daily on startup (nothing installs automatically)"* with iOS-style animated toggle switch (`5401`, wired to `updater.toggle_auto()`).
  - Row 2: Current status text (wrapped up to 2 lines), `"Current version: Wisp X.Y.Z"`, optional download progress bar (`BLUE` fill + `%`) when `Phase::Downloading`, release notes preview (up to 4 wrapped lines when a release is available), and primary action pill (`5400`: `"Check for updates"` / `"Checking..."` / `"Download update"` / `"Downloading..."` / `"Restart & update"` / `"Retry download"`) plus `"Cancel"` pill (`5402`) while downloading.
- Card 2 (informational, non-interactive):
  - **Troubleshooting log** — `"%APPDATA%\Wisp\wisp.log (stays local)"` with right-aligned `"Local only"` (`TERTIARY`).

---

## 6. Architecture & Lifecycle (`src/settings_win.rs` + `src/main.rs`)

### Module Split
Create `src/settings_win.rs` (declared as `mod settings_win;` in `src/main.rs`) to own:
- Layout constants (`SET_WIN_W`, `SET_WIN_H`, sidebar/titlebar/content geometry, hit code constants).
- `pub enum SettingsSection { Notch = 0, General = 1, Appearance = 2, Updates = 3 }`.
- `pub struct SettingsWindow`:
  - `hwnd: Hwnd` (lazily created on first `open()`), `open: bool`, `section: SettingsSection`
  - Offscreen DIB resources (`dc`, `bitmap`, `original`, `pixels`, `stride`, `w_px`, `h_px`, `scale`) and its own `render::Renderer`
  - Interaction state: `hover: i32`, `pressed: i32`, `blur_drag: bool`, `auto_switch_t: f64`, `auto_switch_v: f64`
  - Persistent window position `pos: Option<(i32, i32)>` saved on hide/move (in memory for the session)
- Pure, Win32-free hit-testing (`hit_test(&self, state: &State, x: f64, y: f64) -> i32`) and slider mapping (`blur_from_x(x: f64) -> u32`) so unit tests exercise all 4 sections without needing a live HWND.
- `paint` routine drawing into `SettingsWindow`'s DIB and calling `UpdateLayeredWindow` with `ULW_ALPHA` (premultiplied BGRA, antialiased rounded window silhouette).
- `settings_proc` Win32 window procedure (`WM_MOUSEMOVE`, `WM_MOUSELEAVE`, `WM_LBUTTONDOWN`, `WM_LBUTTONUP`, `WM_KEYDOWN` for `Esc`, `WM_CLOSE` → hide instead of quit, `WM_PAINT`).
- Offscreen DIB is sized once at `State.scale` (`(SET_WIN_W * scale).round()` × `(SET_WIN_H * scale).round()`), reusing `State.fonts` directly without per-monitor font tables or `WM_DPICHANGED` rebuilds.

### Integration with `src/main.rs`
- `State` holds `settings_win: settings_win::SettingsWindow` and removes `settings_subpage`.
- `State` keeps `main_hwnd: Hwnd` (or passes notch `hwnd` via `State`) so actions inside `settings_proc` (toggling a setting, changing background style/blur/opacity, clicking `Quit Wisp`, or installing an update) can immediately call `InvalidateRect(main_hwnd, ...)` / `layout(main_hwnd)` / `DestroyWindow(main_hwnd)`.
- `State::advance(dt)` advances `settings_win.auto_switch_t` alongside `switch_t[0..7]` and returns `true` while any switch is animating.
- In the `0x8001` compositor tick handler: if `s.settings_win.open` and (`animating` or updater polled an event), trigger `s.settings_win.paint(s)` (or `InvalidateRect(s.settings_win.hwnd, ...)`).
- Modal image picker (`open_image_dialog`): takes `owner: Hwnd` so when triggered from Settings popup it is owned by `settings_win.hwnd` (preventing Z-order inversion behind the topmost notch).
- `WM_DESTROY` of `WispNotch` calls `s.settings_win.destroy()` so the popup HWND and DIB resources are freed before `PostQuitMessage(0)`.
- `smoke` mode: never opens `SettingsWindow` (smoke test continues exercising notch expand/collapse, shelf, and mode transition cleanly).

---

## 7. Left to the Builder

- Exact pixel spacing/font-index selection within each `412 pt` content card (using existing `State.fonts` indices and `cached_text` helper refactored or shared cleanly between `main.rs` and `settings_win.rs`).
- Choosing the cleanest helper sharing between `src/main.rs` and `src/settings_win.rs` (e.g., making `Renderer`, `rgb`, `blend_color`, `pill_opt`/`round_opt`-style helpers, and `cached_text` reusable across both windows without duplicating cache logic).
- Exact stroke glyph paths for the 4 sidebar category icons (Notch, General/Gear, Appearance/Swatches, Updates/Arrow) using `Renderer` strokes on an 18 pt grid per `CLAUDE.md`.
- Updating existing unit tests (`test_appearance_settings_and_hit_testing`, `updater_settings_targets_stay_inside_the_updates_page`, `test_notification_tab_geometry_and_layout`) and adding new unit tests covering `TAB_COUNT == 7`, `HEADER_SETTINGS == 11`, sidebar navigation, all 4 section hit tests, blur slider mapping, and toggle/hide lifecycle.

---

## 8. Documentation & Verification Plan

- **Docs updated in the same PR:**
  - `docs/BEHAVIOR.md` — document standalone Settings popup (`620×420`), Notch header Gear toggle, 7-tab scroll cycle, and reorganized 4 categories.
  - `CLAUDE.md` — update Architecture (`TAB_COUNT = 7`, `HEADER_SETTINGS`, `src/settings_win.rs`, `SettingsWindow` lifecycle).
  - `CONTEXT.md` — add `src/settings_win.rs` to the file table and update Settings page note.
  - `README.md` — adjust short feature line if it still says "Settings right inside the notch".
  - `UNRELEASED.md` (git-ignored) — record the feature with author attribution for `v0.2.2`.
- **Verification before PR:**
  1. `cargo fmt --check`
  2. `cargo test --locked`
  3. `cargo build --release --locked`
  4. `target/release/wisp.exe --smoke-test` → `PASS` in `smoke-result.txt`
