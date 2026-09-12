use crossterm::event::{KeyCode, KeyModifiers};
use std::collections::{HashMap, VecDeque};

use ratatui::text::Line;

use crate::{data, player, thumb};

#[derive(Clone)]
pub struct Video {
    pub id: String,
    pub title: String,
    pub channel: String,
    pub verified: bool,
    pub views: String,
    pub age: String,
    pub duration: String,
    /// 0-255 hue seed for procedural ASCII thumb (no image download in MVP)
    pub hue: u8,
    pub seed: u64,
}

pub struct App {
    pub videos: Vec<Video>,
    /// indices into `videos` after search filter
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub row_offset: usize,
    pub chips: Vec<String>,
    pub active_chip: usize,
    pub subs: Vec<(String, bool)>,
    pub query: String,
    pub searching: bool,
    pub status: String,
    pub should_quit: bool,
    /// thumb cache: video.id -> rendered lines. LRU-ish via insertion order.
    /// Capped at 30 entries, each ~8 lines -> <500KB. Disk cache intentionally
    /// omitted in MVP (zero storage); add 20MB capped on-disk ANSI cache later.
    thumb_cache: HashMap<String, Vec<Line<'static>>>,
    thumb_order: VecDeque<String>,
}

impl App {
    pub fn new() -> Self {
        let videos = data::mock_videos();
        let filtered = (0..videos.len()).collect();
        Self {
            videos,
            filtered,
            selected: 0,
            row_offset: 0,
            chips: data::chips(),
            active_chip: 0,
            subs: data::subs(),
            query: String::new(),
            searching: false,
            status: String::from(
                "hjkl/arrows navigate • Enter play (mpv) • / search • 1-9 chips • q quit",
            ),
            should_quit: false,
            thumb_cache: HashMap::new(),
            thumb_order: VecDeque::new(),
        }
    }

    pub fn visible_count(&self, cols: usize) -> usize {
        // show 3 rows like the mockup; scroll for the rest
        cols * 3
    }

    pub fn on_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        if self.searching {
            match code {
                KeyCode::Esc => {
                    self.searching = false;
                }
                KeyCode::Enter => {
                    self.searching = false;
                    self.apply_filter();
                }
                KeyCode::Backspace => {
                    self.query.pop();
                    self.apply_filter();
                }
                KeyCode::Char(c) => {
                    if !mods.contains(KeyModifiers::CONTROL) {
                        self.query.push(c);
                        self.apply_filter();
                    }
                }
                _ => {}
            }
            return;
        }

        match code {
            KeyCode::Char('/') => {
                self.searching = true;
            }
            KeyCode::Esc => {
                if !self.query.is_empty() {
                    self.query.clear();
                    self.apply_filter();
                }
            }
            KeyCode::Enter | KeyCode::Char('p') => self.play_selected(),
            // vim + arrows
            KeyCode::Char('h') | KeyCode::Left => self.move_sel(-1, 3),
            KeyCode::Char('l') | KeyCode::Right => self.move_sel(1, 3),
            KeyCode::Char('k') | KeyCode::Up => self.move_sel(-3, 3),
            KeyCode::Char('j') | KeyCode::Down => self.move_sel(3, 3),
            KeyCode::Char('g') => {
                self.selected = 0;
                self.row_offset = 0;
            }
            KeyCode::Char('G') => {
                if !self.filtered.is_empty() {
                    self.selected = self.filtered.len() - 1;
                }
            }
            KeyCode::Char(c) if ('1'..='9').contains(&c) => {
                let i = (c as usize) - ('1' as usize);
                if i < self.chips.len() {
                    self.active_chip = i;
                    self.status = format!("filter: {}", self.chips[i]);
                }
            }
            KeyCode::Tab => {
                // cycle chips like YouTube topic bar
                self.active_chip = (self.active_chip + 1) % self.chips.len();
            }
            _ => {}
        }
    }

    fn move_sel(&mut self, delta: isize, cols: usize) {
        if self.filtered.is_empty() {
            return;
        }
        let n = self.filtered.len() as isize;
        let mut s = self.selected as isize + delta;
        s = s.clamp(0, n - 1);
        self.selected = s as usize;

        // keep selection inside visible 3-row window
        let row = self.selected / cols;
        let vis_rows = 3;
        if row < self.row_offset {
            self.row_offset = row;
        } else if row >= self.row_offset + vis_rows {
            self.row_offset = row + 1 - vis_rows;
        }
    }

    fn apply_filter(&mut self) {
        let q = self.query.to_lowercase();
        if q.is_empty() {
            self.filtered = (0..self.videos.len()).collect();
        } else {
            self.filtered = self
                .videos
                .iter()
                .enumerate()
                .filter(|(_, v)| {
                    v.title.to_lowercase().contains(&q)
                        || v.channel.to_lowercase().contains(&q)
                })
                .map(|(i, _)| i)
                .collect();
        }
        self.selected = 0;
        self.row_offset = 0;
        self.status = if self.filtered.is_empty() {
            format!("no results for \"{}\"", self.query)
        } else {
            format!("{} results", self.filtered.len())
        };
    }

    fn play_selected(&mut self) {
        let Some(&vi) = self.filtered.get(self.selected) else {
            return;
        };
        let v = &self.videos[vi];
        // MVP: mock ids are not playable; real ids go through yt-dlp + mpv.
        if v.id.starts_with("mock") {
            self.status = format!(
                "▶ {} — mock entry (wire real videoId to play via mpv)",
                v.title.lines().next().unwrap_or(&v.title)
            );
            return;
        }
        self.status = format!("▶ resolving {} …", v.title);
        match player::play(&v.id) {
            Ok(msg) => self.status = msg,
            Err(e) => self.status = format!("play failed: {e}"),
        }
    }

    /// Get (cached) procedural ASCII thumb for a video.
    /// Real implementation later: download `default.jpg` (120x90),
    /// convert to half-block ANSI, cache text. Same call site.
    pub fn thumb(&mut self, video_idx: usize, w: u16, h: u16) -> Vec<Line<'static>> {
        let v = &self.videos[video_idx];
        let key = format!("{}-{}x{}", v.id, w, h);
        if let Some(cached) = self.thumb_cache.get(&key) {
            return cached.clone();
        }
        let lines = thumb::procedural(v.seed, v.hue, w.max(8), h.max(4));
        self.thumb_cache.insert(key.clone(), lines.clone());
        self.thumb_order.push_back(key);
        lines
    }

    pub fn prune_thumb_cache(&mut self) {
        while self.thumb_order.len() > 30 {
            if let Some(old) = self.thumb_order.pop_front() {
                self.thumb_cache.remove(&old);
            } else {
                break;
            }
        }
    }
}
