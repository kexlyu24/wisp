//! Time-synced lyrics from LRCLIB (https://lrclib.net), falling back to NetEase Cloud Music's
//! public web API when LRCLIB has none, fetched on a worker thread.
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;
use windows::core::HSTRING;
use windows::Foundation::Uri;
use windows::Web::Http::Filters::{
    HttpBaseProtocolFilter, HttpCacheReadBehavior, HttpCacheWriteBehavior, HttpCookieUsageBehavior,
};
use windows::Web::Http::HttpClient;

pub struct Line {
    pub time: f64,
    pub text: String,
}

pub enum Status {
    Loading,
    /// LRCLIB answered "server is busy"; the worker keeps retrying.
    Busy,
    Synced(Vec<Line>),
    /// LRCLIB only has unsynced text for this track.
    Plain,
    Instrumental,
    NotFound,
    Failed,
}

pub struct Request {
    pub key: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: i64,
    /// Skip the disk cache (a manual retry).
    pub fresh: bool,
}

pub struct Reply {
    pub key: String,
    pub status: Status,
}

/// Worker that answers lyric requests; only the newest pending request is fetched.
pub fn start() -> (Sender<Request>, Receiver<Reply>) {
    let (req_tx, req_rx) = mpsc::channel::<Request>();
    let (reply_tx, reply_rx) = mpsc::channel();
    thread::spawn(move || {
        let mut client: Option<HttpClient> = None;
        let mut pending = req_rx.recv().ok();
        while let Some(mut req) = pending.take() {
            while let Ok(newer) = req_rx.try_recv() {
                req = newer;
            }
            if !req.fresh {
                if let Some(status) = read_cache(&req) {
                    if reply_tx
                        .send(Reply {
                            key: req.key,
                            status,
                        })
                        .is_err()
                    {
                        break;
                    }
                    pending = req_rx.recv().ok();
                    continue;
                }
            }
            if client.is_none() {
                client = http_client();
            }
            // LRCLIB answers 503 when it is overloaded, sometimes for minutes: retry with growing
            // gaps for about ten minutes, and drop the work as soon as a newer track is requested.
            let mut status = Status::Failed;
            for (attempt, delay) in [0u64, 2, 5, 12, 25, 45, 60, 60, 60, 60, 60, 60, 60, 60, 60]
                .into_iter()
                .enumerate()
            {
                if attempt == 1
                    && reply_tx
                        .send(Reply {
                            key: req.key.clone(),
                            status: Status::Busy,
                        })
                        .is_err()
                {
                    return;
                }
                if delay > 0 {
                    match req_rx.recv_timeout(Duration::from_secs(delay)) {
                        Ok(newer) => {
                            pending = Some(newer);
                            break;
                        }
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
                status = match &client {
                    Some(client) => fetch(client, &req),
                    None => Status::Failed,
                };
                if !matches!(status, Status::Failed) {
                    break;
                }
            }
            if pending.is_some() {
                continue;
            }
            write_cache(&req, &status);
            if reply_tx
                .send(Reply {
                    key: req.key,
                    status,
                })
                .is_err()
            {
                break;
            }
            pending = req_rx.recv().ok();
        }
    });
    (req_tx, reply_rx)
}

/// LRCLIB first; NetEase only when LRCLIB has no synced lyrics or is unreachable. NetEase is an
/// unofficial API, so its errors and misses just keep LRCLIB's answer (and its retries).
fn fetch(client: &HttpClient, req: &Request) -> Status {
    let status = lrclib(client, req);
    match status {
        Status::NotFound | Status::Plain | Status::Failed => netease(client, req).unwrap_or(status),
        _ => status,
    }
}

/// Tries an exact `/get`, then progressively looser searches, stopping at the first synced (or
/// instrumental) answer. An overloaded server is only reported as `Failed`, never as "not found",
/// so the worker keeps retrying.
fn lrclib(client: &HttpClient, req: &Request) -> Status {
    let mut get = format!(
        "https://lrclib.net/api/get?artist_name={}&track_name={}",
        encode(&req.artist),
        encode(&req.title)
    );
    if !req.album.is_empty() {
        get += &format!("&album_name={}", encode(&req.album));
    }
    if req.duration > 0 {
        get += &format!("&duration={}", req.duration);
    }
    let (title, artist) = (clean_title(&req.title), primary_artist(&req.artist));
    let mut urls = vec![
        get,
        format!(
            "https://lrclib.net/api/search?track_name={}&artist_name={}",
            encode(&req.title),
            encode(&req.artist)
        ),
    ];
    // Titles like "Song (feat. X) - Remastered 2011" or "A, B & C" artists rarely match exactly.
    if title != req.title || artist != req.artist {
        urls.push(format!(
            "https://lrclib.net/api/search?track_name={}&artist_name={}",
            encode(&title),
            encode(&artist)
        ));
    }
    urls.push(format!(
        "https://lrclib.net/api/search?q={}",
        encode(&format!("{} {}", artist, title))
    ));

    let (mut plain, mut unreachable) = (false, 0);
    for url in urls {
        match classify(http_get(client, &url)) {
            Some(status @ (Status::Synced(_) | Status::Instrumental)) => return status,
            Some(Status::Plain) => plain = true,
            Some(_) => {}
            // Twice busy means the whole API is overloaded; looser searches would be busy too.
            None if unreachable == 1 => return if plain { Status::Plain } else { Status::Failed },
            None => unreachable += 1,
        }
    }
    if plain {
        Status::Plain
    } else if unreachable > 0 {
        Status::Failed
    } else {
        Status::NotFound
    }
}

/// Searches NetEase for the track and reads its LRC; `None` when nothing matches closely or the
/// API does not answer.
fn netease(client: &HttpClient, req: &Request) -> Option<Status> {
    let query = format!(
        "{} {}",
        primary_artist(&req.artist),
        clean_title(&req.title)
    );
    let (code, body) = http_get(
        client,
        &format!(
            "https://music.163.com/api/search/get?s={}&type=1&limit=10",
            encode(&query)
        ),
    )?;
    let id = if code == 200 {
        pick_song(&body, req)?
    } else {
        return None;
    };
    let (code, body) = http_get(
        client,
        &format!("https://music.163.com/api/song/lyric?id={}&lv=1&tv=-1", id),
    )?;
    if code == 200 {
        netease_lyrics(&body)
    } else {
        None
    }
}

/// The id of the first search result whose title, artist and length all match the request.
/// Loose matches are rejected: wrong lyrics are worse than none.
fn pick_song(json: &str, req: &Request) -> Option<u64> {
    let doc: serde_json::Value = serde_json::from_str(json).ok()?;
    let titles = [normalize(&req.title), normalize(&clean_title(&req.title))];
    let artist = normalize(&primary_artist(&req.artist));
    if artist.is_empty() {
        return None;
    }
    let songs = doc["result"]["songs"].as_array()?;
    // An exact title beats one that only matches without its "(Live)"/"- Remastered" suffix.
    let matches = |song: &serde_json::Value, exact: bool| {
        let name = song["name"].as_str().unwrap_or("");
        let named = if exact {
            normalize(name) == titles[0]
        } else {
            titles.contains(&normalize(name)) || titles.contains(&normalize(&clean_title(name)))
        };
        let by = song["artists"].as_array().is_some_and(|artists| {
            artists.iter().any(|a| {
                let a = normalize(a["name"].as_str().unwrap_or(""));
                !a.is_empty() && (a.contains(&artist) || artist.contains(&a))
            })
        });
        let ms = song["duration"].as_i64().unwrap_or(0);
        let timed =
            req.duration <= 0 || ms <= 0 || (ms as f64 / 1000. - req.duration as f64).abs() <= 3.;
        named && by && timed
    };
    songs
        .iter()
        .find(|s| matches(s, true))
        .or_else(|| songs.iter().find(|s| matches(s, false)))?["id"]
        .as_u64()
}

fn normalize(s: &str) -> String {
    s.to_lowercase()
        .replace('\u{2019}', "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Reads a NetEase `/song/lyric` reply, dropping the credit lines it puts at the top.
fn netease_lyrics(json: &str) -> Option<Status> {
    let doc: serde_json::Value = serde_json::from_str(json).ok()?;
    let text = doc["lrc"]["lyric"].as_str().unwrap_or("");
    if doc["pureMusic"].as_bool() == Some(true)
        || doc["nolyric"].as_bool() == Some(true)
        || text.contains("纯音乐")
    {
        return Some(Status::Instrumental);
    }
    let lines: Vec<Line> = parse_lrc(text)
        .into_iter()
        .filter(|l| !is_credit(&l.text))
        .collect();
    lines
        .iter()
        .any(|l| !l.text.trim().is_empty())
        .then_some(Status::Synced(lines))
}

/// "作词 : Someone", "Composer: Someone" and the like.
fn is_credit(text: &str) -> bool {
    let Some((head, _)) = text.split_once(':').or_else(|| text.split_once('：')) else {
        return false;
    };
    let roles = [
        "作词",
        "作曲",
        "编曲",
        "制作人",
        "混音",
        "母带",
        "lyricist",
        "composer",
        "arranger",
        "producer",
    ];
    roles.contains(&head.trim().to_lowercase().as_str())
}

/// 200 is an answer, 404 is "no match"; anything else (503 overload, 429, no connection) is `None`.
fn classify(response: Option<(i32, String)>) -> Option<Status> {
    match response {
        Some((200, body)) => Some(parse_record(&body)),
        Some((404, _)) => Some(Status::NotFound),
        _ => None,
    }
}

/// Drops featured artists and edition suffixes: "Song (feat. X) - Remastered 2011" -> "Song".
pub fn clean_title(title: &str) -> String {
    let mut s = title.to_string();
    let noise = [
        "remaster", "live", "edit", "version", "mono", "stereo", "mix", "bonus", "deluxe",
    ];
    if let Some(i) = s.find(" - ") {
        let tail = s[i + 3..].to_ascii_lowercase();
        if noise.iter().any(|k| tail.contains(k)) {
            s.truncate(i);
        }
    }
    for (open, close) in [('(', ')'), ('[', ']')] {
        let mut from = 0;
        while let Some(start) = s[from..].find(open).map(|i| i + from) {
            let Some(end) = s[start..].find(close).map(|i| i + start) else {
                break;
            };
            let inner = s[start + 1..end].trim().to_ascii_lowercase();
            let featured = ["feat", "ft.", "with "]
                .iter()
                .any(|k| inner.starts_with(k));
            if featured || noise.iter().any(|k| inner.contains(k)) {
                s.replace_range(start..=end, "");
            } else {
                from = end + 1;
            }
        }
    }
    let cleaned = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() {
        title.to_string()
    } else {
        cleaned
    }
}

/// The first of several credited artists: "A, B & C feat. D" -> "A".
pub fn primary_artist(artist: &str) -> String {
    let lower = artist.to_ascii_lowercase();
    let cut = [", ", " & ", " feat", " ft.", " x ", "; ", " and "]
        .iter()
        .filter_map(|sep| lower.find(sep))
        .min();
    let first = cut.map_or(artist, |i| &artist[..i]).trim();
    if first.is_empty() {
        artist.trim().to_string()
    } else {
        first.to_string()
    }
}

/// `%APPDATA%\Wisp\lyrics\<hash>.lrc`, keyed by title and artist.
fn cache_path(req: &Request) -> Option<std::path::PathBuf> {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in format!(
        "{}\0{}",
        req.title.to_lowercase(),
        req.artist.to_lowercase()
    )
    .bytes()
    {
        hash = (hash ^ b as u64).wrapping_mul(0x100000001b3);
    }
    crate::tasks::data_path("lyrics").map(|dir| dir.join(format!("{:016x}.lrc", hash)))
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// "Not found" is kept for a day so new LRCLIB uploads show up; failures are never cached.
const NOT_FOUND_TTL: u64 = 24 * 60 * 60;

fn encode_cache(status: &Status, now: u64) -> Option<String> {
    Some(match status {
        Status::Synced(lines) => {
            let mut out = String::from("#synced\n");
            for line in lines {
                let (min, sec) = ((line.time / 60.).floor(), line.time % 60.);
                out += &format!("[{:02}:{:05.2}]{}\n", min as u64, sec, line.text);
            }
            out
        }
        Status::Plain => "#plain\n".into(),
        Status::Instrumental => "#instrumental\n".into(),
        Status::NotFound => format!("#notfound {}\n", now),
        Status::Loading | Status::Busy | Status::Failed => return None,
    })
}

fn decode_cache(src: &str, now: u64) -> Option<Status> {
    let (head, body) = src.split_once('\n').unwrap_or((src, ""));
    match head.trim() {
        "#synced" => Some(Status::Synced(parse_lrc(body)))
            .filter(|s| matches!(s, Status::Synced(l) if !l.is_empty())),
        "#plain" => Some(Status::Plain),
        "#instrumental" => Some(Status::Instrumental),
        h => {
            let at: u64 = h.strip_prefix("#notfound ")?.parse().ok()?;
            (now.saturating_sub(at) < NOT_FOUND_TTL).then_some(Status::NotFound)
        }
    }
}

fn read_cache(req: &Request) -> Option<Status> {
    decode_cache(&std::fs::read_to_string(cache_path(req)?).ok()?, now_secs())
}

fn write_cache(req: &Request, status: &Status) {
    let (Some(path), Some(text)) = (cache_path(req), encode_cache(status, now_secs())) else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, text);
}

/// Without cookies or the shared WinINet cache: once NetEase's `NMTID` cookie is sent back, its
/// search ignores the query and returns unrelated songs. We keep our own disk cache anyway.
fn http_client() -> Option<HttpClient> {
    let filter = HttpBaseProtocolFilter::new().ok()?;
    let _ = filter.SetCookieUsageBehavior(HttpCookieUsageBehavior::NoCookies);
    if let Ok(cache) = filter.CacheControl() {
        let _ = cache.SetReadBehavior(HttpCacheReadBehavior::NoCache);
        let _ = cache.SetWriteBehavior(HttpCacheWriteBehavior::NoCache);
    }
    let client = HttpClient::Create(&filter).ok()?;
    if let Ok(agent) = client.DefaultRequestHeaders().and_then(|h| h.UserAgent()) {
        let _ = agent.TryParseAdd(&HSTRING::from(concat!(
            "Wisp/",
            env!("CARGO_PKG_VERSION"),
            " (Windows notch)"
        )));
    }
    Some(client)
}

fn http_get(client: &HttpClient, url: &str) -> Option<(i32, String)> {
    let uri = Uri::CreateUri(&HSTRING::from(url)).ok()?;
    let response = client.GetAsync(&uri).ok()?.get().ok()?;
    let code = response.StatusCode().ok()?.0;
    let body = response
        .Content()
        .ok()?
        .ReadAsStringAsync()
        .ok()?
        .get()
        .ok()?
        .to_string();
    Some((code, body))
}

/// Interprets a `/get` object or a `/search` array: synced lyrics win, then instrumental, then plain.
pub fn parse_record(json: &str) -> Status {
    if let Some(text) = json_strings(json, "syncedLyrics")
        .into_iter()
        .find(|t| !t.trim().is_empty())
    {
        let lines = parse_lrc(&text);
        if !lines.is_empty() {
            return Status::Synced(lines);
        }
    }
    if json.contains("\"instrumental\":true") {
        return Status::Instrumental;
    }
    if json_strings(json, "plainLyrics")
        .iter()
        .any(|t| !t.trim().is_empty())
    {
        return Status::Plain;
    }
    Status::NotFound
}

/// `[mm:ss.xx]text` lines; a line may carry several timestamps. Metadata tags are skipped.
pub fn parse_lrc(src: &str) -> Vec<Line> {
    let mut lines = Vec::new();
    for raw in src.lines() {
        let mut rest = raw.trim();
        let mut times = Vec::new();
        while let Some(stripped) = rest.strip_prefix('[') {
            let Some(end) = stripped.find(']') else { break };
            if let Some(t) = parse_stamp(&stripped[..end]) {
                times.push(t);
            }
            rest = &stripped[end + 1..];
        }
        let text = rest.trim().to_string();
        for time in times {
            lines.push(Line {
                time,
                text: text.clone(),
            });
        }
    }
    lines.sort_by(|a, b| a.time.total_cmp(&b.time));
    lines
}

fn parse_stamp(tag: &str) -> Option<f64> {
    let (min, sec) = tag.split_once(':')?;
    let min: f64 = min.trim().parse().ok()?;
    let sec: f64 = sec.trim().replace(':', ".").parse().ok()?;
    Some(min * 60. + sec)
}

/// Every string value of `"key":` in the document, in order (null values are skipped).
fn json_strings(json: &str, key: &str) -> Vec<String> {
    let needle = format!("\"{}\":", key);
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(pos) = json[from..].find(&needle) {
        let start = from + pos + needle.len();
        from = start;
        let value = json[start..].trim_start();
        if let Some(body) = value.strip_prefix('"') {
            if let Some(s) = unescape(body) {
                out.push(s);
            }
        }
    }
    out
}

/// Decodes a JSON string body up to its closing quote.
fn unescape(body: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = body.chars();
    let mut pending_high: Option<u16> = None;
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                'b' | 'f' => {}
                'u' => {
                    let hex: String = chars.by_ref().take(4).collect();
                    let unit = u16::from_str_radix(&hex, 16).ok()?;
                    if (0xD800..0xDC00).contains(&unit) {
                        pending_high = Some(unit);
                        continue;
                    }
                    let decoded = match pending_high.take() {
                        Some(high) => char::decode_utf16([high, unit]).next()?.ok(),
                        None => char::from_u32(unit as u32),
                    };
                    if let Some(ch) = decoded {
                        out.push(ch);
                    }
                }
                other => out.push(other),
            },
            c => out.push(c),
        }
    }
    None
}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{:02X}", b),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lrc_with_multiple_stamps_and_metadata() {
        let lines =
            parse_lrc("[ar:Someone]\n[00:12.50]Hello\n[01:02.00][00:05.00]Again\n[00:20.00]\n");
        let got: Vec<(f64, &str)> = lines.iter().map(|l| (l.time, l.text.as_str())).collect();
        assert_eq!(
            got,
            [(5., "Again"), (12.5, "Hello"), (20., ""), (62., "Again")]
        );
    }

    #[test]
    fn reads_synced_lyrics_from_json() {
        let json = r#"{"id":1,"instrumental":false,"plainLyrics":"Hi","syncedLyrics":"[00:01.00]Café \"quoted\"\n[00:02.00]Line two"}"#;
        match parse_record(json) {
            Status::Synced(lines) => {
                assert_eq!(lines.len(), 2);
                assert_eq!(lines[0].text, "Café \"quoted\"");
            }
            _ => panic!("expected synced lyrics"),
        }
    }

    #[test]
    fn search_skips_entries_without_synced_lyrics() {
        let json =
            r#"[{"syncedLyrics":null,"plainLyrics":"x"},{"syncedLyrics":"[00:03.00]Found"}]"#;
        assert!(matches!(parse_record(json), Status::Synced(ref l) if l[0].text == "Found"));
        assert!(matches!(
            parse_record(r#"{"instrumental":true,"syncedLyrics":null}"#),
            Status::Instrumental
        ));
        assert!(matches!(
            parse_record(r#"{"plainLyrics":"words","syncedLyrics":null}"#),
            Status::Plain
        ));
        assert!(matches!(parse_record("[]"), Status::NotFound));
    }

    #[test]
    fn overload_is_a_failure_not_a_miss() {
        let busy = r#"{"message":"The server is busy","name":"ServerOverloaded","statusCode":503}"#;
        assert!(classify(Some((503, busy.into()))).is_none());
        assert!(classify(None).is_none());
        assert!(matches!(
            classify(Some((404, String::new()))),
            Some(Status::NotFound)
        ));
        assert!(matches!(
            classify(Some((200, "[]".into()))),
            Some(Status::NotFound)
        ));
    }

    #[test]
    fn loosens_titles_and_artists() {
        assert_eq!(clean_title("Blah Blah"), "Blah Blah");
        assert_eq!(
            clean_title("Song (feat. Someone) - Remastered 2011"),
            "Song"
        );
        assert_eq!(clean_title("Track [Live at Wembley]"), "Track");
        assert_eq!(clean_title("Love (Is Strange)"), "Love (Is Strange)");
        assert_eq!(clean_title("Hey - Jude"), "Hey - Jude");
        assert_eq!(primary_artist("Joji"), "Joji");
        assert_eq!(primary_artist("Joji, Diplo"), "Joji");
        assert_eq!(primary_artist("Calvin Harris & Dua Lipa"), "Calvin Harris");
        assert_eq!(primary_artist("A Feat. B"), "A");
    }

    #[test]
    fn cache_round_trip_and_expiry() {
        let synced = Status::Synced(parse_lrc("[00:03.73]Got me feeling crazy\n[01:12.50]Again"));
        let text = encode_cache(&synced, 0).unwrap();
        match decode_cache(&text, 0) {
            Some(Status::Synced(lines)) => {
                assert_eq!(lines.len(), 2);
                assert!((lines[0].time - 3.73).abs() < 0.01 && (lines[1].time - 72.5).abs() < 0.01);
                assert_eq!(lines[0].text, "Got me feeling crazy");
            }
            _ => panic!("expected synced lyrics"),
        }
        let missing = encode_cache(&Status::NotFound, 1000).unwrap();
        assert!(matches!(
            decode_cache(&missing, 1000 + 60),
            Some(Status::NotFound)
        ));
        assert!(decode_cache(&missing, 1000 + NOT_FOUND_TTL).is_none());
        assert!(encode_cache(&Status::Failed, 0).is_none());
        assert!(decode_cache("garbage", 0).is_none());
    }

    /// Hits the real LRCLIB: `cargo test live_lrclib -- --ignored`.
    #[test]
    #[ignore]
    fn live_lrclib() {
        let client = http_client().unwrap();
        let req = Request {
            key: String::new(),
            title: "Blah Blah".into(),
            artist: "Joji".into(),
            album: String::new(),
            duration: 162,
            fresh: true,
        };
        let status = fetch(&client, &req);
        assert!(
            matches!(status, Status::Synced(_) | Status::Failed),
            "a busy server must not read as not found"
        );
        assert!(
            matches!(status, Status::Synced(_)),
            "LRCLIB was unreachable on every attempt"
        );
    }

    fn request(title: &str, artist: &str, duration: i64) -> Request {
        Request {
            key: String::new(),
            title: title.into(),
            artist: artist.into(),
            album: String::new(),
            duration,
            fresh: true,
        }
    }

    #[test]
    fn picks_only_a_close_netease_match() {
        let json = r#"{"result":{"songs":[
            {"id":1,"name":"Yellow","duration":200000,"artists":[{"name":"Coldplay"}]},
            {"id":2,"name":"Yellow","duration":266773,"artists":[{"name":"Someone Else"}]},
            {"id":3,"name":"Yellow (Live)","duration":266773,"artists":[{"name":"Coldplay"}]},
            {"id":4,"name":"Yellow","duration":266773,"artists":[{"name":"Coldplay"}]}]}}"#;
        assert_eq!(
            pick_song(json, &request("Yellow", "Coldplay", 267)),
            Some(4)
        );
        assert_eq!(
            pick_song(json, &request("Yellow [Live]", "Coldplay", 267)),
            Some(3)
        );
        assert_eq!(
            pick_song(
                json,
                &request("Yellow - Remastered 2011", "Coldplay feat. X", 0)
            ),
            Some(1)
        );
        assert_eq!(pick_song(json, &request("Yellow", "Coldplay", 280)), None);
        assert_eq!(pick_song(json, &request("Fix You", "Coldplay", 0)), None);
        assert_eq!(
            pick_song("not json", &request("Yellow", "Coldplay", 0)),
            None
        );
    }

    #[test]
    fn reads_netease_lyrics_without_credits() {
        let json = r#"{"lrc":{"lyric":"[00:00.000] 作词 : A/B\n[00:01.000] 作曲 : A\n[00:33.790]Look at the stars\n[00:36.780]Look how they shine"},"code":200}"#;
        match netease_lyrics(json) {
            Some(Status::Synced(lines)) => {
                assert_eq!(lines.len(), 2);
                assert_eq!(lines[0].text, "Look at the stars");
            }
            _ => panic!("expected synced lyrics"),
        }
        assert!(matches!(
            netease_lyrics(r#"{"pureMusic":true,"lrc":{"lyric":""}}"#),
            Some(Status::Instrumental)
        ));
        assert!(matches!(
            netease_lyrics(r#"{"lrc":{"lyric":"[00:00.00] 纯音乐，请欣赏"}}"#),
            Some(Status::Instrumental)
        ));
        assert!(netease_lyrics(r#"{"uncollected":true,"lrc":{"lyric":""}}"#).is_none());
    }

    /// Hits the real NetEase API: `cargo test live_netease -- --ignored`.
    #[test]
    #[ignore]
    fn live_netease() {
        let client = http_client().unwrap();
        let status = netease(&client, &request("Yellow", "Coldplay", 267));
        assert!(
            matches!(status, Some(Status::Synced(ref l)) if l.iter().any(|l| l.text.contains("stars")))
        );
    }

    #[test]
    fn percent_encodes_queries() {
        assert_eq!(encode("Beyoncé & Co"), "Beyonc%C3%A9%20%26%20Co");
    }
}
