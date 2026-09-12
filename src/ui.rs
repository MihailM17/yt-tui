use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::App;

const BG: Color = Color::Rgb(10, 14, 22);
const PANEL: Color = Color::Rgb(16, 22, 34);
const ACCENT: Color = Color::Rgb(80, 160, 255);
const DIM: Color = Color::Rgb(130, 150, 175);
const CHIP_BG: Color = Color::Rgb(30, 40, 58);

pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();
    f.render_widget(Block::default().style(Style::default().bg(BG)), area);

    // remember cols for vim nav (set again in render_grid)
    app.cols = if area.width >= 178 { 3 } else if area.width >= 128 { 2 } else { 1 };

    // outer: header / chips / main / status
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
    render_chips(f, app, outer[1]);

    // main: sidebar + grid
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
        render_grid(f, app, main_chunks[1]);
    } else {
        render_grid(f, app, main_chunks[0]);
    }

    let status_txt = if app.loading {
        " ⟳ loading via yt-dlp… (UI stays responsive)".to_string()
    } else if app.live {
        format!(" ●LIVE  {}", past_status(app))
    } else {
        format!(" ○MOCK  {}", past_status(app))
    };
    let status = Paragraph::new(Line::from(vec![Span::styled(
        format!(" {status_txt}"),
        Style::default().fg(DIM),
    )]))
    .style(Style::default().bg(BG));
    f.render_widget(status, outer[3]);

    if app.searching {
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

    let logo = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("☰ ", Style::default().fg(DIM)),
            Span::styled("▶ ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::styled(
                "YouTube ",
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ),
            Span::styled("BG", Style::default().fg(DIM)),
        ]),
    ])
    .block(block(""));
    f.render_widget(logo, chunks[0]);

    let search_txt = if app.searching {
        format!("{}▌", app.query)
    } else if app.query.is_empty() {
        "Search...".to_string()
    } else {
        app.query.clone()
    };
    let search = Paragraph::new(Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled(
            search_txt,
            Style::default().fg(if app.query.is_empty() && !app.searching {
                DIM
            } else {
                Color::White
            }),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if app.searching { ACCENT } else { DIM }))
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

fn render_sidebar(f: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = vec![];
    lines.push(menu_line("⌂ Home", true));
    lines.push(Line::from(Span::styled(
        "────────────────────",
        Style::default().fg(DIM),
    )));
    lines.push(Line::from(Span::styled(
        "Subscriptions  ›",
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
    )));
    for (name, fresh) in &app.subs {
        let dot = if *fresh { " •" } else { "" };
        lines.push(Line::from(vec![
            Span::styled("◉ ", Style::default().fg(Color::Red)),
            Span::styled(name.clone(), Style::default().fg(Color::White)),
            Span::styled(dot, Style::default().fg(ACCENT)),
        ]));
    }
    lines.push(Line::from(Span::styled("﹀ Show more", Style::default().fg(DIM))));
    lines.push(Line::from(Span::styled(
        "────────────────────",
        Style::default().fg(DIM),
    )));
    lines.push(Line::from(Span::styled(
        "You  ›",
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
    )));
    for item in [
        "◉ Your channel",
        "↻ History",
        "☰ Playlists",
        "◷ Watch later",
        "♡ Liked videos",
        "▷ Your videos",
        "⬇ Downloads",
    ] {
        lines.push(Line::from(Span::styled(item, Style::default().fg(DIM))));
    }
    lines.push(Line::from(Span::styled("﹀ Show more", Style::default().fg(DIM))));

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

    // thumb takes top ~60%, info bottom
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(4)])
        .split(inner);
    let thumb_area = chunks[0];
    let info_area = chunks[1];

    let v = app.videos[video_idx].clone();
    let thumb_h = thumb_area.height;
    let thumb_w = thumb_area.width;
    let lines = app.thumb(video_idx, thumb_w, thumb_h.saturating_sub(1));
    let thumb_para = Paragraph::new(lines);
    f.render_widget(thumb_para, thumb_area);

    // duration overlay bottom-right of thumb
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
            Paragraph::new(dur).style(
                Style::default().bg(Color::Black).fg(Color::White),
            ),
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
            Span::styled(
                format!("{}{}", v.channel, check),
                Style::default().fg(DIM),
            ),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                format!("{} • {}", v.views, v.age),
                Style::default().fg(DIM),
            ),
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
    Block::default()
        .title(title)
        .style(Style::default().bg(BG))
}
