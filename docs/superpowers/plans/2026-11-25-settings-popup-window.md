# Standalone Settings Popup Window Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move Wisp's Settings out of the notch (`TAB_SETTINGS`) into a standalone `620×420` Apple dark-mode floating popup window (`src/settings_win.rs`) with a 4-category sidebar (**Notch**, **General**, **Appearance**, **Updates**), toggled from the notch header Gear button or tray `"Settings…"`.

**Architecture:** Add a second borderless `WS_POPUP | WS_EX_LAYERED | WS_EX_TOOLWINDOW` Win32 window (`WispSettings`) on the existing UI thread sharing `*mut State` via `GWLP_USERDATA`, with its own premultiplied BGRA DIB and `render::Renderer` reusing `State.scale`, `State.fonts`, and `State.text_cache`. Shrink notch pages from `TAB_COUNT = 8` to `7` (`TAB_MUSIC..=TAB_NOTIF`) and turn the right-edge Gear icon into a dedicated header toggle action (`HEADER_SETTINGS = 11`). Keep Settings fully decoupled from notch expand/collapse state.

**Tech Stack:** Rust (edition 2021, `stable-x86_64-pc-windows-gnu`), raw Win32/GDI/GDI+ (`src/render.rs`, `UpdateLayeredWindow`), zero new Cargo dependencies.

**Spec:** `docs/superpowers/specs/2026-11-25-settings-popup-design.md`

## Global Constraints

- Windows 10/11 only; toolchain `stable-x86_64-pc-windows-gnu`; zero new Cargo dependencies.
- Visual language (`AGENTS.md`): Apple dark mode only — window `#141416`, sidebar/titlebar `#0e0e10`, grouped cards `#1c1c1e`, hairline dividers `#2c2c2e`, titlebar/sidebar dividers `#242426`, switches off `#39393d` / on system `GREEN`, active sidebar & primary CTA `BLUE`, labels `WHITE` / `SECONDARY` / `TERTIARY`. No card borders, no colored body text, and **Quit Wisp** uses neutral `WHITE` text on a dark pill (`#1c1c1e`, hover `#2c2c2e`, pressed `#36363a`).
- Fonts: reuse `State.fonts` (system SF Pro Display or bundled Inter); never draw GDI text directly onto a layered DIB — always blit via cached grayscale coverage masks (`cached_text_into`). No per-frame surface allocations.
- DPI: stay System-DPI-Aware (`SetProcessDPIAware()`), sharing `State.scale` and `State.fonts` (no Per-Monitor V2 upgrade or `WM_DPICHANGED` font tables).
- Decoupled notch: opening Settings or viewing **Appearance** never auto-expands or pins the notch open; clicking outside Settings keeps it open.
- Frame pacing: any active Settings switch spring keeps `PACE_MOTION`; idle open Settings does not spin CPU.
- Verification required before completion: `cargo fmt --check`, `cargo test --locked`, `cargo build --release --locked`, and `target/release/wisp.exe --smoke-test` (`PASS` in `smoke-result.txt`).

## Review Focus

1. **Closing Settings vs. quitting Wisp:** Clicking `✕`, pressing `Esc`, or receiving `WM_CLOSE` (`Alt+F4`) must hide `SettingsWindow` (`open = false`) without quitting Wisp; clicking **Quit Wisp** must destroy `WispNotch` and cleanly release `SettingsWindow` resources. → Pinned by Task 1 & Task 4 tests.
2. **Conditional hit targets stay inert when hidden/disabled:** Clicking the Blur slider area on `OLED`, `Reset` when no custom image exists, `Framing`/`Choose…` on non-Image styles when no image is loaded, `Cancel` when not downloading, or the Updates CTA while `Checking`/`Downloading` must return `-1`. → Pinned by Task 1 tests.
3. **Position clamping across multi-monitor / negative coordinates:** Reopening Settings after moving it near or outside a monitor edge (including negative virtual coordinates left/above primary) must clamp `(x, y)` inside the target work area so the titlebar stays reachable. → Pinned by Task 1 tests.
4. **Notch 7-tab boundary & Gear toggle isolation:** Wheel/swipe tab switching must clamp at `TAB_NOTIF` (`6`), never panicking or addressing removed `TAB_SETTINGS`; clicking `HEADER_SETTINGS` (`11`) must not modify `active_tab` or `expanded`. → Pinned by Task 2 & Task 4 tests.
5. **Blur slider drag release & section switch safety:** Dragging the Blur slider and releasing outside the window or switching sidebar sections must clamp `0..=30`, clear `blur_drag`, and avoid stuck capture. → Pinned by Task 1 & Task 3.

---

## File Structure

- **Create `src/settings_win.rs`:** Standalone `SettingsWindow`, Option A (`620×420`) layout constants, `SettingsSection` enum, pure Win32-free hit-testing (`hit_test`), blur coordinate mapping (`blur_from_x`), work-area clamping (`clamp_window_pos`), auto-update switch spring, DIB + `render::Renderer` management, GDI+ painting for titlebar/sidebar/4 sections, and `settings_proc` window procedure.
- **Modify `src/main.rs`:**
  - Declare `mod settings_win;`.
  - Expose `pub(crate)` shared types, colors, fonts/text cache helper (`cached_text_into` / `blend_mask_into`), Win32 FFI bindings, `SETTING_ROWS`, `spring`, `wide`, `update_note_lines`, and `State` helpers needed by `settings_win.rs`.
  - Reduce `TAB_COUNT` from `8` to `7`; add `HEADER_SETTINGS: i32 = 11` and `gear_rect(panel_w)` in the notch header.
  - Store `main_hwnd: Hwnd` and `settings_win: settings_win::SettingsWindow` on `State`; remove `settings_subpage`, `SET_W`/`SET_H`/`SET_CARD_*`, `settings_page`, `settings_background_page`, `settings_updates_page`, `settings_hit`, `settings_click`.
  - Wire Gear click, `tray::item::SETTINGS`, `State::advance(dt)`, `0x8001` repaint, `open_image_dialog(owner)` owner parameter, `WM_DESTROY` cleanup, and update unit tests.
- **Modify `docs/BEHAVIOR.md`, `CLAUDE.md`, `CONTEXT.md`, `README.md` (+ git-ignored `UNRELEASED.md`):** Document the standalone Settings popup, 7-tab notch header, Gear toggle, and `src/settings_win.rs`.

---

### Task 1: Pure `SettingsWindow` Layout, Hit-Testing, Clamping & Switch Spring (`src/settings_win.rs`)

**Files:**
- Create: `src/settings_win.rs`
- Modify: `src/main.rs` (add `mod settings_win;` only)

**Interfaces:**
- Consumes: `crate::spring`, `crate::tray::item`, `crate::SETTING_ROWS`
- Produces:
  - Constants: `SET_WIN_W = 620.`, `SET_WIN_H = 420.`, `TITLE_H = 40.`, `SIDEBAR_W = 168.`, `CONTENT_X = 188.`, `CONTENT_W = 412.`
  - Hit codes: `HIT_NONE = -1`, `HIT_TITLEBAR = 1`, `HIT_CLOSE = 2`, `HIT_QUIT = 5200`, `HIT_NAV_BASE = 5100` (`+0..4`), `HIT_SWITCH_BASE = 5000` (`+0..7`), `HIT_BG_STYLE_BASE = 5300` (`+0..5`), `HIT_BG_OPACITY_BASE = 5310` (`+0..4`), `HIT_BG_CHOOSE = 5320`, `HIT_BG_RESET = 5321`, `HIT_BG_POS_BASE = 5330` (`+0..3`), `HIT_BG_BLUR = 5340`, `HIT_UPD_ACTION = 5400`, `HIT_UPD_AUTO = 5401`, `HIT_UPD_CANCEL = 5402`
  - `pub enum SettingsSection { Notch = 0, General = 1, Appearance = 2, Updates = 3 }`
  - `pub struct HitContext { pub section: SettingsSection, pub has_custom_image: bool, pub bg_style: u32, pub update_action_enabled: bool, pub update_downloading: bool }`
  - `pub fn hit_test(ctx: HitContext, x: f64, y: f64) -> i32`
  - `pub fn blur_from_x(x: f64) -> u32`
  - `pub fn clamp_window_pos(x: i32, y: i32, win_w: i32, win_h: i32, work: (i32, i32, i32, i32)) -> (i32, i32)`
  - `pub fn advance_switch(pos: &mut f64, vel: &mut f64, target: bool, reduced_motion: bool, dt: f64) -> bool`

- [ ] **Step 1: Create `src/settings_win.rs` skeleton with unit tests first (TDD)**

Write the constants, `SettingsSection`, `HitContext`, stub functions returning placeholders (`HIT_NONE`, `0`, `(0, 0)`, `false`), and the `#[cfg(test)] mod tests` suite containing:
1. `titlebar_and_sidebar_hit_targets`: verifies `HIT_CLOSE` at `(596., 20.)`, `HIT_TITLEBAR` at `(200., 20.)`, `HIT_NAV_BASE + 0..4` for all 4 sidebar buttons, `HIT_QUIT` at `(84., 390.)`, and `HIT_NONE` outside `620×420`.
2. `notch_and_general_switch_hit_targets`: verifies `HIT_SWITCH_BASE + 0..=3` in `SettingsSection::Notch`, `HIT_SWITCH_BASE + 4..=6` in `SettingsSection::General`, and `HIT_NONE` for out-of-section switch areas.
3. `appearance_conditional_hit_targets_and_blur_slider`: verifies all 5 style tiles (`5300..=5304`), Opacity pills (`5310..=5313`), that `HIT_BG_BLUR` returns `5340` on `Frost` (`bg_style = 0`) and `HIT_NONE` on `OLED` (`bg_style = 3`), that `HIT_BG_CHOOSE`/`HIT_BG_RESET`/`HIT_BG_POS_BASE` appear on `Image` (`bg_style = 4`) with `Reset` hidden when `has_custom_image == false`, and that `blur_from_x` clamps left edge to `0`, midpoint to `15`, right edge to `30`, and beyond-bounds cleanly.
4. `updates_conditional_hit_targets`: verifies `HIT_UPD_AUTO` (`5401`), `HIT_UPD_ACTION` (`5400`) enabled vs disabled (`HIT_NONE` when `update_action_enabled == false`), and `HIT_UPD_CANCEL` (`5402`) only when `update_downloading == true`.
5. `clamp_window_pos_handles_primary_and_negative_secondary_monitors`: verifies centering/clamping inside `(0, 0, 1920, 1040)` and negative secondary monitor `(-1920, 0, 0, 1080)` plus oversized window fallback.
6. `auto_switch_spring_animates_and_snaps_on_reduced_motion`: verifies `advance_switch` moves smoothly when `reduced_motion == false` and snaps immediately to `0.0`/`1.0` with `vel == 0.` when `reduced_motion == true`.

Register `mod settings_win;` at the top of `src/main.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --locked settings_win::tests`
Expected: FAIL on assertions from placeholder stubs.

- [ ] **Step 3: Implement pure geometry, `hit_test`, `blur_from_x`, `clamp_window_pos`, and `advance_switch` in `src/settings_win.rs`**

Pin exact Option A geometry constants shared by both `hit_test` and Task 3's `paint`:
- Window: `SET_WIN_W = 620.`, `SET_WIN_H = 420.`, `TITLE_H = 40.`, `SIDEBAR_W = 168.`, `CONTENT_X = 188.`, `CONTENT_W = 412.`
- Close button: `(584., 8., 24., 24.)`
- Sidebar nav items `i in 0..4`: `x = 10..158`, `y = 50 + i * 38`, `h = 34`
- Sidebar Quit pill: `x = 12..156`, `y = 376..406`
- Notch (`4` rows) & General (`3` rows): card at `x = CONTENT_X`, `w = CONTENT_W`, `y0 = 120.`, `row_h = 54.` (room for 13 pt label + 11 pt subtitle + right-aligned `36×20` switch).
- Appearance:
  - Style tiles card (`y = 114., h = 68.`): 5 equal tiles across `CONTENT_W` with `8 pt` padding and `6 pt` gaps.
  - Options card (`y = 194.`): dynamic stacked rows (`row_h = 44.` for Image/Framing/Opacity, `54.` for Blur slider) computed by one shared helper `appearance_layout(has_custom_image: bool, bg_style: u32) -> AppearanceLayout` used by BOTH `hit_test` and `paint` so hit boxes and rendered controls can never drift!
  - Blur slider track: `x = CONTENT_X + 14. .. CONTENT_X + CONTENT_W - 14.` (`202. .. 586.`), mapped linearly to `0..=30`.
- Updates:
  - Card 1 (`y = 114.`): Row 1 (`y = 114..166`) **Check automatically** switch (`5401`); Row 2 (`y = 166..314`) status, optional progress bar, release notes, primary CTA pill (`x = 202..362, y = 272..302`, `5400`), optional Cancel pill (`x = 372..460, y = 272..302`, `5402`).
  - Card 2 (`y = 328..384`): non-interactive Troubleshooting log info card.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --locked settings_win::tests`
Expected: PASS (all 6 tests green).

- [ ] **Step 5: Commit**

```bash
git add src/settings_win.rs src/main.rs
git commit -m "feat(settings-win): add Option A geometry, hit testing, clamping, and unit tests"
```

---

### Task 2: Refactor Notch Header (`TAB_COUNT = 7` + `HEADER_SETTINGS = 11`) & Shared Text Blitting in `src/main.rs`

**Files:**
- Modify: `src/main.rs` (constants around lines 748–790, `tab_x`/`header_hit` around lines 1766–1825, `cached_text`/`blend_mask` around lines 4220–4286, header painting around lines 8764–8925, wheel tab switching around lines 10790–10810, tests around lines 12394–12420)

**Interfaces:**
- Consumes: `settings_win` module from Task 1
- Produces:
  - `pub(crate) const HEADER_SETTINGS: i32 = 11;`
  - `const TAB_COUNT: usize = 7;` (`TAB_MUSIC=0..=TAB_NOTIF=6`, `TAB_SETTINGS` removed from notch pages)
  - `fn gear_rect(panel_w: f64) -> (f64, f64)` returning `(panel_w - 24. - 34., 34.)`
  - `pub(crate) unsafe fn cached_text_into(dc: Handle, pixels: *mut u32, stride: usize, height: i32, text_cache: &std::cell::RefCell<TextCache>, ticks: u32, text: &[u16], rect: Rect, flags: u32)` (called by existing `cached_text` for the notch AND by `settings_win.rs` for the popup!)
  - `pub(crate)` visibility on shared FFI, structs (`Rect`, `Point`, `Size`, `Blend`, `BitmapInfo`, `WindowClass`, `Handle`, `Hwnd`), colors (`WHITE`, `SECONDARY`, `TERTIARY`, `BLUE`, `GREEN`, `rgb`, `blend_color`), `wide`, `spring`, `SETTING_ROWS`, `update_note_lines`.

- [ ] **Step 1: Write failing unit tests for 7-tab notch header & Gear action target**

Update `test_notification_tab_geometry_and_layout` and add `notch_header_gear_is_action_not_tab`:
```rust
#[test]
fn notch_header_gear_is_action_not_tab() {
    assert_eq!(TAB_COUNT, 7);
    let mut state = create_test_state();
    state.active_tab = TAB_NOTIF;
    state.tab_from = TAB_NOTIF;
    state.tab_to = TAB_NOTIF;
    state.panel_w = NOTIF_W;
    let (gx, gw) = gear_rect(state.panel_w_now());
    assert_eq!(state.header_hit(gx + gw * 0.5, 20.), HEADER_SETTINGS);
    let (nx, nw) = state.tab_x(TAB_NOTIF);
    assert_eq!(nx + nw + 4., gx);
    assert_eq!(state.header_hit(nx + nw * 0.5, 20.), TAB_NOTIF as i32);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --locked notch_header_gear_is_action_not_tab`
Expected: FAIL (`HEADER_SETTINGS` / `gear_rect` not found, `TAB_COUNT` is 8).

- [ ] **Step 3: Refactor `TAB_COUNT = 7`, `HEADER_SETTINGS = 11`, `gear_rect`, and `cached_text_into` in `src/main.rs`**

- Remove `TAB_SETTINGS` from `TAB_COUNT` (`7`), `page_w`, `page_h`, `tab_x` (only `0..TAB_COUNT`), and the `paint` page match arm; keep Gear icon painted at `gear_rect(cur_w)` with hover highlight (`s.tab_hover == HEADER_SETTINGS`) and open-state highlight (placeholder `false` until Task 4 wires `s.settings_win.is_open()`).
- In `header_hit`, check `gear_rect(self.panel_w_now())` and return `HEADER_SETTINGS` (`11`).
- In `WM_MOUSEMOVE` hover tracking (`tab_hover`) and `WM_LBUTTONUP` header click handling, recognize `HEADER_SETTINGS` without altering `active_tab` (Task 4 plugs in `s.settings_win.toggle(...)`).
- Extract `blend_mask_into` and `cached_text_into` from `blend_mask` / `cached_text` (thin wrapper keeps every existing call in `src/main.rs` unchanged) and add `pub(crate)` on the shared symbols listed in **Interfaces**.
- Temporarily gate or remove references to `TAB_SETTINGS` in `main.rs` so everything compiles cleanly (old `settings_hit`/`settings_click` tests are migrated in Task 4, so keep them compiling or update in Task 4 — do NOT leave broken build).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --locked test_notification_tab_geometry_and_layout notch_header_gear_is_action_not_tab`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/main.rs
git commit -m "refactor(notch): decouple Gear header action from 7-tab cycle and expose shared render primitives"
```

---

### Task 3: Implement `SettingsWindow` Win32 Surface, GDI+ Painting & `settings_proc` (`src/settings_win.rs`)

**Files:**
- Modify: `src/settings_win.rs`
- Modify: `src/main.rs` (add missing `MonitorFromPoint` / `GetMonitorInfoW` / `WM_NCLBUTTONDOWN` FFI declarations if not already present, and expose `State` helper methods needed by `settings_proc`: `change_setting`, `import_custom_bg`, `reset_custom_bg`, `reload_custom_bg`, `apply_settings`, `set_bg_blur`, `open_image_dialog`)

**Interfaces:**
- Consumes: `crate::render::Renderer`, `crate::cached_text_into`, `crate::State`, `crate::SETTING_ROWS`, `crate::update_note_lines`, `crate::updater::Phase`
- Produces:
  - `pub struct SettingsWindow` with methods:
    - `pub fn new() -> Self`
    - `pub fn is_open(&self) -> bool`
    - `pub fn section(&self) -> SettingsSection`
    - `pub fn set_section(&mut self, section: SettingsSection)`
    - `pub fn auto_switch_t(&self) -> f64`
    - `pub fn snap_auto_switch(&mut self, auto_on: bool)`
    - `pub fn advance(&mut self, auto_on: bool, reduced_motion: bool, dt: f64) -> bool`
    - `pub unsafe fn open(state: &mut State)`
    - `pub unsafe fn toggle(state: &mut State)`
    - `pub unsafe fn hide(&mut self)`
    - `pub unsafe fn paint(state: &State)`
    - `pub unsafe fn destroy(&mut self)`
    - `pub unsafe fn handle_click(state: &mut State, hit: i32)` (unit-testable action dispatcher for section switching, close/hide, blur drag release, and smoke-safe mutations!)

- [ ] **Step 1: Write failing unit tests for `SettingsWindow` state transitions & click dispatch**

Add unit tests in `src/settings_win.rs`:
1. `settings_window_section_switch_and_blur_drag_cleanup`: verifies default section is `Notch`, `set_section` updates section and clears `blur_drag` & `pressed`, and `hide` clears `open`, `hover`, `pressed`, `blur_drag`.
2. `settings_window_appearance_style_and_opacity_clicks_update_settings`: using `create_test_state()` (with `smoke = true` so disk/Win32 side effects are skipped), verifies clicking `HIT_BG_OPACITY_BASE + 1` sets `bg_opacity = 80`, clicking `HIT_BG_STYLE_BASE + 0` sets `bg_style = 0` (`Frost`), clicking `HIT_NAV_BASE + 2` switches section to `Appearance`, and clicking `HIT_CLOSE` sets `open = false` without touching `active_tab` or `expanded`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --locked settings_win::tests`
Expected: FAIL (`SettingsWindow` struct/methods not defined yet).

- [ ] **Step 3: Implement `SettingsWindow`, GDI+ painting, and `settings_proc` in `src/settings_win.rs`**

Implement cleanly following the spec:
- **Window class & creation:** Register `WispSettings` once (`OnceLock` / `AtomicBool`), create `WS_POPUP` (`0x80000000`) with `WS_EX_LAYERED | WS_EX_TOOLWINDOW` (`0x80000 | 0x80`), store `state as *mut State as isize` in `GWLP_USERDATA` (`-21`). Allocate DIB `(SET_WIN_W * scale).round() × (SET_WIN_H * scale).round()` and `render::Renderer::new(...)` once when created; free in `destroy()` and `Drop`.
- **Positioning:** On first `open`, center inside primary work area via `SystemParametersInfoW(0x0030 /* SPI_GETWORKAREA */, ...)`. On `hide` or `WM_EXITSIZEMOVE` (`0x0232`), save `(rect.left, rect.top)`. On subsequent `open`, clamp saved position via `MonitorFromPoint` (`MONITOR_DEFAULTTONEAREST = 2`) + `GetMonitorInfoW` (fallback `SPI_GETWORKAREA`) using `clamp_window_pos`. Never alter `State.expanded` or `State.active_tab`.
- **Painting (`paint`):** Clear DIB to transparent `0`, draw antialiased rounded window background (`r = 14.`, `#141416`), clip/fill titlebar + left sidebar (`#0e0e10`), draw hairline dividers (`#242426`), draw Wisp pill emblem + `"Wisp Settings"` + circular `✕` close button, draw 4 sidebar items with SF-style stroke icons on an 18 pt grid (`Notch`, `General`, `Appearance`, `Updates`) + active `BLUE` capsule + version footer + neutral `WHITE` `"Quit Wisp"` pill, and draw the active right-pane section per Spec Section 5.3. Present with `UpdateLayeredWindow` (`ULW_ALPHA = 2`, `AC_SRC_ALPHA = 1`).
- **Window procedure (`settings_proc`):**
  - `WM_MOUSEMOVE` (`0x200`): `TrackMouseEvent`, update `hover`; if `blur_drag`, update `settings.bg_blur = blur_from_x(x)`, call `apply_settings()`, repaint both Settings and notch (`InvalidateRect(main_hwnd)`).
  - `WM_MOUSELEAVE` (`0x2a3`): clear `hover`, repaint Settings.
  - `WM_LBUTTONDOWN` (`0x201`): hit-test; if `HIT_TITLEBAR` → `ReleaseCapture(); SendMessageW(hwnd, 0x00A1, 2, 0)` (native smooth OS window drag!); if `HIT_BG_BLUR` → set `blur_drag = true`, `SetCapture(hwnd)`, update `bg_blur`, repaint; otherwise set `pressed = hit`, repaint.
  - `WM_LBUTTONUP` (`0x202`): if `blur_drag` → `blur_drag = false`, `ReleaseCapture()`, `reload_custom_bg()` (if custom image loaded), `apply_settings()`, `save()`, repaint both; otherwise if `hit == pressed` dispatch `handle_click(state, hit)`.
  - `WM_CAPTURECHANGED` (`0x215`): if `blur_drag`, clear `blur_drag` and finalize `reload_custom_bg`/`save` cleanly so capture loss never leaves drag stuck.
  - `WM_KEYDOWN` (`0x100`): `VK_ESCAPE` (`0x1B`) → `hide()`.
  - `WM_CLOSE` (`0x0010`): `hide()` and return `0` (never destroy/quit Wisp).
  - `WM_PAINT` (`0x000F`): `BeginPaint`/`EndPaint` + `SettingsWindow::paint(state)`.
- **File picker owner:** Update `open_image_dialog(owner: Hwnd)` / `import_custom_bg(owner: Hwnd)` so when triggered from `SettingsWindow`, `owner` is `settings_win.hwnd` (centered above Settings, never hidden behind the topmost notch), and invalidate `main_hwnd` + repaint Settings after import/reset.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --locked settings_win::tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/settings_win.rs src/main.rs
git commit -m "feat(settings-win): implement native Win32 layered SettingsWindow, GDI+ painting, and message loop"
```

---

### Task 4: Wire `SettingsWindow` into `State`, Tray & Compositor Loop and Remove Legacy In-Notch Settings

**Files:**
- Modify: `src/main.rs` (`State` fields & init, `advance`, `0x8001` tick, `WM_LBUTTONUP` `HEADER_SETTINGS`, `tray::CALLBACK` `tray::item::SETTINGS`, `WM_DESTROY`, removal of old in-notch `settings_page`/`settings_background_page`/`settings_updates_page`/`settings_hit`/`settings_click`/`settings_subpage`, and unit test migration)

**Interfaces:**
- Consumes: `settings_win::SettingsWindow` API from Task 3
- Produces: Complete working standalone Settings popup integrated end-to-end with zero dead in-notch Settings code.

- [ ] **Step 1: Update & add integration unit tests in `src/main.rs`**

Replace the obsolete in-notch `settings_hit` assertions in `test_appearance_settings_and_hit_testing` and `updater_settings_targets_stay_inside_the_updates_page` with tests exercising `SettingsWindow::hit_test` + `State::settings_win`, and add `settings_popup_leaves_notch_decoupled`:
- Verifies toggling `SettingsWindow` (in smoke/test state) does not change `state.active_tab` or `state.expanded`.
- Verifies `State::advance(dt)` returns `true` while `settings_win` auto-update switch is springing and settles cleanly under `reduced_motion`.
- Verifies `State` no longer has `TAB_SETTINGS` and `TAB_COUNT == 7`.

- [ ] **Step 2: Wire `SettingsWindow` and delete legacy in-notch Settings code in `src/main.rs`**

- Add `pub(crate) main_hwnd: Hwnd` and `pub(crate) settings_win: settings_win::SettingsWindow` to `State` (set `main_hwnd = hwnd` in `State::new`, `null_mut()` in `create_test_state`).
- Remove `settings_subpage`, `SET_W`, `SET_H`, `SET_CARD_Y`, `SET_ROW_H`, `SET_CARD_W`, `setting_row`, `settings_page`, `settings_background_page`, `settings_updates_page`, `settings_hit`, `settings_click`, `set_blur_x`, and `blur_drag` from `State` (now owned by `SettingsWindow`).
- In notch `WM_LBUTTONUP` (`header_hit == HEADER_SETTINGS`): call `settings_win::SettingsWindow::toggle(s)` and `InvalidateRect(hwnd, null(), 0)`.
- In notch `paint`: highlight the Gear button subtly (`pill_opt` + `WHITE` icon) when `s.settings_win.is_open()` or hovered.
- In `tray::CALLBACK` (`tray::item::SETTINGS`): refresh `s.autostart = settings::starts_with_windows()`, call `settings_win::SettingsWindow::open(s)` — do **not** open or hold the notch (`expanded` / `alert_until` untouched).
- In `State::advance(dt)`: call `let auto_sw = self.settings_win.advance(self.updater.auto, self.reduced_motion, dt);` and include `|| auto_sw` in the return boolean (so `PACE_MOTION` drives smooth switch animation in the popup!).
- In `0x8001` frame tick: if `s.settings_win.is_open()` and (`animating` or `s.updater.poll()` returned `true`), call `settings_win::SettingsWindow::paint(s)`! Also ensure `if s.settings_win.is_open() && s.updater.phase == updater::Phase::Downloading` keeps pace at least `PACE_AMBIENT` so progress bar stays fluid.
- In `WM_DESTROY` (`0x0002`) of `WispNotch`: call `s.settings_win.destroy()` before cleaning up notch resources and calling `PostQuitMessage(0)`.
- In `WM_SETTINGCHANGE` (`0x001A`) / `apply_settings`: if `s.settings_win.is_open()`, repaint Settings window so `reduced_motion` / switch state stays synced.

- [ ] **Step 3: Run full unit tests, release build, and smoke test**

Run:
```powershell
cargo fmt --check
cargo test --locked
cargo build --release --locked
```
Then run `target/release/wisp.exe --smoke-test` (wait for exit) and read `smoke-result.txt`.
Expected: `cargo fmt --check` clean, all unit tests PASS (0 failed), release build succeeds with zero warnings, `smoke-result.txt` starts with `PASS`.

- [ ] **Step 4: Commit**

```bash
git add src/main.rs src/settings_win.rs
git commit -m "feat(settings): move Settings out of notch into standalone popup window"
```

---

### Task 5: Update Docs (`BEHAVIOR.md`, `CLAUDE.md`, `CONTEXT.md`, `README.md`, `UNRELEASED.md`) & Final Verification

**Files:**
- Modify: `docs/BEHAVIOR.md`
- Modify: `CLAUDE.md`
- Modify: `CONTEXT.md`
- Modify: `README.md`
- Modify: `UNRELEASED.md` (git-ignored)

**Interfaces:**
- Consumes: Final behavior and symbols from Tasks 1–4.
- Produces: Synchronized documentation per `AGENTS.md` ("When you change behaviour").

- [ ] **Step 1: Update documentation**

- `docs/BEHAVIOR.md`: Update Settings section to describe the standalone `620×420` popup window (4 sidebar categories: Notch, General, Appearance, Updates), Gear icon toggle on the notch header, 7-page notch scroll cycle, tray `"Settings…"` trigger, stay-open-on-outside-click behavior, and `✕`/`Esc` hiding.
- `CLAUDE.md`: Update Architecture (`TAB_COUNT = 7`, `HEADER_SETTINGS = 11`, `src/settings_win.rs`, `SettingsWindow` lifecycle, shared `cached_text_into`).
- `CONTEXT.md`: Add `src/settings_win.rs` to the module table and update the Settings page overview.
- `README.md`: Update the Settings bullet from `"Settings right inside the notch"` to `"Settings popup window"` (keeping README short).
- `UNRELEASED.md` (git-ignored): Add the standalone Settings popup feature with author attribution for `v0.2.2`.

- [ ] **Step 2: Run final verification checklist**

Run:
```powershell
cargo fmt --check
cargo test --locked
cargo build --release --locked
```
Run `target/release/wisp.exe --smoke-test`, wait for exit, and verify `smoke-result.txt` begins with `PASS`.

- [ ] **Step 3: Commit docs**

```bash
git add docs/BEHAVIOR.md CLAUDE.md CONTEXT.md README.md
git commit -m "docs(settings): document standalone Settings popup window and 7-tab notch header"
```
