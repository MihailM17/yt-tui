use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{self, Receiver};

use ratatui::{layout::Rect, text::Line};

use crate::{config, data, engage, player, thumb, youtube};
use crate::youtube::{SortMode, VideoInfo, Comment};
use ratatui_image::{picker::{Picker, ProtocolType}, protocol::StatefulProtocol};

#[derive(Clone, serde::Serialize, serde::Deserialize)]
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
    #[serde(default)]
    pub channel_id: String,
    #[serde(default)]
    pub channel_url: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum View {
    Home,
    Subs,
    History,
}

#[derive(Clone, PartialEq)]
pub enum Overlay {
    Info { vid: String, info: VideoInfo },
    Comments { vid: String, items: Vec<Comment> },
    Queue,
    Actions { idx: usize },
    Settings,
    Help,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TransportAction {
    Pause,
    Next,
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
    pub overlay: Option<Overlay>,
    pub overlay_scroll: usize,
    pub queue: Vec<Video>,
    pub sort: SortMode,
    rx_info: Option<std::sync::mpsc::Receiver<Result<(String, VideoInfo), String>>>,
    rx_comments: Option<std::sync::mpsc::Receiver<Result<(String, Vec<Comment>), String>>>,
    rx_dl: Option<std::sync::mpsc::Receiver<Result<String, String>>>,
    rx_new: Option<std::sync::mpsc::Receiver<Vec<String>>>,
    rx_act: Option<std::sync::mpsc::Receiver<Result<String, String>>>,
    rx_import: Option<std::sync::mpsc::Receiver<Result<Vec<String>, String>>>,
    mpv: Option<std::process::Child>,
    last_play: Option<std::time::Instant>,
    last_play_id: String,
    pub picker: Option<Picker>,
    img_protos: std::collections::HashMap<String, StatefulProtocol>,
    pub settings_sel: usize,
    pub settings_hits: Vec<(Rect, usize)>,
    pub close_rect: Rect,
    pub gear_rect: Rect,
    pub transport_hits: Vec<(Rect, TransportAction)>,
    pub hover: Option<(u16, u16)>,
}

impl App {
    pub fn new(picker: Option<Picker>) -> Self {
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
            "0/s/y views • / search • i info • c comments • a queue • d dl • ? help • q quit",
        );
        if !mpv_ok {
            status.push_str(" • mpv missing");
        }
        let mut app = Self {
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
            overlay: None,
            overlay_scroll: 0,
            queue: vec![],
            sort: SortMode::Relevance,
            rx_info: None,
            rx_comments: None,
            rx_dl: None,
            rx_new: None,
            rx_import: None,
            rx_act: None,
            mpv: None,
            last_play: None,
            last_play_id: String::new(),
            picker,
            img_protos: std::collections::HashMap::new(),
            settings_sel: 0,
            settings_hits: vec![],
            close_rect: Rect::default(),
            gear_rect: Rect::default(),
            transport_hits: vec![],
            hover: None,
        };
        // instant startup: show last feed from disk (<15min old), bg refresh anyway
        app.load_cached_feed();
        app
    }

    /// Real images only when mode allows AND terminal speaks kitty/sixel/iterm.
    pub fn want_images(&self) -> bool {
        match self.cfg.thumb_mode.as_str() {
            "blocks" => false,
            "images" => self.picker.is_some(),
            _ => matches!(
                self.picker.as_ref().map(|p| p.protocol_type()),
                Some(ProtocolType::Kitty) | Some(ProtocolType::Sixel) | Some(ProtocolType::Iterm2)
            ),
        }
    }

    pub fn gfx_label(&self) -> String {
        match self.picker.as_ref().map(|p| p.protocol_type()) {
            Some(ProtocolType::Kitty) => "kitty graphics".into(),
            Some(ProtocolType::Sixel) => "sixel graphics".into(),
            Some(ProtocolType::Iterm2) => "iterm graphics".into(),
            _ => "no image protocol (blocks mode)".into(),
        }
    }

    /// Cached stateful image protocol for a card (None → caller uses blocks).
    pub fn img_proto(&mut self, video_id: &str, w: u16, h: u16) -> Option<&mut StatefulProtocol> {
        if !self.want_images() || video_id.starts_with("mock") { return None; }
        let key = format!("img-{video_id}-{}x{}", w.max(8), h.max(4));
        if self.img_protos.contains_key(&key) {
            return self.img_protos.get_mut(&key);
        }
        let dyn_img = thumb::load_dynamic(video_id, self.cfg.thumb_cache_mb, &self.cfg.thumb_quality)?;
        let picker = self.picker.as_ref()?;
        let proto = picker.new_resize_protocol(dyn_img);
        self.img_protos.insert(key.clone(), proto);
        while self.img_protos.len() > 24 {
            // drop an arbitrary old entry (HashMap has no order; size cap is what matters)
            if let Some(k) = self.img_protos.keys().next().cloned() {
                self.img_protos.remove(&k);
            } else { break; }
        }
        self.img_protos.get_mut(&key)
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
                        // warm jpg cache in background so thumbs pop in without render jank
                        let ids: Vec<String> =
                            self.videos.iter().map(|v| v.id.clone()).collect();
                        let (cmb, q) =
                            (self.cfg.thumb_cache_mb, self.cfg.thumb_quality.clone());
                        std::thread::spawn(move || thumb::warm_all(&ids, cmb, &q));
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
        if let Some(rx) = &self.rx_info {
            match rx.try_recv() {
                Ok(Ok((vid, info))) => {
                    self.loading = false;
                    self.overlay = Some(Overlay::Info { vid, info });
                    self.overlay_scroll = 0;
                    self.rx_info = None;
                }
                Ok(Err(e)) => {
                    self.loading = false;
                    self.status = format!("info failed: {e}");
                    self.rx_info = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.loading = false;
                    self.rx_info = None;
                }
            }
        }
        if let Some(rx) = &self.rx_comments {
            match rx.try_recv() {
                Ok(Ok((vid, items))) => {
                    self.loading = false;
                    self.overlay = Some(Overlay::Comments { vid, items });
                    self.overlay_scroll = 0;
                    self.rx_comments = None;
                }
                Ok(Err(e)) => {
                    self.loading = false;
                    self.status = format!("comments: {e}");
                    self.rx_comments = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.loading = false;
                    self.rx_comments = None;
                }
            }
        }
        if let Some(rx) = &self.rx_dl {
            match rx.try_recv() {
                Ok(Ok(msg)) => {
                    self.status = format!("download done — {msg}");
                    self.rx_dl = None;
                }
                Ok(Err(e)) => {
                    self.status = e;
                    self.rx_dl = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.rx_dl = None;
                }
            }
        }
        if let Some(rx) = &self.rx_import {
            match rx.try_recv() {
                Ok(Ok(channels)) => {
                    self.loading = false;
                    let mut added = 0;
                    for ch in channels {
                        if !self.cfg.subscriptions.iter().any(|s| s.to_lowercase() == ch.to_lowercase()) {
                            self.cfg.subscriptions.push(ch.clone());
                            added += 1;
                        }
                    }
                    config::save(&self.cfg);
                    self.subs = self.cfg.subscriptions.iter().map(|s| (s.clone(), false)).collect();
                    self.status = if added == 0 {
                        format!("account subs already in config ({} total)", self.cfg.subscriptions.len())
                    } else {
                        format!("imported {added} channels ({} total) — r refreshes feed", self.cfg.subscriptions.len())
                    };
                    self.rx_import = None;
                }
                Ok(Err(e)) => { self.loading = false; self.status = format!("import failed: {e}"); self.rx_import = None; }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(_) => { self.loading = false; self.rx_import = None; }
            }
        }
        if let Some(rx) = &self.rx_act {
            match rx.try_recv() {
                Ok(Ok(msg)) => { self.status = msg; self.rx_act = None; }
                Ok(Err(e)) => { self.status = e; self.rx_act = None; }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(_) => { self.rx_act = None; }
            }
        }
        if let Some(rx) = &self.rx_new {
            match rx.try_recv() {
                Ok(fresh) => {
                    self.loading = false;
                    for (name, dot) in self.subs.iter_mut() {
                        *dot = fresh.contains(name);
                    }
                    self.status = if fresh.is_empty() {
                        "no new uploads".into()
                    } else {
                        format!(
                            "{} new: {} (S to browse)",
                            fresh.len(),
                            fresh.iter().take(4).cloned().collect::<Vec<_>>().join(", ")
                        )
                    };
                    self.rx_new = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(_) => {
                    self.loading = false;
                    self.rx_new = None;
                }
            }
        }
    }

    // ---------------- mouse ----------------

    /// Mouse: click search to type, click video to select (again to play),
    /// click sidebar to switch views, wheel to scroll.
    pub fn on_mouse(&mut self, ev: MouseEvent) {
        // right-click a card = action menu (like, subscribe, save…)
        if matches!(ev.kind, MouseEventKind::Down(MouseButton::Right)) && self.view == View::Home {
            for (rect, idx) in self.card_hits.clone() {
                if inside(rect, ev.column, ev.row) {
                    self.selected = idx;
                    self.open_actions();
                    return;
                }
            }
        }
        match ev.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let (x, y) = (ev.column, ev.row);
                if inside(self.search_rect, x, y) {
                    self.searching = true;
                    self.adding_sub = false;
                    return;
                }
                if inside(self.close_rect, x, y) && self.overlay.is_some() {
                    self.overlay = None;
                    self.overlay_scroll = 0;
                    return;
                }
                if inside(self.gear_rect, x, y) {
                    self.overlay = Some(Overlay::Settings);
                    self.overlay_scroll = 0;
                    self.settings_sel = 0;
                    return;
                }
                for (rect, act) in self.transport_hits.clone() {
                    if inside(rect, x, y) {
                        match act {
                            TransportAction::Pause => self.toggle_pause(),
                            TransportAction::Next => self.next_track(),
                        }
                        return;
                    }
                }
                if self.overlay == Some(Overlay::Settings) {
                    for (rect, idx) in self.settings_hits.clone() {
                        if inside(rect, x, y) {
                            self.settings_sel = idx;
                            self.settings_cycle(idx);
                            return;
                        }
                    }
                    return;
                }
                if matches!(self.overlay, Some(Overlay::Actions { .. })) {
                    for (rect, idx) in self.settings_hits.clone() {
                        if inside(rect, x, y) {
                            self.settings_sel = idx;
                            self.execute_action(idx);
                            return;
                        }
                    }
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
            MouseEventKind::Moved => { self.hover = Some((ev.column, ev.row)); }
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
        // overlays eat keys first
        if self.overlay.is_some() {
            let is_settings = matches!(self.overlay, Some(Overlay::Settings));
            let is_actions = matches!(self.overlay, Some(Overlay::Actions { .. }));
            match code {
                KeyCode::Esc | KeyCode::Char('q') => { self.overlay = None; self.overlay_scroll = 0; return; }
                KeyCode::Char('j') | KeyCode::Down => {
                    if is_settings {
                        let n = self.settings_rows().len();
                        self.settings_sel = (self.settings_sel + 1).min(n.saturating_sub(1));
                    } else if is_actions {
                        let n = self.action_rows().len();
                        self.settings_sel = (self.settings_sel + 1).min(n.saturating_sub(1));
                    } else { self.overlay_scroll += 1; }
                    return;
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if is_settings || is_actions {
                        self.settings_sel = self.settings_sel.saturating_sub(1);
                    } else { self.overlay_scroll = self.overlay_scroll.saturating_sub(1); }
                    return;
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    if matches!(self.overlay, Some(Overlay::Queue)) {
                        self.overlay = None;
                        self.play_queue();
                    } else if is_settings {
                        let i = self.settings_sel;
                        self.settings_cycle(i);
                    } else if is_actions {
                        let i = self.settings_sel;
                        self.execute_action(i);
                    }
                    return;
                }
                _ => return,
            }
        }
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
            KeyCode::Char('i') if self.view == View::Home => self.open_info(),
            KeyCode::Char('c') if self.view == View::Home => self.open_comments(),
            KeyCode::Char('x') if self.view == View::Home => self.open_actions(),
            KeyCode::Char('a') if self.view == View::Home => self.queue_add(),
            KeyCode::Char('Q') => { self.overlay = Some(Overlay::Queue); self.overlay_scroll = 0; }
            KeyCode::Char('P') if self.view == View::Home => self.play_queue(),
            KeyCode::Char('d') if self.view == View::Home => self.start_download(false),
            KeyCode::Char('D') if self.view == View::Home => self.start_download(true),
            KeyCode::Char('f') if self.view == View::Home => self.cycle_sort(),
            KeyCode::Char('n') => self.check_new(),
            KeyCode::Char('v') => {
                let q = player::cycle_quality(&mut self.cfg);
                self.status = format!("quality for next play: {q}");
            }
            KeyCode::Char('[') => self.nudge_speed(-1.0),
            KeyCode::Char(']') => self.nudge_speed(1.0),
            KeyCode::Char(',') => { self.overlay = Some(Overlay::Settings); self.overlay_scroll = 0; self.settings_sel = 0; }
            KeyCode::Char(' ') => self.toggle_pause(),
            KeyCode::Char('>') => self.next_track(),
            KeyCode::Char('?') => { self.overlay = Some(Overlay::Help); self.overlay_scroll = 0; }
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
        let sort = self.sort;
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let res = youtube::search_sorted(&cfg, &q, cfg.search_limit, sort);
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
        self.status = format!("loading feed ({} subs in parallel)…", subs.len());
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            // parallel: one yt-dlp per channel (~2-4s each → wall time of slowest)
            let mut handles = vec![];
            for s in subs.iter() {
                let url = if s.starts_with('@') {
                    format!("https://www.youtube.com/{s}/videos")
                } else {
                    s.clone()
                };
                let cfg = cfg.clone();
                let per = cfg.feed_per_channel;
                handles.push(std::thread::spawn(move || {
                    youtube::channel_videos(&cfg, &url, per).unwrap_or_default()
                }));
            }
            let mut per: Vec<Vec<crate::app::Video>> = vec![];
            for h in handles {
                per.push(h.join().unwrap_or_default());
            }
            let failed = per.iter().filter(|v| v.is_empty()).count();
            // round-robin interleave so one channel can't dominate
            let total_cap = cfg.feed_total.max(9);
            let mut all = Vec::with_capacity(total_cap);
            let depth = per.iter().map(|v| v.len()).max().unwrap_or(0);
            'outer: for i in 0..depth {
                for ch in per.iter() {
                    if let Some(v) = ch.get(i) {
                        all.push(v.clone());
                        if all.len() >= total_cap {
                            break 'outer;
                        }
                    }
                }
            }
            if all.is_empty() {
                let _ = tx.send(Err(format!(
                    "feed empty — {failed}/{} channels failed (private/renamed? try one with Enter in S view)",
                    subs.len()
                )));
            } else {
                save_feed_cache(&all);
                let _ = tx.send(Ok(all));
            }
        });
    }

    /// Instant startup: show last feed from disk if <15min old.
    fn load_cached_feed(&mut self) {
        let p = config::cache_dir().join("feed.json");
        let Ok(bytes) = std::fs::read(&p) else {
            return;
        };
        let Ok(cached): Result<CachedFeed, _> = serde_json::from_slice(&bytes) else {
            return;
        };
        let age = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(u64::MAX)
            .saturating_sub(cached.saved_at);
        if age > 15 * 60 || cached.videos.is_empty() {
            return;
        }
        self.videos = cached.videos;
        self.filtered = (0..self.videos.len()).collect();
        self.live = true;
        self.status = format!("{} cached videos • refreshing…", self.videos.len());
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
        if !self.cfg.subscriptions.iter().any(|s| s.to_lowercase() == h.to_lowercase()) {
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
        let (id, title) = (e.id.clone(), e.title.clone());
        self.launch(&id, &title, None);
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


    // ---------------- easy-features ----------------

    fn current_video(&self) -> Option<Video> {
        if self.view != View::Home { return None; }
        self.filtered.get(self.selected).and_then(|vi| self.videos.get(*vi)).cloned()
    }

    fn open_info(&mut self) {
        let Some(v) = self.current_video() else { return; };
        if v.id.starts_with("mock") { self.status = "mock entry — no info (live-search first)".into(); return; }
        if self.loading { return; }
        self.loading = true;
        self.status = format!("info for {} …", short(&v.title));
        let (tx, rx) = mpsc::channel();
        self.rx_info = Some(rx);
        std::thread::spawn(move || {
            let id = v.id.clone();
            let res = youtube::video_info(&id).map(|info| (id, info));
            let _ = tx.send(res);
        });
    }

    fn open_comments(&mut self) {
        let Some(v) = self.current_video() else { return; };
        if v.id.starts_with("mock") { self.status = "mock entry — no comments (live-search first)".into(); return; }
        if self.loading { return; }
        self.loading = true;
        self.status = format!("comments for {} …", short(&v.title));
        let (tx, rx) = mpsc::channel();
        self.rx_comments = Some(rx);
        std::thread::spawn(move || {
            let id = v.id.clone();
            let res = youtube::video_comments(&id, 30).map(|items| (id, items));
            let _ = tx.send(res);
        });
    }

    fn queue_add(&mut self) {
        let Some(v) = self.current_video() else { return; };
        if v.id.starts_with("mock") { self.status = "mock entry — cannot queue".into(); return; }
        if self.queue.iter().any(|q| q.id == v.id) { self.status = "already in queue (Q to view)".into(); return; }
        self.queue.push(v);
        self.status = format!("queued ({} total) — Q view • P play", self.queue.len());
    }

    fn play_queue(&mut self) {
        let ids: Vec<String> = self.queue.iter().map(|v| v.id.clone()).collect();
        if ids.is_empty() {
            self.status = "queue empty — press a on videos to add".into();
            return;
        }
        let first = ids[0].clone();
        self.launch(&first, "queue", Some(ids));
    }

    fn start_download(&mut self, audio_only: bool) {
        let Some(v) = self.current_video() else { return; };
        if v.id.starts_with("mock") { self.status = "mock entry — cannot download".into(); return; }
        if self.rx_dl.is_some() { self.status = "download already running…".into(); return; }
        self.status = format!("downloading {} to {} …", if audio_only { "audio" } else { "video" }, config::download_dir(&self.cfg).to_string_lossy());
        self.rx_dl = Some(player::download(&v.id, audio_only, &self.cfg));
    }

    fn cycle_sort(&mut self) {
        self.sort = self.sort.next();
        self.status = format!("sort: {} — re-running search…", self.sort.label());
        if let Some(q) = self.last_query.clone() {
            if self.loading { return; }
            self.loading = true;
            let cfg = self.cfg.clone();
            let sort = self.sort;
            let (tx, rx) = mpsc::channel();
            self.rx = Some(rx);
            std::thread::spawn(move || {
                let res = youtube::search_sorted(&cfg, &q, cfg.search_limit, sort);
                let _ = tx.send(res);
            });
        } else {
            self.status = format!("sort: {} (applies to next search)", self.sort.label());
        }
    }

    fn check_new(&mut self) {
        if self.loading { return; }
        let cfg = self.cfg.clone();
        let subs = cfg.subscriptions.clone();
        if subs.is_empty() { self.status = "no subs to check".into(); return; }
        self.loading = true;
        self.status = format!("checking {} subs for new uploads…", subs.len());
        let (tx, rx) = mpsc::channel();
        self.rx_new = Some(rx);
        std::thread::spawn(move || {
            let cache = config::cache_dir().join("last_seen.json");
            let mut seen: std::collections::HashMap<String, String> =
                std::fs::read(&cache).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
            let mut fresh: Vec<String> = vec![];
            let mut latest_ids: std::collections::HashMap<String, String> = std::collections::HashMap::new();
            for s in subs.iter() {
                let url = if s.starts_with('@') { format!("https://www.youtube.com/{s}/videos") } else { s.clone() };
                if let Ok(v) = youtube::channel_videos(&cfg, &url, 1) {
                    if let Some(latest) = v.first() {
                        latest_ids.insert(s.clone(), latest.id.clone());
                        if seen.get(s).map(|id| id != &latest.id).unwrap_or(true) {
                            fresh.push(s.clone());
                        }
                    }
                }
            }
            for (s, id) in latest_ids.iter() { seen.insert(s.clone(), id.clone()); }
            let _ = std::fs::create_dir_all(config::cache_dir());
            let _ = std::fs::write(&cache, serde_json::to_string_pretty(&seen).unwrap_or_default());
            let _ = tx.send(fresh);
        });
    }

    fn nudge_speed(&mut self, delta: f64) {
        match player::ipc_speed(delta) {
            Ok(msg) => self.status = msg,
            Err(_) => self.status = "mpv not playing (speed needs active mpv)".into(),
        }
    }

    /// Single player window: kills the previous mpv (if ours is still alive),
    /// debounces double-clicks, and reports state immediately so you never
    /// wonder "did it open?".
    fn launch(&mut self, id: &str, title: &str, queue: Option<Vec<String>>) {
        // debounce: same video twice within 2s = one launch
        if self.last_play_id == id {
            if let Some(t) = self.last_play {
                if t.elapsed() < std::time::Duration::from_secs(2) {
                    self.status = "already opening… (one player window)".into();
                    return;
                }
            }
        }
        // single instance: close our previous player first
        if let Some(mut child) = self.mpv.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.status = format!("▶ opening {} …", short(title));
        let res = match queue {
            Some(ids) => player::play_queue(&ids, &self.cfg),
            None => player::play_with(id, &self.cfg),
        };
        match res {
            Ok((child, msg)) => {
                self.mpv = Some(child);
                self.last_play = Some(std::time::Instant::now());
                self.last_play_id = id.to_string();
                self.status = msg;
            }
            Err(e) => self.status = format!("play failed: {e}"),
        }
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
        let (id, title) = (v.id.clone(), v.title.clone());
        self.launch(&id, &title, None);
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

    // ---------------- settings ----------------

    /// (label, value) rows for the Settings menu. Action rows have value "→".
    pub fn settings_rows(&self) -> Vec<(String, String)> {
        let onoff = |b: bool| if b { "on".to_string() } else { "off".to_string() };
        vec![
            ("Player".into(), self.cfg.player.clone()),
            ("Stream quality".into(), self.cfg.quality.clone()),
            ("Thumbnails".into(), format!("{} ({})", self.cfg.thumb_mode, self.gfx_label())),
            ("Thumb detail".into(), self.cfg.thumb_quality.clone()),
            ("Cookie browser".into(), self.cfg.browser.clone()),
            ("Use cookies".into(), onoff(self.cfg.use_cookies)),
            ("Cookies file".into(), if self.cfg.cookies_file.is_empty() { "not set".into() } else { self.cfg.cookies_file.clone() }),
            ("Feed per channel".into(), self.cfg.feed_per_channel.to_string()),
            ("Feed total".into(), self.cfg.feed_total.to_string()),
            ("Search results".into(), self.cfg.search_limit.to_string()),
            ("Downloads".into(), config::download_dir(&self.cfg).to_string_lossy().to_string()),
            ("Test login".into(), "→".into()),
            ("Import subs from account".into(), "→".into()),
            ("Clear thumb cache".into(), "→".into()),
        ]
    }

    pub fn settings_cycle(&mut self, idx: usize) {
        let n = self.settings_rows().len();
        self.settings_sel = idx.min(n.saturating_sub(1));
        match idx {
            0 => {
                self.cfg.player = match self.cfg.player.as_str() {
                    "mpv" => "iina".into(), "iina" => "vlc".into(),
                    "vlc" => "browser".into(), _ => "mpv".into(),
                };
            }
            1 => { let _ = player::cycle_quality(&mut self.cfg); }
            2 => {
                self.cfg.thumb_mode = match self.cfg.thumb_mode.as_str() {
                    "auto" => "images".into(), "images" => "blocks".into(), _ => "auto".into(),
                };
            }
            3 => {
                self.cfg.thumb_quality = match self.cfg.thumb_quality.as_str() {
                    "default" => "mq".into(), "mq" => "hq".into(),
                    "hq" => "sd".into(), _ => "default".into(),
                };
                self.img_protos.clear();
            }
            4 => {
                self.cfg.browser = match self.cfg.browser.as_str() {
                    "chrome" => "firefox".into(), "firefox" => "zen".into(),
                    "zen" => "brave".into(), "brave" => "edge".into(), _ => "chrome".into(),
                };
            }
            5 => { self.cfg.use_cookies = !self.cfg.use_cookies; }
            6 => {
                // cycle cookies file: unset → default path → unset
                self.cfg.cookies_file = if self.cfg.cookies_file.is_empty() {
                    "~/.config/yt-tui/cookies.txt".into()
                } else { String::new() };
            }
            7 => { self.cfg.feed_per_channel = match self.cfg.feed_per_channel { 3 => 5, 5 => 8, _ => 3 }; }
            8 => { self.cfg.feed_total = match self.cfg.feed_total { 20 => 40, 40 => 80, _ => 20 }; }
            9 => { self.cfg.search_limit = match self.cfg.search_limit { 12 => 24, 24 => 36, _ => 12 }; }
            11 => { self.test_login(); config::save(&self.cfg); return; }
            12 => { self.import_subs(); config::save(&self.cfg); return; }
            13 => {
                let _ = std::fs::remove_dir_all(thumb::thumb_dir());
                self.img_protos.clear();
                self.thumb_cache.clear();
                self.thumb_order.clear();
                self.status = "thumb cache cleared".into();
            }
            _ => {}
        }
        config::save(&self.cfg);
        self.status = format!("saved config.json");
    }

    pub fn import_subs(&mut self) {
        if self.loading { return; }
        self.loading = true;
        self.status = "importing subscriptions from account…".into();
        let cfg = self.cfg.clone();
        let (tx, rx) = mpsc::channel();
        self.rx_import = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(youtube::import_subscriptions(&cfg));
        });
    }

    pub fn toggle_pause(&mut self) {
        match player::ipc_send(r#"{"command":["cycle","pause"]}"#) {
            Ok(_) => self.status = "mpv: play/pause toggled".into(),
            Err(_) => self.status = "mpv not playing".into(),
        }
    }

    pub fn next_track(&mut self) {
        match player::ipc_send(r#"{"command":["playlist-next"]}"#) {
            Ok(_) => self.status = "mpv: next".into(),
            Err(_) => self.status = "mpv not playing".into(),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CachedFeed {
    saved_at: u64,
    videos: Vec<Video>,
}

fn save_feed_cache(videos: &[Video]) {
    let saved_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let cached = CachedFeed {
        saved_at,
        videos: videos.to_vec(),
    };
    let _ = std::fs::create_dir_all(config::cache_dir());
    let _ = std::fs::write(
        config::cache_dir().join("feed.json"),
        serde_json::to_string(&cached).unwrap_or_default(),
    );
}

impl App {
    // ---------------- engagement (x menu / right-click) ----------------

    pub fn open_actions(&mut self) {
        if self.view != View::Home || self.filtered.get(self.selected).is_none() { return; }
        self.overlay = Some(Overlay::Actions { idx: self.selected });
        self.settings_sel = 0;
        self.overlay_scroll = 0;
    }

    fn action_video(&self) -> Option<Video> {
        match self.overlay {
            Some(Overlay::Actions { idx }) => self.filtered.get(idx).and_then(|vi| self.videos.get(*vi)).cloned(),
            _ => None,
        }
    }

    /// (label, value) rows for the action menu.
    pub fn action_rows(&self) -> Vec<(String, String)> {
        let subbed = self.action_subbed();
        vec![
            ("▶ Play".into(), "Enter".into()),
            ("♥ Like".into(), "account".into()),
            ("♡ Remove rating".into(), "account".into()),
            ("👎 Dislike".into(), "account".into()),
            ("＋ Queue".into(), format!("{} queued", self.queue.len())),
            ("◷ Save to Watch Later".into(), "account".into()),
            ("⬇ Download video".into(), "file".into()),
            ("🎵 Download audio".into(), "mp3".into()),
            (if subbed { "－ Unsubscribe".into() } else { "＋ Subscribe".into() }, "account".into()),
            ("ⓘ Info".into(), "→".into()),
            ("💬 Comments".into(), "→".into()),
        ]
    }

    fn action_subbed(&self) -> bool {
        let Some(v) = self.action_video() else { return false };
        self.cfg.subscriptions.iter().any(|s| {
            let a = s.to_lowercase();
            a == v.channel.to_lowercase()
                || (!v.channel_id.is_empty() && a.contains(&v.channel_id.to_lowercase()))
                || (!v.channel_url.is_empty() && a == v.channel_url.to_lowercase())
        })
    }

    fn channel_ref(&self, v: &Video) -> Option<String> {
        if !v.channel_id.is_empty() { return Some(v.channel_id.clone()); }
        if !v.channel_url.is_empty() { return Some(v.channel_url.clone()); }
        None
    }

    pub fn execute_action(&mut self, row: usize) {
        let Some(v) = self.action_video() else { return; };
        if v.id.starts_with("mock") && !matches!(row, 0) {
            self.status = "mock entry — live-search first".into();
            return;
        }
        match row {
            0 => {
                let (id, title) = (v.id.clone(), v.title.clone());
                config::push_history(&self.cfg, &v.id, &v.title, &v.channel);
                self.overlay = None;
                self.launch(&id, &title, None);
            }
            1 => self.engage(|cfg, id| engage::like(cfg, &id), v.id.clone(), "♥"),
            2 => self.engage(|cfg, id| engage::remove_rating(cfg, &id), v.id.clone(), "♡"),
            3 => self.engage(|cfg, id| engage::dislike(cfg, &id), v.id.clone(), "👎"),
            4 => {
                if !self.queue.iter().any(|q| q.id == v.id) { self.queue.push(v); }
                self.status = format!("queued ({} total) — Q view • P play", self.queue.len());
            }
            5 => self.engage(|cfg, id| engage::save_watch_later(cfg, &id), v.id.clone(), "◷"),
            6 => { self.overlay = None; self.selected = self.filtered.iter().position(|vi| self.videos.get(*vi).map(|x| x.id == v.id).unwrap_or(false)).unwrap_or(self.selected); self.start_download(false); }
            7 => { self.overlay = None; self.selected = self.filtered.iter().position(|vi| self.videos.get(*vi).map(|x| x.id == v.id).unwrap_or(false)).unwrap_or(self.selected); self.start_download(true); }
            8 => self.toggle_subscribe(v),
            9 => { let id = v.id.clone(); self.overlay = None; self.selected = self.filtered.iter().position(|vi| self.videos.get(*vi).map(|x| x.id == id).unwrap_or(false)).unwrap_or(self.selected); self.open_info(); }
            10 => { let id = v.id.clone(); self.overlay = None; self.selected = self.filtered.iter().position(|vi| self.videos.get(*vi).map(|x| x.id == id).unwrap_or(false)).unwrap_or(self.selected); self.open_comments(); }
            _ => {}
        }
    }

    fn engage(&mut self, f: fn(&config::Config, &str) -> Result<String, String>, id: String, _icon: &str) {
        self.status = "syncing with your account…".into();
        let cfg = self.cfg.clone();
        let (tx, rx) = mpsc::channel();
        self.rx_act = Some(rx);
        std::thread::spawn(move || { let _ = tx.send(f(&cfg, &id)); });
    }

    fn toggle_subscribe(&mut self, v: Video) {
        let Some(chan_ref) = self.channel_ref(&v) else {
            self.status = "no channel link on this video".into();
            return;
        };
        let subbed = self.action_subbed();
        let cfg = self.cfg.clone();
        let (tx, rx) = mpsc::channel();
        self.rx_act = Some(rx);
        self.status = if subbed { "unsubscribing…".into() } else { "subscribing…".into() };
        let chan_for_thread = chan_ref.clone();
        std::thread::spawn(move || {
            let res = if subbed {
                engage::unsubscribe(&cfg, &chan_for_thread)
            } else {
                engage::subscribe(&cfg, &chan_for_thread)
            };
            let _ = tx.send(res);
        });
        // mirror locally right away (config is the subs source of truth)
        if subbed {
            let name = v.channel.clone();
            let url = v.channel_url.clone();
            let cid = v.channel_id.clone();
            self.cfg.subscriptions.retain(|s| {
                s.to_lowercase() != name.to_lowercase() && s != &url && s != &cid
            });
            self.subs.retain(|(s, _)| {
                s.to_lowercase() != name.to_lowercase() && s != &url && s != &cid
            });
        } else if !self.cfg.subscriptions.iter().any(|s| s.to_lowercase() == chan_ref.to_lowercase()) {
            self.cfg.subscriptions.push(chan_ref);
            let last = self.cfg.subscriptions.last().cloned().unwrap();
            self.subs.push((last, false));
        }
        config::save(&self.cfg);
    }
}

fn short(s: &str) -> String {
    s.chars().take(50).collect()
}

fn inside(r: ratatui::layout::Rect, x: u16, y: u16) -> bool {
    x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
}
