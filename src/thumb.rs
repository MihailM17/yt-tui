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
    // Half-block mode: each terminal cell holds 2 vertical pixels ('▀' with
    // fg=top, bg=bottom). Terminal cells are ~2x taller than wide, so 1px=1cell
    // looks vertically squashed. This fixes the squash for free (same RAM).
    let w = w_cells as usize;
    let h = h_rows as usize;

    let mut out = Vec::with_capacity(h);
    for y in 0..h {
        let mut spans = Vec::with_capacity(w);
        for x in 0..w {
            let n_top = hash(seed, x as u64, (y * 2) as u64, w as u64);
            let n_bot = hash(seed, x as u64, (y * 2 + 1) as u64, w as u64);
            let t = n_top as f32 / u64::MAX as f32;
            let b = n_bot as f32 / u64::MAX as f32;
            let (r1, g1, b1) = hue_rgb(hue, x, y * 2, w, h * 2, t);
            let (r2, g2, b2) = hue_rgb(hue, x, y * 2 + 1, w, h * 2, b);
            spans.push(Span::styled(
                "▀".to_string(),
                Style::default()
                    .fg(Color::Rgb(r1, g1, b1))
                    .bg(Color::Rgb(r2, g2, b2)),
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

/// Raw decoded thumbnail for real-image (kitty/sixel) rendering.
/// None for mock entries or any fetch/decode failure (caller falls back).
pub fn load_dynamic(
    video_id: &str,
    _cache_mb: u64,
    quality: &str,
) -> Option<image::DynamicImage> {
    if video_id.starts_with("mock") {
        return None;
    }
    let bytes = read_cached(video_id, quality)?;
    image::load_from_memory(&bytes).ok()
}

/// Render-path thumbnail source: DISK ONLY, never network.
/// Warmed in background by `warm_all` after each feed/search.

/// Try real thumbnail, fall back to procedural on any failure.
/// `is_mock` ids (mock1..) always use procedural — no network.
/// Quality from config: default/mq/hq/sd (see config.rs).
pub fn get(
    video_id: &str,
    seed: u64,
    hue: u8,
    w: u16,
    h: u16,
    cache_mb: u64,
    quality: &str,
) -> Vec<Line<'static>> {
    let _ = cache_mb; // cap enforced by background warmer, not render path
    if video_id.starts_with("mock") {
        return procedural(seed, hue, w.max(8), h.max(4));
    }
    if let Some(bytes) = read_cached(video_id, quality) {
        if let Some(lines) = from_jpeg(&bytes, w.max(8), h.max(4)) {
            return lines;
        }
    }
    procedural(seed, hue, w.max(8), h.max(4))
}

pub fn quality_file(quality: &str) -> &'static str {
    match quality {
        "hq" => "hqdefault.jpg",   // 480x360 ~30KB
        "sd" => "sddefault.jpg",   // 640x480 ~60KB
        "default" => "default.jpg", // 120x90 ~3KB
        _ => "mqdefault.jpg",      // 320x180 ~10KB (default: sharper, still tiny)
    }
}

fn read_cached(video_id: &str, quality: &str) -> Option<Vec<u8>> {
    let dir = thumb_dir();
    let path = dir.join(format!("{}-{}", video_id, quality_file(quality)));
    if let Ok(b) = fs::read(&path) {
        if !b.is_empty() {
            let _ = file_touch(&path);
            return Some(b);
        }
    }
    None
}

/// Background pre-fetcher: downloads jpgs missing from disk.
/// Call once per feed/search in a worker thread — NEVER on the render path
/// (render-time curl was freezing the UI ~200ms per new thumb).
pub fn warm_all(ids: &[String], cache_mb: u64, quality: &str) {
    let dir = thumb_dir();
    let _ = fs::create_dir_all(&dir);
    let qf = quality_file(quality);
    let missing: Vec<String> = ids
        .iter()
        .take(40)
        .filter(|id| !id.starts_with("mock") && !dir.join(format!("{id}-{qf}")).exists())
        .cloned()
        .collect();
    if missing.is_empty() {
        return;
    }
    // 6 curl workers: whole grid (~9 files x 10KB) in ~1s
    let chunks: Vec<Vec<String>> = missing
        .chunks((missing.len() / 6).max(1))
        .map(|c| c.to_vec())
        .collect();
    let mut handles = vec![];
    for chunk in chunks {
        let dir = dir.clone();
        let qf = qf.to_string();
        handles.push(std::thread::spawn(move || {
            for id in chunk {
                let url = format!("https://i.ytimg.com/vi/{id}/{qf}");
                if let Ok(out) = Command::new("curl")
                    .args(["-sL", "--max-time", "10", &url])
                    .output()
                {
                    if out.status.success() && out.stdout.len() > 500 {
                        let _ = fs::write(dir.join(format!("{id}-{qf}")), &out.stdout);
                    }
                }
            }
        }));
    }
    for h in handles {
        let _ = h.join();
    }
    enforce_cap(&dir, cache_mb);
}

fn from_jpeg(bytes: &[u8], w_cells: u16, h_rows: u16) -> Option<Vec<Line<'static>>> {
    // You were right — old code resized to (w x h) pixels for (w x h) cells,
    // but a terminal cell is ~2x taller than wide, so everything looked
    // squashed vertically. Fix: decode at (w x h*2) and pack 2 pixels per
    // cell with '▀' (fg=top, bg=bottom). Aspect-fit with letterbox so 16:9
    // thumbs don't stretch to fill the card. Same RAM, correct geometry.
    let img = image::load_from_memory(bytes).ok()?;
    let rgb = img.to_rgb8();
    let w = w_cells.max(8) as u32;
    let h = h_rows.max(4) as u32;
    let buf_h = h * 2;
    let (sw, sh) = (rgb.width().max(1), rgb.height().max(1));

    // fit source inside (w x buf_h), preserve aspect
    let scale = (w as f32 / sw as f32).min(buf_h as f32 / sh as f32);
    let nw = ((sw as f32 * scale) as u32).clamp(1, w);
    let nh = ((sh as f32 * scale) as u32).clamp(1, buf_h);
    let fitted = image::imageops::resize(&rgb, nw, nh, image::imageops::FilterType::Triangle);

    // letterbox onto black canvas
    let mut canvas = image::RgbImage::from_pixel(w, buf_h, image::Rgb([8, 10, 16]));
    let ox = (w - nw) / 2;
    let oy = (buf_h - nh) / 2;
    image::imageops::replace(&mut canvas, &fitted, ox as i64, oy as i64);

    let mut out = Vec::with_capacity(h as usize);
    for y in 0..h {
        let mut spans = Vec::with_capacity(w as usize);
        for x in 0..w {
            let t = canvas.get_pixel(x, y * 2);
            let b = canvas.get_pixel(x, y * 2 + 1);
            spans.push(Span::styled(
                "▀".to_string(),
                Style::default()
                    .fg(Color::Rgb(t[0], t[1], t[2]))
                    .bg(Color::Rgb(b[0], b[1], b[2])),
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
