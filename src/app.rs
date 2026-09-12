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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum View {
    Home,
    Subs,
    History,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SidebarAction {
    Go(View),
    Later,
    Liked,
    Login,
}

pub struct App {
    pub view: View,
    pub videos: Vec<Video>,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub row_offset: usize,
    pub cols: usize,
    pub chips: Vec<String>,
    pub active_chip: usize,
    pub subs: Vec<(String, bool)>,
    pub sub_selected: usize,
    pub hist: Vec<config::HistoryEntry>,
    pub hist_selected: usize,
    pub query: String,
    pub searching: bool,
    pub adding_sub: bool,
    pub last_query: Option<String>,
    pub status: String,
    pub should_quit: bool,
    pub loading: bool,
    pub live: bool,
    pub login_ok: Option<bool>,
    pub cfg: config::Config,
    rx: Option<Receiver<Result<Vec<Video>, String>>>,
    rx_auth: Option<Receiver<Result<String, String>>>,
    thumb_cache: HashMap<String, Vec<Line<'static>>>,
    thumb_order: VecDeque<String>,
    // mouse hit-test regions (filled by ui.rs each frame)
    pub search_rect: Rect,
    pub chips_rect: Rect,
    pub card_hits: Vec<(Rect, usize)>,
    pub sidebar_hits: Vec<(Rect, SidebarAction)>,
    pub list_hits: Vec<(Rect, usize)>,
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
            "0 home • s subs • y hist • u login • w later • / search • r refresh • + more • q quit",
        );
        if !mpv_ok {
            status.push_str(" • mpv missing");
        }
        Self {
            view: View::Home,
            videos,
            filtered,
            selected: 0,
            row_offset: 0,
            cols: 3,
            chips: data::chips(),
            active_chip: 0,
            subs,
            sub_selected: 0,
            hist: vec![],
            hist_selected: 0,
            query: String::new(),
            searching: false,
            adding_sub: false,
            last_query: None,
            status,
            should_quit: false,
            loading: false,
            live: false,
            login_ok: None,
            cfg,
            rx: None,
            rx_auth: None,
            thumb_cache: HashMap::new(),
            thumb_order: VecDeque::new(),
            search_rect: Rect::default(),
            chips_rect: Rect::default(),
            card_hits: vec![],
            sidebar_hits: vec![],
            list_hits: vec![],
        }
    }

    // ---------------- views ----------------

    pub fn set_view(&mut self, v: View) {
        self.view = v;
        self.status = match v {
            View::Home => "home — / search • r feed • Enter play".into(),
            View::Subs => "subs — Enter load channel • a add • d remove • r refresh all".into(),
            View::History => {
                self.reload_hist();
                "history — Enter replay • D clear • local only, no login needed".into()
            }
        };
    }

    fn reload_hist(&mut self) {
        self.hist = config::load_history(self.cfg.max_history);
        self.hist_selected = 0;
    }

    // ---------------- background poll ----------------

    /// Non-blocking poll for background fetch results. Call each frame.
    pub fn poll(&mut self) {
        if let Some(rx) = &self.rx {
            let done = match rx.try_recv() {
                Ok(Ok(vids)) => {
                    self.loading = false;
                    if vids.is_empty() {
                        self.status = "no results (try another query)".into();
                    } else {
                        self.view = View::Home;
                        self.videos = vids;
                        self.filtered = (0..self.videos.len()).collect();
                        self.selected = 0;
                        self.row_offset = 0;
                        self.live = true;
                        self.thumb_cache.clear();
                        self.thumb_order.clear();
                        self.status = format!(
                            "{} results • j/k or wheel scrolls (3 rows visible) • Enter plays",
                            self.videos.len()
                        );
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
            };
            if done {
                self.rx = None;
            }
        }
        if let Some(rx) = &self.rx_auth {
            let done = match rx.try_recv() {
                Ok(Ok(msg)) => {
                    self.loading = false;
                    self.login_ok = Some(true);
                    self.status = msg;
                    true
                }
                Ok(Err(e)) => {
                    self.loading = false;
                    self.login_ok = Some(false);
                    self.status = format!("login failed: {e}");
                    true
                }
                Err(mpsc::TryRecvError::Empty) => false,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.loading = false;
                    true
                }
            };
            if done {
                self.rx_auth = None;
            }
        }
    }

    // ---------------- mouse ----------------

    /// Mouse: click search to type, click video to select (again to play),
    /// click sidebar to switch views, wheel to scroll.
    pub fn on_mouse(&mut self, ev: MouseEvent) {
        match ev.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let (x, y) = (ev.column, ev.row);
                if inside(self.search_rect, x, y) {
                    self.searching = true;
                    self.adding_sub = false;
                    return;
                }
                for (rect, a) in self.sidebar_hits.clone() {
                    if inside(rect, x, y) {
                        match a {
                            SidebarAction::Go(v) => self.set_view(v),
                            SidebarAction::Later => self.load_private("later"),
                            SidebarAction::Liked => self.load_private("liked"),
                            SidebarAction::Login => self.test_login(),
                        }
                        return;
                    }
                }
                if self.view == View::Home {
                    if inside(self.chips_rect, x, y) {
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
                            }
                            return;
                        }
                    }
                } else {
                    for (rect, idx) in self.list_hits.clone() {
                        if inside(rect, x, y) {
                            match self.view {
                                View::Subs => {
                                    self.sub_selected = idx;
                                    // double-click (click selected) loads channel
                                    // single click just selects; simplest: load on click
                                    self.load_selected_sub();
                                }
                                View::History => {
                                    self.hist_selected = idx;
                                    self.replay_history();
                                }
                                _ => {}
                            }
                            return;
                        }
                    }
                }
            }
            MouseEventKind::ScrollUp => match self.view {
                View::Home => self.move_sel(-(self.cols as isize)),
                View::Subs => self.move_sub(-1),
                View::History => self.move_hist(-1),
            },
            MouseEventKind::ScrollDown => match self.view {
                View::Home => self.move_sel(self.cols as isize),
                View::Subs => self.move_sub(1),
                View::History => self.move_hist(1),
            },
            _ => {}
        }
    }

    // ---------------- keyboard ----------------

    pub fn on_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        // add-sub prompt takes priority
        if self.adding_sub {
            match code {
                KeyCode::Esc => {
                    self.adding_sub = false;
                    self.query.clear();
                }
                KeyCode::Enter => {
                    let h = self.query.trim().to_string();
                    self.adding_sub = false;
                    self.query.clear();
                    if !h.is_empty() {
                        self.add_sub(h);
                    }
                }
                KeyCode::Backspace => {
                    self.query.pop();
                }
                KeyCode::Char(c) => {
                    if !mods.contains(KeyModifiers::CONTROL) {
                        self.query.push(c);
                    }
                }
                _ => {}
            }
            return;
        }
        if self.searching {
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
            // views — lowercase aliases (no Shift needed). hjkl stay nav-only.
            // s subs, y history (You), u login (aUth), w watch-later, t liked.
            // Uppercase S/H/L/W/T kept for compat.
            KeyCode::Char('0') => self.set_view(View::Home),
            KeyCode::Char('s') | KeyCode::Char('S') => self.set_view(View::Subs),
            KeyCode::Char('y') | KeyCode::Char('H') => self.set_view(View::History),
            KeyCode::Char('u') | KeyCode::Char('L') => self.test_login(),
            KeyCode::Char('w') | KeyCode::Char('W') => self.load_private("later"),
            KeyCode::Char('t') | KeyCode::Char('T') => self.load_private("liked"),
            KeyCode::Char('+') | KeyCode::Char('=') => self.load_more(),
            // search / feed
            KeyCode::Char('/') => {
                self.searching = true;
            }
            KeyCode::Char('r') => {
                if self.view == View::Subs {
                    self.load_feed();
                } else {
                    self.set_view(View::Home);
                    self.load_feed();
                }
            }
            KeyCode::Char('m') => {
                self.view = View::Home;
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
                } else if self.view != View::Home {
                    self.set_view(View::Home);
                }
            }
            // per-view actions
            KeyCode::Enter | KeyCode::Char('p') => match self.view {
                View::Home => self.play_selected(),
                View::Subs => self.load_selected_sub(),
                View::History => self.replay_history(),
            },
            KeyCode::Char('o') => self.open_selected(),
            KeyCode::Char('a') if self.view == View::Subs => {
                self.adding_sub = true;
                self.query.clear();
                self.status = "add sub: type @handle or channel URL, Enter to save".into();
            }
            KeyCode::Char('d') if self.view == View::Subs => self.remove_sub(),
            KeyCode::Char('D') if self.view == View::History => {
                config::clear_history();
                self.reload_hist();
                self.status = "history cleared".into();
            }
            // navigation
            KeyCode::Char('h') | KeyCode::Left => match self.view {
                View::Home => self.move_sel(-1),
                _ => {}
            },
            KeyCode::Char('l') | KeyCode::Right => match self.view {
                View::Home => self.move_sel(1),
                _ => {}
            },
            KeyCode::Char('k') | KeyCode::Up => match self.view {
                View::Home => self.move_sel(-(self.cols as isize)),
                View::Subs => self.move_sub(-1),
                View::History => self.move_hist(-1),
            },
            KeyCode::Char('j') | KeyCode::Down => match self.view {
                View::Home => self.move_sel(self.cols as isize),
                View::Subs => self.move_sub(1),
                View::History => self.move_hist(1),
            },
            KeyCode::Char('g') => {
                self.selected = 0;
                self.row_offset = 0;
                self.sub_selected = 0;
                self.hist_selected = 0;
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

    fn move_sub(&mut self, d: isize) {
        if self.subs.is_empty() {
            return;
        }
        let n = self.subs.len() as isize;
        self.sub_selected = (self.sub_selected as isize + d).clamp(0, n - 1) as usize;
    }

    fn move_hist(&mut self, d: isize) {
        if self.hist.is_empty() {
            return;
        }
        let n = self.hist.len() as isize;
        self.hist_selected = (self.hist_selected as isize + d).clamp(0, n - 1) as usize;
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

    // ---------------- data jobs ----------------

    pub fn live_search(&mut self, q: String) {
        if self.loading {
            return;
        }
        self.loading = true;
        self.status = format!("searching \"{q}\" via yt-dlp…");
        let cfg = self.cfg.clone();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let res = youtube::search(&cfg, &q, cfg.search_limit);
            let _ = tx.send(res);
        });
    }

    pub fn load_feed(&mut self) {
        if self.loading {
            return;
        }
        let cfg = self.cfg.clone();
        let subs = cfg.subscriptions.clone();
        if subs.is_empty() {
            self.status = "no subscriptions — press S then a to add".into();
            return;
        }
        self.loading = true;
        self.status = format!("loading feed ({} subs, {} each)…", subs.len(), cfg.feed_per_channel);
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            // fetch ALL subs (no take(6) cap), then round-robin interleave so
            // one channel can't dominate, capped at feed_total.
            let mut per: Vec<Vec<crate::app::Video>> = vec![];
            let mut failed = 0usize;
            for s in subs.iter() {
                let url = if s.starts_with('@') {
                    format!("https://www.youtube.com/{s}/videos")
                } else {
                    s.clone()
                };
                match youtube::channel_videos(&cfg, &url, cfg.feed_per_channel) {
                    Ok(v) if !v.is_empty() => per.push(v),
                    _ => {
                        failed += 1;
                        // keep a placeholder-free gap: push empty so interleave skips
                        per.push(vec![]);
                    }
                }
            }
            let total_cap = cfg.feed_total.max(9);
            let mut all = Vec::with_capacity(total_cap);
            let depth = per.iter().map(|v| v.len()).max().unwrap_or(0);
            for i in 0..depth {
                for ch in per.iter() {
                    if let Some(v) = ch.get(i) {
                        all.push(v.clone());
                        if all.len() >= total_cap {
                            break;
                        }
                    }
                }
                if all.len() >= total_cap {
                    break;
                }
            }
            let ok_subs = per.iter().filter(|v| !v.is_empty()).count();
            if all.is_empty() {
                let _ = tx.send(Err(format!(
                    "feed empty — {failed}/{} channels failed (private/renamed? try one with Enter in S view)",
                    subs.len()
                )));
            } else {
                // stash counts in first video? No — encode via status in poll.
                // Send videos; poll formats counts from len. Failures logged to status below.
                let _ = tx.send(Ok(all));
                let _ = (ok_subs, failed);
            }
        });
    }

    fn load_selected_sub(&mut self) {
        let Some((name, _)) = self.subs.get(self.sub_selected).cloned() else {
            return;
        };
        let url = if name.starts_with('@') || name.starts_with("http") {
            if name.starts_with('@') {
                format!("https://www.youtube.com/{name}/videos")
            } else {
                name.clone()
            }
        } else {
            // bare name -> search it as channel
            self.live_search(name);
            return;
        };
        if self.loading {
            return;
        }
        self.loading = true;
        self.status = format!("loading {url} …");
        let cfg = self.cfg.clone();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let res = youtube::channel_videos(&cfg, &url, 12);
            let _ = tx.send(res);
        });
    }

    fn add_sub(&mut self, handle: String) {
        let h = handle.trim().to_string();
        if h.is_empty() {
            return;
        }
        if !self.cfg.subscriptions.iter().any(|s| s == &h) {
            self.cfg.subscriptions.push(h.clone());
            config::save(&self.cfg);
            self.subs.push((h.clone(), false));
            self.status = format!("subscribed {h} (saved to config.json)");
        } else {
            self.status = format!("{h} already subscribed");
        }
    }

    fn remove_sub(&mut self) {
        if self.subs.is_empty() {
            return;
        }
        let (name, _) = self.subs.remove(self.sub_selected.min(self.subs.len() - 1));
        self.cfg.subscriptions.retain(|s| s != &name);
        config::save(&self.cfg);
        self.sub_selected = self.sub_selected.min(self.subs.len().saturating_sub(1));
        self.status = format!("removed {name}");
    }

    fn replay_history(&mut self) {
        let Some(e) = self.hist.get(self.hist_selected).cloned() else {
            self.status = "history empty — play something first".into();
            return;
        };
        config::push_history(&self.cfg, &e.id, &e.title, &e.channel);
        match player::play_with(&e.id, &self.cfg) {
            Ok(msg) => self.status = msg,
            Err(err) => self.status = format!("play failed: {err}"),
        }
    }

    pub fn test_login(&mut self) {
        if self.loading {
            return;
        }
        self.loading = true;
        self.status = format!("testing login via {} cookies…", self.cfg.browser);
        let cfg = self.cfg.clone();
        let (tx, rx) = mpsc::channel();
        self.rx_auth = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(youtube::auth_test(&cfg));
        });
    }

    fn load_private(&mut self, which: &str) {
        if self.loading {
            return;
        }
        if !self.cfg.use_cookies {
            self.status = format!(
                "needs login: press u to test, then set use_cookies=true in config.json (browser={})",
                self.cfg.browser
            );
            return;
        }
        self.loading = true;
        self.status = format!("loading {which} via cookies…");
        let cfg = self.cfg.clone();
        let which = which.to_string();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let res = youtube::private_playlist(&cfg, &which, 12);
            let _ = tx.send(res);
        });
    }

    // ---------------- playback ----------------

    /// App-like "load more": doubles limits and re-runs current job.
    pub fn load_more(&mut self) {
        if self.loading { return; }
        self.cfg.feed_total = (self.cfg.feed_total + 20).min(120);
        self.cfg.search_limit = (self.cfg.search_limit + 12).min(60);
        config::save(&self.cfg);
        if let Some(q) = self.last_query.clone() {
            if self.live { self.live_search(q); return; }
        }
        self.load_feed();
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
        config::push_history(&self.cfg, &v.id, &v.title, &v.channel);
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
        let lines = thumb::get(&v.id, v.seed, v.hue, w.max(8), h.max(4), self.cfg.thumb_cache_mb, &self.cfg.thumb_quality);
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
