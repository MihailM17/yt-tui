use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

/// Procedural ASCII-art thumbnail.
///
/// Stand-in for the efficient pipeline:
///   download `default.jpg` (120x90, ~3KB) -> resize to ~52x14 px in RAM
///   -> map to half-block ANSI -> drop image, keep text (~5KB).
/// Same RAM/storage profile, zero network in MVP so `cargo run` works offline.
///
/// Uses deterministic xorshift noise + hue rotation so each video has a
/// distinct "scene" like the mockup, without storing any images.
pub fn procedural(seed: u64, hue: u8, w_cells: u16, h_rows: u16) -> Vec<Line<'static>> {
    // Each terminal row holds 2 pixel rows when using half-blocks... here we
    // simplify to 1 char per cell with block shades for max compatibility
    // (works without Kitty/Sixel, like chafa block mode).
    const SHADES: &[char] = &[' ', '░', '▒', '▓', '█', '▚', '▞', '▙', '▛', '#', '@'];
    let w = w_cells as usize;
    let h = h_rows as usize;

    let mut out = Vec::with_capacity(h);
    for y in 0..h {
        let mut spans = Vec::with_capacity(w);
        for x in 0..w {
            // cheap value noise: mix coordinates with seed
            let n = hash(seed, x as u64, y as u64, w as u64);
            let shade = SHADES[(n % SHADES.len() as u64) as usize];

            // hue-rotate RGB for colorful thumbs like the screenshot
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

fn hash(seed: u64, x: u64, y: u64, w: u64) -> u64 {
    // xorshift + coordinate mix; deterministic per (video, x, y)
    let mut h = seed
        .wrapping_add(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(x.wrapping_mul(0xBF58_476D_1CE4_E5B9))
        .wrapping_add(y.wrapping_mul(0x94D0_49BB_1331_11EB))
        .wrapping_add(w.wrapping_mul(0xD1B5_4A32_846B_4E87));
    h ^= h >> 30;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 31;
    // add horizontal banding so it reads as "scene" not static
    let band = ((x * 7 + y * 13) % 11) as u64;
    h.wrapping_add(band.wrapping_mul(0x9E37_79B9))
}

fn hue_rgb(hue: u8, x: usize, y: usize, w: usize, h: usize, t: f32) -> (u8, u8, u8) {
    // base palette rotated by hue, with vertical gradient + scanline variation
    let fx = x as f32 / w.max(1) as f32;
    let fy = y as f32 / h.max(1) as f32;
    let hh = hue as f32 / 255.0 * 6.0;
    let (r0, g0, b0) = hsvish(hh + fx * 1.2, 0.75, 0.55 + 0.45 * t);
    // darken edges like a video frame, brighten center band
    let vignette = 0.65 + 0.35 * (1.0 - ((fx - 0.5).abs() + (fy - 0.5).abs()));
    (
        (r0 * vignette) as u8,
        (g0 * vignette) as u8,
        (b0 * vignette) as u8,
    )
}

fn hsvish(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let c = v * s;
    let x = c * (1.0 - ((h % 2.0) - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h as i32 % 6 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    ((r + m) * 255.0, (g + m) * 255.0, (b + m) * 255.0)
}
