//! Standalone Settings popup window (`620×420`, Option A macOS System Settings sidebar).
//! Pure geometry, hit-testing, position clamping, and switch springs live alongside the Win32
//! layered-window implementation so every layout boundary is unit-tested without an HWND.

use std::{
    mem::size_of,
    ptr::null_mut,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::{
    blend_color, cached_text_into, render, rgb, settings, update_note_lines, updater, wide,
    BeginPaint, BitmapInfo, Blend, CreateCompatibleDC, CreateDIBSection, CreateWindowExW,
    DefWindowProcW, DeleteDC, DeleteObject, DestroyWindow, EndPaint, GetModuleHandleW,
    GetWindowLongPtrW, GetWindowRect, Handle, Hwnd, InvalidateRect, LoadCursorW, MouseTracking,
    Paint, Point, PostMessageW, Rect, RegisterClassExW, ReleaseCapture, SelectObject, SendMessageW,
    SetBkMode, SetCapture, SetForegroundWindow, SetTextColor, SetWindowLongPtrW, SetWindowPos,
    ShowWindow, Size, State, SystemParametersInfoW, TrackMouseEvent, UpdateLayeredWindow,
    WindowClass, BLUE, GREEN, PACE_MOTION, SECONDARY, SETTING_ROWS, TERTIARY, WHITE,
    WM_SETTINGS_IMPORT_BG,
};

#[repr(C)]
struct MonitorInfo {
    size: u32,
    monitor: Rect,
    work: Rect,
    flags: u32,
}

#[link(name = "user32")]
extern "system" {
    fn MonitorFromPoint(pt: Point, flags: u32) -> Handle;
    fn GetMonitorInfoW(monitor: Handle, info: *mut MonitorInfo) -> i32;
}

const WIN_BG: u32 = 0x161414; // rgb(20, 20, 22) (#141416)
const SIDEBAR_BG: u32 = 0x100e0e; // rgb(14, 14, 16) (#0e0e10)
const CARD_BG: u32 = 0x1e1c1c; // rgb(28, 28, 30) (#1c1c1e)
const DIVIDER: u32 = 0x262424; // rgb(36, 36, 38) (#242426)
const ROW_SEP: u32 = 0x2e2c2c; // rgb(44, 44, 46) (#2c2c2e)

const NOTCH_ROWS: [(&str, &str); 4] = [
    (
        "Open on hover",
        "Expand the panel when the pointer reaches the top edge",
    ),
    (
        "Minimal mode",
        "Shrink to a thin sliver when covering browser tabs or content",
    ),
    (
        "Lyrics when closed",
        "Show the live lyric line inside the collapsed notch pill",
    ),
    (
        "Notification popups",
        "Briefly spring open for incoming Windows notifications",
    ),
];

const GENERAL_ROWS: [(&str, &str); 3] = [
    (
        "24-hour clock",
        "Show 14:25 instead of 2:25 PM in the collapsed notch",
    ),
    (
        "Match Windows motion",
        "Snap animations when Windows animation effects are off",
    ),
    (
        "Start with Windows",
        "Launch Wisp automatically when you sign in",
    ),
];

const NAV_LABELS: [&str; 4] = ["Notch", "General", "Appearance", "Updates"];
const BG_STYLE_NAMES: [&str; 5] = ["Frost", "Aura", "Slate", "OLED", "Image"];
const BG_SWATCH_COLORS: [u32; 5] = [
    0x342d28, // Frost cool steel
    0x38212a, // Aura subtle plum
    0x2d2927, // Slate charcoal
    0x000000, // OLED pure black
    0x684b26, // Image blue-slate preview
];

static CLASS_REGISTERED: AtomicBool = AtomicBool::new(false);

pub struct SettingsWindow {
    hwnd: Hwnd,
    open: bool,
    section: SettingsSection,
    dc: Handle,
    bitmap: Handle,
    original: Handle,
    pixels: *mut u32,
    stride: usize,
    w_px: i32,
    h_px: i32,
    renderer: Option<render::Renderer>,
    hover: i32,
    pressed: i32,
    blur_drag: bool,
    open_t: f64,
    open_v: f64,
    nav_y: f64,
    nav_v: f64,
    section_t: f64,
    section_v: f64,
    section_dir: f64,
    style_t: f64,
    style_v: f64,
    opacity_t: f64,
    opacity_v: f64,
    framing_t: f64,
    framing_v: f64,
    auto_switch_t: f64,
    auto_switch_v: f64,
    pos: Option<(i32, i32)>,
}

impl SettingsWindow {
    pub fn new() -> Self {
        Self {
            hwnd: null_mut(),
            open: false,
            section: SettingsSection::Notch,
            dc: null_mut(),
            bitmap: null_mut(),
            original: null_mut(),
            pixels: null_mut(),
            stride: 0,
            w_px: 0,
            h_px: 0,
            renderer: None,
            hover: HIT_NONE,
            pressed: HIT_NONE,
            blur_drag: false,
            open_t: 0.,
            open_v: 0.,
            nav_y: NAV_Y0,
            nav_v: 0.,
            section_t: 1.,
            section_v: 0.,
            section_dir: 1.,
            style_t: 3.,
            style_v: 0.,
            opacity_t: 3.,
            opacity_v: 0.,
            framing_t: 1.,
            framing_v: 0.,
            auto_switch_t: 0.,
            auto_switch_v: 0.,
            pos: None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn is_visible(&self) -> bool {
        self.open || self.open_t > 0.004
    }

    pub fn hwnd(&self) -> Hwnd {
        self.hwnd
    }

    #[cfg(test)]
    pub fn section(&self) -> SettingsSection {
        self.section
    }

    pub fn set_section(&mut self, section: SettingsSection) {
        if self.blur_drag {
            self.blur_drag = false;
            if !self.hwnd.is_null() {
                unsafe {
                    ReleaseCapture();
                }
            }
        }
        if section != self.section {
            self.section_dir = if (section as usize) > (self.section as usize) {
                1.
            } else {
                -1.
            };
            self.section_t = 0.;
            self.section_v = 0.;
        }
        self.section = section;
        self.pressed = HIT_NONE;
    }

    #[cfg(test)]
    pub fn auto_switch_t(&self) -> f64 {
        self.auto_switch_t
    }

    pub fn snap_auto_switch(&mut self, auto_on: bool) {
        self.auto_switch_t = if auto_on { 1. } else { 0. };
        self.auto_switch_v = 0.;
    }

    fn snap_visuals(&mut self, bg_style: u32, bg_opacity: u32, bg_pos: u32) {
        self.nav_y = NAV_Y0 + self.section as usize as f64 * NAV_STEP;
        self.nav_v = 0.;
        self.section_t = 1.;
        self.section_v = 0.;
        self.style_t = style_index(bg_style);
        self.style_v = 0.;
        self.opacity_t = opacity_index(bg_opacity);
        self.opacity_v = 0.;
        self.framing_t = framing_index(bg_pos);
        self.framing_v = 0.;
    }

    pub fn advance(
        &mut self,
        auto_on: bool,
        bg_style: u32,
        bg_opacity: u32,
        bg_pos: u32,
        reduced_motion: bool,
        dt: f64,
    ) -> bool {
        let mut moving = advance_switch(
            &mut self.auto_switch_t,
            &mut self.auto_switch_v,
            auto_on,
            reduced_motion,
            dt,
        );
        let open_goal = if self.open { 1. } else { 0. };
        let nav_goal = NAV_Y0 + self.section as usize as f64 * NAV_STEP;
        let style_goal = style_index(bg_style);
        let opacity_goal = opacity_index(bg_opacity);
        let framing_goal = framing_index(bg_pos);

        if reduced_motion {
            if self.open_t != open_goal || self.open_v != 0. {
                self.open_t = open_goal;
                self.open_v = 0.;
                moving = true;
            }
            if self.nav_y != nav_goal || self.nav_v != 0. {
                self.nav_y = nav_goal;
                self.nav_v = 0.;
                moving = true;
            }
            if self.section_t != 1. || self.section_v != 0. {
                self.section_t = 1.;
                self.section_v = 0.;
                moving = true;
            }
            if self.style_t != style_goal || self.style_v != 0. {
                self.style_t = style_goal;
                self.style_v = 0.;
                moving = true;
            }
            if self.opacity_t != opacity_goal || self.opacity_v != 0. {
                self.opacity_t = opacity_goal;
                self.opacity_v = 0.;
                moving = true;
            }
            if self.framing_t != framing_goal || self.framing_v != 0. {
                self.framing_t = framing_goal;
                self.framing_v = 0.;
                moving = true;
            }
        } else {
            moving |= crate::spring(&mut self.open_t, &mut self.open_v, open_goal, 24., 0.88, dt);
            if self.is_visible() {
                moving |= crate::spring(&mut self.nav_y, &mut self.nav_v, nav_goal, 24., 0.85, dt);
                moving |=
                    crate::spring(&mut self.section_t, &mut self.section_v, 1., 22., 0.88, dt);
                moving |= crate::spring(
                    &mut self.style_t,
                    &mut self.style_v,
                    style_goal,
                    24.,
                    0.85,
                    dt,
                );
                moving |= crate::spring(
                    &mut self.opacity_t,
                    &mut self.opacity_v,
                    opacity_goal,
                    24.,
                    0.85,
                    dt,
                );
                moving |= crate::spring(
                    &mut self.framing_t,
                    &mut self.framing_v,
                    framing_goal,
                    24.,
                    0.85,
                    dt,
                );
            }
        }

        if !self.open && self.open_t <= 0.004 {
            self.open_t = 0.;
            self.open_v = 0.;
            if !self.hwnd.is_null() {
                unsafe {
                    ShowWindow(self.hwnd, 0);
                }
            }
        }
        moving
    }

    unsafe fn work_area_for(pt: Option<(i32, i32)>) -> (i32, i32, i32, i32) {
        if let Some((x, y)) = pt {
            let mon = MonitorFromPoint(Point { x, y }, 2);
            if !mon.is_null() {
                let mut info = MonitorInfo {
                    size: size_of::<MonitorInfo>() as u32,
                    monitor: Rect::default(),
                    work: Rect::default(),
                    flags: 0,
                };
                if GetMonitorInfoW(mon, &mut info) != 0 {
                    return (
                        info.work.left,
                        info.work.top,
                        info.work.right,
                        info.work.bottom,
                    );
                }
            }
        }
        let mut work = Rect::default();
        if SystemParametersInfoW(0x0030, 0, (&mut work as *mut Rect).cast(), 0) != 0 {
            return (work.left, work.top, work.right, work.bottom);
        }
        (0, 0, 1920, 1040)
    }

    unsafe fn ensure_window(state: &mut State) -> bool {
        if !state.settings_win.hwnd.is_null() {
            return true;
        }
        let instance = GetModuleHandleW(std::ptr::null());
        let class = wide("WispSettings");
        if !CLASS_REGISTERED.swap(true, Ordering::AcqRel) {
            let wc = WindowClass {
                size: size_of::<WindowClass>() as u32,
                style: 3,
                proc: settings_proc,
                cls_extra: 0,
                wnd_extra: 0,
                instance,
                icon: null_mut(),
                cursor: LoadCursorW(null_mut(), 32512usize as *const u16),
                background: null_mut(),
                menu: std::ptr::null(),
                name: class.as_ptr(),
                small_icon: null_mut(),
            };
            if RegisterClassExW(&wc) == 0 {
                CLASS_REGISTERED.store(false, Ordering::Release);
                return false;
            }
        }
        let scale = state.scale;
        let w_px = (SET_WIN_W * scale).round() as i32;
        let h_px = (SET_WIN_H * scale).round() as i32;
        let work = Self::work_area_for(None);
        let init_x = work.0 + ((work.2 - work.0) - w_px) / 2;
        let init_y = work.1 + ((work.3 - work.1) - h_px) / 2;
        let (x, y) = clamp_window_pos(init_x, init_y, w_px, h_px, work);
        let hwnd = CreateWindowExW(
            0x80000 | 0x80,
            class.as_ptr(),
            wide("Wisp Settings").as_ptr(),
            0x80000000,
            x,
            y,
            w_px,
            h_px,
            null_mut(),
            null_mut(),
            instance,
            null_mut(),
        );
        if hwnd.is_null() {
            return false;
        }
        let dc = CreateCompatibleDC(null_mut());
        let stride = w_px as usize;
        let info = BitmapInfo {
            size: 40,
            width: w_px,
            height: -h_px,
            planes: 1,
            bits: 32,
            compression: 0,
            bytes: 0,
            x: 0,
            y: 0,
            used: 0,
            important: 0,
        };
        let mut pixels = null_mut();
        let bitmap = CreateDIBSection(dc, &info, 0, &mut pixels, null_mut(), 0);
        if bitmap.is_null() || pixels.is_null() {
            DeleteDC(dc);
            DestroyWindow(hwnd);
            return false;
        }
        let original = SelectObject(dc, bitmap);
        let renderer = Some(render::Renderer::new(pixels.cast(), stride, h_px, scale));
        SetWindowLongPtrW(hwnd, -21, (state as *mut State) as isize);
        let sw = &mut state.settings_win;
        sw.hwnd = hwnd;
        sw.dc = dc;
        sw.bitmap = bitmap;
        sw.original = original;
        sw.pixels = pixels.cast();
        sw.stride = stride;
        sw.w_px = w_px;
        sw.h_px = h_px;
        sw.renderer = renderer;
        sw.pos = Some((x, y));
        true
    }

    pub unsafe fn open(state: &mut State) {
        if state.smoke {
            state.settings_win.open = true;
            state.settings_win.open_t = 1.;
            state.settings_win.open_v = 0.;
            state.settings_win.snap_auto_switch(state.updater.auto);
            state.settings_win.snap_visuals(
                state.settings.bg_style,
                state.settings.bg_opacity,
                state.settings.bg_pos,
            );
            return;
        }
        let was_visible = state.settings_win.is_visible();
        state.autostart = settings::starts_with_windows();
        state.snap_switches();
        state.settings_win.snap_auto_switch(state.updater.auto);
        state.settings_win.snap_visuals(
            state.settings.bg_style,
            state.settings.bg_opacity,
            state.settings.bg_pos,
        );
        if !Self::ensure_window(state) {
            return;
        }
        let reduced = state.reduced_motion;
        let sw = &mut state.settings_win;
        let work = Self::work_area_for(sw.pos.map(|p| window_anchor_point(p, sw.w_px, sw.h_px)));
        let (x, y) = match sw.pos {
            Some((px, py)) => clamp_window_pos(px, py, sw.w_px, sw.h_px, work),
            None => clamp_window_pos(
                work.0 + ((work.2 - work.0) - sw.w_px) / 2,
                work.1 + ((work.3 - work.1) - sw.h_px) / 2,
                sw.w_px,
                sw.h_px,
                work,
            ),
        };
        sw.pos = Some((x, y));
        sw.open = true;
        if reduced {
            sw.open_t = 1.;
            sw.open_v = 0.;
        } else if !was_visible {
            sw.open_t = 0.01;
            sw.open_v = 0.;
        }
        sw.hover = HIT_NONE;
        sw.pressed = HIT_NONE;
        let win_hwnd = sw.hwnd;
        SetWindowPos(win_hwnd, null_mut(), x, y, sw.w_px, sw.h_px, 0x0004);
        Self::paint(state);
        ShowWindow(win_hwnd, 5);
        SetForegroundWindow(win_hwnd);
        state.pace.store(PACE_MOTION, Ordering::Relaxed);
        if !state.main_hwnd.is_null() {
            InvalidateRect(state.main_hwnd, std::ptr::null(), 0);
        }
    }

    pub unsafe fn toggle(state: &mut State) {
        if state.settings_win.open {
            Self::hide_with_motion(state);
            if !state.main_hwnd.is_null() {
                InvalidateRect(state.main_hwnd, std::ptr::null(), 0);
            }
        } else {
            Self::open(state);
        }
    }

    pub unsafe fn hide_with_motion(state: &mut State) {
        let reduced = state.reduced_motion || state.smoke;
        if reduced {
            state.settings_win.hide();
        } else {
            state.settings_win.begin_hide();
            state.pace.store(PACE_MOTION, Ordering::Relaxed);
        }
    }

    pub unsafe fn begin_hide(&mut self) {
        if self.blur_drag {
            self.blur_drag = false;
            if !self.hwnd.is_null() {
                ReleaseCapture();
            }
        }
        if !self.hwnd.is_null() && self.open_t >= 0.999 {
            let mut r = Rect::default();
            if GetWindowRect(self.hwnd, &mut r) != 0 {
                self.pos = Some((r.left, r.top));
            }
        }
        self.open = false;
        self.hover = HIT_NONE;
        self.pressed = HIT_NONE;
        if self.hwnd.is_null() {
            self.open_t = 0.;
            self.open_v = 0.;
        }
    }

    pub unsafe fn hide(&mut self) {
        if self.blur_drag {
            self.blur_drag = false;
            if !self.hwnd.is_null() {
                ReleaseCapture();
            }
        }
        if !self.hwnd.is_null() {
            if self.open_t >= 0.999 {
                let mut r = Rect::default();
                if GetWindowRect(self.hwnd, &mut r) != 0 {
                    self.pos = Some((r.left, r.top));
                }
            }
            ShowWindow(self.hwnd, 0);
        }
        self.open = false;
        self.open_t = 0.;
        self.open_v = 0.;
        self.hover = HIT_NONE;
        self.pressed = HIT_NONE;
    }

    pub unsafe fn destroy(&mut self) {
        self.open = false;
        self.blur_drag = false;
        self.renderer = None;
        if !self.hwnd.is_null() {
            SetWindowLongPtrW(self.hwnd, -21, 0);
            DestroyWindow(self.hwnd);
            self.hwnd = null_mut();
        }
        if !self.dc.is_null() {
            if !self.original.is_null() {
                SelectObject(self.dc, self.original);
                self.original = null_mut();
            }
            if !self.bitmap.is_null() {
                DeleteObject(self.bitmap);
                self.bitmap = null_mut();
            }
            DeleteDC(self.dc);
            self.dc = null_mut();
        }
        self.pixels = null_mut();
    }

    fn hit_context(state: &State) -> HitContext {
        HitContext {
            section: state.settings_win.section,
            has_custom_image: state.bg_custom_pixels.is_some(),
            bg_style: state.settings.bg_style,
            update_action_enabled: state.updater.action_enabled(),
            update_downloading: state.updater.phase == updater::Phase::Downloading,
        }
    }

    pub unsafe fn handle_click(state: &mut State, hit: i32) {
        let main_hwnd = state.main_hwnd;
        match hit {
            HIT_CLOSE => {
                Self::hide_with_motion(state);
                if !main_hwnd.is_null() {
                    InvalidateRect(main_hwnd, std::ptr::null(), 0);
                }
                return;
            }
            HIT_QUIT => {
                state.settings_win.hide();
                if !main_hwnd.is_null() {
                    PostMessageW(main_hwnd, 0x0010, 0, 0);
                }
                return;
            }
            h if (HIT_NAV_BASE..HIT_NAV_BASE + 4).contains(&h) => {
                state
                    .settings_win
                    .set_section(SettingsSection::from_index((h - HIT_NAV_BASE) as usize));
            }
            h if (HIT_SWITCH_BASE..HIT_SWITCH_BASE + 7).contains(&h) => {
                let item = SETTING_ROWS[(h - HIT_SWITCH_BASE) as usize].1;
                state.change_setting(main_hwnd, item);
            }
            h if (HIT_BG_STYLE_BASE..HIT_BG_STYLE_BASE + 5).contains(&h) => {
                let style = (h - HIT_BG_STYLE_BASE) as u32;
                if style == 4 && state.bg_custom_pixels.is_none() {
                    if !state.smoke && !main_hwnd.is_null() {
                        PostMessageW(main_hwnd, WM_SETTINGS_IMPORT_BG, 0, 0);
                    }
                } else {
                    state.settings.bg_style = style;
                    state.apply_settings();
                    if !state.smoke {
                        state.settings.save();
                    }
                    if !main_hwnd.is_null() {
                        InvalidateRect(main_hwnd, std::ptr::null(), 0);
                    }
                }
            }
            h if (HIT_BG_OPACITY_BASE..HIT_BG_OPACITY_BASE + 4).contains(&h) => {
                let opacities = [70, 80, 90, 100];
                state.settings.bg_opacity = opacities[(h - HIT_BG_OPACITY_BASE) as usize];
                if state.bg_custom_pixels.is_some() {
                    state.reload_custom_bg();
                }
                state.apply_settings();
                if !state.smoke {
                    state.settings.save();
                }
                if !main_hwnd.is_null() {
                    InvalidateRect(main_hwnd, std::ptr::null(), 0);
                }
            }
            HIT_BG_CHOOSE => {
                if !state.smoke && !main_hwnd.is_null() {
                    PostMessageW(main_hwnd, WM_SETTINGS_IMPORT_BG, 0, 0);
                }
            }
            HIT_BG_RESET => {
                if !state.smoke {
                    state.reset_custom_bg(main_hwnd);
                }
            }
            h if (HIT_BG_POS_BASE..HIT_BG_POS_BASE + 3).contains(&h) => {
                state.settings.bg_pos = match h - HIT_BG_POS_BASE {
                    0 => 1,
                    1 => 0,
                    _ => 2,
                };
                if state.bg_custom_pixels.is_some() {
                    state.reload_custom_bg();
                }
                state.apply_settings();
                if !state.smoke {
                    state.settings.save();
                }
                if !main_hwnd.is_null() {
                    InvalidateRect(main_hwnd, std::ptr::null(), 0);
                }
            }
            HIT_UPD_AUTO => {
                if !state.smoke {
                    state.updater.toggle_auto();
                }
            }
            HIT_UPD_CANCEL => {
                state.updater.cancel();
            }
            HIT_UPD_ACTION if !state.smoke && state.updater.action_enabled() => {
                match state.updater.phase {
                    updater::Phase::Ready => match state.updater.install() {
                        Ok(()) => {
                            state.settings_win.hide();
                            if !main_hwnd.is_null() {
                                PostMessageW(main_hwnd, 0x0010, 0, 0);
                            }
                            return;
                        }
                        Err(e) => state.updater.status = e,
                    },
                    updater::Phase::Available => state.updater.download(),
                    updater::Phase::Error if state.updater.release.is_some() => {
                        state.updater.download()
                    }
                    _ => state.updater.check(),
                }
            }
            _ => {}
        }
        state.pace.store(PACE_MOTION, Ordering::Relaxed);
        if state.settings_win.is_open() && !state.settings_win.hwnd.is_null() {
            Self::paint(state);
        }
    }

    pub unsafe fn paint(state: &State) {
        let sw = &state.settings_win;
        if sw.hwnd.is_null() || sw.pixels.is_null() {
            return;
        }
        let Some(r) = sw.renderer.as_ref() else {
            return;
        };
        let scale = state.scale;
        let px = |v: f64| (v * scale).round() as i32;
        let total = sw.stride * sw.h_px as usize;
        std::ptr::write_bytes(sw.pixels, 0, total);
        SetBkMode(sw.dc, 1);

        let draw_text = |font_idx: usize,
                         text: &str,
                         color: u32,
                         x: f64,
                         y: f64,
                         w: f64,
                         h: f64,
                         center: bool| {
            let mut buf = state.text_buffer.borrow_mut();
            buf.clear();
            buf.extend(text.encode_utf16());
            SelectObject(sw.dc, state.fonts[font_idx]);
            SetTextColor(sw.dc, color);
            let rect = Rect {
                left: px(x),
                top: px(y),
                right: px(x + w),
                bottom: px(y + h),
            };
            let flags = TEXT_DRAW_FLAGS | if center { 1 } else { 0 };
            cached_text_into(
                sw.dc,
                sw.pixels,
                sw.stride,
                sw.h_px,
                &state.text_cache,
                state.ticks,
                &buf,
                rect,
                flags,
            );
        };

        // 1. Rounded window base (#141416) + top titlebar & left sidebar (#0e0e10)
        r.round(0., 0., SET_WIN_W, SET_WIN_H, 14., WIN_BG);
        // Sidebar rounded bottom-left + titlebar rounded top corners via layered fills
        r.round(0., 0., SET_WIN_W, TITLE_H + 14., 14., SIDEBAR_BG);
        r.round(0., TITLE_H, SET_WIN_W, 14., 0., WIN_BG);
        r.round(0., TITLE_H, SIDEBAR_W, SET_WIN_H - TITLE_H, 14., SIDEBAR_BG);
        r.round(
            SIDEBAR_W - 14.,
            TITLE_H,
            14.,
            SET_WIN_H - TITLE_H,
            0.,
            SIDEBAR_BG,
        );
        r.round(0., TITLE_H, 14., 14., 0., SIDEBAR_BG);
        // Subtle structural hairlines
        r.round(0., TITLE_H - 1., SET_WIN_W, 1., 0., DIVIDER);
        r.round(
            SIDEBAR_W - 1.,
            TITLE_H,
            1.,
            SET_WIN_H - TITLE_H,
            0.,
            DIVIDER,
        );

        // 2. Titlebar: Wisp pill emblem, "Wisp Settings", circular Close button
        r.round(16., 15., 20., 10., 5., WHITE);
        r.ellipse(31.5, 20., 2., 2., SIDEBAR_BG);
        draw_text(7, "Wisp Settings", WHITE, 44., 10., 180., 20., false);

        let close_hot = sw.hover == HIT_CLOSE || sw.pressed == HIT_CLOSE;
        if close_hot {
            r.round(
                CLOSE_X,
                CLOSE_Y,
                CLOSE_SIZE,
                CLOSE_SIZE,
                CLOSE_SIZE / 2.,
                if sw.pressed == HIT_CLOSE {
                    rgb(54, 54, 58)
                } else {
                    rgb(44, 44, 46)
                },
            );
        }
        let cx = CLOSE_X + CLOSE_SIZE / 2.;
        let cy = CLOSE_Y + CLOSE_SIZE / 2.;
        let ccol = if close_hot { WHITE } else { SECONDARY };
        r.stroke(&[(cx - 4., cy - 4.), (cx + 4., cy + 4.)], false, 1.6, ccol);
        r.stroke(&[(cx + 4., cy - 4.), (cx - 4., cy + 4.)], false, 1.6, ccol);

        // 3. Left sidebar: hover pills, gliding blue active pill, 4 categories + footer
        for i in 0..NAV_LABELS.len() {
            let ny = NAV_Y0 + i as f64 * NAV_STEP;
            let hit = HIT_NAV_BASE + i as i32;
            let active = sw.section as usize == i;
            if !active && (sw.pressed == hit || sw.hover == hit) {
                r.round(
                    NAV_X,
                    ny,
                    NAV_W,
                    NAV_H,
                    8.,
                    if sw.pressed == hit {
                        rgb(36, 36, 40)
                    } else {
                        CARD_BG
                    },
                );
            }
        }
        r.round(NAV_X, sw.nav_y, NAV_W, NAV_H, 8., BLUE);
        for (i, label) in NAV_LABELS.iter().enumerate() {
            let ny = NAV_Y0 + i as f64 * NAV_STEP;
            let hit = HIT_NAV_BASE + i as i32;
            let active = sw.section as usize == i;
            let hot = sw.hover == hit;
            let proximity = (1. - ((sw.nav_y - ny).abs() / NAV_STEP)).clamp(0., 1.);
            let fg = if hot {
                WHITE
            } else {
                blend_color(SECONDARY, WHITE, proximity)
            };
            draw_sidebar_icon(r, i, NAV_X + 10., ny + 9., fg);
            draw_text(
                if active { 7 } else { 2 },
                label,
                fg,
                NAV_X + 34.,
                ny + 7.,
                NAV_W - 42.,
                20.,
                false,
            );
        }

        // Sidebar footer
        r.round(12., 344., SIDEBAR_W - 24., 1., 0., DIVIDER);
        draw_text(
            3,
            concat!("Wisp ", env!("CARGO_PKG_VERSION")),
            TERTIARY,
            14.,
            352.,
            SIDEBAR_W - 28.,
            18.,
            false,
        );
        let quit_bg = if sw.pressed == HIT_QUIT {
            rgb(54, 54, 58)
        } else if sw.hover == HIT_QUIT {
            rgb(44, 44, 46)
        } else {
            CARD_BG
        };
        r.round(QUIT_X, QUIT_Y, QUIT_W, QUIT_H, 8., quit_bg);
        draw_text(2, "Quit Wisp", WHITE, QUIT_X, QUIT_Y, QUIT_W, QUIT_H, true);

        // 4. Right content pane (directional slide + fade on section switch, zero extra surfaces)
        let content_a = sw.section_t.clamp(0., 1.);
        let content_dy = (1. - content_a) * 8. * sw.section_dir;
        let cf = |c: u32| blend_color(WIN_BG, c, content_a);
        let cy_off = |y: f64| y + content_dy;
        let draw_c_text = |font_idx: usize,
                           text: &str,
                           color: u32,
                           x: f64,
                           y: f64,
                           w: f64,
                           h: f64,
                           center: bool| {
            draw_text(font_idx, text, cf(color), x, cy_off(y), w, h, center);
        };
        let draw_c_switch = |tx: f64, ty: f64, t_raw: f64| {
            let t = t_raw.clamp(0., 1.);
            let sy = cy_off(ty);
            r.round(
                tx,
                sy,
                36.,
                20.,
                10.,
                cf(blend_color(rgb(57, 57, 61), GREEN, t)),
            );
            r.round(tx + 2. + 16. * t, sy + 2., 16., 16., 8., cf(WHITE));
        };

        match sw.section {
            SettingsSection::Notch => {
                draw_c_text(0, "Notch", WHITE, CONTENT_X, 56., CONTENT_W, 22., false);
                draw_c_text(
                    3,
                    "How the dynamic notch expands, hides, and shows live info.",
                    SECONDARY,
                    CONTENT_X,
                    79.,
                    CONTENT_W,
                    16.,
                    false,
                );
                draw_c_text(
                    3,
                    "BEHAVIOR",
                    SECONDARY,
                    CONTENT_X + 4.,
                    102.,
                    160.,
                    14.,
                    false,
                );
                r.round(
                    CONTENT_X,
                    cy_off(SWITCH_CARD_Y),
                    CONTENT_W,
                    4. * SWITCH_ROW_H,
                    12.,
                    cf(CARD_BG),
                );
                for (k, &(title, sub)) in NOTCH_ROWS.iter().enumerate() {
                    let ry = SWITCH_CARD_Y + k as f64 * SWITCH_ROW_H;
                    let hit = HIT_SWITCH_BASE + k as i32;
                    if sw.hover == hit || sw.pressed == hit {
                        r.round(
                            CONTENT_X + 4.,
                            cy_off(ry + 4.),
                            CONTENT_W - 8.,
                            SWITCH_ROW_H - 8.,
                            8.,
                            cf(if sw.pressed == hit {
                                rgb(50, 50, 54)
                            } else {
                                rgb(38, 38, 42)
                            }),
                        );
                    }
                    draw_c_text(
                        7,
                        title,
                        WHITE,
                        CONTENT_X + 14.,
                        ry + 9.,
                        CONTENT_W - 74.,
                        18.,
                        false,
                    );
                    draw_c_text(
                        3,
                        sub,
                        SECONDARY,
                        CONTENT_X + 14.,
                        ry + 29.,
                        CONTENT_W - 74.,
                        16.,
                        false,
                    );
                    draw_c_switch(
                        CONTENT_X + CONTENT_W - 50.,
                        ry + (SWITCH_ROW_H - 20.) / 2.,
                        state.switch_t[k],
                    );
                    if k < 3 {
                        r.round(
                            CONTENT_X + 14.,
                            cy_off(ry + SWITCH_ROW_H - 1.),
                            CONTENT_W - 14.,
                            1.,
                            0.,
                            cf(ROW_SEP),
                        );
                    }
                }
                draw_c_text(
                    3,
                    "Changes save automatically.",
                    TERTIARY,
                    CONTENT_X + 4.,
                    SWITCH_CARD_Y + 4. * SWITCH_ROW_H + 10.,
                    CONTENT_W,
                    16.,
                    false,
                );
            }
            SettingsSection::General => {
                draw_c_text(0, "General", WHITE, CONTENT_X, 56., CONTENT_W, 22., false);
                draw_c_text(
                    3,
                    "Clock format, motion preference, and Windows startup.",
                    SECONDARY,
                    CONTENT_X,
                    79.,
                    CONTENT_W,
                    16.,
                    false,
                );
                draw_c_text(
                    3,
                    "SYSTEM & DISPLAY",
                    SECONDARY,
                    CONTENT_X + 4.,
                    102.,
                    180.,
                    14.,
                    false,
                );
                r.round(
                    CONTENT_X,
                    cy_off(SWITCH_CARD_Y),
                    CONTENT_W,
                    3. * SWITCH_ROW_H,
                    12.,
                    cf(CARD_BG),
                );
                for (k, &(title, sub)) in GENERAL_ROWS.iter().enumerate() {
                    let idx = 4 + k;
                    let ry = SWITCH_CARD_Y + k as f64 * SWITCH_ROW_H;
                    let hit = HIT_SWITCH_BASE + idx as i32;
                    if sw.hover == hit || sw.pressed == hit {
                        r.round(
                            CONTENT_X + 4.,
                            cy_off(ry + 4.),
                            CONTENT_W - 8.,
                            SWITCH_ROW_H - 8.,
                            8.,
                            cf(if sw.pressed == hit {
                                rgb(50, 50, 54)
                            } else {
                                rgb(38, 38, 42)
                            }),
                        );
                    }
                    draw_c_text(
                        7,
                        title,
                        WHITE,
                        CONTENT_X + 14.,
                        ry + 9.,
                        CONTENT_W - 74.,
                        18.,
                        false,
                    );
                    draw_c_text(
                        3,
                        sub,
                        SECONDARY,
                        CONTENT_X + 14.,
                        ry + 29.,
                        CONTENT_W - 74.,
                        16.,
                        false,
                    );
                    draw_c_switch(
                        CONTENT_X + CONTENT_W - 50.,
                        ry + (SWITCH_ROW_H - 20.) / 2.,
                        state.switch_t[idx],
                    );
                    if k < 2 {
                        r.round(
                            CONTENT_X + 14.,
                            cy_off(ry + SWITCH_ROW_H - 1.),
                            CONTENT_W - 14.,
                            1.,
                            0.,
                            cf(ROW_SEP),
                        );
                    }
                }
            }
            SettingsSection::Appearance => {
                draw_c_text(
                    0,
                    "Appearance",
                    WHITE,
                    CONTENT_X,
                    56.,
                    CONTENT_W,
                    22.,
                    false,
                );
                draw_c_text(
                    3,
                    "Panel backdrop theme, frost blur, and opacity. Closed notch stays pure black.",
                    SECONDARY,
                    CONTENT_X,
                    79.,
                    CONTENT_W,
                    16.,
                    false,
                );
                draw_c_text(
                    3,
                    "BACKGROUND STYLE",
                    SECONDARY,
                    CONTENT_X + 4.,
                    98.,
                    180.,
                    14.,
                    false,
                );
                r.round(
                    CONTENT_X,
                    cy_off(STYLE_CARD_Y),
                    CONTENT_W,
                    STYLE_CARD_H,
                    12.,
                    cf(CARD_BG),
                );
                // Hover fills behind the gliding active style tile highlight
                for i in 0..BG_STYLE_NAMES.len() {
                    let tr = style_tile_rect(i);
                    let hit = HIT_BG_STYLE_BASE + i as i32;
                    let active = state.settings.bg_style == i as u32;
                    if !active && (sw.hover == hit || sw.pressed == hit) {
                        r.round(tr.0, cy_off(tr.1), tr.2, tr.3, 8., cf(rgb(36, 36, 40)));
                    }
                }
                let tr0 = style_tile_rect(0);
                let tr1 = style_tile_rect(1);
                let style_x = tr0.0 + sw.style_t.clamp(0., 4.) * (tr1.0 - tr0.0);
                r.round(
                    style_x,
                    cy_off(tr0.1),
                    tr0.2,
                    tr0.3,
                    8.,
                    cf(rgb(44, 44, 48)),
                );
                r.stroke_round(
                    style_x + 0.5,
                    cy_off(tr0.1) + 0.5,
                    tr0.2 - 1.,
                    tr0.3 - 1.,
                    8.,
                    1.4,
                    cf(BLUE),
                );
                for (i, &name) in BG_STYLE_NAMES.iter().enumerate() {
                    let tr = style_tile_rect(i);
                    let hit = HIT_BG_STYLE_BASE + i as i32;
                    let active = state.settings.bg_style == i as u32;
                    let hot = sw.hover == hit;
                    r.round(
                        tr.0 + 6.,
                        cy_off(tr.1 + 5.),
                        tr.2 - 12.,
                        24.,
                        5.,
                        cf(BG_SWATCH_COLORS[i]),
                    );
                    draw_c_text(
                        3,
                        name,
                        if active || hot { WHITE } else { SECONDARY },
                        tr.0,
                        tr.1 + 32.,
                        tr.2,
                        16.,
                        true,
                    );
                }

                let has_img = state.bg_custom_pixels.is_some();
                let layout = appearance_layout(has_img, state.settings.bg_style);
                r.round(
                    CONTENT_X,
                    cy_off(OPT_CARD_Y),
                    CONTENT_W,
                    layout.total_h,
                    12.,
                    cf(CARD_BG),
                );

                if let Some(ry) = layout.image_row_y {
                    draw_c_text(
                        2,
                        "Custom image",
                        WHITE,
                        CONTENT_X + 14.,
                        ry + 12.,
                        160.,
                        20.,
                        false,
                    );
                    let (choose_r, reset_r) = image_button_rects(ry, has_img);
                    let c_bg = if sw.pressed == HIT_BG_CHOOSE {
                        rgb(58, 58, 62)
                    } else if sw.hover == HIT_BG_CHOOSE {
                        rgb(50, 50, 54)
                    } else {
                        rgb(42, 42, 46)
                    };
                    r.round(
                        choose_r.0,
                        cy_off(choose_r.1),
                        choose_r.2,
                        choose_r.3,
                        12.,
                        cf(c_bg),
                    );
                    draw_c_text(
                        3,
                        if has_img { "Change..." } else { "Choose..." },
                        WHITE,
                        choose_r.0,
                        choose_r.1,
                        choose_r.2,
                        choose_r.3,
                        true,
                    );
                    if let Some(rr) = reset_r {
                        let r_bg = if sw.pressed == HIT_BG_RESET {
                            rgb(58, 58, 62)
                        } else if sw.hover == HIT_BG_RESET {
                            rgb(50, 50, 54)
                        } else {
                            rgb(42, 42, 46)
                        };
                        r.round(rr.0, cy_off(rr.1), rr.2, rr.3, 12., cf(r_bg));
                        draw_c_text(3, "Reset", SECONDARY, rr.0, rr.1, rr.2, rr.3, true);
                    }
                    r.round(
                        CONTENT_X + 14.,
                        cy_off(ry + OPT_ROW_H - 1.),
                        CONTENT_W - 14.,
                        1.,
                        0.,
                        cf(ROW_SEP),
                    );
                }

                if let Some(ry) = layout.framing_row_y {
                    draw_c_text(
                        2,
                        "Framing",
                        WHITE,
                        CONTENT_X + 14.,
                        ry + 12.,
                        120.,
                        20.,
                        false,
                    );
                    const POS: [(&str, u32); 3] = [("Top", 1), ("Center", 0), ("Bottom", 2)];
                    for i in 0..POS.len() {
                        let pr = framing_pill_rect(ry, i);
                        let hit = HIT_BG_POS_BASE + i as i32;
                        let bg = if sw.hover == hit || sw.pressed == hit {
                            rgb(44, 44, 48)
                        } else {
                            WIN_BG
                        };
                        r.round(pr.0, cy_off(pr.1), pr.2, pr.3, 12., cf(bg));
                    }
                    let fp0 = framing_pill_rect(ry, 0);
                    let fp1 = framing_pill_rect(ry, 1);
                    let fx = fp0.0 + sw.framing_t.clamp(0., 2.) * (fp1.0 - fp0.0);
                    r.round(fx, cy_off(fp0.1), fp0.2, fp0.3, 12., cf(rgb(62, 62, 68)));
                    for (i, &(lbl, val)) in POS.iter().enumerate() {
                        let pr = framing_pill_rect(ry, i);
                        let hit = HIT_BG_POS_BASE + i as i32;
                        let active = state.settings.bg_pos == val;
                        draw_c_text(
                            3,
                            lbl,
                            if active || sw.hover == hit {
                                WHITE
                            } else {
                                SECONDARY
                            },
                            pr.0,
                            pr.1,
                            pr.2,
                            pr.3,
                            true,
                        );
                    }
                    r.round(
                        CONTENT_X + 14.,
                        cy_off(ry + OPT_ROW_H - 1.),
                        CONTENT_W - 14.,
                        1.,
                        0.,
                        cf(ROW_SEP),
                    );
                }

                if let Some(ry) = layout.blur_row_y {
                    draw_c_text(
                        2,
                        "Blur radius",
                        WHITE,
                        CONTENT_X + 14.,
                        ry + 8.,
                        140.,
                        18.,
                        false,
                    );
                    let readout = if state.settings.bg_blur == 0 {
                        "None".to_string()
                    } else {
                        format!("{} px", state.settings.bg_blur)
                    };
                    draw_c_text(
                        3,
                        &readout,
                        SECONDARY,
                        CONTENT_X + CONTENT_W - 74.,
                        ry + 8.,
                        60.,
                        18.,
                        true,
                    );
                    let active = sw.hover == HIT_BG_BLUR || sw.blur_drag;
                    let th = if active { 6. } else { 4. };
                    let track_y = cy_off(ry + 36.);
                    let frac = (state.settings.bg_blur as f64 / 30.).clamp(0., 1.);
                    let thumb_x = BLUR_TRACK_X0 + frac * (BLUR_TRACK_X1 - BLUR_TRACK_X0);
                    r.round(
                        BLUR_TRACK_X0,
                        track_y - th / 2.,
                        BLUR_TRACK_X1 - BLUR_TRACK_X0,
                        th,
                        th / 2.,
                        cf(rgb(52, 52, 55)),
                    );
                    if frac > 0.005 {
                        r.round(
                            BLUR_TRACK_X0,
                            track_y - th / 2.,
                            (thumb_x - BLUR_TRACK_X0).max(th),
                            th,
                            th / 2.,
                            cf(BLUE),
                        );
                    }
                    let knob = if active { 16. } else { 14. };
                    r.round(
                        thumb_x - knob / 2.,
                        track_y - knob / 2.,
                        knob,
                        knob,
                        knob / 2.,
                        cf(WHITE),
                    );
                    r.round(
                        CONTENT_X + 14.,
                        cy_off(ry + BLUR_ROW_H - 1.),
                        CONTENT_W - 14.,
                        1.,
                        0.,
                        cf(ROW_SEP),
                    );
                }

                // Opacity row (always present)
                let oy = layout.opacity_row_y;
                draw_c_text(
                    2,
                    "Opacity",
                    WHITE,
                    CONTENT_X + 14.,
                    oy + 12.,
                    110.,
                    20.,
                    false,
                );
                const OPS: [(u32, &str); 4] =
                    [(70, "70%"), (80, "80%"), (90, "90%"), (100, "100%")];
                for i in 0..OPS.len() {
                    let pr = opacity_pill_rect(oy, i);
                    let hit = HIT_BG_OPACITY_BASE + i as i32;
                    let bg = if sw.hover == hit || sw.pressed == hit {
                        rgb(44, 44, 48)
                    } else {
                        WIN_BG
                    };
                    r.round(pr.0, cy_off(pr.1), pr.2, pr.3, 12., cf(bg));
                }
                let op0 = opacity_pill_rect(oy, 0);
                let op1 = opacity_pill_rect(oy, 1);
                let ox = op0.0 + sw.opacity_t.clamp(0., 3.) * (op1.0 - op0.0);
                r.round(ox, cy_off(op0.1), op0.2, op0.3, 12., cf(rgb(62, 62, 68)));
                for (i, &(val, lbl)) in OPS.iter().enumerate() {
                    let pr = opacity_pill_rect(oy, i);
                    let hit = HIT_BG_OPACITY_BASE + i as i32;
                    let active = state.settings.bg_opacity == val;
                    draw_c_text(
                        3,
                        lbl,
                        if active || sw.hover == hit {
                            WHITE
                        } else {
                            SECONDARY
                        },
                        pr.0,
                        pr.1,
                        pr.2,
                        pr.3,
                        true,
                    );
                }
            }
            SettingsSection::Updates => {
                let u = &state.updater;
                draw_c_text(0, "Updates", WHITE, CONTENT_X, 56., CONTENT_W, 22., false);
                draw_c_text(
                    3,
                    "Keep Wisp up to date with the latest official releases.",
                    SECONDARY,
                    CONTENT_X,
                    79.,
                    CONTENT_W,
                    16.,
                    false,
                );
                r.round(
                    CONTENT_X,
                    cy_off(UPD_CARD_Y),
                    CONTENT_W,
                    UPD_AUTO_H + UPD_BODY_H,
                    12.,
                    cf(CARD_BG),
                );
                if sw.hover == HIT_UPD_AUTO || sw.pressed == HIT_UPD_AUTO {
                    r.round(
                        CONTENT_X + 4.,
                        cy_off(UPD_CARD_Y + 4.),
                        CONTENT_W - 8.,
                        UPD_AUTO_H - 8.,
                        8.,
                        cf(if sw.pressed == HIT_UPD_AUTO {
                            rgb(50, 50, 54)
                        } else {
                            rgb(38, 38, 42)
                        }),
                    );
                }
                draw_c_text(
                    7,
                    "Check automatically",
                    WHITE,
                    CONTENT_X + 14.,
                    UPD_CARD_Y + 8.,
                    CONTENT_W - 74.,
                    18.,
                    false,
                );
                draw_c_text(
                    3,
                    "Check once daily on startup (nothing installs automatically)",
                    SECONDARY,
                    CONTENT_X + 14.,
                    UPD_CARD_Y + 28.,
                    CONTENT_W - 74.,
                    16.,
                    false,
                );
                draw_c_switch(
                    CONTENT_X + CONTENT_W - 50.,
                    UPD_CARD_Y + (UPD_AUTO_H - 20.) / 2.,
                    sw.auto_switch_t,
                );
                r.round(
                    CONTENT_X + 14.,
                    cy_off(UPD_CARD_Y + UPD_AUTO_H - 1.),
                    CONTENT_W - 14.,
                    1.,
                    0.,
                    cf(ROW_SEP),
                );

                // Status + version + progress + release notes + buttons
                let body_y = UPD_CARD_Y + UPD_AUTO_H;
                let status_lines = update_note_lines(&u.status);
                for (i, line) in status_lines.iter().take(2).enumerate() {
                    draw_c_text(
                        7,
                        line,
                        WHITE,
                        CONTENT_X + 14.,
                        body_y + 8. + i as f64 * 16.,
                        CONTENT_W - 28.,
                        16.,
                        false,
                    );
                }
                draw_c_text(
                    3,
                    concat!("Current version: Wisp ", env!("CARGO_PKG_VERSION")),
                    SECONDARY,
                    CONTENT_X + 14.,
                    body_y + 42.,
                    CONTENT_W - 28.,
                    16.,
                    false,
                );
                if u.phase == updater::Phase::Downloading && u.progress.1 > 0 {
                    let frac = (u.progress.0 as f64 / u.progress.1 as f64).clamp(0., 1.);
                    r.round(
                        CONTENT_X + 14.,
                        cy_off(body_y + 66.),
                        CONTENT_W - 74.,
                        4.,
                        2.,
                        cf(rgb(44, 44, 48)),
                    );
                    r.round(
                        CONTENT_X + 14.,
                        cy_off(body_y + 66.),
                        (CONTENT_W - 74.) * frac,
                        4.,
                        2.,
                        cf(BLUE),
                    );
                    draw_c_text(
                        3,
                        &format!("{:.0}%", frac * 100.),
                        SECONDARY,
                        CONTENT_X + CONTENT_W - 56.,
                        body_y + 60.,
                        42.,
                        16.,
                        true,
                    );
                } else if let Some(rel) = &u.release {
                    for (i, line) in update_note_lines(&rel.notes)
                        .iter()
                        .take(UPD_NOTE_LINES)
                        .enumerate()
                    {
                        draw_c_text(
                            3,
                            line,
                            SECONDARY,
                            CONTENT_X + 14.,
                            body_y + 62. + i as f64 * 16.,
                            CONTENT_W - 28.,
                            16.,
                            false,
                        );
                    }
                }
                let action_lbl = match u.phase {
                    updater::Phase::Checking => "Checking...",
                    updater::Phase::Downloading => "Downloading...",
                    updater::Phase::Ready => "Restart & update",
                    updater::Phase::Available => "Download update",
                    updater::Phase::Error if u.release.is_some() => "Retry download",
                    _ => "Check for updates",
                };
                let enabled = u.action_enabled();
                let act_bg = if !enabled {
                    rgb(45, 45, 48)
                } else if sw.pressed == HIT_UPD_ACTION {
                    blend_color(BLUE, rgb(0, 0, 0), 0.18)
                } else {
                    BLUE
                };
                r.round(
                    UPD_ACTION_X,
                    cy_off(UPD_ACTION_Y),
                    UPD_ACTION_W,
                    UPD_ACTION_H,
                    15.,
                    cf(act_bg),
                );
                draw_c_text(
                    2,
                    action_lbl,
                    if enabled { WHITE } else { SECONDARY },
                    UPD_ACTION_X,
                    UPD_ACTION_Y,
                    UPD_ACTION_W,
                    UPD_ACTION_H,
                    true,
                );
                if u.phase == updater::Phase::Downloading {
                    let cancel_bg = if sw.pressed == HIT_UPD_CANCEL {
                        rgb(54, 54, 58)
                    } else if sw.hover == HIT_UPD_CANCEL {
                        rgb(44, 44, 48)
                    } else {
                        rgb(36, 36, 40)
                    };
                    r.round(
                        UPD_CANCEL_X,
                        cy_off(UPD_ACTION_Y),
                        UPD_CANCEL_W,
                        UPD_ACTION_H,
                        15.,
                        cf(cancel_bg),
                    );
                    draw_c_text(
                        2,
                        "Cancel",
                        WHITE,
                        UPD_CANCEL_X,
                        UPD_ACTION_Y,
                        UPD_CANCEL_W,
                        UPD_ACTION_H,
                        true,
                    );
                }

                // Card 2: Troubleshooting log info (non-interactive)
                r.round(
                    CONTENT_X,
                    cy_off(UPD_LOG_CARD_Y),
                    CONTENT_W,
                    UPD_LOG_CARD_H,
                    12.,
                    cf(CARD_BG),
                );
                draw_c_text(
                    7,
                    "Troubleshooting log",
                    WHITE,
                    CONTENT_X + 14.,
                    UPD_LOG_CARD_Y + 9.,
                    260.,
                    18.,
                    false,
                );
                draw_c_text(
                    3,
                    "%APPDATA%\\Wisp\\wisp.log (stays local)",
                    SECONDARY,
                    CONTENT_X + 14.,
                    UPD_LOG_CARD_Y + 29.,
                    300.,
                    16.,
                    false,
                );
                draw_c_text(
                    3,
                    "Local only",
                    TERTIARY,
                    CONTENT_X + CONTENT_W - 80.,
                    UPD_LOG_CARD_Y + 18.,
                    66.,
                    18.,
                    true,
                );
            }
        }

        // Subtle outer border hairline so dark window reads cleanly on dark wallpapers
        r.stroke_round(
            0.5,
            0.5,
            SET_WIN_W - 1.,
            SET_WIN_H - 1.,
            14.,
            1.,
            rgb(42, 42, 46),
        );

        let open_a = sw.open_t.clamp(0., 1.);
        let slide_px = ((1. - open_a) * 10. * scale).round() as i32;
        let dst = if open_a >= 0.999 {
            let mut wr = Rect::default();
            GetWindowRect(sw.hwnd, &mut wr);
            Point {
                x: wr.left,
                y: wr.top,
            }
        } else if let Some((bx, by)) = sw.pos {
            Point {
                x: bx,
                y: by - slide_px,
            }
        } else {
            let mut wr = Rect::default();
            GetWindowRect(sw.hwnd, &mut wr);
            Point {
                x: wr.left,
                y: wr.top - slide_px,
            }
        };
        let size = Size {
            x: sw.w_px,
            y: sw.h_px,
        };
        let src = Point { x: 0, y: 0 };
        let blend = Blend {
            op: 0,
            flags: 0,
            alpha: (open_a * 255.).round() as u8,
            format: 1,
        };
        UpdateLayeredWindow(sw.hwnd, null_mut(), &dst, &size, sw.dc, &src, 0, &blend, 2);
    }
}

impl Drop for SettingsWindow {
    fn drop(&mut self) {
        unsafe {
            self.destroy();
        }
    }
}

unsafe fn draw_sidebar_icon(r: &render::Renderer, idx: usize, x: f64, y: f64, color: u32) {
    match idx {
        // 0: Notch pill icon
        0 => {
            r.stroke_round(x + 1., y + 4., 14., 8., 4., 1.5, color);
            r.ellipse(x + 11.5, y + 8., 1.4, 1.4, color);
        }
        // 1: General sliders icon
        1 => {
            r.stroke(&[(x + 2., y + 5.), (x + 14., y + 5.)], false, 1.4, color);
            r.ellipse(x + 6., y + 5., 2., 2., color);
            r.stroke(&[(x + 2., y + 11.), (x + 14., y + 11.)], false, 1.4, color);
            r.ellipse(x + 10., y + 11., 2., 2., color);
        }
        // 2: Appearance half-circle contrast icon
        2 => {
            r.stroke_ellipse(x + 8., y + 8., 6., 6., 1.5, color);
            r.ring(x + 8., y + 8., 5.2, 5.2, -90., 180., color);
        }
        // 3: Updates upward arrow in circle
        _ => {
            r.stroke_ellipse(x + 8., y + 8., 6., 6., 1.4, color);
            r.stroke(&[(x + 8., y + 11.), (x + 8., y + 5.)], false, 1.4, color);
            r.stroke(
                &[(x + 5.5, y + 7.5), (x + 8., y + 5.), (x + 10.5, y + 7.5)],
                false,
                1.4,
                color,
            );
        }
    }
}

unsafe extern "system" fn settings_proc(hwnd: Hwnd, msg: u32, w: usize, l: isize) -> isize {
    let ptr = GetWindowLongPtrW(hwnd, -21) as *mut State;
    if ptr.is_null() {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    let s = &mut *ptr;
    match msg {
        0x0010 => {
            // WM_CLOSE -> hide instead of quitting Wisp
            if s.modal_dialog_open {
                return 0;
            }
            SettingsWindow::hide_with_motion(s);
            if !s.main_hwnd.is_null() {
                InvalidateRect(s.main_hwnd, std::ptr::null(), 0);
            }
            0
        }
        0x000F => {
            let mut ps: Paint = std::mem::zeroed();
            BeginPaint(hwnd, &mut ps);
            SettingsWindow::paint(s);
            EndPaint(hwnd, &ps);
            0
        }
        0x0232 => {
            // WM_EXITSIZEMOVE -> remember dragged position
            let mut r = Rect::default();
            if GetWindowRect(hwnd, &mut r) != 0 {
                s.settings_win.pos = Some((r.left, r.top));
            }
            0
        }
        0x0100 => {
            // WM_KEYDOWN: Escape hides Settings popup
            if w == 0x1B {
                if s.modal_dialog_open {
                    return 0;
                }
                SettingsWindow::hide_with_motion(s);
                if !s.main_hwnd.is_null() {
                    InvalidateRect(s.main_hwnd, std::ptr::null(), 0);
                }
                return 0;
            }
            DefWindowProcW(hwnd, msg, w, l)
        }
        0x0200 => {
            if !s.settings_win.open {
                return 0;
            }
            let mut tracking = MouseTracking {
                size: size_of::<MouseTracking>() as u32,
                flags: 2,
                hwnd,
                hover_time: 0,
            };
            TrackMouseEvent(&mut tracking);
            let x = (l as i16) as f64 / s.scale;
            let y = ((l >> 16) as i16) as f64 / s.scale;
            if s.settings_win.blur_drag {
                let next = blur_from_x(x);
                if s.settings.bg_blur != next {
                    s.settings.bg_blur = next;
                    s.apply_settings();
                    if !s.main_hwnd.is_null() {
                        InvalidateRect(s.main_hwnd, std::ptr::null(), 0);
                    }
                    SettingsWindow::paint(s);
                }
                return 0;
            }
            let hit = hit_test(SettingsWindow::hit_context(s), x, y);
            if s.settings_win.hover != hit {
                s.settings_win.hover = hit;
                SettingsWindow::paint(s);
            }
            0
        }
        0x02A3 => {
            let mut changed = false;
            if s.settings_win.hover != HIT_NONE {
                s.settings_win.hover = HIT_NONE;
                changed = true;
            }
            if !s.settings_win.blur_drag && s.settings_win.pressed != HIT_NONE {
                s.settings_win.pressed = HIT_NONE;
                changed = true;
            }
            if changed {
                SettingsWindow::paint(s);
            }
            0
        }
        0x0201 => {
            if !s.settings_win.open {
                return 0;
            }
            let x = (l as i16) as f64 / s.scale;
            let y = ((l >> 16) as i16) as f64 / s.scale;
            let hit = hit_test(SettingsWindow::hit_context(s), x, y);
            if hit == HIT_TITLEBAR {
                ReleaseCapture();
                SendMessageW(hwnd, 0x00A1, 2, 0);
                return 0;
            }
            if hit == HIT_BG_BLUR {
                s.settings_win.blur_drag = true;
                s.settings_win.pressed = HIT_BG_BLUR;
                SetCapture(hwnd);
                s.settings.bg_blur = blur_from_x(x);
                s.apply_settings();
                if !s.main_hwnd.is_null() {
                    InvalidateRect(s.main_hwnd, std::ptr::null(), 0);
                }
                SettingsWindow::paint(s);
                return 0;
            }
            s.settings_win.pressed = hit;
            if hit != HIT_NONE {
                SetCapture(hwnd);
            }
            SettingsWindow::paint(s);
            0
        }
        0x0202 => {
            if !s.settings_win.open && !s.settings_win.blur_drag {
                return 0;
            }
            let x = (l as i16) as f64 / s.scale;
            let y = ((l >> 16) as i16) as f64 / s.scale;
            let hit = hit_test(SettingsWindow::hit_context(s), x, y);
            if s.settings_win.blur_drag {
                s.settings_win.blur_drag = false;
                s.settings_win.pressed = HIT_NONE;
                s.settings_win.hover = hit;
                ReleaseCapture();
                if s.bg_custom_pixels.is_some() {
                    s.reload_custom_bg();
                }
                s.apply_settings();
                if !s.smoke {
                    s.settings.save();
                }
                if !s.main_hwnd.is_null() {
                    InvalidateRect(s.main_hwnd, std::ptr::null(), 0);
                }
                SettingsWindow::paint(s);
                return 0;
            }
            let pressed = std::mem::replace(&mut s.settings_win.pressed, HIT_NONE);
            s.settings_win.hover = hit;
            if pressed != HIT_NONE {
                ReleaseCapture();
            }
            if hit != HIT_NONE && hit == pressed {
                SettingsWindow::handle_click(s, hit);
            } else if s.settings_win.is_open() {
                SettingsWindow::paint(s);
            }
            0
        }
        0x0215 => {
            // WM_CAPTURECHANGED: clean up blur drag or button press if capture was stolen
            if s.settings_win.blur_drag {
                s.settings_win.blur_drag = false;
                s.settings_win.pressed = HIT_NONE;
                s.settings_win.hover = HIT_NONE;
                if s.bg_custom_pixels.is_some() {
                    s.reload_custom_bg();
                }
                s.apply_settings();
                if !s.smoke {
                    s.settings.save();
                }
                if !s.main_hwnd.is_null() {
                    InvalidateRect(s.main_hwnd, std::ptr::null(), 0);
                }
                SettingsWindow::paint(s);
            } else if s.settings_win.pressed != HIT_NONE || s.settings_win.hover != HIT_NONE {
                s.settings_win.pressed = HIT_NONE;
                s.settings_win.hover = HIT_NONE;
                if s.settings_win.is_open() {
                    SettingsWindow::paint(s);
                }
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}

pub const SET_WIN_W: f64 = 620.;
pub const SET_WIN_H: f64 = 420.;
pub const TITLE_H: f64 = 40.;
pub const SIDEBAR_W: f64 = 168.;
pub const CONTENT_X: f64 = 188.;
pub const CONTENT_W: f64 = 412.;

pub const CLOSE_X: f64 = 584.;
pub const CLOSE_Y: f64 = 8.;
pub const CLOSE_SIZE: f64 = 24.;

pub const NAV_X: f64 = 10.;
pub const NAV_W: f64 = 148.;
pub const NAV_Y0: f64 = 50.;
pub const NAV_H: f64 = 34.;
pub const NAV_STEP: f64 = 38.;

pub const QUIT_X: f64 = 12.;
pub const QUIT_Y: f64 = 376.;
pub const QUIT_W: f64 = 144.;
pub const QUIT_H: f64 = 30.;

pub const SWITCH_CARD_Y: f64 = 120.;
pub const SWITCH_ROW_H: f64 = 54.;

pub const STYLE_CARD_Y: f64 = 114.;
pub const STYLE_CARD_H: f64 = 68.;
pub const STYLE_TILE_PAD: f64 = 8.;
pub const STYLE_TILE_GAP: f64 = 6.;

pub const OPT_CARD_Y: f64 = 194.;
pub const OPT_ROW_H: f64 = 44.;
pub const BLUR_ROW_H: f64 = 54.;
pub const BLUR_TRACK_X0: f64 = CONTENT_X + 14.;
pub const BLUR_TRACK_X1: f64 = CONTENT_X + CONTENT_W - 14.;

pub const TEXT_DRAW_FLAGS: u32 = 0x20 | 0x4 | 0x0800 | 0x8000;

pub const UPD_CARD_Y: f64 = 114.;
pub const UPD_AUTO_H: f64 = 52.;
pub const UPD_BODY_H: f64 = 172.;
pub const UPD_NOTE_LINES: usize = 4;
pub const UPD_ACTION_X: f64 = CONTENT_X + 14.;
pub const UPD_ACTION_Y: f64 = 298.;
pub const UPD_ACTION_W: f64 = 160.;
pub const UPD_ACTION_H: f64 = 30.;
pub const UPD_CANCEL_X: f64 = 372.;
pub const UPD_CANCEL_W: f64 = 88.;
pub const UPD_LOG_CARD_Y: f64 = 350.;
pub const UPD_LOG_CARD_H: f64 = 54.;

pub const HIT_NONE: i32 = -1;
pub const HIT_TITLEBAR: i32 = 1;
pub const HIT_CLOSE: i32 = 2;
pub const HIT_SWITCH_BASE: i32 = 5000;
pub const HIT_NAV_BASE: i32 = 5100;
pub const HIT_QUIT: i32 = 5200;
pub const HIT_BG_STYLE_BASE: i32 = 5300;
pub const HIT_BG_OPACITY_BASE: i32 = 5310;
pub const HIT_BG_CHOOSE: i32 = 5320;
pub const HIT_BG_RESET: i32 = 5321;
pub const HIT_BG_POS_BASE: i32 = 5330;
pub const HIT_BG_BLUR: i32 = 5340;
pub const HIT_UPD_ACTION: i32 = 5400;
pub const HIT_UPD_AUTO: i32 = 5401;
pub const HIT_UPD_CANCEL: i32 = 5402;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SettingsSection {
    Notch = 0,
    General = 1,
    Appearance = 2,
    Updates = 3,
}

impl SettingsSection {
    pub fn from_index(i: usize) -> Self {
        match i {
            0 => Self::Notch,
            1 => Self::General,
            2 => Self::Appearance,
            _ => Self::Updates,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct HitContext {
    pub section: SettingsSection,
    pub has_custom_image: bool,
    pub bg_style: u32,
    pub update_action_enabled: bool,
    pub update_downloading: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct AppearanceLayout {
    pub image_row_y: Option<f64>,
    pub framing_row_y: Option<f64>,
    pub blur_row_y: Option<f64>,
    pub opacity_row_y: f64,
    pub total_h: f64,
}

pub fn appearance_layout(has_custom_image: bool, bg_style: u32) -> AppearanceLayout {
    let mut y = OPT_CARD_Y;
    let image_row_y = if bg_style == 4 || has_custom_image {
        let cur = y;
        y += OPT_ROW_H;
        Some(cur)
    } else {
        None
    };
    let framing_row_y = if bg_style == 4 {
        let cur = y;
        y += OPT_ROW_H;
        Some(cur)
    } else {
        None
    };
    let blur_row_y = if bg_style != 3 {
        let cur = y;
        y += BLUR_ROW_H;
        Some(cur)
    } else {
        None
    };
    let opacity_row_y = y;
    y += OPT_ROW_H;
    AppearanceLayout {
        image_row_y,
        framing_row_y,
        blur_row_y,
        opacity_row_y,
        total_h: y - OPT_CARD_Y,
    }
}

pub fn style_tile_rect(i: usize) -> (f64, f64, f64, f64) {
    let tile_w = (CONTENT_W - 2. * STYLE_TILE_PAD - 4. * STYLE_TILE_GAP) / 5.;
    let x = CONTENT_X + STYLE_TILE_PAD + i as f64 * (tile_w + STYLE_TILE_GAP);
    (x, STYLE_CARD_Y + 8., tile_w, STYLE_CARD_H - 16.)
}

pub fn opacity_pill_rect(row_y: f64, i: usize) -> (f64, f64, f64, f64) {
    let start_x = CONTENT_X + CONTENT_W - 14. - 4. * 52. - 3. * 4.;
    (start_x + i as f64 * 56., row_y + 10., 52., 24.)
}

pub fn framing_pill_rect(row_y: f64, i: usize) -> (f64, f64, f64, f64) {
    let start_x = CONTENT_X + CONTENT_W - 14. - 3. * 58. - 2. * 4.;
    (start_x + i as f64 * 62., row_y + 10., 58., 24.)
}

pub fn image_button_rects(
    row_y: f64,
    has_custom_image: bool,
) -> ((f64, f64, f64, f64), Option<(f64, f64, f64, f64)>) {
    let right = CONTENT_X + CONTENT_W - 14.;
    if has_custom_image {
        let reset = (right - 56., row_y + 10., 56., 24.);
        let choose = (reset.0 - 6. - 74., row_y + 10., 74., 24.);
        (choose, Some(reset))
    } else {
        let choose = (right - 74., row_y + 10., 74., 24.);
        (choose, None)
    }
}

fn in_rect(x: f64, y: f64, r: (f64, f64, f64, f64)) -> bool {
    (r.0..r.0 + r.2).contains(&x) && (r.1..r.1 + r.3).contains(&y)
}

pub fn hit_test(ctx: HitContext, x: f64, y: f64) -> i32 {
    if !(0. ..SET_WIN_W).contains(&x) || !(0. ..SET_WIN_H).contains(&y) {
        return HIT_NONE;
    }
    // Titlebar
    if y < TITLE_H {
        if in_rect(x, y, (CLOSE_X, CLOSE_Y, CLOSE_SIZE, CLOSE_SIZE)) {
            return HIT_CLOSE;
        }
        return HIT_TITLEBAR;
    }
    // Left sidebar
    if x < SIDEBAR_W {
        for i in 0..4 {
            let ny = NAV_Y0 + i as f64 * NAV_STEP;
            if in_rect(x, y, (NAV_X, ny, NAV_W, NAV_H)) {
                return HIT_NAV_BASE + i as i32;
            }
        }
        if in_rect(x, y, (QUIT_X, QUIT_Y, QUIT_W, QUIT_H)) {
            return HIT_QUIT;
        }
        return HIT_NONE;
    }
    // Right content pane
    match ctx.section {
        SettingsSection::Notch => {
            for k in 0..4 {
                let ry = SWITCH_CARD_Y + k as f64 * SWITCH_ROW_H;
                if in_rect(x, y, (CONTENT_X, ry, CONTENT_W, SWITCH_ROW_H)) {
                    return HIT_SWITCH_BASE + k as i32;
                }
            }
            HIT_NONE
        }
        SettingsSection::General => {
            for k in 0..3 {
                let ry = SWITCH_CARD_Y + k as f64 * SWITCH_ROW_H;
                if in_rect(x, y, (CONTENT_X, ry, CONTENT_W, SWITCH_ROW_H)) {
                    return HIT_SWITCH_BASE + 4 + k as i32;
                }
            }
            HIT_NONE
        }
        SettingsSection::Appearance => {
            for i in 0..5 {
                if in_rect(x, y, style_tile_rect(i)) {
                    return HIT_BG_STYLE_BASE + i as i32;
                }
            }
            let layout = appearance_layout(ctx.has_custom_image, ctx.bg_style);
            if let Some(ry) = layout.image_row_y {
                let (choose_r, reset_r) = image_button_rects(ry, ctx.has_custom_image);
                if in_rect(x, y, choose_r) {
                    return HIT_BG_CHOOSE;
                }
                if let Some(rr) = reset_r {
                    if in_rect(x, y, rr) {
                        return HIT_BG_RESET;
                    }
                }
            }
            if let Some(ry) = layout.framing_row_y {
                for i in 0..3 {
                    if in_rect(x, y, framing_pill_rect(ry, i)) {
                        return HIT_BG_POS_BASE + i as i32;
                    }
                }
            }
            if let Some(ry) = layout.blur_row_y {
                if (BLUR_TRACK_X0 - 8. ..BLUR_TRACK_X1 + 8.).contains(&x)
                    && (ry + 20. ..ry + BLUR_ROW_H).contains(&y)
                {
                    return HIT_BG_BLUR;
                }
            }
            for i in 0..4 {
                if in_rect(x, y, opacity_pill_rect(layout.opacity_row_y, i)) {
                    return HIT_BG_OPACITY_BASE + i as i32;
                }
            }
            HIT_NONE
        }
        SettingsSection::Updates => {
            if in_rect(x, y, (CONTENT_X, UPD_CARD_Y, CONTENT_W, UPD_AUTO_H)) {
                return HIT_UPD_AUTO;
            }
            if in_rect(
                x,
                y,
                (UPD_ACTION_X, UPD_ACTION_Y, UPD_ACTION_W, UPD_ACTION_H),
            ) {
                return if ctx.update_action_enabled {
                    HIT_UPD_ACTION
                } else {
                    HIT_NONE
                };
            }
            if ctx.update_downloading
                && in_rect(
                    x,
                    y,
                    (UPD_CANCEL_X, UPD_ACTION_Y, UPD_CANCEL_W, UPD_ACTION_H),
                )
            {
                return HIT_UPD_CANCEL;
            }
            HIT_NONE
        }
    }
}

pub fn blur_from_x(x: f64) -> u32 {
    let frac = ((x - BLUR_TRACK_X0) / (BLUR_TRACK_X1 - BLUR_TRACK_X0)).clamp(0., 1.);
    (frac * 30.).round() as u32
}

pub fn window_anchor_point(pos: (i32, i32), w_px: i32, h_px: i32) -> (i32, i32) {
    (pos.0 + w_px / 2, pos.1 + h_px / 2)
}

pub fn clamp_window_pos(
    x: i32,
    y: i32,
    win_w: i32,
    win_h: i32,
    work: (i32, i32, i32, i32),
) -> (i32, i32) {
    let (left, top, right, bottom) = work;
    let max_x = (right - win_w).max(left);
    let max_y = (bottom - win_h).max(top);
    (x.clamp(left, max_x), y.clamp(top, max_y))
}

fn style_index(bg_style: u32) -> f64 {
    bg_style.min(4) as f64
}

fn opacity_index(bg_opacity: u32) -> f64 {
    match bg_opacity {
        70 => 0.,
        80 => 1.,
        90 => 2.,
        _ => 3.,
    }
}

fn framing_index(bg_pos: u32) -> f64 {
    match bg_pos {
        1 => 0.,
        0 => 1.,
        _ => 2.,
    }
}

pub fn advance_switch(
    pos: &mut f64,
    vel: &mut f64,
    target: bool,
    reduced_motion: bool,
    dt: f64,
) -> bool {
    let goal = if target { 1. } else { 0. };
    if reduced_motion {
        let moving = *pos != goal || *vel != 0.;
        *pos = goal;
        *vel = 0.;
        moving
    } else {
        crate::spring(pos, vel, goal, 26., 0.9, dt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(section: SettingsSection) -> HitContext {
        HitContext {
            section,
            has_custom_image: false,
            bg_style: 3,
            update_action_enabled: true,
            update_downloading: false,
        }
    }

    #[test]
    fn titlebar_and_sidebar_hit_targets() {
        let c = ctx(SettingsSection::Notch);
        assert_eq!(hit_test(c, 596., 20.), HIT_CLOSE);
        assert_eq!(hit_test(c, 200., 20.), HIT_TITLEBAR);
        for i in 0..4 {
            assert_eq!(
                hit_test(c, 80., NAV_Y0 + i as f64 * NAV_STEP + 16.),
                HIT_NAV_BASE + i as i32
            );
        }
        assert_eq!(hit_test(c, 84., 390.), HIT_QUIT);
        assert_eq!(hit_test(c, -1., 20.), HIT_NONE);
        assert_eq!(hit_test(c, SET_WIN_W + 1., 20.), HIT_NONE);
        assert_eq!(hit_test(c, 84., SET_WIN_H + 1.), HIT_NONE);
    }

    #[test]
    fn notch_and_general_switch_hit_targets() {
        let notch = ctx(SettingsSection::Notch);
        for k in 0..4 {
            assert_eq!(
                hit_test(notch, 350., SWITCH_CARD_Y + k as f64 * SWITCH_ROW_H + 20.),
                HIT_SWITCH_BASE + k as i32
            );
        }
        assert_eq!(
            hit_test(notch, 350., SWITCH_CARD_Y + 4. * SWITCH_ROW_H + 20.),
            HIT_NONE
        );

        let general = ctx(SettingsSection::General);
        for k in 0..3 {
            assert_eq!(
                hit_test(general, 350., SWITCH_CARD_Y + k as f64 * SWITCH_ROW_H + 20.),
                HIT_SWITCH_BASE + 4 + k as i32
            );
        }
        assert_eq!(
            hit_test(general, 350., SWITCH_CARD_Y + 3. * SWITCH_ROW_H + 20.),
            HIT_NONE
        );
    }

    #[test]
    fn appearance_conditional_hit_targets_and_blur_slider() {
        // OLED hides Blur and Image/Framing rows; only Opacity row appears in options card
        let oled = HitContext {
            section: SettingsSection::Appearance,
            has_custom_image: false,
            bg_style: 3,
            update_action_enabled: true,
            update_downloading: false,
        };
        let oled_layout = appearance_layout(oled.has_custom_image, oled.bg_style);
        assert!(oled_layout.blur_row_y.is_none());
        assert!(oled_layout.image_row_y.is_none());
        assert!(oled_layout.framing_row_y.is_none());
        for i in 0..5 {
            let tile_w = (CONTENT_W - 2. * STYLE_TILE_PAD - 4. * STYLE_TILE_GAP) / 5.;
            let tx =
                CONTENT_X + STYLE_TILE_PAD + i as f64 * (tile_w + STYLE_TILE_GAP) + tile_w * 0.5;
            assert_eq!(
                hit_test(oled, tx, STYLE_CARD_Y + 34.),
                HIT_BG_STYLE_BASE + i as i32
            );
        }
        // Opacity pills (70, 80, 90, 100) right-aligned in opacity row
        for i in 0..4 {
            let ox = CONTENT_X + CONTENT_W - 14. - 4. * 52. - 3. * 4. + i as f64 * 56. + 26.;
            assert_eq!(
                hit_test(oled, ox, oled_layout.opacity_row_y + 22.),
                HIT_BG_OPACITY_BASE + i as i32
            );
        }
        // No blur slider hit on OLED
        assert_eq!(
            hit_test(oled, 300., oled_layout.opacity_row_y + 50.),
            HIT_NONE
        );

        // Frost/Aura/Slate without custom image hide Image & Framing rows and show Blur slider
        for style in [0u32, 1, 2] {
            let glass = HitContext {
                bg_style: style,
                has_custom_image: false,
                ..oled
            };
            let glass_layout = appearance_layout(glass.has_custom_image, glass.bg_style);
            assert!(glass_layout.image_row_y.is_none());
            assert!(glass_layout.framing_row_y.is_none());
            let blur_y = glass_layout
                .blur_row_y
                .expect("Frost/Aura/Slate show blur slider");
            assert_eq!(hit_test(glass, 300., blur_y + 34.), HIT_BG_BLUR);
            for probe_y in [OPT_CARD_Y + 22., OPT_CARD_Y + 66., OPT_CARD_Y + 110.] {
                for probe_x in [
                    CONTENT_X + CONTENT_W - 140.,
                    CONTENT_X + CONTENT_W - 108.,
                    CONTENT_X + CONTENT_W - 44.,
                ] {
                    let h = hit_test(glass, probe_x, probe_y);
                    assert_ne!(h, HIT_BG_CHOOSE);
                    assert_ne!(h, HIT_BG_RESET);
                    assert!(!(HIT_BG_POS_BASE..HIT_BG_POS_BASE + 3).contains(&h));
                }
            }
        }

        // Image without custom image: Choose visible, Reset hidden, Framing visible, Blur visible
        let img_empty = HitContext {
            bg_style: 4,
            has_custom_image: false,
            ..oled
        };
        let l_empty = appearance_layout(false, 4);
        let img_y = l_empty.image_row_y.expect("Image row shown");
        let frame_y = l_empty.framing_row_y.expect("Framing row shown");
        assert_eq!(
            hit_test(img_empty, CONTENT_X + CONTENT_W - 44., img_y + 22.),
            HIT_BG_CHOOSE
        );
        assert_eq!(
            hit_test(img_empty, CONTENT_X + CONTENT_W - 110., img_y + 22.),
            HIT_NONE
        );
        for i in 0..3 {
            let fx = CONTENT_X + CONTENT_W - 14. - 3. * 58. - 2. * 4. + i as f64 * 62. + 29.;
            assert_eq!(
                hit_test(img_empty, fx, frame_y + 22.),
                HIT_BG_POS_BASE + i as i32
            );
        }

        // Image with custom image: both Choose and Reset visible
        let img_loaded = HitContext {
            bg_style: 4,
            has_custom_image: true,
            ..oled
        };
        assert_eq!(
            hit_test(img_loaded, CONTENT_X + CONTENT_W - 108., img_y + 22.),
            HIT_BG_CHOOSE
        );
        assert_eq!(
            hit_test(img_loaded, CONTENT_X + CONTENT_W - 38., img_y + 22.),
            HIT_BG_RESET
        );

        // Blur mapping bounds
        assert_eq!(blur_from_x(BLUR_TRACK_X0 - 20.), 0);
        assert_eq!(blur_from_x(BLUR_TRACK_X0), 0);
        assert_eq!(blur_from_x((BLUR_TRACK_X0 + BLUR_TRACK_X1) * 0.5), 15);
        assert_eq!(blur_from_x(BLUR_TRACK_X1), 30);
        assert_eq!(blur_from_x(BLUR_TRACK_X1 + 20.), 30);
    }

    #[test]
    fn updates_conditional_hit_targets() {
        let idle = ctx(SettingsSection::Updates);
        assert_eq!(hit_test(idle, 350., UPD_CARD_Y + 26.), HIT_UPD_AUTO);
        assert_eq!(
            hit_test(idle, UPD_ACTION_X + 60., UPD_ACTION_Y + 15.),
            HIT_UPD_ACTION
        );
        assert_eq!(
            hit_test(idle, UPD_CANCEL_X + 40., UPD_ACTION_Y + 15.),
            HIT_NONE
        );

        let downloading = HitContext {
            update_action_enabled: false,
            update_downloading: true,
            ..idle
        };
        assert_eq!(
            hit_test(downloading, UPD_ACTION_X + 60., UPD_ACTION_Y + 15.),
            HIT_NONE
        );
        assert_eq!(
            hit_test(downloading, UPD_CANCEL_X + 40., UPD_ACTION_Y + 15.),
            HIT_UPD_CANCEL
        );
    }

    #[test]
    fn clamp_window_pos_handles_primary_and_negative_secondary_monitors() {
        // Primary monitor (0, 0, 1920, 1040)
        assert_eq!(
            clamp_window_pos(650, 310, 620, 420, (0, 0, 1920, 1040)),
            (650, 310)
        );
        assert_eq!(
            clamp_window_pos(-100, -50, 620, 420, (0, 0, 1920, 1040)),
            (0, 0)
        );
        assert_eq!(
            clamp_window_pos(1800, 900, 620, 420, (0, 0, 1920, 1040)),
            (1300, 620)
        );

        // Secondary monitor to the left (-1920, 0, 0, 1080)
        assert_eq!(
            clamp_window_pos(-1500, 200, 620, 420, (-1920, 0, 0, 1080)),
            (-1500, 200)
        );
        assert_eq!(
            clamp_window_pos(-2100, 900, 620, 420, (-1920, 0, 0, 1080)),
            (-1920, 660)
        );
        assert_eq!(
            clamp_window_pos(-200, -10, 620, 420, (-1920, 0, 0, 1080)),
            (-620, 0)
        );

        // Oversized window falls back cleanly to work area origin
        assert_eq!(
            clamp_window_pos(100, 100, 800, 600, (10, 20, 500, 400)),
            (10, 20)
        );
    }

    #[test]
    fn auto_switch_spring_animates_and_snaps_on_reduced_motion() {
        let (mut pos, mut vel) = (0., 0.);
        assert!(advance_switch(&mut pos, &mut vel, true, false, 0.016));
        assert!(pos > 0. && pos < 1.);

        assert!(advance_switch(&mut pos, &mut vel, true, true, 0.016));
        assert_eq!(pos, 1.);
        assert_eq!(vel, 0.);
        assert!(!advance_switch(&mut pos, &mut vel, true, true, 0.016));
    }

    #[test]
    fn settings_window_section_switch_and_blur_drag_cleanup() {
        let mut w = SettingsWindow::new();
        assert!(!w.is_open());
        assert_eq!(w.section(), SettingsSection::Notch);
        w.open = true;
        w.blur_drag = true;
        w.pressed = HIT_BG_BLUR;
        w.set_section(SettingsSection::Appearance);
        assert_eq!(w.section(), SettingsSection::Appearance);
        assert!(!w.blur_drag);
        assert_eq!(w.pressed, HIT_NONE);

        unsafe { w.hide() };
        assert!(!w.is_open());
        assert_eq!(w.hover, HIT_NONE);
        assert_eq!(w.pressed, HIT_NONE);
        assert!(!w.blur_drag);
    }

    #[test]
    fn settings_window_appearance_style_and_opacity_clicks_update_settings() {
        let mut s = crate::tests::create_test_state();
        s.smoke = true;
        s.expanded = false;
        s.active_tab = 0;
        s.settings_win.open = true;

        unsafe {
            SettingsWindow::handle_click(&mut s, HIT_NAV_BASE + 2);
            assert_eq!(s.settings_win.section(), SettingsSection::Appearance);
            assert!(!s.expanded);

            SettingsWindow::handle_click(&mut s, HIT_BG_STYLE_BASE);
            assert_eq!(s.settings.bg_style, 0);

            SettingsWindow::handle_click(&mut s, HIT_BG_OPACITY_BASE + 1);
            assert_eq!(s.settings.bg_opacity, 80);

            SettingsWindow::handle_click(&mut s, HIT_CLOSE);
            assert!(!s.settings_win.is_open());
            assert_eq!(s.active_tab, 0);
            assert!(!s.expanded);
        }
    }

    #[test]
    fn window_anchor_point_uses_center_for_multimonitor_selection() {
        assert_eq!(window_anchor_point((-10, 200), 620, 420), (300, 410));
        assert_eq!(window_anchor_point((-700, 100), 620, 420), (-390, 310));
    }

    #[test]
    fn text_draw_flags_include_dt_noprefix_and_updates_fits_four_note_lines() {
        assert_ne!(TEXT_DRAW_FLAGS & 0x0800, 0);
        assert_eq!(UPD_NOTE_LINES, 4);
        assert!(UPD_CARD_Y + UPD_AUTO_H + UPD_BODY_H < UPD_LOG_CARD_Y);
        assert!(UPD_LOG_CARD_Y + UPD_LOG_CARD_H < SET_WIN_H);
    }

    #[test]
    fn settings_proc_handles_close_escape_mouseleave_outside_release_and_capture_loss() {
        unsafe {
            let class = wide("STATIC");
            let hwnd = CreateWindowExW(
                0,
                class.as_ptr(),
                class.as_ptr(),
                0,
                0,
                0,
                100,
                100,
                null_mut(),
                null_mut(),
                GetModuleHandleW(std::ptr::null()),
                null_mut(),
            );
            assert!(!hwnd.is_null());

            let mut s = crate::tests::create_test_state();
            s.smoke = true;
            s.scale = 1.0;
            s.settings_win.open = true;
            SetWindowLongPtrW(hwnd, -21, &mut s as *mut State as isize);

            let pack =
                |x: i16, y: i16| -> isize { ((y as u16 as isize) << 16) | (x as u16 as isize) };

            // WM_CLOSE hides popup and returns 0
            assert_eq!(settings_proc(hwnd, 0x0010, 0, 0), 0);
            assert!(!s.settings_win.is_open());

            // Esc hides popup
            s.settings_win.open = true;
            assert_eq!(settings_proc(hwnd, 0x0100, 0x1B, 0), 0);
            assert!(!s.settings_win.is_open());

            // Modal dialog guard blocks WM_CLOSE and Esc from hiding owner HWND
            s.settings_win.open = true;
            s.modal_dialog_open = true;
            assert_eq!(settings_proc(hwnd, 0x0010, 0, 0), 0);
            assert!(s.settings_win.is_open());
            assert_eq!(settings_proc(hwnd, 0x0100, 0x1B, 0), 0);
            assert!(s.settings_win.is_open());
            s.modal_dialog_open = false;

            // WM_LBUTTONDOWN + WM_MOUSELEAVE clears both hover and pressed
            let nav_l = pack(80, (NAV_Y0 + NAV_STEP + 16.) as i16);
            settings_proc(hwnd, 0x0200, 0, nav_l);
            settings_proc(hwnd, 0x0201, 0, nav_l);
            assert_eq!(s.settings_win.pressed, HIT_NAV_BASE + 1);
            settings_proc(hwnd, 0x02A3, 0, 0);
            assert_eq!(s.settings_win.hover, HIT_NONE);
            assert_eq!(s.settings_win.pressed, HIT_NONE);

            // Press close button then release outside: clears pressed & hover without hiding
            let close_l = pack(596, 20);
            settings_proc(hwnd, 0x0200, 0, close_l);
            settings_proc(hwnd, 0x0201, 0, close_l);
            assert_eq!(s.settings_win.pressed, HIT_CLOSE);
            settings_proc(hwnd, 0x0202, 0, pack(-20, -20));
            assert!(s.settings_win.is_open());
            assert_eq!(s.settings_win.pressed, HIT_NONE);
            assert_eq!(s.settings_win.hover, HIT_NONE);

            // Blur slider drag release outside clears blur_drag, pressed, and hover
            s.settings_win.set_section(SettingsSection::Appearance);
            s.settings.bg_style = 0;
            let blur_y = appearance_layout(false, 0).blur_row_y.unwrap() + 34.;
            settings_proc(hwnd, 0x0201, 0, pack(300, blur_y as i16));
            assert!(s.settings_win.blur_drag);
            settings_proc(hwnd, 0x0202, 0, pack(-50, blur_y as i16));
            assert!(!s.settings_win.blur_drag);
            assert_eq!(s.settings_win.pressed, HIT_NONE);
            assert_eq!(s.settings_win.hover, HIT_NONE);
            assert!(s.settings.bg_blur <= 30);

            // WM_CAPTURECHANGED during blur_drag clears blur_drag, pressed, and hover
            settings_proc(hwnd, 0x0201, 0, pack(300, blur_y as i16));
            assert!(s.settings_win.blur_drag);
            settings_proc(hwnd, 0x0215, 0, 0);
            assert!(!s.settings_win.blur_drag);
            assert_eq!(s.settings_win.pressed, HIT_NONE);
            assert_eq!(s.settings_win.hover, HIT_NONE);

            SetWindowLongPtrW(hwnd, -21, 0);
            DestroyWindow(hwnd);
        }
    }

    #[test]
    fn settings_window_springs_animate_open_nav_section_and_segmented_controls() {
        let mut w = SettingsWindow::new();
        w.open = true;
        w.open_t = 0.01;
        w.set_section(SettingsSection::Appearance);
        assert_eq!(w.section_t, 0.);
        assert_eq!(w.section_dir, 1.);

        // Springs advance smoothly toward targets when reduced_motion is false
        assert!(w.advance(true, 0, 80, 2, false, 0.016));
        assert!(w.open_t > 0.01 && w.open_t < 1.);
        assert!(w.nav_y > NAV_Y0 && w.nav_y < NAV_Y0 + 2. * NAV_STEP);
        assert!(w.section_t > 0. && w.section_t < 1.);
        assert!(w.style_t < 3. && w.style_t > 0.);
        assert!(w.opacity_t < 3. && w.opacity_t > 1.);
        assert!(w.framing_t > 1. && w.framing_t < 2.);

        // Reduced motion snaps every spring immediately
        assert!(w.advance(true, 0, 80, 2, true, 0.016));
        assert_eq!(w.open_t, 1.);
        assert_eq!(w.nav_y, NAV_Y0 + 2. * NAV_STEP);
        assert_eq!(w.section_t, 1.);
        assert_eq!(w.style_t, 0.);
        assert_eq!(w.opacity_t, 1.);
        assert_eq!(w.framing_t, 2.);

        // Closing animation glides down until invisible
        w.open = false;
        assert!(w.is_visible());
        for _ in 0..80 {
            w.advance(true, 0, 80, 2, false, 0.016);
        }
        assert!(!w.is_visible());
        assert_eq!(w.open_t, 0.);
    }
}
