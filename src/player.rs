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

/// Play via `yt-dlp -g` + configured player. No, mpv is not the only option:
///
/// - "mpv" (default): lightest (~30MB), best TUI companion, ugly stock OSC.
///   We pass pretty args below (borderless + autofit + slim OSC).
/// - "iina" (macOS): native macOS GUI, prettiest on Mac. `brew install --cask iina`,
///   set `"player": "iina"` in ~/.config/yt-tui/config.json.
/// - "vlc": familiar GUI, heavier (~100MB+).
/// - "browser": just open YouTube (keeps ads, zero setup).
pub fn play(video_id: &str) -> Result<String, String> {
    play_with(video_id, &crate::config::load())
}

pub fn play_with(video_id: &str, cfg: &config::Config) -> Result<String, String> {
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

    let player = cfg.player.to_lowercase();
    match player.as_str() {
        "browser" => {
            open_browser(&url);
            return Ok("opened in browser".into());
        }
        "iina" => {
            // macOS native player, much prettier than stock mpv
            let st = Command::new("open")
                .args(["-a", "IINA", &stream])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            if st.map(|s| s.success()).unwrap_or(false) {
                return Ok(format!("▶ playing {video_id} via IINA"));
            }
            // fall through to mpv if IINA missing
        }
        "vlc" => {
            let st = Command::new("vlc")
                .arg(&stream)
                .args(&cfg.player_args)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
            if st.is_ok() {
                return Ok(format!("▶ playing {video_id} via vlc"));
            }
            // fall through to mpv
        }
        _ => {}
    }

    if has_mpv() {
        let mut cmd = Command::new("mpv");
        cmd.arg(&stream).arg(format!("--title={video_id}"));
        if cfg.mpv_pretty {
            // slim modern look without extra skins: borderless autofit + slim OSC.
            // For full uosc skin: `brew install uosc` / see mpv.conf docs.
            cmd.args([
                "--no-border",
                "--autofit-larger=90%x90%",
                "--osc=yes",
                "--osd-bar",
                "--msg-level=all=no",
            ]);
        }
        cmd.args(&cfg.player_args);
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
        cmd.spawn()
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
pub fn record_played(video_id: &str, title: &str, channel: &str, cfg: &config::Config) {
    config::push_history(cfg, video_id, title, channel);
}
