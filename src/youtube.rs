use std::process::Command;
use std::time::Duration;

use serde::Deserialize;

use crate::{app::Video, config};

/// Lightweight fetch via installed `yt-dlp` (no API key, no TLS in our binary).
/// Runs in background threads (see App) so UI never blocks.

/// Resolve `--cookies-from-browser` value, including `zen` (Firefox fork).
/// Zen stores cookies at ~/Library/Application Support/zen/Profiles/*/cookies.sqlite
/// which yt-dlp reads via the firefox backend + explicit profile dir.
pub fn browser_spec(browser: &str) -> String {
    if browser.eq_ignore_ascii_case("zen") {
        if let Some(dir) = find_zen_profile() {
            return format!("firefox:{dir}");
        }
        // fall back: yt-dlp will error clearly, auth_test explains cookies.txt
        return "firefox".to_string();
    }
    browser.to_string()
}

fn find_zen_profile() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let base = std::path::Path::new(&home).join("Library/Application Support/zen/Profiles");
    let rd = std::fs::read_dir(&base).ok()?;
    // prefer profile holding cookies.sqlite, newest first
    let mut cands: Vec<_> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.join("cookies.sqlite").exists())
        .filter_map(|p| {
            let mtime = p
                .join("cookies.sqlite")
                .metadata()
                .ok()?
                .modified()
                .ok()?;
            Some((p, mtime))
        })
        .collect();
    cands.sort_by_key(|(_, t)| *t);
    cands.pop().map(|(p, _)| p.to_string_lossy().to_string())
}

fn cookie_args(cfg: &config::Config, force: bool) -> Vec<String> {
    // cookies.txt file wins: reliable when Chrome is open or macOS keychain blocks reads
    if !cfg.cookies_file.is_empty() {
        let p = shellexpand(&cfg.cookies_file);
        if std::path::Path::new(&p).exists() {
            return vec!["--cookies".to_string(), p];
        }
        // file set but missing -> fall through to browser with clear error later
    }
    if cfg.use_cookies || force {
        if !cfg.browser.is_empty() {
            return vec!["--cookies-from-browser".to_string(), browser_spec(&cfg.browser)];
        }
    }
    vec![]
}

fn shellexpand(s: &str) -> String {
    if let Some(rest) = s.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{home}/{rest}");
        }
    }
    s.to_string()
}

fn base_args(cfg: &config::Config) -> Vec<String> {
    let mut a = vec![
        "--flat-playlist".to_string(),
        "-J".to_string(),
        "--no-warnings".to_string(),
    ];
    a.extend(cookie_args(cfg, false));
    a
}

fn run_ytdlp(args: &[String]) -> Result<Vec<u8>, String> {
    let out = Command::new("yt-dlp")
        .args(args)
        .output()
        .map_err(|e| format!("yt-dlp not found ({e})"))?;
    if !out.status.success() {
        return Err(format!(
            "yt-dlp failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(out.stdout)
}

pub fn search(cfg: &config::Config, query: &str, limit: usize) -> Result<Vec<Video>, String> {
    let target = format!("ytsearch{}:{query}", limit.min(20));
    let mut args = base_args(cfg);
    args.push(target);
    parse_flat(&run_ytdlp(&args)?)
}

/// Channel videos via `yt-dlp --flat-playlist -J <url>` (works for @handles).
pub fn channel_videos(
    cfg: &config::Config,
    url: &str,
    limit: usize,
) -> Result<Vec<Video>, String> {
    let mut args = base_args(cfg);
    args.push("--playlist-end".to_string());
    args.push(limit.min(15).to_string());
    args.push(url.to_string());
    parse_flat(&run_ytdlp(&args)?)
}

/// Login-gated playlists. Require `use_cookies=true` + logged-in browser.
/// WL = Watch Later, LL = Liked videos.
pub fn private_playlist(
    cfg: &config::Config,
    which: &str,
    limit: usize,
) -> Result<Vec<Video>, String> {
    let url = match which {
        "later" => "https://www.youtube.com/playlist?list=WL",
        "liked" => "https://www.youtube.com/playlist?list=LL",
        "subs" => "https://www.youtube.com/feed/subscriptions",
        _ => return Err("unknown playlist".into()),
    };
    channel_videos(cfg, url, limit)
}

/// Test cookie login: fetch 1 item from subs feed.
/// Tries cookies.txt file first, then browser. Ok(msg) on success.
pub fn auth_test(cfg: &config::Config) -> Result<String, String> {
    let mut args = vec![
        "--flat-playlist".to_string(),
        "-J".to_string(),
        "--no-warnings".to_string(),
    ];
    let src: String;
    if !cfg.cookies_file.is_empty() {
        let p = shellexpand(&cfg.cookies_file);
        if !std::path::Path::new(&p).exists() {
            return Err(format!(
                "cookies_file not found: {p} — export it first (see README login)"
            ));
        }
        args.push("--cookies".to_string());
        args.push(p.clone());
        src = format!("cookies file {p}");
    } else {
        // force browser cookies for the test even if use_cookies=false
        args.push("--cookies-from-browser".to_string());
        args.push(browser_spec(&cfg.browser));
        src = format!("{} cookies", cfg.browser);
    }
    args.push("--playlist-end".to_string());
    args.push("1".to_string());
    args.push("https://www.youtube.com/feed/subscriptions".to_string());
    let raw = run_ytdlp(&args).map_err(|e| {
        if cfg.cookies_file.is_empty() {
            format!(
                "{e} — Chrome locks cookies while open + macOS keychain often blocks reads. Fix: quit {b}, press L again, or use the reliable path: export cookies.txt (README login) and set cookies_file. Tried: {b} cookies",
                b = cfg.browser
            )
        } else {
            format!("{e} — cookies may be expired, re-export cookies.txt from youtube.com")
        }
    })?;
    let _vids = parse_flat(&raw)?;
    Ok(format!(
        "login OK via {src} — set use_cookies=true (or keep cookies_file) to keep it"
    ))
}

#[derive(Debug, Deserialize)]
struct Flat {
    entries: Option<Vec<Entry>>,
    #[serde(default)]
    channel: Option<String>,
    #[serde(default)]
    uploader: Option<String>,
    #[serde(default)]
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Entry {
    id: Option<String>,
    title: Option<String>,
    #[serde(default)]
    channel: Option<String>,
    #[serde(default)]
    uploader: Option<String>,
    #[serde(default)]
    uploader_id: Option<String>,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    view_count: Option<u64>,
    #[serde(default)]
    timestamp: Option<i64>,
    #[serde(default)]
    channel_is_verified: Option<bool>,
}

pub fn parse_flat(json: &[u8]) -> Result<Vec<Video>, String> {
    let flat: Flat =
        serde_json::from_slice(json).map_err(|e| format!("parse yt-dlp json: {e}"))?;
    // channel feeds omit per-entry channel — fall back to playlist-level name
    // e.g. title "ScrapMan - Videos" -> "ScrapMan"
    let fallback = flat
        .channel
        .or(flat.uploader)
        .or_else(|| {
            flat.title.as_ref().and_then(|t| {
                t.strip_suffix(" - Videos")
                    .or_else(|| t.strip_suffix(" - Live"))
                    .map(|s| s.to_string())
            })
        })
        .unwrap_or_else(|| "YouTube".into());
    let mut out = vec![];
    for e in flat.entries.unwrap_or_default() {
        let (Some(id), Some(title)) = (e.id, e.title) else {
            continue;
        };
        if id.is_empty() || title == "[Private video]" || title == "[Deleted video]" {
            continue;
        }
        let seed: u64 = id.bytes().fold(0xcbf29ce484222325, |a, b| {
            a.wrapping_mul(0x100000001b3).wrapping_add(b as u64)
        });
        let channel = e
            .channel
            .or(e.uploader)
            .or(e.uploader_id)
            .unwrap_or_else(|| fallback.clone());
        out.push(Video {
            hue: (seed % 256) as u8,
            seed,
            id,
            title,
            channel,
            verified: e.channel_is_verified.unwrap_or(false),
            views: e.view_count.map(fmt_views).unwrap_or_default(),
            age: e.timestamp.map(fmt_age).unwrap_or_default(),
            duration: e.duration.map(fmt_dur).unwrap_or_default(),
        });
    }
    Ok(out)
}

pub fn fmt_views(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M views", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K views", n as f64 / 1_000.0)
    } else if n == 0 {
        String::new()
    } else {
        format!("{n} views")
    }
}

pub fn fmt_dur(s: f64) -> String {
    let s = s as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Relative age from unix timestamp. Flat playlists often omit it (None -> "").
pub fn fmt_age(ts: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(ts);
    let d = (now - ts).max(0);
    if d < 3600 {
        format!("{} min ago", (d / 60).max(1))
    } else if d < 86400 {
        format!("{} hours ago", d / 3600)
    } else if d < 86400 * 30 {
        format!("{} days ago", d / 86400)
    } else if d < 86400 * 365 {
        format!("{} months ago", d / (86400 * 30))
    } else {
        format!("{} years ago", d / (86400 * 365))
    }
}

pub fn _timeout() -> Duration {
    Duration::from_secs(25)
}
