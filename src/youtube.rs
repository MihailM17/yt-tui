use std::process::Command;
use std::time::Duration;

use serde::Deserialize;

use crate::{app::Video, config};

/// Lightweight fetch via installed `yt-dlp` (no API key, no TLS in our binary).
/// Runs in background threads (see App) so UI never blocks.

fn base_args(cfg: &config::Config) -> Vec<String> {
    let mut a = vec![
        "--flat-playlist".to_string(),
        "-J".to_string(),
        "--no-warnings".to_string(),
    ];
    if cfg.use_cookies && !cfg.browser.is_empty() {
        a.push("--cookies-from-browser".to_string());
        a.push(cfg.browser.clone());
    }
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
/// Ok(handle) on success, Err(hint) on failure.
pub fn auth_test(cfg: &config::Config) -> Result<String, String> {
    let mut args = base_args(cfg);
    // force cookies even if use_cookies=false, to test before enabling
    if !cfg.use_cookies {
        args.push("--cookies-from-browser".to_string());
        args.push(cfg.browser.clone());
    }
    args.push("--playlist-end".to_string());
    args.push("1".to_string());
    args.push("https://www.youtube.com/feed/subscriptions".to_string());
    let raw = run_ytdlp(&args).map_err(|e| {
        format!("{e} — is YouTube open+logged in under browser=\"{}\" in config.json?", cfg.browser)
    })?;
    let vids = parse_flat(&raw)?;
    Ok(format!(
        "login OK via {} cookies ({} item{}) — set use_cookies=true to keep it",
        cfg.browser,
        vids.len().max(1),
        if vids.len() == 1 { "" } else { "s" }
    ))
}

#[derive(Debug, Deserialize)]
struct Flat {
    entries: Option<Vec<Entry>>,
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
    duration: Option<f64>,
    #[serde(default)]
    view_count: Option<u64>,
}

pub fn parse_flat(json: &[u8]) -> Result<Vec<Video>, String> {
    let flat: Flat =
        serde_json::from_slice(json).map_err(|e| format!("parse yt-dlp json: {e}"))?;
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
        out.push(Video {
            hue: (seed % 256) as u8,
            seed,
            id,
            title,
            channel: e.channel.or(e.uploader).unwrap_or_else(|| "YouTube".into()),
            verified: false,
            views: e.view_count.map(fmt_views).unwrap_or_default(),
            age: String::new(),
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

pub fn _timeout() -> Duration {
    Duration::from_secs(25)
}
