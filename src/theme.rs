//! Color schemes. Names taken from the Omarchy theme list
//! (omarchy.org/themes); hex values mapped to the TUI palette slots.
//!
//! Switch live in Settings → Style (click/Enter). Persisted in config.json.

use ratatui::style::Color;

#[derive(Debug, Clone)]
pub struct Theme {
    pub bg: Color,
    pub panel: Color,
    pub accent: Color,
    pub dim: Color,
    pub chip: Color,
    pub sel: Color,
    pub border: Color,
    pub fg: Color,
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

/// Theme names in cycle order. "Midnight" is yt-tui's own default.
pub const NAMES: &[&str] = &[
    "Midnight",
    "Tokyo Night",
    "Catppuccin",
    "Catppuccin Latte",
    "Everforest",
    "Gruvbox",
    "Kanagawa",
    "Nord",
    "Osaka Jade",
    "Rosé Pine",
    "Ristretto",
];

pub fn get(name: &str) -> Theme {
    match name {
        "Tokyo Night" => Theme {
            bg: rgb(0x1a, 0x1b, 0x26),
            panel: rgb(0x16, 0x16, 0x1e),
            accent: rgb(0x7a, 0xa2, 0xf7),
            dim: rgb(0x56, 0x5f, 0x89),
            chip: rgb(0x29, 0x2e, 0x42),
            sel: rgb(0x28, 0x34, 0x57),
            border: rgb(0x3b, 0x42, 0x61),
            fg: Color::White,
        },
        "Catppuccin" => Theme {
            bg: rgb(0x1e, 0x1e, 0x2e),
            panel: rgb(0x18, 0x18, 0x25),
            accent: rgb(0x89, 0xb4, 0xfa),
            dim: rgb(0x6c, 0x70, 0x86),
            chip: rgb(0x31, 0x32, 0x44),
            sel: rgb(0x45, 0x47, 0x5a),
            border: rgb(0x58, 0x5b, 0x70),
            fg: Color::White,
        },
        "Catppuccin Latte" => Theme {
            bg: rgb(0xef, 0xf1, 0xf5),
            panel: rgb(0xe6, 0xe9, 0xef),
            accent: rgb(0x1e, 0x66, 0xf5),
            dim: rgb(0x6c, 0x6f, 0x85),
            chip: rgb(0xcc, 0xd0, 0xda),
            sel: rgb(0xbc, 0xc0, 0xcc),
            border: rgb(0x9c, 0xa0, 0xb0),
            fg: rgb(0x4c, 0x4f, 0x69),
        },
        "Everforest" => Theme {
            bg: rgb(0x2b, 0x33, 0x39),
            panel: rgb(0x22, 0x29, 0x2f),
            accent: rgb(0x7f, 0xbb, 0xb3),
            dim: rgb(0x7a, 0x84, 0x78),
            chip: rgb(0x32, 0x3c, 0x41),
            sel: rgb(0x3a, 0x45, 0x4b),
            border: rgb(0x4a, 0x55, 0x5c),
            fg: rgb(0xd3, 0xc6, 0xaa),
        },
        "Gruvbox" => Theme {
            bg: rgb(0x28, 0x28, 0x28),
            panel: rgb(0x1d, 0x20, 0x21),
            accent: rgb(0x83, 0xa5, 0x98),
            dim: rgb(0x92, 0x83, 0x74),
            chip: rgb(0x3c, 0x38, 0x36),
            sel: rgb(0x50, 0x49, 0x45),
            border: rgb(0x66, 0x5c, 0x54),
            fg: rgb(0xeb, 0xdb, 0xb2),
        },
        "Kanagawa" => Theme {
            bg: rgb(0x1f, 0x1f, 0x28),
            panel: rgb(0x16, 0x16, 0x1d),
            accent: rgb(0x7e, 0x9c, 0xd8),
            dim: rgb(0x72, 0x71, 0x69),
            chip: rgb(0x2a, 0x2a, 0x37),
            sel: rgb(0x36, 0x36, 0x46),
            border: rgb(0x54, 0x54, 0x6d),
            fg: rgb(0xdc, 0xd7, 0xba),
        },
        "Nord" => Theme {
            bg: rgb(0x2e, 0x34, 0x40),
            panel: rgb(0x24, 0x29, 0x33),
            accent: rgb(0x88, 0xc0, 0xd0),
            dim: rgb(0x61, 0x6e, 0x88),
            chip: rgb(0x3b, 0x42, 0x52),
            sel: rgb(0x43, 0x4c, 0x5e),
            border: rgb(0x4c, 0x56, 0x6a),
            fg: rgb(0xec, 0xef, 0xf4),
        },
        "Osaka Jade" => Theme {
            bg: rgb(0x10, 0x20, 0x1b),
            panel: rgb(0x0c, 0x1a, 0x16),
            accent: rgb(0x5e, 0xea, 0xd4),
            dim: rgb(0x5f, 0x7a, 0x70),
            chip: rgb(0x1a, 0x2f, 0x28),
            sel: rgb(0x23, 0x40, 0x37),
            border: rgb(0x2f, 0x52, 0x48),
            fg: rgb(0xd9, 0xe8, 0xe0),
        },
        "Rosé Pine" => Theme {
            bg: rgb(0x19, 0x17, 0x24),
            panel: rgb(0x1f, 0x1d, 0x2e),
            accent: rgb(0x9c, 0xcf, 0xd8),
            dim: rgb(0x6e, 0x6a, 0x86),
            chip: rgb(0x26, 0x23, 0x3a),
            sel: rgb(0x40, 0x3d, 0x52),
            border: rgb(0x52, 0x4f, 0x67),
            fg: rgb(0xe0, 0xde, 0xf4),
        },
        "Ristretto" => Theme {
            bg: rgb(0x1a, 0x15, 0x12),
            panel: rgb(0x14, 0x10, 0x10),
            accent: rgb(0xd4, 0xa3, 0x73),
            dim: rgb(0x8a, 0x7a, 0x68),
            chip: rgb(0x2a, 0x22, 0x20),
            sel: rgb(0x3d, 0x32, 0x30),
            border: rgb(0x57, 0x45, 0x3d),
            fg: rgb(0xec, 0xe0, 0xd1),
        },
        // Midnight: yt-tui default
        _ => Theme {
            bg: rgb(0x0a, 0x0e, 0x16),
            panel: rgb(0x10, 0x16, 0x22),
            accent: rgb(0x50, 0xa0, 0xff),
            dim: rgb(0x82, 0x96, 0xaf),
            chip: rgb(0x1e, 0x28, 0x3a),
            sel: rgb(0x1e, 0x32, 0x55),
            border: rgb(0x2d, 0x46, 0x64),
            fg: Color::White,
        },
    }
}

/// Next theme name after `cur` (wraps). Unknown names restart at default.
pub fn next(cur: &str) -> String {
    let i = NAMES.iter().position(|n| *n == cur).map(|i| i + 1).unwrap_or(0);
    NAMES[i % NAMES.len()].to_string()
}
