//! HTTPS + SHA-256, opt-in portable updates. The UI never blocks on network or disk work.
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn updater_blocks_only_inflight_actions() {
        let mut u = Updater::default();
        u.last_check = now();
        for phase in [Phase::Idle, Phase::Error] {
            u.phase = phase;
            assert!(u.action_enabled());
        }
        for phase in [Phase::Checking, Phase::Downloading] {
            u.phase = phase;
            assert!(!u.action_enabled());
        }
    }
    #[test]
    fn updater_versions_compare_numerically_and_reject_prerelease() {
        assert!(version("v0.10.0").unwrap() > version("0.2.9").unwrap());
        for bad in ["1.2", "1.2.3-beta", "1.2.3/evil", "01.2.3", "1.2.3.4"] {
            assert!(version(bad).is_err());
        }
    }
    #[test]
    fn updater_manifest_requires_matching_version_size_and_hash() {
        let bytes = br#"{"version":"1.2.3","size":4096,"sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#;
        assert!(manifest(bytes, "v1.2.3").is_ok());
        assert!(manifest(bytes, "v1.2.4").is_err());
        assert!(manifest(b"{}", "v1.2.3").is_err());
        for bad in [
            serde_json::json!({"version":"1.2.3","size":4095,"sha256":"a".repeat(64)}),
            serde_json::json!({"version":"1.2.3","size":4096,"sha256":"z".repeat(64)}),
            serde_json::json!({"version":"1.2.3","size":4096,"sha256":"aa"}),
        ] {
            assert!(manifest(&serde_json::to_vec(&bad).unwrap(), "v1.2.3").is_err());
        }
    }
    #[test]
    fn updater_rejects_untrusted_asset_locations() {
        assert!(asset_url(
            "https://github.com/kexlyu24/wisp/releases/download/v1.2.3/wisp-windows-x64.exe",
            "v1.2.3",
            "wisp-windows-x64.exe"
        )
        .is_ok());
        for bad in [
            "http://github.com/kexlyu24/wisp/releases/download/v1.2.3/wisp-windows-x64.exe",
            "https://evil.test/wisp.exe",
            "https://github.com/kexlyu24/wisp/releases/download/v1.2.3/../wisp-windows-x64.exe",
        ] {
            assert!(asset_url(bad, "v1.2.3", "wisp-windows-x64.exe").is_err());
        }
    }
    #[test]
    fn updater_payload_requires_pe_header_size_and_hash() {
        let mut bytes = vec![0; 4096];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[0x3c..0x40].copy_from_slice(&128u32.to_le_bytes());
        bytes[128..132].copy_from_slice(b"PE\0\0");
        bytes[132..134].copy_from_slice(&0x8664u16.to_le_bytes());
        let m = Manifest {
            version: "1.2.3".into(),
            size: bytes.len() as u64,
            sha256: hex(&Sha256::digest(&bytes)),
        };
        assert!(validate_payload(&bytes, &m).is_ok());
        bytes[400] = 1;
        assert!(validate_payload(&bytes, &m).is_err());
        assert!(validate_payload(&bytes[..10], &m).is_err());
    }
}

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::os::windows::process::CommandExt;
use std::{
    ffi::c_void,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
const API: &str = "https://api.github.com/repos/kexlyu24/wisp/releases/latest";
const MAX_EXE: u64 = 100 * 1024 * 1024;
const EXE: &str = "wisp-windows-x64.exe";
const MANIFEST: &str = "wisp-update.json";
type Result<T> = std::result::Result<T, String>;
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn version(s: &str) -> Result<[u64; 3]> {
    let fields: Vec<_> = s.strip_prefix('v').unwrap_or(s).split('.').collect();
    if fields.len() != 3 {
        return Err("Invalid release version".into());
    }
    let mut v = [0; 3];
    for (i, f) in fields.iter().enumerate() {
        if f.is_empty()
            || (f.len() > 1 && f.starts_with('0'))
            || !f.bytes().all(|b| b.is_ascii_digit())
        {
            return Err("Invalid release version".into());
        }
        v[i] = f.parse().map_err(err)?;
    }
    Ok(v)
}
fn hex(b: &[u8]) -> String {
    b.iter().map(|b| format!("{b:02x}")).collect()
}
fn unhex<const N: usize>(s: &str) -> Result<[u8; N]> {
    let s = s.trim();
    if s.len() != N * 2 || !s.is_ascii() {
        return Err("Invalid hash encoding".into());
    }
    let mut out = [0; N];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).map_err(err)?;
    }
    Ok(out)
}
#[derive(Clone)]
struct Manifest {
    version: String,
    size: u64,
    sha256: String,
}
fn manifest(bytes: &[u8], tag: &str) -> Result<Manifest> {
    let doc: Value = serde_json::from_slice(bytes).map_err(err)?;
    let m = Manifest {
        version: doc["version"].as_str().ok_or("Missing version")?.into(),
        size: doc["size"].as_u64().ok_or("Missing size")?,
        sha256: doc["sha256"].as_str().ok_or("Missing hash")?.into(),
    };
    if version(&m.version)? != version(tag)? || !(4096..=MAX_EXE).contains(&m.size) {
        return Err("Update version or size mismatch".into());
    }
    unhex::<32>(&m.sha256)?;
    Ok(m)
}
fn asset_url(url: &str, tag: &str, name: &str) -> Result<String> {
    version(tag)?;
    if url != format!("https://github.com/kexlyu24/wisp/releases/download/{tag}/{name}") {
        return Err("Untrusted update asset URL".into());
    }
    Ok(url.into())
}
fn validate_payload(bytes: &[u8], m: &Manifest) -> Result<()> {
    if bytes.len() as u64 != m.size || hex(&Sha256::digest(bytes)) != m.sha256.to_lowercase() {
        return Err("Update size or SHA-256 mismatch".into());
    }
    if bytes.get(..2) != Some(b"MZ") {
        return Err("Not a Windows executable".into());
    }
    let offset = u32::from_le_bytes(
        bytes
            .get(0x3c..0x40)
            .ok_or("Invalid executable header")?
            .try_into()
            .map_err(err)?,
    ) as usize;
    if offset > bytes.len().saturating_sub(6)
        || bytes.get(offset..offset + 4) != Some(b"PE\0\0")
        || bytes.get(offset + 4..offset + 6) != Some(&[0x64, 0x86])
    {
        return Err("Update is not a Windows x64 executable".into());
    }
    Ok(())
}
#[derive(Clone)]
pub struct Release {
    pub tag: String,
    pub notes: String,
    url: String,
    manifest_url: String,
}
pub enum Event {
    Checked(Option<Release>),
    Progress(u64, u64),
    Ready(PathBuf),
    Failed(String),
}
#[derive(PartialEq)]
pub enum Phase {
    Idle,
    Checking,
    Available,
    Downloading,
    Ready,
    Error,
}
pub struct Updater {
    pub phase: Phase,
    pub status: String,
    pub release: Option<Release>,
    pub auto: bool,
    pub progress: (u64, u64),
    last_check: u64,
    staged: Option<PathBuf>,
    rx: Option<Receiver<Event>>,
    cancel: Arc<AtomicBool>,
}
impl Default for Updater {
    fn default() -> Self {
        Self {
            phase: Phase::Idle,
            status: "Check for updates.".into(),
            release: None,
            auto: true,
            progress: (0, 0),
            last_check: 0,
            staged: None,
            rx: None,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }
}
impl Drop for Updater {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
impl Updater {
    pub fn new(enabled: bool) -> Self {
        let mut s = Self::default();
        if enabled {
            if let Some(doc) = crate::tasks::data_path("updates.json")
                .and_then(|p| fs::read(p).ok())
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            {
                s.auto = doc["automatic"].as_bool().unwrap_or(true);
                s.last_check = doc["last_check"].as_u64().unwrap_or(0);
            }
            if s.auto && now().saturating_sub(s.last_check) >= 86400 {
                s.check();
            }
        }
        s
    }
    fn save(&self) {
        crate::tasks::write_data(
            "updates.json",
            &serde_json::json!({"automatic":self.auto,"last_check":self.last_check}).to_string(),
        );
    }
    pub fn toggle_auto(&mut self) {
        self.auto = !self.auto;
        self.save();
    }
    pub fn action_enabled(&self) -> bool {
        !matches!(self.phase, Phase::Checking | Phase::Downloading)
    }
    pub fn check(&mut self) {
        if !self.action_enabled() {
            return;
        }
        crate::diagnostics::info("updater", "Stable release check started");
        self.last_check = now();
        self.save();
        self.phase = Phase::Checking;
        self.status = "Checking...".into();
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let result = check_release(&cancel);
            let _ = tx.send(match result {
                Ok(r) => Event::Checked(r),
                Err(e) => Event::Failed(e),
            });
        });
    }
    pub fn download(&mut self) {
        let Some(r) = self.release.clone() else {
            return;
        };
        if self.phase == Phase::Downloading {
            return;
        }
        crate::diagnostics::info("updater", "Signed update download started");
        self.phase = Phase::Downloading;
        self.status = "Downloading...".into();
        self.progress = (0, 0);
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let result = download_release(&r, &cancel, &tx);
            let _ = tx.send(match result {
                Ok(p) => Event::Ready(p),
                Err(e) => Event::Failed(e),
            });
        });
    }
    pub fn cancel(&mut self) {
        crate::diagnostics::info("updater", "Download cancellation requested");
        self.cancel.store(true, Ordering::Relaxed);
        self.status = "Cancelling...".into();
    }
    pub fn poll(&mut self) -> bool {
        let events: Vec<_> = self
            .rx
            .as_ref()
            .map(|r| r.try_iter().collect())
            .unwrap_or_default();
        let changed = !events.is_empty();
        for event in events {
            match event {
                Event::Checked(Some(r)) => {
                    crate::diagnostics::info("updater", "New stable release available");
                    self.status = format!("Version {} available", r.tag);
                    self.release = Some(r);
                    self.phase = Phase::Available;
                }
                Event::Checked(None) => {
                    crate::diagnostics::info("updater", "Installed version is up to date");
                    self.phase = Phase::Idle;
                    self.release = None;
                    self.status = "You're up to date".into();
                }
                Event::Progress(n, total) => self.progress = (n, total),
                Event::Ready(p) => {
                    crate::diagnostics::info(
                        "updater",
                        "Update size and SHA-256 verified; ready to install",
                    );
                    self.phase = Phase::Ready;
                    self.staged = Some(p);
                    self.status = "Ready to install".into();
                }
                Event::Failed(e) => {
                    crate::diagnostics::error(
                        "updater",
                        "Update operation failed; details are shown in Updates UI",
                    );
                    self.phase = Phase::Error;
                    self.status = e;
                }
            }
        }
        changed
    }
    pub fn install(&mut self) -> Result<()> {
        crate::diagnostics::info("updater", "Update installation requested");
        let stage = self.staged.as_ref().ok_or("No verified update is ready")?;
        let m = verified_stage(stage)?;
        if version(&m.version)? <= version(env!("CARGO_PKG_VERSION"))? {
            return Err("Refusing downgrade".into());
        }
        let target = std::env::current_exe()
            .map_err(err)?
            .canonicalize()
            .map_err(err)?;
        if !target
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("wisp.exe"))
        {
            return Err("Rename the application to wisp.exe before updating.".into());
        }
        let helper = target.with_file_name(format!(".wisp-updater-{}.exe", std::process::id()));
        fs::copy(&target, &helper)
            .map_err(|e| format!("Application folder is not writable: {e}"))?;
        Command::new(&helper)
            .arg("--apply-update")
            .arg(stage)
            .arg(&target)
            .arg(std::process::id().to_string())
            .creation_flags(0x08000000)
            .spawn()
            .map_err(err)?;
        Ok(())
    }
}
fn check_release(cancel: &AtomicBool) -> Result<Option<Release>> {
    let bytes = get(API, 1024 * 1024, cancel, |_, _| {})?;
    let doc: Value = serde_json::from_slice(&bytes).map_err(err)?;
    if doc["draft"].as_bool() != Some(false) || doc["prerelease"].as_bool() != Some(false) {
        return Err("No stable release is available".into());
    }
    let tag = doc["tag_name"]
        .as_str()
        .ok_or("Release version is missing")?;
    if version(tag)? <= version(env!("CARGO_PKG_VERSION"))? {
        return Ok(None);
    }
    let assets = doc["assets"]
        .as_array()
        .ok_or("Release has no update files")?;
    let asset = |name: &str| -> Result<String> {
        let a = assets
            .iter()
            .find(|a| a["name"] == name)
            .ok_or("Latest release does not have updater files yet.")?;
        asset_url(
            a["browser_download_url"]
                .as_str()
                .ok_or("Missing download URL")?,
            tag,
            name,
        )
    };
    Ok(Some(Release {
        tag: tag.into(),
        notes: doc["body"]
            .as_str()
            .unwrap_or("No release notes.")
            .chars()
            .take(12000)
            .collect(),
        url: asset(EXE)?,
        manifest_url: asset(MANIFEST)?,
    }))
}
fn download_release(r: &Release, cancel: &AtomicBool, tx: &mpsc::Sender<Event>) -> Result<PathBuf> {
    let bytes = get(&r.manifest_url, 16384, cancel, |_, _| {})?;
    let m = manifest(&bytes, &r.tag)?;
    let exe = get(&r.url, m.size, cancel, |n, _| {
        let _ = tx.send(Event::Progress(n, m.size));
    })?;
    validate_payload(&exe, &m)?;
    if cancel.load(Ordering::Relaxed) {
        return Err("Update download cancelled.".into());
    }
    let root =
        crate::tasks::data_path("updates").ok_or("Application data folder is unavailable")?;
    let dir = root.join(format!("{}-{}-{}", r.tag, std::process::id(), now()));
    fs::create_dir_all(&dir).map_err(err)?;
    let result = (|| {
        fs::write(dir.join(MANIFEST), &bytes).map_err(err)?;
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dir.join(EXE))
            .map_err(err)?;
        f.write_all(&exe).map_err(err)?;
        f.sync_all().map_err(err)?;
        Ok(dir.clone())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&dir);
    }
    result
}
// WinHTTP uses the Windows trust store, HTTPS redirect policy and bounded timeouts.
#[link(name = "winhttp")]
extern "system" {
    fn WinHttpOpen(
        agent: *const u16,
        access: u32,
        proxy: *const u16,
        bypass: *const u16,
        flags: u32,
    ) -> *mut c_void;
    fn WinHttpConnect(
        session: *mut c_void,
        host: *const u16,
        port: u16,
        reserved: u32,
    ) -> *mut c_void;
    fn WinHttpOpenRequest(
        connection: *mut c_void,
        verb: *const u16,
        path: *const u16,
        version: *const u16,
        referer: *const u16,
        accept: *const *const u16,
        flags: u32,
    ) -> *mut c_void;
    fn WinHttpSetTimeouts(
        h: *mut c_void,
        resolve: i32,
        connect: i32,
        send: i32,
        receive: i32,
    ) -> i32;
    fn WinHttpSendRequest(
        h: *mut c_void,
        headers: *const u16,
        len: u32,
        optional: *mut c_void,
        size: u32,
        total: u32,
        context: usize,
    ) -> i32;
    fn WinHttpReceiveResponse(h: *mut c_void, reserved: *mut c_void) -> i32;
    fn WinHttpQueryHeaders(
        h: *mut c_void,
        info: u32,
        name: *const u16,
        buffer: *mut c_void,
        size: *mut u32,
        index: *mut u32,
    ) -> i32;
    fn WinHttpReadData(h: *mut c_void, buffer: *mut c_void, size: u32, read: *mut u32) -> i32;
    fn WinHttpCloseHandle(h: *mut c_void) -> i32;
}
struct Http(*mut c_void);
impl Drop for Http {
    fn drop(&mut self) {
        unsafe {
            WinHttpCloseHandle(self.0);
        }
    }
}
fn http_handle(h: *mut c_void) -> Result<Http> {
    if h.is_null() {
        Err(format!(
            "Network error: {}",
            std::io::Error::last_os_error()
        ))
    } else {
        Ok(Http(h))
    }
}
pub(crate) fn get(
    url: &str,
    limit: u64,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, u64),
) -> Result<Vec<u8>> {
    let tail = url.strip_prefix("https://").ok_or("HTTPS is required")?;
    let (host, path) = tail.split_once('/').ok_or("Invalid update URL")?;
    if host.contains(['@', ':', '\\']) {
        return Err("Invalid update host".into());
    }
    unsafe {
        let session = http_handle(WinHttpOpen(
            crate::wide("Wisp-Updater/1").as_ptr(),
            4,
            std::ptr::null(),
            std::ptr::null(),
            0,
        ))?;
        WinHttpSetTimeouts(session.0, 10000, 15000, 15000, 15000);
        let connection = http_handle(WinHttpConnect(
            session.0,
            crate::wide(host).as_ptr(),
            443,
            0,
        ))?;
        let request = http_handle(WinHttpOpenRequest(
            connection.0,
            crate::wide("GET").as_ptr(),
            crate::wide(&format!("/{path}")).as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            0x00800000,
        ))?;
        let headers = crate::wide(if host == "api.github.com" {
            "Accept: application/vnd.github+json\r\nCache-Control: no-cache\r\n"
        } else {
            "Accept: */*\r\nCache-Control: no-cache\r\n"
        });
        if WinHttpSendRequest(
            request.0,
            headers.as_ptr(),
            u32::MAX,
            std::ptr::null_mut(),
            0,
            0,
            0,
        ) == 0
            || WinHttpReceiveResponse(request.0, std::ptr::null_mut()) == 0
        {
            return Err(format!(
                "Cannot reach update server: {}",
                std::io::Error::last_os_error()
            ));
        }
        let mut status = 0u32;
        let mut size = 4;
        if WinHttpQueryHeaders(
            request.0,
            19 | 0x20000000,
            std::ptr::null(),
            (&mut status as *mut u32).cast(),
            &mut size,
            std::ptr::null_mut(),
        ) == 0
        {
            return Err("Cannot read HTTP status".into());
        }
        if status != 200 {
            crate::diagnostics::error(
                "network",
                &format!("HTTP request rejected; status={status}"),
            );
            return Err(format!(
                "Update server returned HTTP {status}. Try again later."
            ));
        }
        let mut bytes = Vec::new();
        let start = std::time::Instant::now();
        let mut buffer = [0u8; 65536];
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err("Update download cancelled.".into());
            }
            if start.elapsed() > Duration::from_secs(300) {
                return Err("Update download timed out".into());
            }
            let mut n = 0;
            if WinHttpReadData(
                request.0,
                buffer.as_mut_ptr().cast(),
                buffer.len() as u32,
                &mut n,
            ) == 0
            {
                crate::diagnostics::failure(
                    "network",
                    "Read HTTP response",
                    &std::io::Error::last_os_error(),
                );
                return Err(format!(
                    "Download failed: {}",
                    std::io::Error::last_os_error()
                ));
            }
            if n == 0 {
                break;
            }
            if bytes.len() as u64 + n as u64 > limit {
                return Err("Update response exceeds the allowed size".into());
            }
            bytes.extend_from_slice(&buffer[..n as usize]);
            progress(bytes.len() as u64, limit);
        }
        Ok(bytes)
    }
}
#[link(name = "kernel32")]
extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
    fn WaitForSingleObject(handle: *mut c_void, millis: u32) -> u32;
    fn CloseHandle(handle: *mut c_void) -> i32;
}
fn verified_stage(dir: &Path) -> Result<Manifest> {
    if fs::metadata(dir.join(MANIFEST)).map_err(err)?.len() > 16384 {
        return Err("Update metadata exceeds size limit".into());
    }
    let bytes = fs::read(dir.join(MANIFEST)).map_err(err)?;
    if bytes.len() > 16384 {
        return Err("Manifest too large".into());
    }
    let doc: Value = serde_json::from_slice(&bytes).map_err(err)?;
    let tag = doc["version"].as_str().ok_or("Missing version")?;
    let m = manifest(&bytes, tag)?;
    let exe = dir.join(EXE);
    if fs::metadata(&exe).map_err(err)?.len() > MAX_EXE {
        return Err("Update too large".into());
    }
    validate_payload(&fs::read(exe).map_err(err)?, &m)?;
    Ok(m)
}
pub fn helper_mode() -> bool {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_none_or(|a| a != "--apply-update") {
        return false;
    }
    let result = (|| -> Result<()> {
        if args.len() != 5 {
            return Err("Invalid updater arguments".into());
        }
        let stage = PathBuf::from(&args[2]);
        let target = PathBuf::from(&args[3]);
        let helper = std::env::current_exe()
            .map_err(err)?
            .canonicalize()
            .map_err(err)?;
        if target.parent() != helper.parent()
            || !target
                .file_name()
                .is_some_and(|n| n.eq_ignore_ascii_case("wisp.exe"))
        {
            return Err("Invalid update destination".into());
        }
        let root = crate::tasks::data_path("updates")
            .ok_or("Missing update directory")?
            .canonicalize()
            .map_err(err)?;
        let stage = stage.canonicalize().map_err(err)?;
        if stage.parent() != Some(root.as_path()) {
            return Err("Invalid staging directory".into());
        }
        let m = verified_stage(&stage)?;
        if version(&m.version)? <= version(env!("CARGO_PKG_VERSION"))? {
            return Err("Refusing downgrade".into());
        }
        let pid = args[4]
            .to_str()
            .ok_or("Invalid process")?
            .parse::<u32>()
            .map_err(err)?;
        unsafe {
            let h = OpenProcess(0x00100000, 0, pid);
            if !h.is_null() {
                let wait = WaitForSingleObject(h, 30000);
                CloseHandle(h);
                if wait != 0 {
                    return Err("Wisp is still running. Please retry.".into());
                }
            }
        }
        crate::diagnostics::info(
            "updater",
            "Previous Wisp process exited; replacing executable",
        );
        let pending = target.with_file_name(format!(".wisp-new-{pid}.exe"));
        let backup = target.with_file_name(format!("wisp-backup-{pid}.exe"));
        fs::copy(stage.join(EXE), &pending).map_err(err)?;
        validate_payload(&fs::read(&pending).map_err(err)?, &m)?;
        if backup.exists() {
            return Err("Previous update backup exists; refusing overwrite".into());
        }
        fs::rename(&target, &backup).map_err(err)?;
        if let Err(e) = fs::rename(&pending, &target) {
            let restore = fs::rename(&backup, &target);
            if restore.is_ok() {
                let _ = Command::new(&target).creation_flags(0x08000000).spawn();
            }
            return Err(format!("Install failed: {e}; restore: {restore:?}"));
        }
        let health = stage.join("started.ok");
        let _ = fs::remove_file(&health);
        let launch = Command::new(&target)
            .arg("--update-health")
            .arg(&health)
            .current_dir(target.parent().ok_or("Missing application directory")?)
            .stdin(Stdio::null())
            .creation_flags(0x08000000)
            .spawn();
        let mut child = match launch {
            Ok(c) => c,
            Err(e) => {
                let _ = fs::remove_file(&target);
                fs::rename(&backup, &target).map_err(err)?;
                let _ = Command::new(&target).creation_flags(0x08000000).spawn();
                return Err(format!("Cannot launch update; old version restored: {e}"));
            }
        };
        for _ in 0..150 {
            if health.exists() {
                crate::diagnostics::info(
                    "updater",
                    "Updated application reported successful startup",
                );
                let _ = fs::remove_dir_all(&stage);
                return Ok(());
            }
            if child.try_wait().map_err(err)?.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        // Only terminate the child created by this helper, never an unrelated Wisp process.
        if child.try_wait().map_err(err)?.is_none() {
            child.kill().map_err(err)?;
            child.wait().map_err(err)?;
        }
        crate::diagnostics::error(
            "updater",
            "Updated application failed startup; restoring previous executable",
        );
        fs::remove_file(&target).map_err(err)?;
        fs::rename(&backup, &target).map_err(err)?;
        Command::new(&target)
            .creation_flags(0x08000000)
            .spawn()
            .map_err(err)?;
        Err("Updated Wisp did not start; the old executable was restored.".into())
    })();
    if let Err(e) = result {
        crate::diagnostics::error(
            "updater",
            "Update helper failed; separate update-error.txt contains recovery details",
        );
        let message = format!("Wisp update failed:\n{e}\nYour app data was not changed.");
        let _ =
            crate::tasks::data_path("update-error.txt").and_then(|p| fs::write(p, &message).ok());
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                crate::wide(&message).as_ptr(),
                crate::wide("Wisp update").as_ptr(),
                0x10,
            );
        }
    }
    true
}
#[link(name = "user32")]
extern "system" {
    fn MessageBoxW(hwnd: *mut c_void, text: *const u16, caption: *const u16, kind: u32) -> i32;
}
pub fn health_ready() {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() == 3 && args[1] == "--update-health" {
        let path = PathBuf::from(&args[2]);
        if let Some(root) = crate::tasks::data_path("updates") {
            if path.file_name().is_some_and(|n| n == "started.ok")
                && root.canonicalize().ok().is_some_and(|root| {
                    path.parent()
                        .and_then(|p| p.parent())
                        .and_then(|p| p.canonicalize().ok())
                        .is_some_and(|parent| parent == root)
                })
            {
                let _ = fs::write(path, b"ready");
            }
        }
    }
}
