pub mod app;
pub mod config;
pub mod data;
pub mod engage;
pub mod player;
pub mod theme;
pub mod thumb;
pub mod ui;
pub mod youtube;

use std::io::{self, stdout};

use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::App;

fn config_marker() -> std::path::PathBuf {
    config::base_dir().join(".welcomed")
}

fn main() -> io::Result<()> {
    // Detect image protocol BEFORE raw mode / alternate screen: the query
    // writes an escape to stdout and reads the terminal's reply on stdin.
    // Failure simply means ASCII-block thumbs (still great).
    let picker = ratatui_image::picker::Picker::from_query_stdio().ok();

    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(out);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(picker);
    // behave like the app: auto-refresh subs feed on launch (background)
    app.load_feed();
    let res = run(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(e) = res {
        eprintln!("yt-tui error: {e}");
    }
    Ok(())
}

fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> io::Result<()> {
    // first run: explain keys once (marker file, Help overlay)
    let welcome = config_marker();
    if !welcome.exists() {
        app.overlay = Some(app::Overlay::Help);
        let _ = std::fs::create_dir_all(config::base_dir());
        let _ = std::fs::write(&welcome, b"1");
    }
    let mut tick: u64 = 0;
    loop {
        app.poll();
        // ~5s housekeeping: resume-position save + sleep timer
        tick += 1;
        if tick % 50 == 0 {
            app.poll_resume();
            if let Some(deadline) = app.sleep_until {
                if std::time::Instant::now() >= deadline {
                    player::quit_mpv();
                    app.sleep_until = None;
                    app.status = "sleep timer: player stopped".into();
                }
            }
        }
        terminal.draw(|f| ui::render(f, app))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    if (key.code == KeyCode::Char('q') && !app.searching)
                        || (key.code == KeyCode::Char('c')
                            && key.modifiers.contains(KeyModifiers::CONTROL))
                    {
                        // q closes overlays first (handled in on_key); quit only when none open
                        if key.code == KeyCode::Char('q') && app.overlay.is_some() {
                            app.on_key(key.code, key.modifiers);
                            continue;
                        }
                        break;
                    }
                    app.on_key(key.code, key.modifiers);
                    if app.should_quit {
                        break;
                    }
                }
                Event::Mouse(m) => app.on_mouse(m),
                _ => {}
            }
        }

        app.prune_thumb_cache();
    }
    Ok(())
}
