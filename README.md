# yt-tui — YouTube in the terminal

No browser. No ads. A single **1.2MB** binary idling at **~15MB RAM** (browser YouTube: 500MB+).

![Home feed](docs/home-midnight.png)

## Why

- **Lightweight** — Rust + ratatui, no webview, no Electron. Works over SSH.
- **Full YouTube layout** — Home and Subscriptions pages, search, playlists, history, Watch Later, comments, queue with autoplay.
- **Mouse-first, keyboard-fast** — click everything; `hjkl` + shortcuts for the rest.
- **Pretty** — real image thumbnails on Kitty/Ghostty/WezTerm (ASCII-art fallback everywhere), **11 Omarchy themes** in Settings → Style:

![Tokyo Night theme](docs/home-tokyonight.png)

## Install

Needs `mpv` + `yt-dlp` (`brew install mpv yt-dlp`).

```bash
cargo install --path .   # then run `yt-tui` from anywhere
# or: download the macOS binary from Releases
```

## Use

`yt-tui` — Home loads on launch. `/` searches, `Enter` plays in mpv (ad-free), `q` quits. Press `?` in-app for all keys.

![Settings](docs/settings.png)

**Login (for likes, subscriptions, Watch Later):** Google blocks terminal passwords, so export `cookies.txt` ( extension while on youtube.com) → Settings → Cookies file → Test login. History, search, and public feeds need no login.

![Action menu: like, subscribe, save](docs/actions.png)

Right-click (or `x`) any video for like / subscribe / save / download.

## Config

Everything lives in Settings (gear icon / `,`) and persists to `~/.config/yt-tui/config.json`: player (mpv/iina/vlc), quality, themes, feeds, downloads, SponsorBlock, subtitles.

## Security

No shell calls (argv-only subprocesses), video IDs validated at parse, thumbnails sandboxed to the cache dir, titles rendered as plain text, cookies stay in your file. Details in code (`src/engage.rs`, `src/thumb.rs`).

## Dev

```bash
cargo run --release
cargo test
cargo run --example shots > /tmp/shots.json && python3 docs/rasterize.py /tmp/shots.json docs/
```

MIT.
