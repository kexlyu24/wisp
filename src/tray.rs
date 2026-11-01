//! Notification-area (tray) icon. Its menu (right-click, or left-click) opens the Settings page or quits.
use crate::{
    wide, AppendMenuW, BitmapInfo, CreateDIBSection, CreatePopupMenu, DeleteObject, DestroyMenu,
    GetCursorPos, GetSystemMetrics, Point, SetForegroundWindow, TrackPopupMenu,
};
use std::ffi::c_void;
use std::ptr::null_mut;
type Handle = *mut c_void;

/// Window message the shell sends for tray-icon mouse events (lParam holds the mouse message).
pub const CALLBACK: u32 = 0x8010;

/// Menu item ids returned by `Tray::handle`.
pub mod item {
    pub const QUIT: usize = 1;
    pub const SETTINGS: usize = 2;
    pub const NOTIFICATIONS: usize = 10;
    pub const OPEN_ON_HOVER: usize = 11;
    pub const COLLAPSED_LYRICS: usize = 12;
    pub const CLOCK_24H: usize = 13;
    pub const AUTOSTART: usize = 14;
    pub const FOLLOW_ANIMATIONS: usize = 15;
    pub const MINIMAL: usize = 16;
}

#[repr(C)]
struct NotifyIconData {
    size: u32,
    hwnd: Handle,
    id: u32,
    flags: u32,
    callback: u32,
    icon: Handle,
    tip: [u16; 128],
    state: u32,
    state_mask: u32,
    info: [u16; 256],
    version: u32,
    info_title: [u16; 64],
    info_flags: u32,
    guid: [u8; 16],
    balloon_icon: Handle,
}
#[repr(C)]
struct IconInfo {
    icon: i32,
    x: u32,
    y: u32,
    mask: Handle,
    color: Handle,
}

#[link(name = "user32")]
extern "system" {
    fn CreateIconIndirect(info: *const IconInfo) -> Handle;
    fn DestroyIcon(icon: Handle) -> i32;
    fn RegisterWindowMessageW(name: *const u16) -> u32;
    fn PostMessageW(hwnd: Handle, msg: u32, w: usize, l: isize) -> i32;
}
#[link(name = "gdi32")]
extern "system" {
    fn CreateBitmap(w: i32, h: i32, planes: u32, bits: u32, data: *const c_void) -> Handle;
}
#[link(name = "shell32")]
extern "system" {
    fn Shell_NotifyIconW(message: u32, data: *const NotifyIconData) -> i32;
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
}

pub struct Tray {
    hwnd: Handle,
    icon: Handle,
    taskbar_created: u32,
}

impl Tray {
    pub unsafe fn new(hwnd: Handle) -> Self {
        let tray = Self {
            hwnd,
            icon: make_icon(),
            taskbar_created: RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()),
        };
        tray.add();
        tray
    }
    unsafe fn data(&self, flags: u32) -> NotifyIconData {
        let mut data: NotifyIconData = std::mem::zeroed();
        data.size = size_of::<NotifyIconData>() as u32;
        data.hwnd = self.hwnd;
        data.id = 1;
        data.flags = flags;
        data.callback = CALLBACK;
        data.icon = self.icon;
        for (slot, unit) in data.tip.iter_mut().zip("Wisp".encode_utf16()) {
            *slot = unit;
        }
        data
    }
    /// Adds the icon; also called again after Explorer restarts and recreates the taskbar.
    pub unsafe fn add(&self) {
        Shell_NotifyIconW(0, &self.data(1 | 2 | 4));
    }
    pub fn is_taskbar_created(&self, msg: u32) -> bool {
        msg != 0 && msg == self.taskbar_created
    }
    /// Redraws the icon for the current light/dark taskbar.
    pub unsafe fn refresh_theme(&mut self) {
        let old = self.icon;
        self.icon = make_icon();
        Shell_NotifyIconW(1, &self.data(2));
        DestroyIcon(old);
    }
    /// Handles a tray mouse event by showing the menu; returns the chosen item (see `item`), or 0.
    /// The settings themselves live on the notch's Settings page; the menu only opens it.
    pub unsafe fn handle(&self, lparam: isize) -> usize {
        let event = (lparam & 0xffff) as u32;
        if event != 0x205 && event != 0x7b && event != 0x202 {
            return 0;
        }
        let menu = CreatePopupMenu();
        AppendMenuW(menu, 0, item::SETTINGS, wide("Settings…").as_ptr());
        AppendMenuW(menu, 0x800, 0, std::ptr::null());
        AppendMenuW(menu, 0, item::QUIT, wide("Quit Wisp").as_ptr());
        let mut cursor = Point::default();
        GetCursorPos(&mut cursor);
        // The menu only dismisses on outside clicks if our window is foreground (documented quirk).
        SetForegroundWindow(self.hwnd);
        let chosen = TrackPopupMenu(
            menu,
            0x100 | 0x20 | 2,
            cursor.x,
            cursor.y,
            0,
            self.hwnd,
            std::ptr::null(),
        );
        DestroyMenu(menu);
        PostMessageW(self.hwnd, 0, 0, 0);
        chosen.max(0) as usize
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        unsafe {
            Shell_NotifyIconW(2, &self.data(0));
            DestroyIcon(self.icon);
        }
    }
}

fn light_taskbar() -> bool {
    let (mut value, mut size) = (0u32, 4u32);
    let current_user = (0x80000001u32 as i32) as isize as Handle;
    unsafe {
        RegGetValueW(
            current_user,
            wide("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize").as_ptr(),
            wide("SystemUsesLightTheme").as_ptr(),
            0x10,
            null_mut(),
            (&mut value as *mut u32).cast(),
            &mut size,
        ) == 0
            && value == 1
    }
}

/// Dynamic Island glyph: a wide pill with a small lens cut out at its right end, antialiased with
/// 4×4 supersampling, white on a dark taskbar and black on a light one.
unsafe fn make_icon() -> Handle {
    let size = GetSystemMetrics(49).max(16);
    let info = BitmapInfo {
        size: 40,
        width: size,
        height: -size,
        planes: 1,
        bits: 32,
        compression: 0,
        bytes: 0,
        x: 0,
        y: 0,
        used: 0,
        important: 0,
    };
    let mut bits = null_mut();
    let color = CreateDIBSection(null_mut(), &info, 0, &mut bits, null_mut(), 0);
    if color.is_null() {
        return null_mut();
    }
    let rgb: u32 = if light_taskbar() { 0x000000 } else { 0xffffff };
    let s = size as f64;
    let (cx, cy) = (s / 2., s / 2.);
    let (half_w, half_h) = (s * 0.45, s * 0.2);
    let (lens_x, lens_r) = (cx + half_w - half_h, half_h * 0.45);
    let inside = |x: f64, y: f64| {
        let dx = ((x - cx).abs() - (half_w - half_h)).max(0.);
        let dy = (y - cy).abs();
        let pill = dx * dx + dy * dy <= half_h * half_h;
        let lens = (x - lens_x).powi(2) + (y - cy).powi(2) <= lens_r * lens_r;
        pill && !lens
    };
    let pixels = std::slice::from_raw_parts_mut(bits as *mut u32, (size * size) as usize);
    for py in 0..size {
        for px in 0..size {
            let mut hits = 0;
            for sy in 0..4 {
                for sx in 0..4 {
                    if inside(
                        px as f64 + (sx as f64 + 0.5) / 4.,
                        py as f64 + (sy as f64 + 0.5) / 4.,
                    ) {
                        hits += 1;
                    }
                }
            }
            let alpha = (hits * 255 / 16) as u32;
            pixels[(py * size + px) as usize] = (alpha << 24) | rgb;
        }
    }
    let mask = CreateBitmap(size, size, 1, 1, std::ptr::null());
    let icon = CreateIconIndirect(&IconInfo {
        icon: 1,
        x: 0,
        y: 0,
        mask,
        color,
    });
    DeleteObject(mask);
    DeleteObject(color);
    icon
}
