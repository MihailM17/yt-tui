use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::{App, Overlay, SidebarAction, TransportAction, View};
use ratatui_image::{Resize, StatefulImage};

const BG: Color = Color::Rgb(10, 14, 22);
const PANEL: Color = Color::Rgb(16, 22, 34);
const ACCENT: Color = Color::Rgb(80, 160, 255);
const DIM: Color = Color::Rgb(130, 150, 175);
const CHIP_BG: Color = Color::Rgb(30, 40, 58);

pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();
    f.render_widget(Block::default().style(Style::default().bg(BG)), area);

    app.cols = if area.width >= 178 { 3 } else if area.width >= 128 { 2 } else { 1 };

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(1),
        ])
        .split(area);

    render_header(f, app, outer[0]);
    if app.view == View::Home {
        render_chips(f, app, outer[1]);
    } else {
        render_view_bar(f, app, outer[1]);
    }

    let side_w = if area.width < 90 { 0 } else { 28 };
    let main_chunks = if side_w == 0 {
        vec![outer[2]]
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(side_w), Constraint::Min(10)])
            .split(outer[2])
            .to_vec()
    };
    if side_w > 0 {
        render_sidebar(f, app, main_chunks[0]);
        match app.view {
            View::Home => render_grid(f, app, main_chunks[1]),
            View::Subs => render_subs(f, app, main_chunks[1]),
            View::Playlists => render_playlists(f, app, main_chunks[1]),
            View::Downloads => render_downloads(f, app, main_chunks[1]),
            View::History => render_history(f, app, main_chunks[1]),
        }
    } else {
        match app.view {
            View::Home => render_grid(f, app, main_chunks[0]),
            View::Subs => render_subs(f, app, main_chunks[0]),
            View::Playlists => render_playlists(f, app, main_chunks[0]),
            View::Downloads => render_downloads(f, app, main_chunks[0]),
            View::History => render_history(f, app, main_chunks[0]),
        }
    }

    let status_txt = if app.loading {
        " ⟳ loading via yt-dlp… (UI stays responsive)".to_string()
    } else {
        let badge = match app.view {
            View::Home if app.live => "●LIVE",
            View::Home => "○MOCK",
            View::Subs => "◦SUBS",
            View::Playlists => "◦PLAYLISTS",
            View::Downloads => "◦DOWNLOADS",
            View::History => "◦HIST",
        };
        let login = match app.login_ok {
            Some(true) => " 🔓",
            Some(false) => " 🔒login-failed(L)",
            None => "",
        };
        format!(" {badge}{login}  {}", past_status(app))
    };
    let status = Paragraph::new(Line::from(vec![Span::styled(
        format!(" {status_txt}"),
        Style::default().fg(DIM),
    )]))
    .style(Style::default().bg(BG));
    f.render_widget(status, outer[3]);

    if let Some(ov) = app.overlay.clone() {
        render_overlay(f, app, area, &ov);
    }

    if app.searching || app.adding_sub {
        use ratatui::layout::Position;
        f.set_cursor_position(Position::new(
            outer[0].x + 22 + app.query.len() as u16 + 1,
            outer[0].y + 1,
        ));
    }
}

fn past_status(app: &App) -> String {
    app.status.clone()
}

fn render_header(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(20),
            Constraint::Min(20),
            Constraint::Length(26),
        ])
        .split(area);
    app.search_rect = chunks[1];

    let logo = Paragraph::new(vec![Line::from(vec![
        Span::styled("☰ ", Style::default().fg(DIM)),
        Span::styled("▶ ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Span::styled(
            "YouTube ",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
        Span::styled("BG", Style::default().fg(DIM)),
    ])])
    .block(block(""));
    f.render_widget(logo, chunks[0]);

    let (prompt, txt) = if app.adding_sub {
        ("+sub: ", app.query.clone() + "▌")
    } else if app.searching {
        ("", format!("{}▌", app.query))
    } else if app.query.is_empty() {
        ("", "Search... (/ • click)".to_string())
    } else {
        ("", app.query.clone())
    };
    let search = Paragraph::new(Line::from(vec![
        Span::styled(prompt, Style::default().fg(ACCENT)),
        Span::styled(
            txt,
            Style::default().fg(if app.query.is_empty() && !app.searching && !app.adding_sub {
                DIM
            } else {
                Color::White
            }),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(
                if app.searching || app.adding_sub {
                    ACCENT
                } else {
                    DIM
                },
            ))
            .style(Style::default().bg(PANEL)),
    );
    f.render_widget(search, chunks[1]);

    // clickable transport (pause/next via mpv IPC) + settings gear
    app.transport_hits.clear();
    let tx = chunks[2].x;
    let ty = chunks[2].y + 1;
    for (i, (glyph, act)) in [("⏸", TransportAction::Pause), ("⏭", TransportAction::Next)].iter().enumerate() {
        let r = Rect { x: tx + 1 + i as u16 * 4, y: ty, width: 3, height: 1 };
        app.transport_hits.push((r, *act));
        f.render_widget(Paragraph::new(Span::styled(*glyph, Style::default().fg(ACCENT))), r);
    }
    let gr = Rect { x: tx + 10, y: ty, width: 3, height: 1 };
    app.gear_rect = gr;
    f.render_widget(Paragraph::new(Span::styled("⚙", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))), gr);
    let right = Paragraph::new(Line::from(vec![
        Span::styled("  + Create  ", Style::default().fg(Color::White)),
        Span::styled(" 🔔  ", Style::default().fg(DIM)),
        Span::styled(" ● ", Style::default().fg(Color::Yellow)),
    ]))
    .block(block(""));
    f.render_widget(right, Rect { x: tx + 13, y: chunks[2].y, width: chunks[2].width.saturating_sub(13), height: chunks[2].height });
}

fn render_chips(f: &mut Frame, app: &mut App, area: Rect) {
    app.chips_rect = area;
    let mut spans: Vec<Span> = vec![Span::raw(" ")];
    for (i, c) in app.chips.iter().enumerate() {
        let active = i == app.active_chip;
        spans.push(Span::styled(
            format!(" {c} "),
            Style::default()
                .fg(if active { Color::Black } else { Color::White })
                .bg(if active { ACCENT } else { CHIP_BG })
                .add_modifier(if active { Modifier::BOLD } else { Modifier::empty() }),
        ));
        spans.push(Span::raw(" "));
    }
    let p = Paragraph::new(Line::from(spans)).block(block(""));
    f.render_widget(p, area);
}

fn render_view_bar(f: &mut Frame, app: &mut App, area: Rect) {
    app.chips_rect = Rect::default();
    let title = match app.view {
        View::Subs => "Subscriptions — Enter load • a add • d remove • r refresh all",
        View::History => "History (local, no login) — Enter replay • D clear",
        _ => "",
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(title, Style::default().fg(DIM)))).block(block("")),
        area,
    );
}

fn render_sidebar(f: &mut Frame, app: &mut App, area: Rect) {
    app.sidebar_hits.clear();
    // every row below is clickable now (Later/Liked/Login are actions, not views)
    let rows: Vec<(&str, Option<SidebarAction>)> = vec![
        ("⌂ Home  (0)", Some(SidebarAction::Go(View::Home))),
        ("", None),
        ("Subscriptions  ›", None),
        ("◦ Subs  (s)", Some(SidebarAction::Go(View::Subs))),
        ("☰ Playlists  (;)", Some(SidebarAction::Go(View::Playlists))),
        ("", None),
        ("You  ›", None),
        ("↻ History  (y)", Some(SidebarAction::Go(View::History))),
        ("⬇ Downloads  (b)", Some(SidebarAction::Go(View::Downloads))),
        ("◷ Later  (w)", Some(SidebarAction::Later)),
        ("♡ Liked  (t)", Some(SidebarAction::Liked)),
        ("🔑 Login  (u)", Some(SidebarAction::Login)),
    ];
    let mut lines: Vec<Line> = vec![];
    let mut y = area.y;
    for (label, view) in &rows {
        if label.is_empty() {
            lines.push(Line::from(Span::styled(
                "────────────────────",
                Style::default().fg(DIM),
            )));
            y += 1;
            continue;
        }
        let active = view
            .map(|a| matches!(a, SidebarAction::Go(v) if v == app.view))
            .unwrap_or(false);
        lines.push(menu_line(label, active));
        if let Some(a) = view {
            app.sidebar_hits.push((
                Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: 1,
                },
                *a,
            ));
        }
        y += 1;
    }
    // channel list preview (first 7)
    lines.push(Line::from(Span::styled(
        "────────────────────",
        Style::default().fg(DIM),
    )));
    for (name, fresh) in app.subs.iter().take(7) {
        let dot = if *fresh { " •" } else { "" };
        lines.push(Line::from(vec![
            Span::styled("◉ ", Style::default().fg(Color::Red)),
            Span::styled(name.clone(), Style::default().fg(Color::White)),
            Span::styled(dot, Style::default().fg(ACCENT)),
        ]));
    }

    let p = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::RIGHT)
            .border_style(Style::default().fg(Color::Rgb(40, 60, 90)))
            .style(Style::default().bg(BG)),
    );
    f.render_widget(p, area);
}

fn menu_line(label: &str, active: bool) -> Line<'static> {
    Line::from(Span::styled(
        format!(" {label} "),
        Style::default()
            .fg(if active { Color::White } else { DIM })
            .bg(if active { Color::Rgb(30, 50, 85) } else { BG })
            .add_modifier(if active { Modifier::BOLD } else { Modifier::empty() }),
    ))
}

fn render_grid(f: &mut Frame, app: &mut App, area: Rect) {
    let cols = if area.width >= 150 {
        3
    } else if area.width >= 100 {
        2
    } else {
        1
    };
    app.cols = cols;
    app.card_hits.clear();
    app.list_hits.clear();
    let rows: usize = 3;
    let row_h = area.height / rows.max(1) as u16;

    let row_areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Length(row_h); rows])
        .split(area);

    for r in 0..rows {
        let grid_row = app.row_offset + r;
        let col_areas = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Ratio(1, cols as u32); cols])
            .split(row_areas[r]);

        for c in 0..cols {
            let idx = grid_row * cols + c;
            if idx >= app.filtered.len() {
                continue;
            }
            let flat_pos = idx;
            let video_idx = app.filtered[idx];
            let is_sel = flat_pos == app.selected;
            app.card_hits.push((col_areas[c], flat_pos));
            render_card(f, app, col_areas[c], video_idx, is_sel);
        }
    }
}

fn render_subs(f: &mut Frame, app: &mut App, area: Rect) {
    app.card_hits.clear();
    app.list_hits.clear();
    let mut lines: Vec<Line> = vec![];
    if app.subs.is_empty() {
        lines.push(Line::from(Span::styled(
            "No subscriptions — press a to add @handle",
            Style::default().fg(DIM),
        )));
    }
    for (i, (name, _)) in app.subs.iter().enumerate() {
        let sel = i == app.sub_selected;
        let row_rect = Rect {
            x: area.x + 1,
            y: area.y + 1 + i as u16,
            width: area.width.saturating_sub(2),
            height: 1,
        };
        if row_rect.y < area.y + area.height {
            app.list_hits.push((row_rect, i));
        }
        lines.push(Line::from(Span::styled(
            format!("{} {name}", if sel { "▶" } else { " ◉" }),
            Style::default()
                .fg(if sel { Color::White } else { DIM })
                .bg(if sel {
                    Color::Rgb(30, 50, 85)
                } else {
                    BG
                })
                .add_modifier(if sel { Modifier::BOLD } else { Modifier::empty() }),
        )));
    }
    lines.push(Line::from(Span::raw("")));
    lines.push(Line::from(Span::styled(
        "Enter load channel • a add • d remove • r refresh all feed",
        Style::default().fg(DIM),
    )));
    f.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Subscriptions")
                .style(Style::default().bg(BG)),
        ),
        area,
    );
}

fn render_history(f: &mut Frame, app: &mut App, area: Rect) {
    app.card_hits.clear();
    app.list_hits.clear();
    let mut lines: Vec<Line> = vec![];
    if app.hist.is_empty() {
        lines.push(Line::from(Span::styled(
            "Empty — play a video from Home and it lands here (local only)",
            Style::default().fg(DIM),
        )));
    }
    for (i, e) in app.hist.iter().enumerate().take(area.height as usize - 3) {
        let sel = i == app.hist_selected;
        let row_rect = Rect {
            x: area.x + 1,
            y: area.y + 1 + i as u16,
            width: area.width.saturating_sub(2),
            height: 1,
        };
        app.list_hits.push((row_rect, i));
        let ch = if e.channel.is_empty() {
            "".to_string()
        } else {
            format!(" — {}", e.channel)
        };
        lines.push(Line::from(Span::styled(
            format!("{} {}{}", if sel { "▶" } else { " " }, truncate(&e.title, 70), ch),
            Style::default()
                .fg(if sel { Color::White } else { DIM })
                .bg(if sel {
                    Color::Rgb(30, 50, 85)
                } else {
                    BG
                }),
        )));
    }
    lines.push(Line::from(Span::raw("")));
    lines.push(Line::from(Span::styled(
        "Enter replay • D clear history",
        Style::default().fg(DIM),
    )));
    f.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title("History")
                .style(Style::default().bg(BG)),
        ),
        area,
    );
}

fn render_playlists(f: &mut Frame, app: &mut App, area: Rect) {
    app.card_hits.clear();
    app.list_hits.clear();
    let mut lines: Vec<Line> = vec![];
    if app.pl_names.is_empty() {
        lines.push(Line::from(Span::styled(
            "No playlists — hover videos in Home, a adds to queue, then s here saves it",
            Style::default().fg(DIM),
        )));
    }
    for (i, name) in app.pl_names.iter().enumerate() {
        let sel = i == app.pl_sel;
        let rr = Rect { x: area.x + 1, y: area.y + 1 + i as u16, width: area.width.saturating_sub(2), height: 1 };
        if rr.y < area.y + area.height {
            app.list_hits.push((rr, i));
        }
        lines.push(Line::from(Span::styled(
            format!("{} {name}", if sel { "▶" } else { " ♫" }),
            Style::default()
                .fg(if sel { Color::White } else { DIM })
                .bg(if sel { Color::Rgb(30, 50, 85) } else { BG })
                .add_modifier(if sel { Modifier::BOLD } else { Modifier::empty() }),
        )));
    }
    lines.push(Line::from(Span::raw("")));
    lines.push(Line::from(Span::styled("Enter open in Home • s save queue • d delete", Style::default().fg(DIM))));
    f.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Playlists").style(Style::default().bg(BG))),
        area,
    );
}

fn render_downloads(f: &mut Frame, app: &mut App, area: Rect) {
    app.card_hits.clear();
    app.list_hits.clear();
    let mut lines: Vec<Line> = vec![];
    if app.dl_files.is_empty() {
        lines.push(Line::from(Span::styled(
            "Empty — d on a Home video downloads it here",
            Style::default().fg(DIM),
        )));
    }
    for (i, (_, name)) in app.dl_files.iter().enumerate() {
        let sel = i == app.dl_sel;
        let rr = Rect { x: area.x + 1, y: area.y + 1 + i as u16, width: area.width.saturating_sub(2), height: 1 };
        if rr.y < area.y + area.height {
            app.list_hits.push((rr, i));
        }
        lines.push(Line::from(Span::styled(
            format!("{} {name}", if sel { "▶" } else { " ⬇" }),
            Style::default()
                .fg(if sel { Color::White } else { DIM })
                .bg(if sel { Color::Rgb(30, 50, 85) } else { BG })
                .add_modifier(if sel { Modifier::BOLD } else { Modifier::empty() }),
        )));
    }
    lines.push(Line::from(Span::raw("")));
    lines.push(Line::from(Span::styled("Enter play • d delete file", Style::default().fg(DIM))));
    f.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Downloads").style(Style::default().bg(BG))),
        area,
    );
}

fn render_card(f: &mut Frame, app: &mut App, area: Rect, video_idx: usize, selected: bool) {
    if area.height < 10 || area.width < 20 {
        return;
    }
    let hovered = app.hover.map(|(hx, hy)| inside(area, hx, hy)).unwrap_or(false);
    let border_col = if selected {
        ACCENT
    } else if hovered {
        Color::Rgb(120, 170, 220)
    } else {
        Color::Rgb(45, 70, 100)
    };
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_col))
        .style(Style::default().bg(BG));
    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(4)])
        .split(inner);
    let thumb_area = chunks[0];
    let info_area = chunks[1];

    let v = app.videos[video_idx].clone();
    // real image thumbs on kitty/sixel terminals, ASCII blocks otherwise
    let tw = thumb_area.width;
    let th = thumb_area.height.saturating_sub(1);
    let proto = app.img_proto(&v.id, tw, th.max(4));
    if let Some(state) = proto {
        // Scale (not Fit): small mq thumbs upscale to fill the card like youtube.com
        f.render_stateful_widget(StatefulImage::new().resize(Resize::Scale(None)), thumb_area, state);
    } else {
        let lines = app.thumb(video_idx, tw, th);
        f.render_widget(Paragraph::new(lines), thumb_area);
    }

    let dur = format!(" {} ", v.duration);
    let dw = dur.len() as u16;
    if thumb_area.width > dw + 2 && thumb_area.height > 1 {
        let r = Rect {
            x: thumb_area.x + thumb_area.width - dw - 1,
            y: thumb_area.y + thumb_area.height - 1,
            width: dw,
            height: 1,
        };
        f.render_widget(
            Paragraph::new(dur).style(Style::default().bg(Color::Black).fg(Color::White)),
            r,
        );
    }

    let check = if v.verified { " ✓" } else { "" };
    let meta = match (v.views.is_empty(), v.age.is_empty()) {
        (true, true) => "live / new".to_string(),
        (false, true) => v.views.clone(),
        (true, false) => v.age.clone(),
        (false, false) => format!("{} • {}", v.views, v.age),
    };
    let info = vec![
        Line::from(vec![
            Span::styled("◉ ", Style::default().fg(Color::Red)),
            Span::styled(
                truncate(&v.title, (info_area.width as usize).saturating_sub(6)),
                Style::default().fg(Color::White).add_modifier(if selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
            ),
            Span::styled(" ⋮", Style::default().fg(DIM)),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{}{}", v.channel, check), Style::default().fg(DIM)),
        ]),
        Line::from(vec![Span::raw("  "), Span::styled(meta, Style::default().fg(DIM))]),
    ];
    f.render_widget(Paragraph::new(info), info_area);
}

fn render_overlay(f: &mut Frame, app: &mut App, area: Rect, ov: &Overlay) {
    let w = (area.width * 3 / 4).clamp(40, 110);
    let h = (area.height * 3 / 4).clamp(12, 40);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = Rect { x, y, width: w, height: h };
    f.render_widget(ratatui::widgets::Clear, rect);
    // clickable close button (also Esc/q)
    let xr = Rect { x: x + w.saturating_sub(5), y, width: 4, height: 1 };
    app.close_rect = xr;
    f.render_widget(Paragraph::new(Span::styled(" ✕ ", Style::default().fg(Color::White).bg(Color::Rgb(150, 50, 50)).add_modifier(Modifier::BOLD))), xr);
    let (title, lines): (String, Vec<Line>) = match ov {
        Overlay::Info { vid: _, info, related } => {
            // related rows clickable via settings_hits (cleared per overlay)
            app.settings_hits.clear();
            let mut l = vec![
                Line::from(Span::styled(info.title.clone(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled(format!("{} • {} • {} • {} • ♥ {}", info.channel, info.views, info.date, info.duration, info.likes), Style::default().fg(ACCENT))),
                Line::from(Span::raw("")),
            ];
            for para in info.desc.split("

").take(30) {
                let p: String = para.split_whitespace().collect::<Vec<_>>().join(" ");
                if p.is_empty() { continue; }
                l.push(Line::from(Span::styled(p, Style::default().fg(Color::White))));
                l.push(Line::from(Span::raw("")));
            }
            if !info.chapters.is_empty() {
                l.push(Line::from(Span::styled("Chapters:", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))));
                for (ts, name) in &info.chapters {
                    l.push(Line::from(Span::styled(format!("  {ts}  {name}"), Style::default().fg(DIM))));
                }
            }
            app.info_hits.clear();
            if !related.is_empty() {
                l.push(Line::from(Span::raw("")));
                l.push(Line::from(Span::styled("More from this channel (click/1-9):", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))));
                let first_row = l.len();
                for (k, rv) in related.iter().take(9).enumerate() {
                    l.push(Line::from(vec![
                        Span::styled(format!("{} ", k + 1), Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
                        Span::styled(truncate(&rv.title, 60), Style::default().fg(Color::White)),
                        Span::styled(format!("  {}", rv.duration), Style::default().fg(DIM)),
                    ]));
                    // screen row of this line, scroll-aware
                    let sy = (first_row + k) as i32 - app.overlay_scroll as i32;
                    if sy >= 0 {
                        let rr = Rect { x: rect.x + 2, y: rect.y + 2 + sy as u16, width: rect.width.saturating_sub(4), height: 1 };
                        if rr.y > rect.y && rr.y < rect.y + rect.height.saturating_sub(1) {
                            app.info_hits.push((rr, k));
                        }
                    }
                }
            }
            ("Info  (j/k scroll • 1-9 plays related • Esc close)".into(), l)
        }
        Overlay::Comments { vid: _, items } => {
            let mut l = vec![Line::from(Span::styled(format!("{} comments", items.len()), Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)))];
            for c in items {
                l.push(Line::from(vec![
                    Span::styled(c.author.clone() + " ", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
                    Span::styled(if c.likes.is_empty() { String::new() } else { format!("♥{} ", c.likes) }, Style::default().fg(DIM)),
                ]));
                l.push(Line::from(Span::styled(c.text.clone(), Style::default().fg(Color::White))));
                l.push(Line::from(Span::raw("")));
            }
            ("Comments  (j/k scroll • Esc close)".into(), l)
        }
        Overlay::Settings => {
            app.settings_hits.clear();
            let mut l = vec![];
            let rows = app.settings_rows();
            for (i, (label, value)) in rows.iter().enumerate() {
                let sel = i == app.settings_sel;
                let rr = Rect { x: rect.x + 2, y: rect.y + 2 + i as u16, width: rect.width.saturating_sub(4), height: 1 };
                if rr.y < rect.y + rect.height.saturating_sub(2) {
                    app.settings_hits.push((rr, i));
                }
                let val_style = if value == "→" { Style::default().fg(ACCENT).add_modifier(Modifier::BOLD) }
                    else { Style::default().fg(Color::Rgb(150, 220, 150)) };
                l.push(Line::from(vec![
                    Span::styled(format!("{} ", if sel { "▶" } else { " " }), Style::default().fg(if sel { Color::White } else { DIM })),
                    Span::styled(format!("{label:<18}"), Style::default().fg(if sel { Color::White } else { DIM }).add_modifier(if sel { Modifier::BOLD } else { Modifier::empty() })),
                    Span::styled(value.clone(), val_style),
                ]));
            }
            l.push(Line::from(Span::raw("")));
            l.push(Line::from(Span::styled("Enter/click changes • saved to config.json", Style::default().fg(DIM))));
            ("Settings  (click or j/k + Enter • Esc closes)".into(), l)
        }
        Overlay::Actions { .. } => {
            app.settings_hits.clear();
            let mut l = vec![];
            let rows = app.action_rows();
            for (i, (label, value)) in rows.iter().enumerate() {
                let sel = i == app.settings_sel;
                let rr = Rect { x: rect.x + 2, y: rect.y + 2 + i as u16, width: rect.width.saturating_sub(4), height: 1 };
                if rr.y < rect.y + rect.height.saturating_sub(2) {
                    app.settings_hits.push((rr, i));
                }
                l.push(Line::from(vec![
                    Span::styled(format!("{} ", if sel { "▶" } else { " " }), Style::default().fg(if sel { Color::White } else { DIM })),
                    Span::styled(format!("{label:<24}"), Style::default().fg(if sel { Color::White } else { DIM }).add_modifier(if sel { Modifier::BOLD } else { Modifier::empty() })),
                    Span::styled(value.clone(), Style::default().fg(ACCENT)),
                ]));
            }
            l.push(Line::from(Span::raw("")));
            l.push(Line::from(Span::styled("Enter/click runs • account rows need cookies.txt", Style::default().fg(DIM))));
            ("Actions  (x or right-click • Esc closes)".into(), l)
        }
        Overlay::Queue => {
            let mut l = vec![];
            if app.queue.is_empty() {
                l.push(Line::from(Span::styled("Empty — hover a video in Home, press a to add", Style::default().fg(DIM))));
            }
            for (i, v) in app.queue.iter().enumerate() {
                l.push(Line::from(Span::styled(format!("{}. {} — {}", i + 1, v.title, v.channel), Style::default().fg(Color::White))));
            }
            l.push(Line::from(Span::raw("")));
            l.push(Line::from(Span::styled("Enter plays all in mpv (autoplay-next)", Style::default().fg(ACCENT))));
            ("Queue  (Enter play • Esc close)".into(), l)
        }
        Overlay::Help => {
            let rows = [
                ("0/s/y", "home / subs / history"),
                ("u", "test login (zen/chrome cookies)"),
                ("w/t", "watch later / liked (needs login)"),
                ("/", "live search"),
                ("f", "cycle sort: relevance/views/longest/shortest"), ("n", "check subs for new uploads"),
                ("i/c", "info+chapters / comments overlay"), ("a/Q/P", "queue add / view / play all"),
                ("x/right", "action menu: like, subscribe, save"),
                ("d/D", "download video / audio mp3"), ("v", "quality best→720p→480p→audio"),
                ("[/]", "mpv speed -/+ (while playing)"), ("r/+", "refresh / load more"),
                ("mouse", "click everything: search, sidebar, thumbs, settings"),
                (",/space", "settings • pause mpv"),
            ];
            // (label, desc) pairs rendered simply
            let mut l = vec![];
            for (k, d) in rows { l.push(Line::from(vec![Span::styled(format!("{k:8}"), Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)), Span::styled(d, Style::default().fg(Color::White))])); }
            let _ = &rows;
            ("Help  (? or Esc close)".into(), l)
        }
    };
    let scroll = app.overlay_scroll as u16;
    let inner_h = h.saturating_sub(2) as usize;
    let max_scroll = lines.len().saturating_sub(inner_h);
    let s = (scroll as usize).min(max_scroll);
    let visible: Vec<Line> = lines.into_iter().skip(s).take(inner_h).collect();
    f.render_widget(
        Paragraph::new(visible).block(Block::default().borders(Borders::ALL).title(title.as_str()).style(Style::default().bg(Color::Rgb(14, 19, 30)))),
        rect,
    );
}

fn truncate(s: &str, max: usize) -> String {
    if max < 4 {
        return String::new();
    }
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        return s.to_string();
    }
    chars[..max - 1].iter().collect::<String>() + "…"
}

fn inside(r: Rect, x: u16, y: u16) -> bool {
    x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
}

fn block(title: &str) -> Block<'_> {
    Block::default().title(title).style(Style::default().bg(BG))
}
