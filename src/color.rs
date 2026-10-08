//! sRGB/linear conversion and the DE's hex colour strings — the pure half of
//! `cce_ui::color`, which re-exports every item here.

pub fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Parse a hex color string into raw RGBA bytes.
///
/// Accepts `#RRGGBB` or `#RRGGBBAA`, tolerating surrounding quotes/whitespace and
/// an optional leading `#`. 6-digit input yields alpha `255`. Returns `None` for
/// any shorter/invalid input. This is the primitive the `[f32;_]` parsers build on.
pub fn parse_hex_bytes(s: &str) -> Option<[u8; 4]> {
    let hex = s
        .trim_matches(|c| c == '"' || c == '\'' || c == ' ')
        .trim_start_matches('#');
    let b = |i: usize| u8::from_str_radix(hex.get(i * 2..i * 2 + 2)?, 16).ok();
    if hex.len() >= 8 {
        Some([b(0)?, b(1)?, b(2)?, b(3)?])
    } else if hex.len() >= 6 {
        Some([b(0)?, b(1)?, b(2)?, 255])
    } else {
        None
    }
}

/// Parse a hex color string into raw sRGB RGBA in `[0,1]` (no gamma conversion).
/// See [`parse_hex_bytes`] for the accepted formats. Apply [`srgb_to_linear`] to
/// the RGB channels yourself if your render target expects linear color.
pub fn parse_hex_rgba(s: &str) -> Option<[f32; 4]> {
    parse_hex_bytes(s).map(|[r, g, b, a]| {
        [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a as f32 / 255.0]
    })
}

/// Like [`parse_hex_rgba`] but drops alpha, returning raw sRGB RGB in `[0,1]`.
pub fn parse_hex_rgb(s: &str) -> Option<[f32; 3]> {
    parse_hex_rgba(s).map(|[r, g, b, _]| [r, g, b])
}

/// Like [`parse_hex_rgba`] but converts RGB from sRGB to linear (alpha kept as-is).
/// Use when your render target samples colors in linear space.
pub fn parse_hex_rgba_linear(s: &str) -> Option<[f32; 4]> {
    parse_hex_rgba(s).map(|[r, g, b, a]| {
        [srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b), a]
    })
}

pub fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}
