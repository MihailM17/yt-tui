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

- Mouse-first: click search, sidebar, thumbs (again = play), chips, transport ⏸⏭, gear ⚙, overlay ✕, settings rows; wheel scrolls; hover highlights
- `0/s/y` views • `u` login • `w/t` later/liked • `r` refresh (auto on launch) • `+` more
- `⚙`/`,` Settings: player, quality, thumbs, browser, cookies, feeds — click/Enter, persists
- `i` info+chapters • `c` comments • `f` sort (relevance/views/longest/shortest)
- `a/Q/P` queue add / view / play-all (autoplay) • `d/D` download video / mp3
- `x`/right-click action menu: ♥ like, remove rating, 👎 dislike, save to Watch Later, subscribe (needs cookies.txt)
- `v` quality best→720p→480p→audio • `[/]` mpv speed • `n` new-upload check • `?` help
- Subs view: `Enter` load channel • `a` add @handle • `d` remove • `r` refresh all
- History view: `Enter` replay • `D` clear (local only)
- Home: `hjkl`/arrows, `Enter`/`p` play, `o` browser, `/` live-search, `r` feed, `m` mock
- `q` or `Ctrl-C` quit

## Login (no Google password in TUI — Google blocks it)

There is no username/password login for terminals (Settings → Test login / Import subs): Google requires OAuth +
bot-checks/2FA, and `yt-dlp` removed password auth years ago. The supported
path is cookies — the TUI never sees your password.

**If `L` failed, it's almost always one of these (you're not dumb):**
- Chrome was open → it locks the cookie DB, read fails
- macOS keychain blocked the read (popup denied / no prompt)
- Wrong `"browser"` name in config (you use Brave but config says chrome)

**Reliable path (do this):**

1. Install "Get cookies.txt LOCALLY" extension (Chrome/Firefox), open youtube.com logged in, Export → save as `~/.config/yt-tui/cookies.txt`
2. In `~/.config/yt-tui/config.json` set `"cookies_file": "~/.config/yt-tui/cookies.txt"`
3. In yt-tui press `u` — should say `login OK via cookies file`
4. Set `"use_cookies": true`, restart. `w`/`t` load Watch Later / Liked.

**Quick path (flaky):** quit your browser completely, set `"browser": "chrome"` (or brave/firefox — whichever holds the login), press `L`.

Real Google OAuth (Cloud project + API key + quota) is possible but heavy —
10k units/day, verification, client secrets. Say the word and I'll add it as
`yt-tui login --oauth`, but cookies cover Watch Later/Liked/subs today.

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
- Search/feed via installed `yt-dlp -J` in background threads (all channels in parallel, no API key). Last feed cached 15min for instant startup. Thumbs warmed in background (render path is disk-only, never blocks).
- Thumbnails (`thumb_quality` in config: default/mq/hq/sd, default mq 320x180 ~10KB): jpg → aspect-fit → half-block ANSI → drop image:
  - RAM: LRU 30 entries, <500KB
  - Disk: `~/.cache/yt-tui/thumbs/`, 20MB LRU cap, auto-pruned
- History: `~/.local/share/yt-tui/history.json`, capped 100. Config: `~/.config/yt-tui/config.json`.
- Redraw on input, 10fps poll, only visible 9 cards rendered.
- Release: `opt-level=z, lto, strip` → ~1.2MB binary (kitty/sixel included), ~15-30MB idle RAM.

## Roadmap

- [x] mockup UI (sidebar/chips/grid/ASCII thumbs)
- [x] live search + subs feed via yt-dlp, real thumbs via curl+jpeg, disk cache 20MB
- [x] `yt-dlp -g` + `mpv` playback (+browser fallback), history
- [x] queue+autoplay, downloads, sort, new-check, quality/speed
- [x] settings menu, real image thumbs (kitty/sixel, blocks fallback), import subs from account
- [x] engagement: like/dislike/remove-rating, subscribe/unsubscribe, save to Watch Later (Innertube, no new deps)
- [ ] SponsorBlock skip
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
