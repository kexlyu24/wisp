//! Notification-area (tray) icon. Its menu (right-click, or left-click) opens the Settings popup or quits.
use crate::{
    wide, AppendMenuW, BitmapInfo, CreateDIBSection, CreatePopupMenu, DeleteObject, DestroyMenu,
    GetCursorPos, GetSystemMetrics, Point, SetForegroundWindow, TrackPopupMenu,
};
use std::ffi::c_void;
use std::ptr::null_mut;
type Handle = *mut c_void;

const TRAY_DARK_PNG: &[u8] = include_bytes!("../assets/icons/wisp-tray-dark.png");
const TRAY_LIGHT_PNG: &[u8] = include_bytes!("../assets/icons/wisp-tray-light.png");

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
    /// The settings live in the standalone Settings popup window; the menu only opens it.
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

/// Decodes an embedded PNG into premultiplied BGRA pixels scaled cleanly to `size × size`.
pub(crate) fn decode_icon_bgra(bytes: &[u8], size: u32) -> Option<Vec<u32>> {
    let size = size.max(1);
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let resized = if img.width() == size && img.height() == size {
        img
    } else {
        image::imageops::resize(&img, size, size, image::imageops::FilterType::Lanczos3)
    };
    let mut out = Vec::with_capacity((size * size) as usize);
    for px in resized.pixels() {
        let [r, g, b, a] = px.0;
        let a32 = a as u32;
        let pr = (r as u32 * a32 + 127) / 255;
        let pg = (g as u32 * a32 + 127) / 255;
        let pb = (b as u32 * a32 + 127) / 255;
        out.push((a32 << 24) | (pr << 16) | (pg << 8) | pb);
    }
    Some(out)
}

/// Decodes the dark-surface Wisp W mark for in-app dark UI (e.g. Settings popup titlebar).
pub(crate) fn titlebar_mark_bgra(size: u32) -> Option<Vec<u32>> {
    decode_icon_bgra(TRAY_DARK_PNG, size)
}

/// Builds a Win32 `HICON` from premultiplied BGRA pixels of dimensions `size × size`.
unsafe fn hicon_from_bgra(src: &[u32], size: i32) -> Handle {
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
    if color.is_null() || bits.is_null() {
        if !color.is_null() {
            DeleteObject(color);
        }
        return null_mut();
    }
    let pixels = std::slice::from_raw_parts_mut(bits as *mut u32, (size * size) as usize);
    pixels.copy_from_slice(&src[..(size * size) as usize]);
    let row_bytes = (((size + 31) / 32) * 4) as usize;
    let mask_bytes = vec![0u8; row_bytes * size as usize];
    let mask = CreateBitmap(size, size, 1, 1, mask_bytes.as_ptr().cast());
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

/// Creates a Win32 window `HICON` for Wisp (used by `WispNotch` and `WispSettings` window classes).
pub(crate) unsafe fn make_window_icon(size: i32) -> Handle {
    let size = size.max(16);
    match decode_icon_bgra(TRAY_LIGHT_PNG, size as u32) {
        Some(bgra) => hicon_from_bgra(&bgra, size),
        None => null_mut(),
    }
}

/// Wisp tray icon: uses the notch+W icon on light taskbars and the vibrant W mark on dark ones.
unsafe fn make_icon() -> Handle {
    let size = GetSystemMetrics(49).max(16);
    let bytes = if light_taskbar() {
        TRAY_LIGHT_PNG
    } else {
        TRAY_DARK_PNG
    };
    match decode_icon_bgra(bytes, size as u32) {
        Some(bgra) => hicon_from_bgra(&bgra, size),
        None => null_mut(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_wisp_icons_decode_to_premultiplied_bgra() {
        for bytes in [TRAY_DARK_PNG, TRAY_LIGHT_PNG] {
            let pixels = decode_icon_bgra(bytes, 24).expect("embedded icon should decode");
            assert_eq!(pixels.len(), 24 * 24);
            let opaque_count = pixels.iter().filter(|&&p| (p >> 24) > 200).count();
            assert!(opaque_count > 30, "expected visible opaque icon pixels");
            for &p in &pixels {
                let a = p >> 24;
                assert!(((p >> 16) & 0xff) <= a);
                assert!(((p >> 8) & 0xff) <= a);
                assert!((p & 0xff) <= a);
            }
        }
    }
}
