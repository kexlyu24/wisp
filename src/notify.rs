//! Windows notifications for the notch. Windows keeps every toast in the Action Center's SQLite
//! database (`%LOCALAPPDATA%\Microsoft\Windows\Notifications\wpndatabase.db`); a worker thread
//! reads new rows read-only through the system's `winsqlite3.dll`, so nothing extra is linked.
//! This is not a documented API, so every step fails quietly: no database, no notifications.
use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr::null_mut;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;
use windows::core::{Interface, HSTRING};
use windows::Win32::Foundation::SIZE;
use windows::Win32::Graphics::Gdi::{CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS};
use windows::Win32::System::Com::{CoInitializeEx, CoTaskMemFree, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Shell::{
    FOLDERID_AppsFolder, IShellItem, IShellItemImageFactory, SHCreateItemInKnownFolder, KF_FLAG_DEFAULT, SIGDN_NORMALDISPLAY,
    SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY,
};
type Handle = *mut c_void;

/// Icons are handed over as `ICON_PX`² premultiplied BGRA pixels.
pub const ICON_PX: usize = 96;

pub struct Notice {
    /// Application User Model ID; `shell:AppsFolder\<aumid>` opens the app.
    pub aumid: String,
    pub app: String,
    pub title: String,
    pub body: String,
    pub icon: Option<Vec<u32>>,
    /// The icon is a picture (an avatar or logo override) rather than the app's own icon.
    pub picture: bool,
    /// The sender asked for a circular crop (`hint-crop="circle"`).
    pub circle: bool,
}

static MODULE: std::sync::LazyLock<usize> = std::sync::LazyLock::new(|| unsafe {
    crate::LoadLibraryW(crate::wide("winsqlite3.dll").as_ptr()) as usize
});
macro_rules! sqlite {
    ($(fn $name:ident($($arg:ident:$ty:ty),*) -> $ret:ty;)*) => {$(
        #[allow(non_snake_case)]
        unsafe fn $name($($arg:$ty),*) -> Option<$ret> {
            static FUNCTION: std::sync::LazyLock<usize> = std::sync::LazyLock::new(|| unsafe {
                if *MODULE == 0 { 0 } else { crate::GetProcAddress(*MODULE as Handle, concat!(stringify!($name), "\0").as_ptr()) as usize }
            });
            if *FUNCTION == 0 { return None; }
            let f: unsafe extern "C" fn($($ty),*) -> $ret = std::mem::transmute(*FUNCTION);
            Some(f($($arg),*))
        }
    )*};
}
sqlite! {
    fn sqlite3_open_v2(name: *const u8, db: *mut Handle, flags: i32, vfs: *const u8) -> i32;
    fn sqlite3_busy_timeout(db: Handle, ms: i32) -> i32;
    fn sqlite3_prepare_v2(db: Handle, sql: *const u8, bytes: i32, stmt: *mut Handle, tail: *mut *const u8) -> i32;
    fn sqlite3_bind_int64(stmt: Handle, index: i32, value: i64) -> i32;
    fn sqlite3_step(stmt: Handle) -> i32;
    fn sqlite3_column_int64(stmt: Handle, column: i32) -> i64;
    fn sqlite3_column_blob(stmt: Handle, column: i32) -> *const u8;
    fn sqlite3_column_text(stmt: Handle, column: i32) -> *const u8;
    fn sqlite3_column_bytes(stmt: Handle, column: i32) -> i32;
    fn sqlite3_finalize(stmt: Handle) -> i32;
}
const ROW: i32 = 100;

struct Db(Handle);
impl Db {
    unsafe fn open() -> Option<Self> {
        let path = std::env::var("LOCALAPPDATA").ok()? + "\\Microsoft\\Windows\\Notifications\\wpndatabase.db";
        if !std::path::Path::new(&path).exists() {
            return None;
        }
        let name = format!("{path}\0");
        let mut db = null_mut();
        // SQLITE_OPEN_READONLY: Windows owns the file; we only ever read it.
        if sqlite3_open_v2(name.as_ptr(), &mut db, 1, std::ptr::null())? != 0 || db.is_null() {
            return None;
        }
        sqlite3_busy_timeout(db, 250);
        Some(Self(db))
    }
    /// Runs `sql` with `?1` bound to `arg`, calling `row` for each result row.
    unsafe fn query(&self, sql: &str, arg: i64, mut row: impl FnMut(Handle)) -> bool {
        let sql = format!("{sql}\0");
        let mut stmt = null_mut();
        if sqlite3_prepare_v2(self.0, sql.as_ptr(), -1, &mut stmt, null_mut()) != Some(0) || stmt.is_null() {
            return false;
        }
        sqlite3_bind_int64(stmt, 1, arg);
        while sqlite3_step(stmt) == Some(ROW) {
            row(stmt);
        }
        sqlite3_finalize(stmt);
        true
    }
}

unsafe fn column_bytes(stmt: Handle, column: i32, blob: bool) -> Vec<u8> {
    let ptr = if blob { sqlite3_column_blob(stmt, column) } else { sqlite3_column_text(stmt, column) }.unwrap_or(std::ptr::null());
    let len = sqlite3_column_bytes(stmt, column).unwrap_or(0);
    if ptr.is_null() || len <= 0 {
        return Vec::new();
    }
    std::slice::from_raw_parts(ptr, len as usize).to_vec()
}

/// Worker that reports each new toast once. Notifications already in the database at start-up
/// are skipped, as are ones that arrived more than 30 s before they were seen.
pub fn start() -> Receiver<Notice> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || unsafe { run(tx) });
    rx
}

unsafe fn run(tx: Sender<Notice>) {
    let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    let Some(db) = Db::open() else { return };
    let mut last = 0i64;
    db.query("select max([Order]) from Notification where ?1=?1", 0, |stmt| last = sqlite3_column_int64(stmt, 0).unwrap_or(0));
    let mut apps: HashMap<String, (String, Option<Vec<u32>>)> = HashMap::new();
    loop {
        std::thread::sleep(Duration::from_millis(700));
        let mut rows = Vec::new();
        let ok = db.query(
            "select n.[Order], h.PrimaryId, n.Payload, n.ArrivalTime from Notification n \
             join NotificationHandler h on h.RecordId = n.HandlerId \
             where n.[Order] > ?1 and n.Type = 'toast' order by n.[Order]",
            last,
            |stmt| {
                rows.push((
                    sqlite3_column_int64(stmt, 0).unwrap_or(0),
                    String::from_utf8_lossy(&column_bytes(stmt, 1, false)).into_owned(),
                    String::from_utf8_lossy(&column_bytes(stmt, 2, true)).into_owned(),
                    sqlite3_column_int64(stmt, 3).unwrap_or(0),
                ))
            },
        );
        if !ok {
            continue;
        }
        for (order, aumid, payload, arrival) in rows {
            last = last.max(order);
            if filetime_now() - arrival > 30 * 10_000_000 {
                continue;
            }
            let Some(toast) = parse_toast(&payload) else { continue };
            let (app, app_icon) = apps.entry(aumid.clone()).or_insert_with(|| app_info(&aumid)).clone();
            let picture = toast.image.as_deref().and_then(load_picture);
            let notice = Notice {
                app,
                title: toast.title,
                body: toast.body,
                picture: picture.is_some(),
                icon: picture.or(app_icon),
                circle: toast.circle,
                aumid,
            };
            if tx.send(notice).is_err() {
                return;
            }
        }
    }
}

fn filetime_now() -> i64 {
    let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    (unix.as_nanos() / 100) as i64 + 116_444_736_000_000_000
}

#[derive(Debug, PartialEq)]
pub struct Toast {
    pub title: String,
    pub body: String,
    /// `appLogoOverride` image path, if it is a local file.
    pub image: Option<String>,
    pub circle: bool,
}

/// Reads the visible text of a toast's XML: the first `<text>` is the title, the rest the body.
pub fn parse_toast(xml: &str) -> Option<Toast> {
    let binding = xml.find("<binding").map(|i| &xml[i..]).unwrap_or(xml);
    let mut texts = Vec::new();
    let mut rest = binding;
    while let Some(start) = rest.find("<text") {
        let after = &rest[start + 5..];
        let Some(open_end) = after.find('>') else { break };
        let attrs = &after[..open_end];
        if attrs.ends_with('/') {
            rest = &after[open_end + 1..];
            continue;
        }
        let content = &after[open_end + 1..];
        let Some(close) = content.find("</text>") else { break };
        let text = unescape_xml(&content[..close]);
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if !text.is_empty() && !attrs.contains("placement=\"attribution\"") {
            texts.push(text);
        }
        rest = &content[close + 7..];
    }
    let mut texts = texts.into_iter();
    let title = texts.next()?;
    let body = texts.collect::<Vec<_>>().join(" · ");
    let mut image = None;
    let mut circle = false;
    let mut from = 0;
    while let Some(i) = xml[from..].find("<image") {
        let tag_start = from + i;
        let tag = &xml[tag_start..xml[tag_start..].find('>').map_or(xml.len(), |e| tag_start + e)];
        from = tag_start + 6;
        if !tag.contains("placement=\"appLogoOverride\"") {
            continue;
        }
        circle = tag.contains("hint-crop=\"circle\"");
        image = attribute(tag, "src").and_then(|src| local_path(&unescape_xml(src)));
        break;
    }
    Some(Toast { title, body, image, circle })
}

fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("{name}=\"");
    let start = tag.find(&key)? + key.len();
    let end = tag[start..].find('"')? + start;
    Some(&tag[start..end])
}

/// `C:\…` or `file:///C:/…` (percent-encoded) become a local path; web and app-data URIs are skipped.
fn local_path(src: &str) -> Option<String> {
    let path = if let Some(rest) = src.strip_prefix("file:///") {
        percent_decode(rest).replace('/', "\\")
    } else if src.len() > 2 && src.as_bytes()[1] == b':' {
        src.to_string()
    } else {
        return None;
    };
    Some(path)
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = |b: u8| (b as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn unescape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let tail = &rest[amp..];
        let Some(semi) = tail.find(';').filter(|&n| n <= 10) else {
            out.push('&');
            rest = &tail[1..];
            continue;
        };
        let entity = &tail[1..semi];
        let ch = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            e if e.starts_with("#x") || e.starts_with("#X") => u32::from_str_radix(&e[2..], 16).ok().and_then(char::from_u32),
            e if e.starts_with('#') => e[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &tail[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// A logo-override picture from disk, square-cropped, as premultiplied BGRA.
fn load_picture(path: &str) -> Option<Vec<u32>> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() > 8_000_000 {
        return None;
    }
    let img = image::open(path).ok()?;
    let img = img.resize_to_fill(ICON_PX as u32, ICON_PX as u32, image::imageops::FilterType::Triangle).to_rgba8();
    Some(
        img.pixels()
            .map(|p| {
                let a = p[3] as u32;
                let pm = |c: u8| c as u32 * a / 255;
                (a << 24) | (pm(p[0]) << 16) | (pm(p[1]) << 8) | pm(p[2])
            })
            .collect(),
    )
}

/// The app's display name and icon from the shell's Apps folder (what Start shows).
unsafe fn app_info(aumid: &str) -> (String, Option<Vec<u32>>) {
    let fallback = aumid.rsplit(['!', '.']).next().unwrap_or(aumid).to_string();
    let Ok(item) = SHCreateItemInKnownFolder::<_, IShellItem>(&FOLDERID_AppsFolder, KF_FLAG_DEFAULT, &HSTRING::from(aumid)) else {
        return (fallback, None);
    };
    let name = match item.GetDisplayName(SIGDN_NORMALDISPLAY) {
        Ok(p) => {
            let s = p.to_string().unwrap_or_default();
            CoTaskMemFree(Some(p.0 as *const c_void));
            if s.is_empty() { fallback } else { s }
        }
        Err(_) => fallback,
    };
    let icon = item.cast::<IShellItemImageFactory>().ok().and_then(|factory| {
        let bitmap = factory.GetImage(SIZE { cx: ICON_PX as i32, cy: ICON_PX as i32 }, SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK).ok()?;
        let pixels = bitmap_pixels(bitmap);
        let _ = DeleteObject(bitmap);
        pixels
    });
    (name, icon)
}

/// Reads a shell bitmap as ICON_PX² premultiplied BGRA (scaled down if the shell gave a bigger one).
unsafe fn bitmap_pixels(bitmap: windows::Win32::Graphics::Gdi::HBITMAP) -> Option<Vec<u32>> {
    let dc = CreateCompatibleDC(None);
    let mut info = BITMAPINFO::default();
    info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
    if GetDIBits(dc, bitmap, 0, 0, None, &mut info, DIB_RGB_COLORS) == 0 {
        let _ = DeleteDC(dc);
        return None;
    }
    let (w, h) = (info.bmiHeader.biWidth, info.bmiHeader.biHeight.abs());
    if w <= 0 || h <= 0 || w > 1024 || h > 1024 {
        let _ = DeleteDC(dc);
        return None;
    }
    info.bmiHeader.biBitCount = 32;
    info.bmiHeader.biCompression = 0;
    info.bmiHeader.biHeight = -h;
    let mut raw = vec![0u32; (w * h) as usize];
    let lines = GetDIBits(dc, bitmap, 0, h as u32, Some(raw.as_mut_ptr().cast()), &mut info, DIB_RGB_COLORS);
    let _ = DeleteDC(dc);
    if lines == 0 {
        return None;
    }
    // Some shell bitmaps carry straight alpha, some premultiplied; any channel above alpha means straight.
    let straight = raw.iter().any(|&p| {
        let a = p >> 24;
        (p >> 16 & 255) > a || (p >> 8 & 255) > a || (p & 255) > a
    });
    let all_clear = raw.iter().all(|&p| p >> 24 == 0);
    let rgba = image::RgbaImage::from_fn(w as u32, h as u32, |x, y| {
        let p = raw[(y * w as u32 + x) as usize];
        let a = if all_clear { 255 } else { (p >> 24) as u8 };
        let un = |c: u32| if straight || a == 0 || all_clear { c as u8 } else { (c * 255 / a as u32).min(255) as u8 };
        image::Rgba([un(p >> 16 & 255), un(p >> 8 & 255), un(p & 255), a])
    });
    let img = image::imageops::resize(&rgba, ICON_PX as u32, ICON_PX as u32, image::imageops::FilterType::Triangle);
    Some(
        img.pixels()
            .map(|p| {
                let a = p[3] as u32;
                let pm = |c: u8| c as u32 * a / 255;
                (a << 24) | (pm(p[0]) << 16) | (pm(p[1]) << 8) | pm(p[2])
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_title_body_and_logo() {
        let xml = r#"<toast><visual><binding template="ToastGeneric"><text>Mira &amp; Co</text><text>Hey, are you
            coming?</text><text placement="attribution">via Web</text><image placement="appLogoOverride" hint-crop="circle" src="file:///C:/Users/Me/avatar%20one.png"/></binding></visual></toast>"#;
        let toast = parse_toast(xml).unwrap();
        assert_eq!(toast.title, "Mira & Co");
        assert_eq!(toast.body, "Hey, are you coming?");
        assert_eq!(toast.image.as_deref(), Some("C:\\Users\\Me\\avatar one.png"));
        assert!(toast.circle);
    }

    #[test]
    fn handles_title_only_and_empty_toasts() {
        let toast = parse_toast(r#"<toast><visual><binding template="ToastGeneric"><text>Done</text><text/></binding></visual></toast>"#).unwrap();
        assert_eq!((toast.title.as_str(), toast.body.as_str(), toast.image), ("Done", "", None));
        assert!(parse_toast("<toast><visual><binding template=\"ToastGeneric\"></binding></visual></toast>").is_none());
        let web = parse_toast(r#"<toast><visual><binding><text>A</text><image placement="appLogoOverride" src="https://x/y.png"/></binding></visual></toast>"#).unwrap();
        assert_eq!(web.image, None);
        assert_eq!(unescape_xml("&#x1F44B; &#65; &bogus; &"), "\u{1F44B} A &bogus; &");
    }
}

#[cfg(test)]
mod live {
    use super::*;

    /// Reads this PC's notification database: `cargo test live_notifications -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_notifications() {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let db = Db::open().expect("notification database");
            let mut rows = Vec::new();
            assert!(db.query(
                "select h.PrimaryId, n.Payload from Notification n join NotificationHandler h on h.RecordId = n.HandlerId \
                 where n.Type = 'toast' order by n.[Order] desc limit ?1",
                5,
                |stmt| rows.push((String::from_utf8_lossy(&column_bytes(stmt, 0, false)).into_owned(), String::from_utf8_lossy(&column_bytes(stmt, 1, true)).into_owned())),
            ));
            for (aumid, payload) in rows {
                let (app, icon) = app_info(&aumid);
                let toast = parse_toast(&payload);
                println!(
                    "{app:?} icon={} parsed={} title_len={} body_len={} picture={}",
                    icon.as_ref().map_or(0, |i| i.len()),
                    toast.is_some(),
                    toast.as_ref().map_or(0, |t| t.title.chars().count()),
                    toast.as_ref().map_or(0, |t| t.body.chars().count()),
                    toast.as_ref().and_then(|t| t.image.as_deref()).and_then(load_picture).is_some(),
                );
            }
        }
    }
}

#[cfg(test)]
mod leak {
    use super::*;
    #[repr(C)]
    #[derive(Default)]
    struct Counters { cb: u32, faults: u32, peak_ws: usize, ws: usize, qpp: usize, qp: usize, qnpp: usize, qnp: usize, pagefile: usize, peak_pagefile: usize, private: usize }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
        fn K32GetProcessMemoryInfo(p: *mut std::ffi::c_void, c: *mut Counters, cb: u32) -> i32;
    }
    fn private_mb() -> f64 {
        let mut c = Counters { cb: size_of::<Counters>() as u32, ..Default::default() };
        unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
        c.private as f64 / 1048576.
    }
    /// `cargo test notification_poll_memory -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn notification_poll_memory() {
        unsafe {
            let db = Db::open().expect("database");
            let poll = |db: &Db| {
                let mut n = 0;
                db.query(
                    "select n.[Order], h.PrimaryId, n.Payload, n.ArrivalTime from Notification n \
                     join NotificationHandler h on h.RecordId = n.HandlerId \
                     where n.[Order] > ?1 and n.Type = 'toast' order by n.[Order]",
                    i64::MAX - 1,
                    |_| n += 1,
                );
            };
            for _ in 0..50 { poll(&db); }
            let before = private_mb();
            let start = std::time::Instant::now();
            for _ in 0..3000 { poll(&db); }
            println!("3000 polls: {:+.1} MB, {:.2} ms per poll", private_mb() - before, start.elapsed().as_secs_f64() * 1000. / 3000.);
        }
    }
}
