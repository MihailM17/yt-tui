#!/usr/bin/env python3
"""Rasterize yt-tui TestBackend dumps to PNGs.

Usage: python3 docs/rasterize.py /tmp/shots.json docs/
Fonts: JetBrains Mono -> Apple Symbols -> Apple Color Emoji (per-char).
"""
import json
import os
import sys

from fontTools.ttLib import TTFont
from PIL import Image, ImageDraw, ImageFont

HOME = os.path.expanduser("~")
FONTS = [
    (os.path.join(HOME, "Library/Fonts/JetBrainsMono-Regular.ttf"), False),
    (os.path.join(HOME, "Library/Fonts/JetBrainsMono-Bold.ttf"), True),
    ("/System/Library/Fonts/Apple Symbols.ttf", False),
    ("/System/Library/Fonts/Apple Color Emoji.ttc", False),
]
SIZE = 17

# Shots-only glyph swaps: color-emoji has no FreeType-rasterizable font here,
# so map the 7 emoji to covered glyphs (app itself is untouched).
EMOJI_FIX = {"⏸": "‖", "⏭": "»", "🎵": "♪", "🔑": "", "🎙": "", "🔔": "", "👎": ""}


def coverage(path):
    cov = set()
    for table in TTFont(path, lazy=True)["cmap"].tables:
        cov.update(table.cmap.keys())
    return cov


def load_font(path, size):
    try:
        return ImageFont.truetype(path, size)
    except OSError:
        pass
    for index in (1, 2, 3):  # .ttc subfonts (emoji lives past index 0)
        try:
            return ImageFont.truetype(path, size, index=index)
        except OSError:
            continue
    return None


def main():
    src, outdir = sys.argv[1], sys.argv[2]
    os.makedirs(outdir, exist_ok=True)
    data = json.load(open(src))
    loaded = []
    for p, bold in FONTS:
        f = load_font(p, SIZE)
        if f is None:
            print("skip font:", p)
            continue
        loaded.append((f, coverage(p), bold))
    assert loaded, "no fonts loaded"
    base = loaded[0][0]
    box = base.getbbox("M")
    cw, ch = box[2] - box[0] + 2, box[3] - box[1] + 6

    def font_for(char, want_bold):
        cp = ord(char)
        for font, cov, is_bold in loaded:
            if is_bold != want_bold:
                continue
            if cp in cov:
                return font
        for font, cov, _ in loaded:  # any weight, then tofu
            if cp in cov:
                return font
        return base

    for name, shot in data.items():
        w, h = shot["w"], shot["h"]
        img = Image.new("RGB", (w * cw, h * ch), (10, 14, 22))
        d = ImageDraw.Draw(img)
        for y in range(h):
            for x in range(w):
                sym, fr, fg, fb, br, bg, bb, b = shot["cells"][y * w + x]
                d.rectangle(
                    [x * cw, y * ch, (x + 1) * cw, (y + 1) * ch],
                    fill=(br, bg, bb),
                )
                if sym.strip():
                    ch1 = EMOJI_FIX.get(sym[0], sym[0])
                    if not ch1.strip():
                        continue
                    d.text(
                        (x * cw + 1, y * ch + 1),
                        ch1,
                        font=font_for(ch1, bool(b)),
                        fill=(fr, fg, fb),
                    )
        path = os.path.join(outdir, name + ".png")
        img.save(path)
        print("wrote", path)


if __name__ == "__main__":
    main()
