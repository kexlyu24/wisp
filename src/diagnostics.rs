//! Bounded local diagnostics. Never log user content or credentials.
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};
const MAX_BYTES: u64 = 2 * 1024 * 1024;
static LOCK: Mutex<()> = Mutex::new(());
static ROOT: OnceLock<PathBuf> = OnceLock::new();
#[repr(C)]
#[derive(Default)]
struct Time {
    year: u16,
    month: u16,
    weekday: u16,
    day: u16,
    hour: u16,
    minute: u16,
    second: u16,
    millis: u16,
}
#[link(name = "kernel32")]
extern "system" {
    fn GetLocalTime(time: *mut Time);
}
fn line(level: &str, component: &str, message: &str) -> String {
    let mut time = Time::default();
    unsafe {
        GetLocalTime(&mut time);
    }
    let clean: String = message
        .chars()
        .take(2048)
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    format!(
        "[{:04}-{:02}-{:02} {:02}:{:02}:{:02} local] [{}] [{}] [pid={}] {}\n",
        time.year,
        time.month,
        time.day,
        time.hour,
        time.minute,
        time.second,
        level,
        component,
        std::process::id(),
        clean
    )
}
fn append(root: &Path, record: &str, limit: u64) -> std::io::Result<()> {
    fs::create_dir_all(root)?;
    let path = root.join("wisp.log");
    if fs::metadata(&path).map(|m| m.len()).unwrap_or(0) + record.len() as u64 > limit {
        let oldest = root.join("wisp.log.3");
        if oldest.exists() {
            fs::remove_file(oldest)?;
        }
        for n in (1..=2).rev() {
            let from = root.join(format!("wisp.log.{n}"));
            if from.exists() {
                fs::rename(from, root.join(format!("wisp.log.{}", n + 1)))?;
            }
        }
        if path.exists() {
            fs::rename(&path, root.join("wisp.log.1"))?;
        }
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(record.as_bytes())?;
    file.flush()
}
pub fn init() {
    if let Some(root) = std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("Wisp")) {
        let _ = ROOT.set(root);
    }
    info(
        "app",
        &format!(
            "Wisp {} started; architecture={}; timestamps=local time",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::ARCH
        ),
    );
}
pub fn log(level: &str, component: &str, message: &str) {
    let Some(root) = ROOT.get() else { return };
    // A poisoned/busy logger must never deadlock panic handling or block other workers.
    let Ok(_guard) = LOCK.try_lock() else { return };
    let _ = append(root, &line(level, component, message), MAX_BYTES);
}
pub fn read_text(path: impl AsRef<Path>) -> Option<String> {
    match fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            failure("storage", "Read application data", &e);
            None
        }
    }
}
pub fn info(component: &str, message: &str) {
    log("INFO", component, message);
}
pub fn error(component: &str, message: &str) {
    log("ERROR", component, message);
}
pub fn limited_error(component: &'static str, message: &'static str) {
    static RECENT: Mutex<Vec<(&'static str, &'static str, std::time::Instant)>> =
        Mutex::new(Vec::new());
    let Ok(mut recent) = RECENT.try_lock() else {
        return;
    };
    if let Some((_, _, last)) = recent
        .iter_mut()
        .find(|(c, m, _)| *c == component && *m == message)
    {
        if last.elapsed().as_secs() < 60 {
            return;
        }
        *last = std::time::Instant::now();
    } else {
        recent.push((component, message, std::time::Instant::now()));
    }
    drop(recent);
    error(component, message);
}
pub fn failure(component: &str, operation: &str, e: &std::io::Error) {
    error(
        component,
        &format!(
            "{operation} failed; kind={:?}; os_error={:?}",
            e.kind(),
            e.raw_os_error()
        ),
    );
}
/// Separate, best-effort crash record avoids the normal logger mutex.
pub fn crash(code: u32) {
    if let Some(root) = ROOT.get() {
        let _ = fs::write(
            root.join("wisp_crash.log"),
            line(
                "ERROR",
                "crash",
                &format!("Unhandled native exception; code=0x{code:08X}"),
            ),
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostics_records_are_bounded_single_line_english() {
        let record = line(
            "ERROR",
            "network",
            &format!("Download failed\n{}", "x".repeat(3000)),
        );
        assert_eq!(record.lines().count(), 1);
        assert!(record.contains("[ERROR] [network]"));
        assert!(record.len() < 2300);
    }
    #[test]
    fn diagnostics_rotation_keeps_three_archives() {
        let root = std::env::temp_dir().join(format!("wisp-log-test-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        for _ in 0..6 {
            append(&root, "test record\n", 13).unwrap();
        }
        assert!(root.join("wisp.log.3").exists());
        assert!(!root.join("wisp.log.4").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
