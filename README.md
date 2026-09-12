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

- Mouse: click search/sidebar/videos, wheel scroll
- `0` home • `S` subs • `H` history • `L` login test • `W` watch later • `T` liked
- Subs view: `Enter` load channel • `a` add @handle • `d` remove • `r` refresh all
- History view: `Enter` replay • `D` clear (local only)
- Home: `hjkl`/arrows, `Enter`/`p` play, `o` browser, `/` live-search, `r` feed, `m` mock
- `q` or `Ctrl-C` quit

## Login (no password in TUI — by design)

Google blocks password logins from terminals. Instead yt-tui borrows your
normal browser's YouTube cookies (same trick `yt-dlp --cookies-from-browser` uses):

1. Open YouTube in Chrome and log in normally
2. In `~/.config/yt-tui/config.json` set `"browser": "chrome"` (or brave/edge/firefox), `"use_cookies": false` first
3. In yt-tui press `L` — should say `login OK via chrome cookies`
4. Set `"use_cookies": true`, restart. `W`/`T` now load Watch Later / Liked, `r` includes private subs.

History never needs login — it's local (`~/.local/share/yt-tui/history.json`).
Subs are just `@handles` in config.json — `S` then `a` to add, no login needed for public channels.

## Player (mpv not the only option)

`~/.config/yt-tui/config.json`:
```json
{ "player": "mpv", "player_args": [], "mpv_pretty": true }
```
- `"mpv"` — lightest, best TUI match. Pretty args (borderless+autofit+slim OSC) on by default.
- `"iina"` — macOS native, prettiest on Mac. `brew install --cask iina`.
- `"vlc"` — familiar GUI, heavier.
- `"browser"` — stock YouTube (ads return).

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
