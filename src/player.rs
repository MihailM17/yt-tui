use std::process::Command;

/// Minimal external-player hook (keeps RSS tiny: no ffmpeg linked).
///
/// Efficient flow:
///   1. `yt-dlp -g --no-playlist <id>` -> direct stream URL (no browser, no ads)
///   2. spawn `mpv <url>` detached, return immediately
/// SponsorBlock skipping is done mpv-side later (`--script` or chapter skip).
pub fn play(video_id: &str) -> Result<String, String> {
    let url = format!("https://www.youtube.com/watch?v={video_id}");

    let out = Command::new("yt-dlp")
        .args(["-g", "--no-playlist", &url])
        .output()
        .map_err(|e| format!("yt-dlp not found ({e}). install yt-dlp + mpv"))?;

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

    Command::new("mpv")
        .arg(stream)
        .spawn()
        .map_err(|e| format!("mpv not found ({e})"))?;

    Ok(format!("▶ playing {video_id} via mpv"))
}
