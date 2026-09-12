use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::{App, View};

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
            View::History => render_history(f, app, main_chunks[1]),
        }
    } else {
        match app.view {
            View::Home => render_grid(f, app, main_chunks[0]),
            View::Subs => render_subs(f, app, main_chunks[0]),
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

    let right = Paragraph::new(Line::from(vec![
        Span::styled(" ⌕ ", Style::default().fg(ACCENT)),
        Span::styled(" 🎙 ", Style::default().fg(DIM)),
        Span::styled("  + Create  ", Style::default().fg(Color::White)),
        Span::styled(" 🔔  ", Style::default().fg(DIM)),
        Span::styled(" ● ", Style::default().fg(Color::Yellow)),
    ]))
    .block(block(""));
    f.render_widget(right, chunks[2]);
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
    // rows: Home / Subs / History are clickable; rest is info
    let rows: Vec<(&str, Option<View>)> = vec![
        ("⌂ Home  (0)", Some(View::Home)),
        ("", None),
        ("Subscriptions  ›", None),
        ("◦ Subs  (s)", Some(View::Subs)),
        ("", None),
        ("You  ›", None),
        ("↻ History  (y)", Some(View::History)),
        ("◷ Later  (w)", None),
        ("♡ Liked  (t)", None),
        ("🔑 Login  (u)", None),
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
        let active = view.map(|v| v == app.view).unwrap_or(false);
        lines.push(menu_line(label, active));
        if let Some(v) = view {
            app.sidebar_hits.push((
                Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: 1,
                },
                *v,
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

fn render_card(f: &mut Frame, app: &mut App, area: Rect, video_idx: usize, selected: bool) {
    if area.height < 10 || area.width < 20 {
        return;
    }
    let border_col = if selected { ACCENT } else { Color::Rgb(45, 70, 100) };
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
    let lines = app.thumb(video_idx, thumb_area.width, thumb_area.height.saturating_sub(1));
    f.render_widget(Paragraph::new(lines), thumb_area);

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
        Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{} • {}", v.views, v.age), Style::default().fg(DIM)),
        ]),
    ];
    f.render_widget(Paragraph::new(info), info_area);
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

fn block(title: &str) -> Block<'_> {
    Block::default().title(title).style(Style::default().bg(BG))
}
