//! Screenshot harness: renders real UI frames headlessly (mock data, offline)
//! and dumps cell grids as JSON for PNG rasterizing.
//!
//! NOTE: docs/*.png are real terminal captures. Point rasterize at a scratch
//! dir so mock previews never overwrite them.
//!
//! ```bash
//! cargo run --example shots > /tmp/shots.json
//! python3 docs/rasterize.py /tmp/shots.json /tmp/mockshots/
//! ```

use ratatui::{backend::TestBackend, Terminal};
use yt_tui::app::{App, Overlay};

fn main() {
    // One theme per image (no duplicates) so the README shows the range:
    // Midnight + YouTube Dark/Light + Gruvbox.
    let shots = vec![
        ("home-midnight", "Midnight", "home"),
        ("home-youtube-dark", "YouTube Dark", "home"),
        ("settings-youtube-light", "YouTube Light", "settings"),
        ("actions-gruvbox", "Gruvbox", "actions"),
    ];
    println!("{{");
    for (i, (name, theme, scene)) in shots.iter().enumerate() {
        let mut app = App::new(None);
        app.cfg.theme = theme.to_string();
        // Mirror a lived-in sidebar (same density as a real config).
        app.subs = vec![
            ("@ScrapMan".into(), true),
            ("@Grian".into(), true),
            ("@kanGaming".into(), false),
            ("@LoserfruitDaily".into(), false),
            ("@TheClashersGaming".into(), false),
            ("@scrapman".into(), false),
            ("@AethelthrythGaming".into(), true),
            ("@AethelthrythClips".into(), false),
            ("@ludwig".into(), false),
            ("@Valkyrae".into(), false),
            ("@DangerouslyFunny".into(), false),
            ("@techlinked".into(), false),
            ("@OtzStreams".into(), false),
            ("@impulseSV2".into(), false),
            ("@ShortCircuit".into(), false),
            ("@techquickie".into(), false),
            ("@MarcoMeatball".into(), false),
            ("@xisumavoid".into(), false),
            ("@Sinvicta".into(), false),
            ("@LinusTechTips".into(), true),
            ("@jacksepticeye".into(), false),
            ("@GameLinked".into(), false),
            ("@SourceMaster".into(), true),
            ("@SmallishBeans".into(), false),
        ];
        app.live = true; // demo data; badge reads LIVE
        app.cols = 3;
        match *scene {
            "settings" => {
                app.overlay = Some(Overlay::Settings);
                // Showcase the new mouse dropdown: Style expanded.
                app.settings_sel = 4;
                app.settings_open = Some(4);
            }
            "actions" => {
                app.selected = 1;
                app.overlay = Some(Overlay::Actions { idx: 1 });
            }
            _ => {}
        }
        // 180 cols: real 3-column grid (render_grid needs >= 150 for 3).
        let backend = TestBackend::new(180, 44);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| yt_tui::ui::render(f, &mut app)).unwrap();
        let buf = terminal.backend().buffer().clone();
        let w = buf.area.width as usize;
        let h = buf.area.height as usize;
        print!("  \"{name}\": {{\"w\":{w},\"h\":{h},\"cells\":[");
        for (ci, cell) in buf.content.iter().enumerate() {
            let (fr, fg_, fb) = rat(cell.fg);
            let (br, bg_, bb) = rat_bg(cell.bg);
            let sym = cell.symbol().replace('\\', "\\\\").replace('"', "\\\"");
            print!(
                "[\"{sym}\",{fr},{fg_},{fb},{br},{bg_},{bb},{}]",
                if cell.modifier.contains(ratatui::style::Modifier::BOLD) {
                    1
                } else {
                    0
                }
            );
            if ci + 1 < buf.content.len() {
                print!(",");
            }
        }
        print!("]}}");
        if i + 1 < shots.len() {
            println!(",");
        } else {
            println!();
        }
    }
    println!("}}");
}

fn rat(c: ratatui::style::Color) -> (u8, u8, u8) {
    use ratatui::style::Color as C;
    match c {
        C::Reset => (255, 255, 255),
        C::Black => (0, 0, 0),
        C::Red => (205, 49, 49),
        C::Green => (13, 188, 121),
        C::Yellow => (229, 229, 16),
        C::Blue => (36, 114, 200),
        C::Magenta => (188, 63, 188),
        C::Cyan => (17, 168, 205),
        C::Gray => (153, 153, 153),
        C::DarkGray => (102, 102, 102),
        C::LightRed => (241, 76, 76),
        C::LightGreen => (35, 231, 140),
        C::LightYellow => (245, 245, 67),
        C::LightBlue => (59, 142, 234),
        C::LightMagenta => (214, 112, 214),
        C::LightCyan => (51, 199, 239),
        C::White => (255, 255, 255),
        C::Rgb(r, g, b) => (r, g, b),
        C::Indexed(i) => xterm(i),
    }
}

fn rat_bg(c: ratatui::style::Color) -> (u8, u8, u8) {
    use ratatui::style::Color as C;
    match c {
        C::Reset => (10, 14, 22),
        other => rat(other),
    }
}

fn xterm(i: u8) -> (u8, u8, u8) {
    // standard 16 + 6x6x6 cube + grayscale
    const BASE: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (205, 0, 0),
        (0, 205, 0),
        (205, 205, 0),
        (0, 0, 238),
        (205, 0, 205),
        (0, 205, 205),
        (229, 229, 229),
        (127, 127, 127),
        (255, 0, 0),
        (0, 255, 0),
        (255, 255, 0),
        (92, 92, 255),
        (255, 0, 255),
        (0, 255, 255),
        (255, 255, 255),
    ];
    if i < 16 {
        return BASE[i as usize];
    }
    if i < 232 {
        let v = i - 16;
        let levels = [0, 95, 135, 175, 215, 255];
        return (
            levels[(v / 36) as usize],
            levels[((v % 36) / 6) as usize],
            levels[(v % 6) as usize],
        );
    }
    let g = 8 + (i - 232) * 10;
    (g, g, g)
}
