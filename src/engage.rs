//! YouTube engagement actions (like, subscribe, save) via the Innertube API.
//!
//! No Google password, no OAuth project, no new Rust deps: auth reuses the
//! `cookies.txt` export (Settings → Cookies file) and all HTTPS goes through
//! the system `curl` binary (same trick as thumbnails).
//!
//! Why cookies.txt and not `--cookies-from-browser`? Chrome/Zen lock their
//! cookie DBs while running and macOS encrypts values in the keychain —
//! reading SAPISID out of them reliably is a losing game. The exported
//! Netscape file is plain text and always works.

use std::collections::HashMap;
use std::process::Command;

use crate::config;

// Public web-client key (shipped in every youtube.com page; stable for years).
const API_KEY: &str = "AIzaSyAO_FJ2SlqU8Q4STEHLGCilw_Y9_11qcW8";
const CLIENT_VERSION: &str = "2.20250101.00.00";
const ORIGIN: &str = "https://www.youtube.com";
const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36";

fn shellexpand(s: &str) -> String {
    if let Some(rest) = s.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{home}/{rest}");
        }
    }
    s.to_string()
}

/// Parse a Netscape cookies.txt into name→value for youtube.com.
/// Returns (all_cookies_for_header, sapisid).
fn load_cookies(cfg: &config::Config) -> Result<(String, String), String> {
    if cfg.cookies_file.is_empty() {
        return Err("needs cookies.txt — Settings → Cookies file → export, then Test login".into());
    }
    let p = shellexpand(&cfg.cookies_file);
    let text = std::fs::read_to_string(&p)
        .map_err(|_| format!("cookies file not found: {p} — export it first (README login)"))?;
    let mut jar: HashMap<String, String> = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 7 {
            continue;
        }
        let domain = cols[0];
        if !domain.contains("youtube.com") {
            continue;
        }
        jar.insert(cols[5].to_string(), cols[6].to_string());
    }
    let sapisid = jar.get("SAPISID").cloned().unwrap_or_default();
    if sapisid.is_empty() {
        return Err("no login in cookies.txt — open youtube.com logged in, re-export".into());
    }
    // nonzero expiry check on SID is overkill; SAPISID presence + API test decides.
    let header = jar
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; ");
    Ok((header, sapisid))
}

/// Minimal SHA-1 (FIPS 180-4) so SAPISIDHASH needs no new dependency.
fn sha1_hex(msg: &[u8]) -> String {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let mut data = msg.to_vec();
    let bit_len = (data.len() as u64).wrapping_mul(8);
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in data.chunks_exact(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[4 * i], chunk[4 * i + 1], chunk[4 * i + 2], chunk[4 * i + 3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (i, &x) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let tmp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(x);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = tmp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

fn auth_header(sapisid: &str) -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let hash = sha1_hex(format!("{ts} {sapisid} {ORIGIN}").as_bytes());
    format!("SAPISIDHASH {ts}_{hash}")
}

/// `"context":{...}` fragment shared by every Innertube call.
fn context_inner() -> String {
    format!(r#""context":{{"client":{{"clientName":"WEB","clientVersion":"{CLIENT_VERSION}","hl":"en","gl":"US"}}}}"#)
}

/// POST a JSON body to an Innertube endpoint. Returns Ok(()) on clean 200.
fn post(cfg: &config::Config, endpoint: &str, params_json: &str) -> Result<(), String> {
    let (cookies, sapisid) = load_cookies(cfg)?;
    let body = format!("{{{},{}}}", context_inner(), params_json);
    let url = format!("https://www.youtube.com/youtubei/v1/{endpoint}?key={API_KEY}&prettyPrint=false");
    let out = Command::new("curl")
        .args([
            "-s", "--max-time", "20", "-X", "POST", &url,
            "-H", &format!("Authorization: {}", auth_header(&sapisid)),
            "-H", &format!("X-Origin: {ORIGIN}"),
            "-H", &format!("Origin: {ORIGIN}"),
            "-H", "Content-Type: application/json",
            "-H", &format!("User-Agent: {UA}"),
            "-H", &format!("Cookie: {cookies}"),
            "-d", &body,
        ])
        .output()
        .map_err(|e| format!("curl failed: {e}"))?;
    let resp = String::from_utf8_lossy(&out.stdout);
    if resp.contains(r#""error""#) || resp.contains("LOGIN_REQUIRED") {
        return Err("YouTube rejected it — cookies expired? re-export cookies.txt".into());
    }
    if !out.status.success() || resp.trim().is_empty() {
        return Err("empty response — network hiccup, try again".into());
    }
    Ok(())
}

pub fn like(cfg: &config::Config, video_id: &str) -> Result<String, String> {
    post(cfg, "like/like", &format!(r#""target":{{"videoId":"{video_id}"}}"#))?;
    Ok("♥ liked (synced to your account)".into())
}

pub fn remove_rating(cfg: &config::Config, video_id: &str) -> Result<String, String> {
    post(cfg, "like/removelike", &format!(r#""target":{{"videoId":"{video_id}"}}"#))?;
    Ok("rating removed".into())
}

pub fn dislike(cfg: &config::Config, video_id: &str) -> Result<String, String> {
    post(cfg, "like/dislike", &format!(r#""target":{{"videoId":"{video_id}"}}"#))?;
    Ok("👎 disliked (synced to your account)".into())
}

pub fn save_watch_later(cfg: &config::Config, video_id: &str) -> Result<String, String> {
    post(
        cfg,
        "browse/edit_playlist",
        &format!(r#""playlistId":"WL","actions":[{{"action":"ACTION_ADD_VIDEO","addedVideoId":"{video_id}"}}]"#),
    )?;
    Ok("◷ saved to Watch Later".into())
}

/// Resolve anything we know (UC id, @handle, channel URL) to a UC channel id.
pub fn resolve_channel_id(ref_: &str) -> Result<String, String> {
    let r = ref_.trim();
    if r.starts_with("UC") && r.len() == 24 && r.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Ok(r.to_string());
    }
    let url = if r.starts_with("http") {
        r.to_string()
    } else if r.starts_with('@') {
        format!("https://www.youtube.com/{r}")
    } else {
        return Err("no channel link on this video (search result)".into());
    };
    let out = Command::new("yt-dlp")
        .args(["--print", "channel_id", "--playlist-end", "1", "--no-warnings", &url])
        .output()
        .map_err(|e| format!("yt-dlp not found ({e})"))?;
    let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if id.starts_with("UC") {
        Ok(id)
    } else {
        Err("couldn't resolve channel id".into())
    }
}

pub fn subscribe(cfg: &config::Config, channel_ref: &str) -> Result<String, String> {
    let id = resolve_channel_id(channel_ref)?;
    post(cfg, "subscription/subscribe", &format!(r#""channelIds":["{id}"],"params":"EgIIAg==""#))?;
    Ok("＋ subscribed (synced to your account)".into())
}

pub fn unsubscribe(cfg: &config::Config, channel_ref: &str) -> Result<String, String> {
    let id = resolve_channel_id(channel_ref)?;
    post(cfg, "subscription/unsubscribe", &format!(r#""channelIds":["{id}"]"#))?;
    Ok("unsubscribed (synced to your account)".into())
}

#[cfg(test)]
mod tests {
    use super::sha1_hex;

    #[test]
    fn sha1_vectors() {
        // FIPS 180-4 test vectors
        assert_eq!(
            sha1_hex(b"abc"),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(
            sha1_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
        assert_eq!(
            sha1_hex(b""),
            "da39a3ee5e6b4b0d3255bfef95601890afd80709"
        );
    }
}
