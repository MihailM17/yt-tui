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
    search_sorted(cfg, query, limit, SortMode::Relevance)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    Relevance,
    Views,
    Longest,
    Shortest,
}

impl SortMode {
    pub fn next(self) -> Self {
        match self {
            Self::Relevance => Self::Views,
            Self::Views => Self::Longest,
            Self::Longest => Self::Shortest,
            Self::Shortest => Self::Relevance,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Relevance => "relevance",
            Self::Views => "most viewed",
            Self::Longest => "longest",
            Self::Shortest => "shortest",
        }
    }
}

/// Server-side date/views sort prefixes (ytsearchdate:) don't exist in this
/// yt-dlp, so sort locally — zero extra network, instant.
pub fn search_sorted(
    cfg: &config::Config,
    query: &str,
    limit: usize,
    sort: SortMode,
) -> Result<Vec<Video>, String> {
    let target = format!("ytsearch{}:{query}", limit.min(30));
    let mut args = base_args(cfg);
    args.push(target);
    let mut vids = parse_flat(&run_ytdlp(&args)?)?;
    match sort {
        SortMode::Relevance => {}
        SortMode::Views => vids.sort_by_key(|v| std::cmp::Reverse(view_key(&v.views))),
        SortMode::Longest => vids.sort_by_key(|v| std::cmp::Reverse(dur_key(&v.duration))),
        SortMode::Shortest => vids.sort_by_key(|v| dur_key(&v.duration)),
    }
    Ok(vids)
}

fn view_key(s: &str) -> u64 {
    // "1.2M views" -> 1200000; "" -> 0
    let s = s.replace(" views", "").replace(" view", "");
    if let Some(n) = s.strip_suffix('M') {
        (n.parse::<f64>().unwrap_or(0.0) * 1_000_000.0) as u64
    } else if let Some(n) = s.strip_suffix('K') {
        (n.parse::<f64>().unwrap_or(0.0) * 1_000.0) as u64
    } else {
        s.parse::<u64>().unwrap_or(0)
    }
}

fn dur_key(s: &str) -> u64 {
    // "m:ss" or "h:mm:ss" -> seconds
    let parts: Vec<u64> = s.split(':').filter_map(|p| p.parse().ok()).collect();
    parts.iter().fold(0, |a, b| a * 60 + b)
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

// ---------------------------------------------------------------------------
// Info + comments (background threads; ~1MB info.json for comments, parsed+deleted)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq)]
pub struct VideoInfo {
    pub title: String,
    pub channel: String,
    pub views: String,
    pub likes: String,
    pub date: String,
    pub duration: String,
    pub desc: String,
    pub chapters: Vec<(String, String)>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Comment {
    pub author: String,
    pub text: String,
    pub likes: String,
}

pub fn video_info(video_id: &str) -> Result<VideoInfo, String> {
    let url = format!("https://www.youtube.com/watch?v={video_id}");
    let out = std::process::Command::new("yt-dlp")
        .args(["--dump-single-json", "--skip-download", "--no-warnings", &url])
        .output()
        .map_err(|e| format!("yt-dlp not found ({e})"))?;
    if !out.status.success() {
        return Err(format!("info failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).map_err(|e| format!("parse info: {e}"))?;
    let strf = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let mut chapters = vec![];
    if let Some(ch) = v.get("chapters").and_then(|c| c.as_array()) {
        for c in ch.iter().take(20) {
            let t = c.get("start_time").and_then(|x| x.as_f64()).unwrap_or(0.0) as u64;
            let title = c.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string();
            chapters.push((format!("{}:{:02}", t / 60, t % 60), title));
        }
    }
    let likes = v.get("like_count").and_then(|x| x.as_u64()).map(fmt_views).unwrap_or_default();
    let views = v.get("view_count").and_then(|x| x.as_u64()).map(fmt_views).unwrap_or_default();
    let date = v.get("upload_date").and_then(|x| x.as_str()).map(|d| {
        if d.len() == 8 { format!("{}-{}-{}", &d[0..4], &d[4..6], &d[6..8]) } else { d.to_string() }
    }).unwrap_or_default();
    Ok(VideoInfo {
        title: strf("title"), channel: strf("uploader"),
        views, likes, date,
        duration: v.get("duration").and_then(|x| x.as_f64()).map(fmt_dur).unwrap_or_default(),
        desc: strf("description"), chapters,
    })
}

pub fn video_comments(video_id: &str, max: usize) -> Result<Vec<Comment>, String> {
    let url = format!("https://www.youtube.com/watch?v={video_id}");
    let dir = config::cache_dir().join("comments");
    let _ = std::fs::create_dir_all(&dir);
    let tpl = format!("{}/c-%(id)s.%(ext)s", dir.to_string_lossy());
    let out = std::process::Command::new("yt-dlp")
        .args(["--skip-download", "--write-info-json", "--no-warnings",
               "--max-downloads", "1", "-o", &tpl, &url])
        .output()
        .map_err(|e| format!("yt-dlp not found ({e})"))?;
    if !out.status.success() {
        return Err(format!("comments failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    // find newest info.json, parse top comments, delete
    let mut newest: Option<(std::path::PathBuf, std::time::SystemTime)> = None;
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.filter_map(|e| e.ok()) {
            let path = e.path();
            if path.extension().map(|x| x == "json").unwrap_or(false) {
                if let Ok(m) = e.metadata().and_then(|m| m.modified()) {
                    if newest.as_ref().map(|(_, t)| m > *t).unwrap_or(true) {
                        newest = Some((path, m));
                    }
                }
            }
        }
    }
    let Some((path, _)) = newest else { return Err("no comments file".into()) };
    let data = std::fs::read(&path).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&path);
    let v: serde_json::Value =
        serde_json::from_slice(&data).map_err(|e| format!("parse comments: {e}"))?;
    let mut out = vec![];
    if let Some(arr) = v.get("comments").and_then(|c| c.as_array()) {
        for c in arr.iter().take(max.max(5)) {
            let author = c.get("author").and_then(|x| x.as_str()).unwrap_or("?").to_string();
            let mut text = c.get("text").and_then(|x| x.as_str()).unwrap_or("").to_string();
            text = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if text.len() > 220 { text.truncate(220); text.push('…'); }
            let likes = c.get("like_count").and_then(|x| x.as_u64()).map(|n| if n > 1000 { format!("{}K", n/1000) } else { n.to_string() }).unwrap_or_default();
            out.push(Comment { author, text, likes });
        }
    }
    if out.is_empty() { return Err("comments disabled for this video".into()); }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Import channel list from the logged-in account (fixes "missing subs").
// Manual config subs only cover what you typed; this merges every channel
// YouTube itself lists under feed/subscriptions (needs cookies).
// Returns deduplicated @handles / channel URLs ready for config.
// ---------------------------------------------------------------------------

pub fn import_subscriptions(cfg: &config::Config) -> Result<Vec<String>, String> {
    let mut args = vec![
        "--flat-playlist".to_string(),
        "-J".to_string(),
        "--no-warnings".to_string(),
    ];
    // cookies required: file wins, else browser
    if !cfg.cookies_file.is_empty() {
        let p = shellexpand(&cfg.cookies_file);
        if !std::path::Path::new(&p).exists() {
            return Err(format!("cookies_file not found: {p}"));
        }
        args.push("--cookies".to_string());
        args.push(p);
    } else if !cfg.browser.is_empty() {
        args.push("--cookies-from-browser".to_string());
        args.push(browser_spec(&cfg.browser));
    } else {
        return Err("no login configured — set browser/coookies_file in Settings".into());
    }
    args.push("--playlist-end".to_string());
    args.push("60".to_string());
    args.push("https://www.youtube.com/feed/subscriptions".to_string());
    let raw = run_ytdlp(&args).map_err(|e| format!("{e} — login first (Settings → Test login)"))?;
    let v: serde_json::Value =
        serde_json::from_slice(&raw).map_err(|e| format!("parse subs: {e}"))?;
    let mut seen = std::collections::HashSet::new();
    let mut out = vec![];
    if let Some(arr) = v.get("entries").and_then(|e| e.as_array()) {
        for e in arr {
            // prefer @handle (stable, readable), else channel URL
            let handle = e.get("uploader_id").and_then(|x| x.as_str()).unwrap_or("");
            let curl = e.get("channel_url").and_then(|x| x.as_str()).unwrap_or("");
            let chid = e.get("channel_id").and_then(|x| x.as_str()).unwrap_or("");
            let cand = if handle.starts_with('@') { handle.to_string() }
                else if !curl.is_empty() { curl.to_string() }
                else if !chid.is_empty() { format!("https://www.youtube.com/channel/{chid}") }
                else { continue };
            if seen.insert(cand.clone()) { out.push(cand); }
        }
    }
    if out.is_empty() { return Err("no channels found — cookies may lack a login".into()); }
    Ok(out)
}
