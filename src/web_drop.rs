//! Browser image drops. Untrusted content is bounded and decoded before caching.
use sha2::{Digest, Sha256};
use std::{
    fs,
    sync::{
        atomic::AtomicBool,
        mpsc::{self, Receiver},
    },
};
const MAX: usize = 20 * 1024 * 1024;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn web_drop_accepts_image_source_not_page_source_url() {
        assert_eq!(image_source("SourceURL:https://page.test/\r\n<img src=\"https://images.test/a.png?a=1&amp;b=2\">"), Some("https://images.test/a.png?a=1&b=2".into()));
        assert_eq!(image_source("<a href='https://page.test/'>link</a>"), None);
        assert_eq!(
            image_source("<img srcset='https://wrong.test/a.png' src='https://right.test/a.png'>"),
            Some("https://right.test/a.png".into())
        );
    }
    #[test]
    fn web_drop_rejects_unsafe_or_unsupported_urls() {
        for url in [
            "file:///C:/secret.png",
            "javascript:evil",
            "https://user:pass@host/a",
            "https://host/a\nfoo",
            "blob:https://host/a",
            "https://host:443/a",
            "https://host/a\tfoo",
        ] {
            assert!(safe_url(url).is_none());
        }
        assert_eq!(
            safe_url("https://example.test/a.png"),
            Some("https://example.test/a.png".into())
        );
    }
    #[test]
    fn web_drop_only_caches_decodable_bounded_images() {
        assert!(image_png(b"<html>not image</html>").is_err());
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(2, 2)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        assert!(image_png(bytes.get_ref()).is_ok());
    }
}

pub fn safe_url(raw: &str) -> Option<String> {
    let url = raw.trim_matches('\0').trim();
    if url.len() > 16384 || url.chars().any(char::is_control) {
        return None;
    }
    let tail = url.strip_prefix("https://")?;
    let host = tail.split('/').next()?;
    if host.is_empty() || host.contains(['@', ':', '\\', '#', '?', '%']) || !host.is_ascii() {
        return None;
    }
    Some(url.into())
}
fn entity_decode(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(end) = rest.find(';').filter(|n| *n <= 12) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let token = &rest[1..end];
        let decoded = match token {
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "lt" => Some('<'),
            "gt" => Some('>'),
            _ => token
                .strip_prefix("#x")
                .and_then(|n| u32::from_str_radix(n, 16).ok())
                .or_else(|| token.strip_prefix('#').and_then(|n| n.parse().ok()))
                .and_then(char::from_u32),
        };
        if let Some(ch) = decoded {
            out.push(ch);
            rest = &rest[end + 1..];
        } else {
            out.push('&');
            rest = &rest[1..];
        }
    }
    out.push_str(rest);
    out
}
pub fn image_source(html: &str) -> Option<String> {
    // Parse attributes, not a page SourceURL or an anchor href.
    let lower = html.to_ascii_lowercase();
    let mut at = 0;
    while let Some(i) = lower[at..].find("<img") {
        let start = at + i + 4;
        at = start;
        if !html
            .as_bytes()
            .get(start)
            .is_some_and(|c| c.is_ascii_whitespace() || *c == b'>')
        {
            continue;
        }
        let bytes = html.as_bytes();
        let mut p = start;
        while p < bytes.len() && bytes[p] != b'>' {
            while p < bytes.len() && (bytes[p].is_ascii_whitespace() || bytes[p] == b'/') {
                p += 1;
            }
            let a = p;
            while p < bytes.len() && !bytes[p].is_ascii_whitespace() && !b"=>/".contains(&bytes[p])
            {
                p += 1;
            }
            if p == a {
                break;
            }
            let name = &lower[a..p];
            while p < bytes.len() && bytes[p].is_ascii_whitespace() {
                p += 1;
            }
            if bytes.get(p) != Some(&b'=') {
                continue;
            }
            p += 1;
            while p < bytes.len() && bytes[p].is_ascii_whitespace() {
                p += 1;
            }
            let quote = bytes.get(p).copied().filter(|c| *c == b'\'' || *c == b'"');
            if quote.is_some() {
                p += 1;
            }
            let value = p;
            while p < bytes.len()
                && if let Some(q) = quote {
                    bytes[p] != q
                } else {
                    !bytes[p].is_ascii_whitespace() && bytes[p] != b'>'
                }
            {
                p += 1;
            }
            if name == "src" {
                if let Some(url) = safe_url(&entity_decode(&html[value..p])) {
                    return Some(url);
                }
            }
            if quote.is_some() && p < bytes.len() {
                p += 1;
            }
        }
    }
    None
}
fn image_png(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.len() > MAX {
        return Err("Image exceeds 20 MiB.".into());
    }
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|_| "This drop is not a supported PNG/JPEG/WebP/BMP image.".to_string())?;
    let mut output = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut output, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    if output.get_ref().len() > MAX {
        return Err("Decoded image exceeds 20 MiB.".into());
    }
    Ok(output.into_inner())
}
pub fn start(url: String) -> Receiver<Result<String, String>> {
    let (tx, rx) = mpsc::channel();
    crate::diagnostics::info(
        "shelf",
        "Browser image worker started; URL omitted for privacy",
    );
    std::thread::spawn(move || {
        let result = (|| {
            let url = safe_url(&url).ok_or("Only direct HTTPS image URLs are supported.")?;
            let bytes = crate::updater::get(&url, MAX as u64, &AtomicBool::new(false), |_, _| {})?;
            let png = image_png(&bytes)?;
            let name = format!("web-image-{:x}.png", Sha256::digest(&png));
            let dir =
                crate::tasks::data_path("web-images").ok_or("Application data is unavailable.")?;
            fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let path = dir.join(name);
            if !path.exists() {
                use std::io::Write;
                let temp = dir.join(format!(".download-{}.tmp", std::process::id()));
                let result = (|| {
                    let mut file = fs::OpenOptions::new()
                        .create_new(true)
                        .write(true)
                        .open(&temp)
                        .map_err(|e| e.to_string())?;
                    file.write_all(&png).map_err(|e| e.to_string())?;
                    file.sync_all().map_err(|e| e.to_string())?;
                    drop(file);
                    fs::rename(&temp, &path).map_err(|e| e.to_string())
                })();
                if result.is_err() {
                    let _ = fs::remove_file(&temp);
                }
                result?;
            }
            Ok(path.to_string_lossy().into_owned())
        })();
        match &result {
            Ok(_) => crate::diagnostics::info("shelf", "Browser image validated and cached as PNG"),
            Err(_) => crate::diagnostics::error(
                "shelf",
                "Browser image fetch, decode or cache operation failed",
            ),
        };
        let _ = tx.send(result);
    });
    rx
}
