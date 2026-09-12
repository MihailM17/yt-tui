use std::process::Command;
use std::time::Duration;

use serde::Deserialize;

use crate::app::Video;

/// Lightweight search via installed `yt-dlp` (no API key, no TLS in our binary).
/// `yt-dlp --flat-playlist -J "ytsearch12:query"` returns compact JSON.
/// Runs in a background thread (see App) so UI never blocks.
pub fn search(query: &str, limit: usize) -> Result<Vec<Video>, String> {
    let target = format!("ytsearch{}:{query}", limit.min(20));
    let out = Command::new("yt-dlp")
        .args(["--flat-playlist", "-J", "--no-warnings", &target])
        .output()
        .map_err(|e| format!("yt-dlp not found ({e})"))?;

    if !out.status.success() {
        return Err(format!(
            "yt-dlp search failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    parse_flat(&out.stdout)
}

/// Channel videos via `yt-dlp --flat-playlist -J <url>` (works for @handles).
pub fn channel_videos(url: &str, limit: usize) -> Result<Vec<Video>, String> {
    let mut cmd = Command::new("yt-dlp");
    cmd.args(["--flat-playlist", "-J", "--no-warnings"]);
    // playlist end keeps it fast + small
    cmd.args(["--playlist-end", &limit.min(15).to_string(), url]);
    let out = cmd
        .output()
        .map_err(|e| format!("yt-dlp not found ({e})"))?;

    if !out.status.success() {
        return Err(format!(
            "yt-dlp feed failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    parse_flat(&out.stdout)
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

fn parse_flat(json: &[u8]) -> Result<Vec<Video>, String> {
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

fn fmt_views(n: u64) -> String {
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

fn fmt_dur(s: f64) -> String {
    let s = s as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

pub fn _timeout() -> Duration {
    Duration::from_secs(25)
}
