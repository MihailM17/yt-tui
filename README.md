# yt-tui

Lightweight terminal YouTube client — YouTube.com layout in the terminal, ASCII thumbs, no browser.

![layout](docs/mockup.png)

MVP matches the mockup: header + sidebar + topic chips + 3x3 video grid with procedural ASCII thumbnails. Zero network in MVP (mock data) so `cargo run` works offline.

## Run

```bash
cargo run --release
```

Requires: terminal with truecolor + Nerd Font. Best in Kitty / WezTerm / Ghostty / foot. Falls back to block-art anywhere.

Deps at runtime for playback (wired in `src/player.rs`):
- `mpv`
- `yt-dlp`

## Keys

- `hjkl` / arrows — navigate grid
- `Enter` — play via `mpv` (real videoIds only; mock entries show stub status)
- `/` — search, `Esc` — clear/exit search, `Enter` — apply
- `1-9`, `Tab` — topic chips
- `g` / `G` — top / bottom
- `q` or `Ctrl-C` — quit

## Efficient by design (low RAM/storage)

- Rust + `ratatui` + `crossterm` only. No tokio-full, no ffmpeg linked, no browser.
- External `mpv` for playback (spawned detached).
- Thumbnails: `default.jpg` (120x90, ~3KB) → resize in RAM → half-block ANSI → drop image. Cached as text:
  - RAM: LRU 30 entries, <500KB
  - Disk: none in MVP (add 20MB capped ANSI cache later)
- Listing via lightweight Innertube/RSS (TODO), `yt-dlp -g` only on Enter for stream URL. No per-search Python spawn.
- Redraw on input, 10fps poll, only visible 9 cards rendered.
- Release profile: `opt-level=z, lto, strip` → ~3-6MB binary, ~8-15MB idle RAM.

Target: `cargo build --release` → single binary, XDG cache auto-pruned.

## Roadmap

- [x] mockup UI (sidebar/chips/grid/ASCII thumbs)
- [ ] real feed: RSS subs + Innertube search (`ureq`, no python for listing)
- [ ] on-disk ANSI thumb cache (20MB LRU, `~/.cache/yt-tui/`)
- [ ] `yt-dlp -g` + `mpv` playback + resume positions (`~/.local/share/yt-tui/`)
- [ ] SponsorBlock skip, quality picker via mpv IPC
- [ ] Kitty/Sixel real-image thumbs as opt-in (`--gfx kitty|sixel|blocks`)

## Layout

- `src/main.rs` — terminal setup + event loop
- `src/app.rs` — state, filtering, 30-entry thumb LRU
- `src/ui.rs` — YouTube.com layout in ratatui
- `src/thumb.rs` — procedural ASCII thumb (stand-in for chafa pipeline)
- `src/data.rs` — mock feed (swap for API)
- `src/player.rs` — `yt-dlp -g` + `mpv` hook
