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
}

fn default_player() -> String {
    "mpv".into()
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
            return c;
        }
    }
    let cfg = Config::default();
    let _ = fs::create_dir_all(base_dir());
    let _ = fs::write(&p, serde_json::to_string_pretty(&cfg).unwrap());
    cfg
}

// --- watch history (capped JSON, tiny) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub title: String,
    pub at: String,
}

pub fn load_history(max: usize) -> Vec<HistoryEntry> {
    let p = data_dir().join("history.json");
    if let Ok(bytes) = fs::read(&p) {
        if let Ok(mut h) = serde_json::from_slice::<Vec<HistoryEntry>>(&bytes) {
            h.truncate(max);
            return h;
        }
    }
    vec![]
}

pub fn push_history(cfg: &Config, id: &str, title: &str) {
    let mut h = load_history(cfg.max_history);
    h.retain(|e| e.id != id);
    h.insert(
        0,
        HistoryEntry {
            id: id.to_string(),
            title: title.to_string(),
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

fn chrono_stamp() -> String {
    // no chrono dep on purpose (keep binary tiny): seconds since epoch
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_secs().to_string(),
        Err(_) => "0".into(),
    }
}
