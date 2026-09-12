# yt-tui

Lightweight terminal YouTube client — YouTube.com layout in the terminal, ASCII thumbs, no browser.

![layout](docs/mockup.png)

MVP matches the mockup: header + sidebar + topic chips + 3x3 video grid.
Starts offline (mock data), press `/` for live search, `r` for subs feed.

## Run

```bash
cargo run --release
# live search: press / type query Enter (uses yt-dlp, background thread)
# feed: press r (reads ~/.config/yt-tui/config.json subscriptions)
# back to offline mock: press m
```

Requires: terminal with truecolor + Nerd Font. Best in Kitty / WezTerm / Ghostty / foot. Falls back to block-art anywhere.

Deps at runtime for playback (wired in `src/player.rs`):
- `mpv`
- `yt-dlp`

## Keys

- `hjkl` / arrows — navigate grid
- `Enter` / `p` — play via `mpv` (falls back to browser if mpv missing)
- `o` — open in browser
- `/` — live-search via yt-dlp (background, UI stays responsive), `Esc` — exit
- `r` — load subs feed, `m` — back to offline mock
- `1-9`, `Tab` — topic chips
- `g` / `G` — top / bottom
- `q` or `Ctrl-C` — quit

## Efficient by design (low RAM/storage)

- Rust + `ratatui` + `crossterm` + `image/jpeg-only`. No tokio, no TLS in binary, no ffmpeg linked.
- Search/feed via installed `yt-dlp -J` in background thread (no API key). Thumbs via system `curl` + `default.jpg` (120x90, ~3KB).
- Thumbnails: jpg → resize in RAM → block ANSI → drop image:
  - RAM: LRU 30 entries, <500KB
  - Disk: `~/.cache/yt-tui/thumbs/`, 20MB LRU cap, auto-pruned
- History: `~/.local/share/yt-tui/history.json`, capped 100. Config: `~/.config/yt-tui/config.json`.
- Redraw on input, 10fps poll, only visible 9 cards rendered.
- Release: `opt-level=z, lto, strip` → 754K binary (measured), ~10-20MB idle RAM.

## Roadmap

- [x] mockup UI (sidebar/chips/grid/ASCII thumbs)
- [x] live search + subs feed via yt-dlp, real thumbs via curl+jpeg, disk cache 20MB
- [x] `yt-dlp -g` + `mpv` playback (+browser fallback), history
- [ ] SponsorBlock skip, quality picker via mpv IPC
- [ ] Kitty/Sixel real-image thumbs as opt-in (`--gfx kitty|sixel|blocks`)

## Layout

- `src/main.rs` — terminal setup + event loop + background poll
- `src/app.rs` — state, live-search/feed workers, 30-entry thumb RAM LRU
- `src/youtube.rs` — `yt-dlp -J` search + channel fetch, no API key
- `src/thumb.rs` — `curl default.jpg` + `image/jpeg` → ANSI, 20MB disk LRU
- `src/config.rs` — XDG config/cache/data, history (cap 100)
- `src/ui.rs` — YouTube.com layout in ratatui (LIVE/MOCK badge)
- `src/data.rs` — offline mock feed
- `src/player.rs` — `yt-dlp -g` + `mpv` (+browser fallback if mpv missing)
