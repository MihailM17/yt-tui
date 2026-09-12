use std::{fs, process::Command, time::Duration};

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

use crate::config;

// ---------------------------------------------------------------------------
// Procedural fallback (offline, zero storage). Distinct hue per video.
// ---------------------------------------------------------------------------

pub fn procedural(seed: u64, hue: u8, w_cells: u16, h_rows: u16) -> Vec<Line<'static>> {
    const SHADES: &[char] = &[' ', '░', '▒', '▓', '█', '▚', '▞', '▙', '▛', '#', '@'];
    let w = w_cells as usize;
    let h = h_rows as usize;

    let mut out = Vec::with_capacity(h);
    for y in 0..h {
        let mut spans = Vec::with_capacity(w);
        for x in 0..w {
            let n = hash(seed, x as u64, y as u64, w as u64);
            let shade = SHADES[(n % SHADES.len() as u64) as usize];
            let t = n as f32 / u64::MAX as f32;
            let (r, g, b) = hue_rgb(hue, x, y, w, h, t);
            spans.push(Span::styled(
                shade.to_string(),
                Style::default().fg(Color::Rgb(r, g, b)),
            ));
        }
        out.push(Line::from(spans));
    }
    out
}

// ---------------------------------------------------------------------------
// Real thumbnails: default.jpg (120x90, ~3KB) -> ANSI blocks.
// No TLS in our binary: fetch via system `curl`, decode via `image/jpeg` only.
// Disk: ~/.cache/yt-tui/thumbs/<id>.jpg, 20MB LRU cap (configurable).
// ---------------------------------------------------------------------------

pub fn thumb_dir() -> std::path::PathBuf {
    crate::config::cache_dir().join("thumbs")
}

/// Try real thumbnail, fall back to procedural on any failure.
/// `is_mock` ids (mock1..) always use procedural — no network.
pub fn get(
    video_id: &str,
    seed: u64,
    hue: u8,
    w: u16,
    h: u16,
    cache_mb: u64,
) -> Vec<Line<'static>> {
    if video_id.starts_with("mock") {
        return procedural(seed, hue, w.max(8), h.max(4));
    }
    if let Some(bytes) = fetch_jpg(video_id, cache_mb) {
        if let Some(lines) = from_jpeg(&bytes, w.max(8), h.max(4)) {
            return lines;
        }
    }
    procedural(seed, hue, w.max(8), h.max(4))
}

fn fetch_jpg(video_id: &str, cache_mb: u64) -> Option<Vec<u8>> {
    let dir = thumb_dir();
    let _ = fs::create_dir_all(&dir);
    let path = dir.join(format!("{video_id}.jpg"));

    if let Ok(b) = fs::read(&path) {
        if !b.is_empty() {
            // touch mtime for LRU
            let _ = file_touch(&path);
            return Some(b);
        }
    }

    // system curl: tiny binary, shared TLS, no rustls bloat in our binary
    let url = format!("https://i.ytimg.com/vi/{video_id}/default.jpg");
    let out = Command::new("curl")
        .args(["-sL", "--max-time", "10", &url])
        .output()
        .ok()?;
    if !out.status.success() || out.stdout.len() < 500 {
        return None;
    }
    let _ = fs::write(&path, &out.stdout);
    enforce_cap(&dir, cache_mb);
    Some(out.stdout)
}

fn from_jpeg(bytes: &[u8], w_cells: u16, h_rows: u16) -> Option<Vec<Line<'static>>> {
    let img = image::load_from_memory(bytes).ok()?;
    let rgb = img.to_rgb8();
    let w = w_cells as u32;
    let h = h_rows as u32;
    let small = image::imageops::resize(&rgb, w, h, image::imageops::FilterType::Triangle);

    const SHADES: &[char] = &[' ', '░', '▒', '▓', '█'];
    let mut out = Vec::with_capacity(h as usize);
    for y in 0..h {
        let mut spans = Vec::with_capacity(w as usize);
        for x in 0..w {
            let p = small.get_pixel(x, y);
            let (r, g, b) = (p[0], p[1], p[2]);
            // luminance -> shade, color = pixel (truecolor thumb like mockup)
            let lum = (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) / 255.0;
            let shade = SHADES[(lum * (SHADES.len() - 1) as f32) as usize];
            spans.push(Span::styled(
                shade.to_string(),
                Style::default().fg(Color::Rgb(r, g, b)),
            ));
        }
        out.push(Line::from(spans));
    }
    Some(out)
}

fn enforce_cap(dir: &std::path::Path, cap_mb: u64) {    let cap = cap_mb.max(5) * 1024 * 1024;
    let mut entries: Vec<_> = fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter_map(|e| {
                    e.metadata()
                        .ok()
                        .map(|m| (e.path(), m.len(), m.modified().unwrap_or(std::time::UNIX_EPOCH)))
                })
                .collect()
        })
        .unwrap_or_default();
    let total: u64 = entries.iter().map(|(_, s, _)| *s).sum();
    if total <= cap {
        return;
    }
    // oldest first
    entries.sort_by_key(|(_, _, t)| *t);
    let mut freed = 0u64;
    let need = total - cap;
    for (p, s, _) in entries {
        if freed >= need {
            break;
        }
        if fs::remove_file(&p).is_ok() {
            freed += s;
        }
    }
}

fn file_touch(p: &std::path::Path) -> std::io::Result<()> {
    // cheap mtime bump: re-write mtime via filetime-less trick (read+write same len not needed).
    // Use `touch` semantics via setting len (no-op if same).
    let _ = Command::new("touch").arg(p).output();
    let _ = Duration::from_secs(0);
    let _ = config::cache_dir();
    Ok(())
}

fn hash(seed: u64, x: u64, y: u64, w: u64) -> u64 {
    let mut h = seed.wrapping_add(0x9E3779B97F4A7C15).wrapping_add(x.wrapping_mul(0xBF58476D1CE4E5B9)).wrapping_add(y.wrapping_mul(0x94D049BB133111EB)).wrapping_add(w.wrapping_mul(0xD1B54A32846B4E87));
    h ^= h >> 30; h = h.wrapping_mul(0xBF58476D1CE4E5B9); h ^= h >> 27; h = h.wrapping_mul(0x94D049BB133111EB); h ^= h >> 31;
    let band = ((x * 7 + y * 13) % 11) as u64;
    h.wrapping_add(band.wrapping_mul(0x9E3779B9))
}
fn hue_rgb(hue: u8, x: usize, y: usize, w: usize, h: usize, t: f32) -> (u8, u8, u8) {
    let fx = x as f32 / w.max(1) as f32; let fy = y as f32 / h.max(1) as f32;
    let hh = hue as f32 / 255.0 * 6.0;
    let (r0,g0,b0) = hsvish(hh + fx*1.2, 0.75, 0.55+0.45*t);
    let v = 0.65 + 0.35*(1.0-((fx-0.5).abs()+(fy-0.5).abs()));
    ((r0*v) as u8,(g0*v) as u8,(b0*v) as u8)
}
fn hsvish(h: f32, s: f32, v: f32) -> (f32,f32,f32) {
    let c=v*s; let x=c*(1.0-((h%2.0)-1.0).abs()); let m=v-c;
    let (r,g,b)=match h as i32 % 6 {0=>(c,x,0.0),1=>(x,c,0.0),2=>(0.0,c,x),3=>(0.0,x,c),4=>(x,0.0,c),_=>(c,0.0,x)};
    ((r+m)*255.0,(g+m)*255.0,(b+m)*255.0)
}
