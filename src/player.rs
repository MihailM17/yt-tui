use std::process::{Command, Stdio};

use crate::config;

/// Returns true if `mpv` is on PATH.
pub fn has_mpv() -> bool {
    Command::new("mpv")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Play via `yt-dlp -g` + detached `mpv`. Falls back to browser if mpv missing.
pub fn play(video_id: &str) -> Result<String, String> {
    let url = format!("https://www.youtube.com/watch?v={video_id}");

    let out = Command::new("yt-dlp")
        .args(["-g", "--no-playlist", "--no-warnings", &url])
        .output()
        .map_err(|e| format!("yt-dlp not found ({e})"))?;

    if !out.status.success() {
        return Err(format!(
            "yt-dlp failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }

    let stream = String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    if stream.is_empty() {
        return Err("empty stream url".into());
    }

    if has_mpv() {
        Command::new("mpv")
            .arg(&stream)
            .arg(format!("--title={video_id}"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("mpv spawn failed ({e})"))?;
        return Ok(format!("▶ playing {video_id} via mpv (no ads)"));
    }

    // graceful fallback on machines without mpv (like this Mac right now)
    open_browser(&url);
    Ok("mpv not found — opened in browser instead (brew install mpv)".into())
}

pub fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg(url).spawn();
    #[cfg(target_os = "linux")]
    let _ = Command::new("xdg-open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let _ = Command::new("cmd").args(["/C", "start", url]).spawn();
}

#[allow(dead_code)]
pub fn record_played(video_id: &str, title: &str, cfg: &config::Config) {
    config::push_history(cfg, video_id, title);
}
