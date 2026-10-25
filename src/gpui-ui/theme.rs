//! Application palette for the GPUI Kit interface.
//!
//! GPUI Kit supplies the component defaults; these colors give the download
//! manager its own look and keep both themes consistent.
use gpui_kit::{Hsla, Rgba};

pub(crate) fn rgb(hex: u32) -> Hsla {
    let r = ((hex >> 16) & 0xff) as u8;
    let g = ((hex >> 8) & 0xff) as u8;
    let b = (hex & 0xff) as u8;
    Rgba { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a: 1.0 }.into()
}

/// Same hex with a different alpha, for tints and subtle fills.
pub(crate) fn alpha(hex: u32, a: f32) -> Hsla {
    let r = ((hex >> 16) & 0xff) as u8;
    let g = ((hex >> 8) & 0xff) as u8;
    let b = (hex & 0xff) as u8;
    Rgba { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a }.into()
}

/// The same color at a different alpha, for pills and tints.
pub(crate) fn with_alpha(c: Hsla, a: f32) -> Hsla {
    gpui_kit::hsla(c.h, c.s, c.l, a)
}

#[derive(Clone, Copy)]
pub(crate) struct Palette {
    pub(crate) dark: bool,
    pub(crate) bg: Hsla,
    pub(crate) panel: Hsla,
    pub(crate) card: Hsla,
    pub(crate) card_hover: Hsla,
    pub(crate) row_hover: Hsla,
    pub(crate) line: Hsla,
    pub(crate) text: Hsla,
    pub(crate) muted: Hsla,
    pub(crate) accent: Hsla,
    pub(crate) accent_dim: Hsla,
    pub(crate) ok: Hsla,
    pub(crate) warn: Hsla,
    pub(crate) bad: Hsla,
    pub(crate) info: Hsla,
    pub(crate) paused: Hsla,
    pub(crate) track: Hsla,
    pub(crate) list_colors: [Hsla; 8],
}

impl Palette {
    pub(crate) fn new(dark: bool) -> Self {
        if dark {
            Self {
                dark,
                bg: rgb(0x0a0b0e),
                panel: rgb(0x0f1116),
                card: rgb(0x14161d),
                card_hover: rgb(0x1b1f29),
                row_hover: rgb(0x232733),
                line: rgb(0x2b3040),
                text: rgb(0xf0f2f8),
                muted: rgb(0x878ea0),
                accent: rgb(0x58a6ff),
                accent_dim: rgb(0x1c3460),
                ok: rgb(0x12ec91),
                warn: rgb(0xffac22),
                bad: rgb(0xe65858),
                info: rgb(0x2ddaff),
                paused: rgb(0xd15cff),
                track: rgb(0x1a1d26),
                list_colors: [
                    rgb(0x0091ff),
                    rgb(0x00e676),
                    rgb(0xffb300),
                    rgb(0xff3b5c),
                    rgb(0xb347ff),
                    rgb(0x00e5ff),
                    rgb(0xff40a0),
                    rgb(0xbeff00),
                ],
            }
        } else {
            Self {
                dark,
                bg: rgb(0xf4f6fb),
                panel: rgb(0xffffff),
                card: rgb(0xffffff),
                card_hover: rgb(0xf7f9fd),
                row_hover: rgb(0xe9eef7),
                line: rgb(0xd8dee9),
                text: rgb(0x14171f),
                muted: rgb(0x606a7d),
                accent: rgb(0x0b66d0),
                accent_dim: rgb(0xdbe8fb),
                ok: rgb(0x0a9d5e),
                warn: rgb(0xb26a00),
                bad: rgb(0xc0392b),
                info: rgb(0x0f7ea6),
                paused: rgb(0x9333ea),
                track: rgb(0xe4e9f2),
                list_colors: [
                    rgb(0x0076d6),
                    rgb(0x0a9d5e),
                    rgb(0xb26a00),
                    rgb(0xc0392b),
                    rgb(0x8b3fd9),
                    rgb(0x0f7ea6),
                    rgb(0xc2185b),
                    rgb(0x6a8f00),
                ],
            }
        }
    }

    pub(crate) fn list_color(&self, i: u8) -> Hsla {
        self.list_colors[(i as usize) % self.list_colors.len()]
    }

    pub(crate) fn status(&self, status: crate::engine::Status) -> Hsla {
        use crate::engine::Status::*;
        match status {
            Connecting => self.accent,
            Downloading => self.info,
            Paused => self.paused,
            Done => self.ok,
            Failed => self.bad,
        }
    }

    /// Tint for a file-type chip, by extension.
    pub(crate) fn file_color(&self, name: &str) -> Hsla {
        let ext = name
            .rsplit_once('.')
            .map(|(_, e)| e.to_ascii_lowercase())
            .unwrap_or_default();
        match ext.as_str() {
            "zip" | "rar" | "7z" | "gz" | "tar" | "iso" | "xz" => self.list_color(2),
            "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "txt" => self.accent,
            "mp3" | "wav" | "flac" | "mp4" | "mkv" | "avi" | "mov" | "webm" => self.list_color(4),
            "exe" | "msi" | "apk" | "dmg" | "deb" => self.ok,
            _ => self.muted,
        }
    }
}
