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

    // NOTE (audio fix): `yt-dlp -g` returns 2 URLs — video-only + audio-only —
    // and we used to pass only the first to mpv, hence silent video.
    // Now: mpv plays the YouTube URL directly via its ytdl backend, which
    // merges bestvideo+bestaudio (audio works, no manual URL plumbing).
    // vlc/iina/browser still need a direct single-file URL -> `-f best`
    // (progressive mp4 with audio, <=720p) instead of split streams.
    let player = cfg.player.to_lowercase();
    match player.as_str() {
        "browser" => {
            open_browser(&url);
            return Ok("opened in browser".into());
        }
        "iina" => {
            match single_file_url(&url) {
                Ok(stream) => {
                    let st = Command::new("open")
                        .args(["-a", "IINA", &stream])
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status();
                    if st.map(|s| s.success()).unwrap_or(false) {
                        return Ok(format!("▶ playing {video_id} via IINA (with audio)"));
                    }
                }
                Err(e) => return Err(e),
            }
            // fall through to mpv if IINA missing
        }
        "vlc" => {
            match single_file_url(&url) {
                Ok(stream) => {
                    let st = Command::new("vlc")
                        .arg(&stream)
                        .args(&cfg.player_args)
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn();
                    if st.is_ok() {
                        return Ok(format!("▶ playing {video_id} via vlc (with audio)"));
                    }
                }
                Err(e) => return Err(e),
            }
            // fall through to mpv
        }
        _ => {}
    }

    if has_mpv() {
        let mut cmd = Command::new("mpv");
        cmd.arg(&url).arg(format!("--title={video_id}"));
        // merge audio+video via yt-dlp backend inside mpv; quality from config (v cycles)
        cmd.arg(format!("--ytdl-format={}", quality_format(&cfg.quality)));
        // IPC socket enables [ ] speed control while playing
        cmd.arg(format!("--input-ipc-server={}", ipc_sock()));
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

/// Single progressive file URL (video+audio in one stream, <=720p).
/// Used for vlc/iina which can't merge split streams. mpv doesn't need this
/// since it merges via --ytdl-format itself.
fn single_file_url(youtube_url: &str) -> Result<String, String> {
    let out = Command::new("yt-dlp")
        .args(["-g", "-f", "best", "--no-playlist", "--no-warnings", youtube_url])
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
    Ok(stream)
}


/// mpv IPC socket for speed commands (std UnixStream, no extra deps).
pub fn ipc_sock() -> String {
    format!("{}/yt-tui-mpv.sock", std::env::var("TMPDIR").unwrap_or("/tmp".into()))
}

/// Map config quality -> mpv --ytdl-format.
pub fn quality_format(q: &str) -> &'static str {
    match q {
        "720p" => "bestvideo[height<=720]+bestaudio/best[height<=720]/best",
        "480p" => "bestvideo[height<=480]+bestaudio/best[height<=480]/best",
        "audio" => "bestaudio/best",
        _ => "bestvideo+bestaudio/best",
    }
}

pub fn cycle_quality(cfg: &mut config::Config) -> String {
    cfg.quality = match cfg.quality.as_str() {
        "best" => "720p".into(),
        "720p" => "480p".into(),
        "480p" => "audio".into(),
        _ => "best".into(),
    };
    config::save(cfg);
    cfg.quality.clone()
}

pub fn ipc_send(json_cmd: &str) -> Result<String, String> {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    let mut s = UnixStream::connect(ipc_sock()).map_err(|_| "mpv not running".to_string())?;
    s.write_all(json_cmd.as_bytes()).map_err(|e| e.to_string())?;
    s.write_all(b"
").map_err(|e| e.to_string())?;
    s.set_read_timeout(Some(std::time::Duration::from_secs(2))).ok();
    let mut buf = vec![0u8; 4096];
    let n = s.read(&mut buf).map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&buf[..n]).to_string())
}

pub fn ipc_speed(delta: f64) -> Result<String, String> {
    let cmd = if delta > 0.0 { r#"{"command":["add","speed",0.25]}"# } else { r#"{"command":["add","speed",-0.25]}"# };
    ipc_send(cmd)?;
    let resp = ipc_send(r#"{"command":["get_property","speed"]}"#).unwrap_or_default();
    Ok(format!("speed {}", resp.trim()))
}

/// Play a queue of ids natively in mpv (autoplay-next free via playlist).
pub fn play_queue(ids: &[String], cfg: &config::Config) -> Result<String, String> {
    if ids.is_empty() { return Err("queue empty — press a on videos to add".into()); }
    if !has_mpv() { return Err("mpv not found".into()); }
    let mut cmd = Command::new("mpv");
    for id in ids.iter().take(25) {
        cmd.arg(format!("https://www.youtube.com/watch?v={id}"));
    }
    cmd.arg(format!("--title=yt-tui queue ({} videos)", ids.len().min(25)));
    cmd.arg(format!("--ytdl-format={}", quality_format(&cfg.quality)));
    cmd.arg(format!("--input-ipc-server={}", ipc_sock()));
    if cfg.mpv_pretty {
        cmd.args(["--no-border", "--autofit-larger=90%x90%", "--msg-level=all=no"]);
    }
    cmd.args(&cfg.player_args);
    cmd.stdout(Stdio::null()).stderr(Stdio::null());
    cmd.spawn().map_err(|e| format!("mpv spawn failed ({e})"))?;
    Ok(format!("▶ queue: {} videos (mpv playlist, no ads)", ids.len().min(25)))
}

/// Download video (or audio-only) to download_dir in background.
pub fn download(video_id: &str, audio_only: bool, cfg: &config::Config) -> std::sync::mpsc::Receiver<Result<String, String>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let url = format!("https://www.youtube.com/watch?v={video_id}");
    let dir = config::download_dir(cfg);
    let _ = std::fs::create_dir_all(&dir);
    let dir_s = dir.to_string_lossy().to_string();
    std::thread::spawn(move || {
        let mut cmd = Command::new("yt-dlp");
        if audio_only { cmd.args(["-x", "--audio-format", "mp3"]); }
        else { cmd.args(["-f", "best"]); }
        cmd.args(["--no-playlist", "--no-warnings", "-o", &format!("{dir_s}/%(title)s.%(ext)s"), &url]);
        match cmd.output() {
            Ok(o) if o.status.success() => { let _ = tx.send(Ok(format!("saved to {dir_s}"))); }
            Ok(o) => { let _ = tx.send(Err(format!("dl failed: {}", String::from_utf8_lossy(&o.stderr).trim()))); }
            Err(e) => { let _ = tx.send(Err(format!("yt-dlp: {e}"))); }
        }
    });
    rx
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
