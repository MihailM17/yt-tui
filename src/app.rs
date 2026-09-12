use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{self, Receiver};

use ratatui::{layout::Rect, text::Line};

use crate::{config, data, player, thumb, youtube};

#[derive(Clone)]
pub struct Video {
    pub id: String,
    pub title: String,
    pub channel: String,
    pub verified: bool,
    pub views: String,
    pub age: String,
    pub duration: String,
    pub hue: u8,
    pub seed: u64,
}

pub struct App {
    pub videos: Vec<Video>,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub row_offset: usize,
    pub cols: usize,
    pub chips: Vec<String>,
    pub active_chip: usize,
    pub subs: Vec<(String, bool)>,
    pub query: String,
    pub searching: bool,
    pub status: String,
    pub should_quit: bool,
    pub loading: bool,
    pub live: bool,
    pub cfg: config::Config,
    rx: Option<Receiver<Result<Vec<Video>, String>>>,
    thumb_cache: HashMap<String, Vec<Line<'static>>>,
    thumb_order: VecDeque<String>,
    // mouse hit-test regions (filled by ui.rs each frame)
    pub search_rect: Rect,
    pub chips_rect: Rect,
    pub card_hits: Vec<(Rect, usize)>,
}

impl App {
    pub fn new() -> Self {
        let cfg = config::load();
        let videos = data::mock_videos();
        let filtered = (0..videos.len()).collect();
        let subs = if cfg.subscriptions.is_empty() {
            data::subs()
        } else {
            cfg.subscriptions
                .iter()
                .map(|s| (s.clone(), false))
                .collect()
        };
        let mpv_ok = player::has_mpv();
        let mut status = String::from(
            "click search/videos • wheel scroll • hjkl • Enter play • / search • r feed • q quit",
        );
        if !mpv_ok {
            status.push_str(" • mpv missing (Enter opens browser)");
        }
        Self {
            videos,
            filtered,
            selected: 0,
            row_offset: 0,
            cols: 3,
            chips: data::chips(),
            active_chip: 0,
            subs,
            query: String::new(),
            searching: false,
            status,
            should_quit: false,
            loading: false,
            live: false,
            cfg,
            rx: None,
            thumb_cache: HashMap::new(),
            thumb_order: VecDeque::new(),
            search_rect: Rect::default(),
            chips_rect: Rect::default(),
            card_hits: vec![],
        }
    }

    /// Non-blocking poll for background fetch results. Call each frame.
    pub fn poll(&mut self) {
        let done = if let Some(rx) = &self.rx {
            match rx.try_recv() {
                Ok(Ok(vids)) => {
                    self.loading = false;
                    if vids.is_empty() {
                        self.status = "no results (try another query)".into();
                    } else {
                        self.videos = vids;
                        self.filtered = (0..self.videos.len()).collect();
                        self.selected = 0;
                        self.row_offset = 0;
                        self.live = true;
                        self.thumb_cache.clear();
                        self.thumb_order.clear();
                        self.status = format!("{} live results • Enter plays via mpv", self.videos.len());
                    }
                    true
                }
                Ok(Err(e)) => {
                    self.loading = false;
                    self.status = format!("fetch failed: {e}");
                    true
                }
                Err(mpsc::TryRecvError::Empty) => false,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.loading = false;
                    true
                }
            }
        } else {
            false
        };
        if done {
            self.rx = None;
        }
    }

    /// Mouse: click search to type, click video to select (again to play),
    /// click chips row to cycle filter, wheel to scroll.
    pub fn on_mouse(&mut self, ev: MouseEvent) {
        match ev.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let (x, y) = (ev.column, ev.row);
                if inside(self.search_rect, x, y) {
                    self.searching = true;
                    return;
                }
                if inside(self.chips_rect, x, y) {
                    // proportional chip pick (chips laid left-to-right)
                    let w = self.chips_rect.width.max(1) as usize;
                    let rel = x.saturating_sub(self.chips_rect.x) as usize;
                    let i = rel * self.chips.len() / w;
                    self.active_chip = i.min(self.chips.len() - 1);
                    self.status = format!("filter: {}", self.chips[self.active_chip]);
                    return;
                }
                for (rect, idx) in self.card_hits.clone() {
                    if inside(rect, x, y) {
                        if self.selected == idx {
                            self.play_selected();
                        } else {
                            self.selected = idx;
                            let row = self.selected / self.cols.max(1);
                            if row < self.row_offset {
                                self.row_offset = row;
                            }
                        }
                        return;
                    }
                }
            }
            MouseEventKind::ScrollUp => self.move_sel(-(self.cols as isize)),
            MouseEventKind::ScrollDown => self.move_sel(self.cols as isize),
            _ => {}
        }
    }

    pub fn on_key(&mut self, code: KeyCode, mods: KeyModifiers) {        if self.searching {
            match code {
                KeyCode::Esc => {
                    self.searching = false;
                }
                KeyCode::Enter => {
                    let q = self.query.clone();
                    self.searching = false;
                    if q.len() >= 2 {
                        self.live_search(q);
                    } else {
                        self.apply_filter();
                    }
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
            KeyCode::Char('r') => self.load_feed(),
            KeyCode::Char('m') => {
                // back to offline mock (zero network, zero storage)
                self.videos = data::mock_videos();
                self.filtered = (0..self.videos.len()).collect();
                self.selected = 0;
                self.row_offset = 0;
                self.live = false;
                self.query.clear();
                self.thumb_cache.clear();
                self.thumb_order.clear();
                self.status = "offline mock feed (press / to live-search)".into();
            }
            KeyCode::Esc => {
                if !self.query.is_empty() {
                    self.query.clear();
                    self.apply_filter();
                }
            }
            KeyCode::Enter | KeyCode::Char('p') => self.play_selected(),
            KeyCode::Char('o') => self.open_selected(),
            KeyCode::Char('h') | KeyCode::Left => self.move_sel(-1),
            KeyCode::Char('l') | KeyCode::Right => self.move_sel(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_sel(-(self.cols as isize)),
            KeyCode::Char('j') | KeyCode::Down => self.move_sel(self.cols as isize),
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
                self.active_chip = (self.active_chip + 1) % self.chips.len();
            }
            _ => {}
        }
    }

    fn move_sel(&mut self, delta: isize) {
        if self.filtered.is_empty() {
            return;
        }
        let cols = self.cols.max(1);
        let n = self.filtered.len() as isize;
        self.selected = (self.selected as isize + delta).clamp(0, n - 1) as usize;
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
        if !self.live {
            self.status = format!("{} results (local filter — Enter for live search)", self.filtered.len());
        }
    }

    pub fn live_search(&mut self, q: String) {
        if self.loading {
            return;
        }
        self.loading = true;
        self.status = format!("searching \"{q}\" via yt-dlp…");
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let res = youtube::search(&q, 12);
            let _ = tx.send(res);
        });
    }

    pub fn load_feed(&mut self) {
        if self.loading {
            return;
        }
        let subs = self.cfg.subscriptions.clone();
        if subs.is_empty() {
            self.status = "no subscriptions in ~/.config/yt-tui/config.json".into();
            return;
        }
        self.loading = true;
        self.status = format!("loading feed ({} subs) via yt-dlp…", subs.len());
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let mut all = vec![];
            for s in subs.iter().take(6) {
                let url = if s.starts_with('@') {
                    format!("https://www.youtube.com/{s}/videos")
                } else {
                    s.clone()
                };
                if let Ok(mut v) = youtube::channel_videos(&url, 4) {
                    all.append(&mut v);
                }
                if all.len() >= 18 {
                    break;
                }
            }
            // interleave so one channel doesn't dominate
            let _ = tx.send(Ok(all));
        });
    }

    fn play_selected(&mut self) {
        let Some(&vi) = self.filtered.get(self.selected) else {
            return;
        };
        let v = &self.videos[vi];
        if v.id.starts_with("mock") {
            self.status = format!(
                "▶ {} — mock entry (press / + Enter for playable results)",
                v.title.chars().take(60).collect::<String>()
            );
            return;
        }
        config::push_history(&self.cfg, &v.id, &v.title);
        self.status = format!("▶ resolving {} …", short(&v.title));
        match player::play_with(&v.id, &self.cfg) {
            Ok(msg) => self.status = msg,
            Err(e) => self.status = format!("play failed: {e}"),
        }
    }

    fn open_selected(&mut self) {
        let Some(&vi) = self.filtered.get(self.selected) else {
            return;
        };
        let v = &self.videos[vi];
        if v.id.starts_with("mock") {
            return;
        }
        let url = format!("https://www.youtube.com/watch?v={}", v.id);
        player::open_browser(&url);
        self.status = format!("opened {url}");
    }

    pub fn thumb(&mut self, video_idx: usize, w: u16, h: u16) -> Vec<Line<'static>> {
        let v = &self.videos[video_idx];
        let key = format!("{}-{}x{}", v.id, w, h);
        if let Some(cached) = self.thumb_cache.get(&key) {
            return cached.clone();
        }
        let lines = thumb::get(&v.id, v.seed, v.hue, w.max(8), h.max(4), self.cfg.thumb_cache_mb);
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

fn short(s: &str) -> String {
    s.chars().take(50).collect()
}

fn inside(r: ratatui::layout::Rect, x: u16, y: u16) -> bool {
    x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
}
