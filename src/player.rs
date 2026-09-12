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
pub fn play(video_id: &str) -> Result<(std::process::Child, String), String> {
    play_with(video_id, &crate::config::load(), None)
}

pub fn play_with(video_id: &str, cfg: &config::Config, start_secs: Option<u64>) -> Result<(std::process::Child, String), String> {
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
            let child = open_browser(&url);
            return Ok((child, "opened in browser".into()));
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
                        let dummy = Command::new("sleep")
                            .arg("0")
                            .stdout(Stdio::null())
                            .stderr(Stdio::null())
                            .spawn()
                            .map_err(|e| e.to_string())?;
                        return Ok((dummy, format!("▶ playing {video_id} via IINA (with audio)")));
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
                    if let Ok(child) = st {
                        return Ok((child, format!("▶ playing {video_id} via vlc (with audio)")));
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
        if let Some(s) = start_secs.filter(|s| *s > 10) {
            cmd.arg(format!("--start={s}"));
        }
        if let Some(script_args) = sponsorblock_args(video_id, cfg) {
            cmd.args(script_args);
        }
        if cfg.subtitles {
            cmd.args(["--ytdl-sub-langs=en", "--sub-auto=fuzzy"]);
        }
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
        let child = cmd.spawn().map_err(|e| format!("mpv spawn failed ({e})"))?;
        return Ok((child, format!("▶ playing {video_id} via mpv (no ads)")));
    }

    // graceful fallback when mpv is missing
    let child = open_browser(&url);
    Ok((child, "mpv not found — opened in browser instead (brew install mpv)".into()))
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
pub fn play_queue(ids: &[String], cfg: &config::Config) -> Result<(std::process::Child, String), String> {
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
    let child = cmd.spawn().map_err(|e| format!("mpv spawn failed ({e})"))?;
    Ok((child, format!("▶ queue: {} videos (mpv playlist, no ads)", ids.len().min(25))))
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


// ---------------------------------------------------------------------------
// SponsorBlock: fetch skip ranges, install a tiny mpv lua skipper once.
// ---------------------------------------------------------------------------

const SB_LUA: &str = r#"-- yt-tui sponsorblock skipper (auto-installed, safe to delete)
local segs = {}
do
    local path = mp.get_opt("segments")
    if path then
        local f = io.open(path, "r")
        if f then
            local body = f:read("*a")
            f:close()
            for s, e in body:gmatch("%[%s*([%d%.]+)%s*,%s*([%d%.]+)%s*%]") do
                local a, b = tonumber(s), tonumber(e)
                if a and b and b > a then segs[#segs + 1] = {a, b} end
            end
        end
    end
end
mp.observe_property("time-pos", "number", function(_, pos)
    if not pos then return end
    for _, r in ipairs(segs) do
        if pos >= r[1] and pos < r[2] - 0.3 then
            mp.set_property("time-pos", r[2])
            mp.osd_message("skipped sponsor", 1.2)
            return
        end
    end
end)
"#;

/// Returns extra mpv args, or None when disabled/unavailable.
pub fn sponsorblock_args(video_id: &str, cfg: &config::Config) -> Option<Vec<String>> {
    if !cfg.sponsorblock || video_id.starts_with("mock") {
        return None;
    }
    let segs = fetch_segments(video_id)?;
    if segs.is_empty() {
        return None;
    }
    let dir = std::env::var("TMPDIR").unwrap_or("/tmp".into());
    let json_path = format!("{dir}/yt-tui-sb-{video_id}.json");
    let body = format!("[{}]", segs.iter().map(|(a, b)| format!("[{a},{b}] ")).collect::<Vec<_>>().join(","));
    std::fs::write(&json_path, body).ok()?;
    let script = ensure_lua()?;
    Some(vec![
        format!("--script={script}"),
        format!("--script-opts=yt-tui-sb-segments={json_path}"),
    ])
}

fn fetch_segments(video_id: &str) -> Option<Vec<(f64, f64)>> {
    let url = format!(
        "https://sponsor.ajay.app/api/skipSegments?videoID={video_id}&category=sponsor&category=selfpromo&category=interaction"
    );
    let out = Command::new("curl")
        .args(["-sL", "--max-time", "6", &url])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let mut segs = vec![];
    for s in v.as_array()? {
        let a = s.get("segment")?.get(0)?.as_f64()?;
        let b = s.get("segment")?.get(1)?.as_f64()?;
        if b > a {
            segs.push((a, b));
        }
    }
    Some(segs)
}

fn ensure_lua() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let dir = format!("{home}/.config/mpv/scripts");
    std::fs::create_dir_all(&dir).ok()?;
    let path = format!("{dir}/yt-tui-sb.lua");
    let cur = std::fs::read_to_string(&path).unwrap_or_default();
    if cur != SB_LUA {
        std::fs::write(&path, SB_LUA).ok()?;
    }
    Some(path)
}

// ---------------------------------------------------------------------------
// Resume positions (continue watching), stored locally per video id.
// ---------------------------------------------------------------------------

fn resume_path() -> std::path::PathBuf {
    config::data_dir().join("resume.json")
}

pub fn load_resume() -> std::collections::HashMap<String, u64> {
    std::fs::read(resume_path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

pub fn save_resume(map: &std::collections::HashMap<String, u64>) {
    let _ = std::fs::create_dir_all(config::data_dir());
    let _ = std::fs::write(resume_path(), serde_json::to_vec(map).unwrap_or_default());
}

/// Current mpv position in seconds, if anything is playing.
pub fn time_pos() -> Option<f64> {
    let resp = ipc_send(r#"{"command":["get_property","time-pos"]}"#).ok()?;
    parse_ipc_number(&resp)
}

pub fn duration_pos() -> Option<f64> {
    let resp = ipc_send(r#"{"command":["get_property","duration"]}"#).ok()?;
    parse_ipc_number(&resp)
}

fn parse_ipc_number(resp: &str) -> Option<f64> {
    // {"data":12.34,"request_id":0,"error":"success"}
    let v: serde_json::Value = serde_json::from_str(resp).ok()?;
    if v.get("error").and_then(|e| e.as_str()) != Some("success") {
        return None;
    }
    v.get("data")?.as_f64()
}

pub fn quit_mpv() {
    let _ = ipc_send(r#"{"command":["quit"]}"#);
}

/// Play a local file (downloads view) with the same single-instance + IPC setup.
pub fn play_file(path: &str, cfg: &config::Config) -> Result<std::process::Child, String> {
    if !has_mpv() {
        return Err("mpv not found".into());
    }
    let mut cmd = Command::new("mpv");
    cmd.arg(path).arg(format!("--title={}", path));
    cmd.arg(format!("--input-ipc-server={}", ipc_sock()));
    if cfg.mpv_pretty {
        cmd.args(["--no-border", "--autofit-larger=90%x90%", "--msg-level=all=no"]);
    }
    cmd.args(&cfg.player_args);
    cmd.stdout(Stdio::null()).stderr(Stdio::null());
    cmd.spawn().map_err(|e| format!("mpv spawn failed ({e})"))
}

pub fn open_browser(url: &str) -> std::process::Child {
    #[cfg(target_os = "macos")]
    let child = Command::new("open").arg(url).spawn();
    #[cfg(target_os = "linux")]
    let child = Command::new("xdg-open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let child = Command::new("cmd").args(["/C", "start", url]).spawn();
    // never panic the TUI: fall back to an already-exited dummy child
    child.or_else(|_| {
        Command::new("true")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
    })
    .or_else(|_| {
        Command::new("sleep")
            .arg("0")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
    })
    .expect("no process spawner available")
}

#[allow(dead_code)]
pub fn record_played(video_id: &str, title: &str, channel: &str, cfg: &config::Config) {
    config::push_history(cfg, video_id, title, channel);
}
