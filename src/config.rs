use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// YouTube handles / channel URLs for feed (`r` to refresh).
    /// Example: ["@ScrapMan", "@Grian"]
    pub subscriptions: Vec<String>,
    pub max_history: usize,
    /// on-disk thumb jpg cache cap in MB
    pub thumb_cache_mb: u64,
    /// Player: "mpv" (default, lightest) | "iina" (macOS native, prettiest)
    /// | "vlc" | "browser". Extra args appended to player command.
    #[serde(default = "default_player")]
    pub player: String,
    #[serde(default)]
    pub player_args: Vec<String>,
    /// mpv window niceties (borderless + autofit). Set false for stock mpv.
    #[serde(default = "default_true")]
    pub mpv_pretty: bool,
    /// Browser to read YouTube cookies from for login-gated feeds
    /// (Watch Later, Liked, private subs). You stay logged in via your
    /// normal browser — the TUI never sees your password.
    /// One of: chrome, chromium, brave, edge, firefox, zen, safari.
    /// Zen (Firefox fork) auto-resolves to its profile dir.
    #[serde(default = "default_browser")]
    pub browser: String,
    /// If true, yt-dlp calls add `--cookies-from-browser <browser>`.
    /// Turn on after `L` login test succeeds.
    #[serde(default)]
    pub use_cookies: bool,
    /// More reliable than browser reading (which fails when Chrome is open
    /// or macOS keychain blocks it): export via "Get cookies.txt LOCALLY"
    /// extension while on youtube.com, save to e.g. ~/.config/yt-tui/cookies.txt,
    /// set this path here. Takes priority over `browser` when set.
    #[serde(default)]
    pub cookies_file: String,
    /// Thumbnail resolution: "default" (120x90, ~3KB, sharp enough for cells),
    /// "mq" (320x180, ~10KB, recommended sharper), "hq" (480x360, ~30KB),
    /// "sd" (640x480, ~60KB). Higher = sharper on big terminals, fewer thumbs
    /// fit in the 20MB disk cap. Needs restart + cache clear to take effect.
    #[serde(default = "default_thumb_quality")]
    pub thumb_quality: String,
    /// Feed sizes. `r` loads ALL subs (no more take(6) cap):
    /// per-channel videos x subs, interleaved, capped at total.
    #[serde(default = "default_feed_per_channel")]
    pub feed_per_channel: usize,
    #[serde(default = "default_feed_total")]
    pub feed_total: usize,
    #[serde(default = "default_search_limit")]
    pub search_limit: usize,
    /// mpv stream quality (`v` cycles): best, 720p, 480p, audio.
    /// Maps to --ytdl-format. Single-file downloads always use best.
    #[serde(default = "default_quality")]
    pub quality: String,
    /// Where `d`/`D` save. Default ~/Downloads/yt-tui.
    #[serde(default)]
    pub download_dir: String,
    /// Thumbnails: "auto" (images if terminal supports kitty/sixel, else blocks),
    /// "images" (force real images, blocks fallback per-card), "blocks" (ASCII art).
    /// Toggle live in Settings (gear icon / `,`).
    #[serde(default = "default_thumb_mode")]
    pub thumb_mode: String,
    /// Auto-skip sponsors/self-promos via SponsorBlock (mpv lua, zero setup).
    #[serde(default = "default_true")]
    pub sponsorblock: bool,
    /// English subtitles when available (mpv ytdl subs).
    #[serde(default)]
    pub subtitles: bool,
    /// Color scheme (Settings → Style). 11 Omarchy themes + YouTube Dark/Light.
    #[serde(default = "default_theme")]
    pub theme: String,
}

fn default_player() -> String {
    "mpv".into()
}
fn default_browser() -> String {
    "chrome".into()
}
fn default_thumb_quality() -> String {
    "mq".into()
}
fn default_feed_per_channel() -> usize {
    5
}
fn default_feed_total() -> usize {
    40
}
fn default_search_limit() -> usize {
    24
}
fn default_quality() -> String {
    "best".into()
}
fn default_thumb_mode() -> String {
    "auto".into()
}
fn default_theme() -> String {
    "Midnight".into()
}
fn default_true() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            subscriptions: vec![
                "@ScrapMan".into(),
                "@Grian".into(),
                "@kanGaming".into(),
            ],
            max_history: 100,
            thumb_cache_mb: 20,
            player: default_player(),
            player_args: vec![],
            mpv_pretty: true,
            browser: default_browser(),
            use_cookies: false,
            cookies_file: String::new(),
            thumb_quality: default_thumb_quality(),
            feed_per_channel: default_feed_per_channel(),
            feed_total: default_feed_total(),
            search_limit: default_search_limit(),
            quality: default_quality(),
            download_dir: String::new(),
            thumb_mode: default_thumb_mode(),
            sponsorblock: true,
            subtitles: false,
            theme: default_theme(),
        }
    }
}

pub fn base_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg).join("yt-tui")
    } else {
        dirs_home().join(".config").join("yt-tui")
    }
}

pub fn cache_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        PathBuf::from(xdg).join("yt-tui")
    } else {
        dirs_home().join(".cache").join("yt-tui")
    }
}

pub fn data_dir() -> PathBuf {
    dirs_home().join(".local").join("share").join("yt-tui")
}

fn dirs_home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

pub fn load() -> Config {
    let p = base_dir().join("config.json");
    if let Ok(bytes) = fs::read(&p) {
        if let Ok(c) = serde_json::from_slice::<Config>(&bytes) {
            let mut c = c;
            // migrate old configs (first-run file lacks new keys): fill + autosave
            let raw = String::from_utf8_lossy(&bytes);
            let mut dirty = false;
            // auto-pick zen when its profile exists and config still says default chrome
            if c.browser == "chrome" && !raw.contains("\"browser\"") && zen_exists() {
                c.browser = "zen".into();
                dirty = true;
            }
            for key in [
                "\"player\"",
                "\"browser\"",
                "\"thumb_quality\"",
                "\"thumb_mode\"",
                "\"sponsorblock\"",
                "\"theme\"",
                "\"feed_per_channel\"",
                "\"cookies_file\"",
            ] {
                if !raw.contains(key) {
                    dirty = true;
                    break;
                }
            }
            if dirty {
                save(&c);
            }
            return c;
        }
    }
    let mut cfg = Config::default();
    // fresh install on a Zen machine -> default to zen straight away
    if zen_exists() {
        cfg.browser = "zen".into();
    }
    save(&cfg);
    cfg
}

fn zen_exists() -> bool {
    let Ok(home) = std::env::var("HOME") else {
        return false;
    };
    let base =
        std::path::PathBuf::from(home).join("Library/Application Support/zen/Profiles");
    std::fs::read_dir(&base)
        .map(|rd| {
            rd.filter_map(|e| e.ok()).any(|e| {
                e.path().join("cookies.sqlite").exists()
                    || e.file_name().to_string_lossy().contains("Default")
            })
        })
        .unwrap_or(false)
}

pub fn download_dir(cfg: &Config) -> std::path::PathBuf {
    if !cfg.download_dir.is_empty() {
        let s = cfg.download_dir.clone();
        if let Some(rest) = s.strip_prefix("~/") {
            if let Ok(home) = std::env::var("HOME") {
                return std::path::PathBuf::from(format!("{home}/{rest}"));
            }
        }
        return std::path::PathBuf::from(s);
    }
    dirs_home().join("Downloads").join("yt-tui")
}

pub fn save(cfg: &Config) {
    let _ = fs::create_dir_all(base_dir());
    let _ = fs::write(
        base_dir().join("config.json"),
        serde_json::to_string_pretty(cfg).unwrap_or_default(),
    );
}

// --- watch history (capped JSON, tiny) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub title: String,
    pub channel: String,
    pub at: String,
}

pub fn load_history(max: usize) -> Vec<HistoryEntry> {
    let p = data_dir().join("history.json");
    if let Ok(bytes) = fs::read(&p) {
        if let Ok(mut h) = serde_json::from_slice::<Vec<HistoryEntry>>(&bytes) {
            h.truncate(max);
            // back-compat: old entries lack `channel`
            return h;
        }
        // migrate old {id,title,at} shape
        if let Ok(old) = serde_json::from_slice::<Vec<OldHistory>>(&bytes) {
            return old
                .into_iter()
                .map(|o| HistoryEntry {
                    id: o.id,
                    title: o.title,
                    channel: String::new(),
                    at: o.at,
                })
                .take(max)
                .collect();
        }
    }
    vec![]
}

#[derive(Debug, Deserialize)]
struct OldHistory {
    id: String,
    title: String,
    at: String,
}

pub fn push_history(cfg: &Config, id: &str, title: &str, channel: &str) {
    let mut h = load_history(cfg.max_history);
    h.retain(|e| e.id != id);
    h.insert(
        0,
        HistoryEntry {
            id: id.to_string(),
            title: title.to_string(),
            channel: channel.to_string(),
            at: chrono_stamp(),
        },
    );
    h.truncate(cfg.max_history);
    let _ = fs::create_dir_all(data_dir());
    let _ = fs::write(
        data_dir().join("history.json"),
        serde_json::to_string_pretty(&h).unwrap_or_default(),
    );
}

pub fn clear_history() {
    let _ = fs::write(
        data_dir().join("history.json"),
        "[]",
    );
}

fn chrono_stamp() -> String {
    // no chrono dep on purpose (keep binary tiny): seconds since epoch
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_secs().to_string(),
        Err(_) => "0".into(),
    }
}
