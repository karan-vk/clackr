//! Turns a theme plus the terminal's capabilities into concrete styles.
//!
//! Themes are authored in 24-bit colour. Most terminals can show that; some can
//! only manage 256 or 16 colours, and some none at all. Rather than letting
//! ratatui pick a fallback per cell, we resolve once here so the whole UI
//! degrades consistently — and at zero colours we switch to carrying meaning
//! through text attributes instead, which is the difference between "hard to
//! read" and "impossible to use".

use ratatui::style::{Color, Modifier, Style};

use clackr_core::theme::{Rgb, Theme};

use crate::term::caps::{Caps, ColorDepth};

#[derive(Clone, Debug)]
pub struct Palette {
    pub bg: Style,
    /// Accent: hero numbers, the caret, the WPM series.
    pub main: Style,
    /// De-emphasised chrome: labels, axes, untyped text, the raw series.
    pub sub: Style,
    pub sub_alt: Style,
    /// Correctly typed characters.
    pub text: Style,
    pub error: Style,
    pub error_extra: Style,
    /// Characters skipped by pressing space early.
    pub missed: Style,
}

impl Palette {
    pub fn resolve(theme: &Theme, caps: &Caps) -> Self {
        if caps.color == ColorDepth::None {
            return Self::attributes_only();
        }
        let c = |rgb: Rgb| Style::default().fg(convert(rgb, caps.color));
        Self {
            bg: Style::default().bg(convert(theme.bg, caps.color)),
            main: c(theme.main),
            sub: c(theme.sub),
            sub_alt: c(theme.sub_alt),
            text: c(theme.text),
            error: c(theme.error),
            error_extra: c(theme.error_extra),
            missed: c(theme.error).add_modifier(Modifier::UNDERLINED),
        }
    }

    /// The no-colour palette. Every distinction the coloured version makes is
    /// preserved, just carried by an attribute instead of a hue.
    fn attributes_only() -> Self {
        let plain = Style::default();
        Self {
            bg: plain,
            main: plain.add_modifier(Modifier::BOLD),
            sub: plain.add_modifier(Modifier::DIM),
            sub_alt: plain.add_modifier(Modifier::DIM),
            text: plain,
            error: plain.add_modifier(Modifier::REVERSED),
            error_extra: plain.add_modifier(Modifier::UNDERLINED),
            missed: plain.add_modifier(Modifier::UNDERLINED | Modifier::DIM),
        }
    }
}

fn convert(rgb: Rgb, depth: ColorDepth) -> Color {
    match depth {
        ColorDepth::TrueColor => Color::Rgb(rgb.r, rgb.g, rgb.b),
        ColorDepth::Ansi256 => Color::Indexed(to_ansi256(rgb)),
        ColorDepth::Ansi16 | ColorDepth::None => to_ansi16(rgb),
    }
}

/// Maps into the xterm-256 cube, preferring the 24-step greyscale ramp for
/// near-grey colours — most terminal themes are mostly greys, and the 6x6x6
/// cube quantises those badly.
fn to_ansi256(rgb: Rgb) -> u8 {
    let (r, g, b) = (rgb.r as i32, rgb.g as i32, rgb.b as i32);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);

    if max - min < 16 {
        // Greyscale ramp: indices 232..=255 run from #080808 to #eeeeee.
        let level = ((r + g + b) / 3 - 8).clamp(0, 238);
        return 232 + (level * 23 / 238) as u8;
    }

    let q = |v: i32| ((v - 35).max(0) * 5 / 220) as u8; // 6 levels: 0,95,135,175,215,255
    16 + 36 * q(r) + 6 * q(g) + q(b)
}

/// Nearest of the 16 ANSI colours, matching on *hue first* rather than on
/// straight RGB distance.
///
/// Euclidean RGB distance gives the wrong answer for exactly the colours that
/// matter here. The reference theme's error red `#ca4754` is muted, and its
/// closest ANSI neighbour by RGB distance is `DarkGray` (10661) rather than
/// `LightRed` (14906) — so a 16-colour terminal would render mistyped
/// characters in grey, silently destroying the one distinction the typing test
/// exists to show. Redmean weighting does not fix it either; the green-channel
/// term still favours the grey.
///
/// So: decide achromatic-vs-chromatic by saturation, then stay inside that
/// family. A saturated colour keeps its hue and only picks between the normal
/// and bright variant by brightness; a near-grey picks along the grey ramp.
/// Preserving the hue is what keeps "red means wrong" true at every colour
/// depth.
fn to_ansi16(rgb: Rgb) -> Color {
    /// Below this saturation a colour reads as grey, not as a hue.
    const ACHROMATIC: f64 = 0.15;
    /// Above this value the bright variant is the better match.
    const BRIGHT: f64 = 0.6;

    let (r, g, b) = (
        rgb.r as f64 / 255.0,
        rgb.g as f64 / 255.0,
        rgb.b as f64 / 255.0,
    );
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let saturation = if max == 0.0 { 0.0 } else { delta / max };

    if saturation < ACHROMATIC {
        // Greys, by brightness along the ramp.
        return [
            (0.0, Color::Black),
            (0.502, Color::DarkGray),
            (0.753, Color::Gray),
            (1.0, Color::White),
        ]
        .into_iter()
        .min_by(|(a, _), (c, _)| (a - max).abs().total_cmp(&(c - max).abs()))
        .map(|(_, c)| c)
        .unwrap_or(Color::Gray);
    }

    // Hue in degrees.
    let hue = 60.0
        * if max == r {
            ((g - b) / delta).rem_euclid(6.0)
        } else if max == g {
            (b - r) / delta + 2.0
        } else {
            (r - g) / delta + 4.0
        };

    let (dim, bright) = [
        (0.0, (Color::Red, Color::LightRed)),
        (60.0, (Color::Yellow, Color::LightYellow)),
        (120.0, (Color::Green, Color::LightGreen)),
        (180.0, (Color::Cyan, Color::LightCyan)),
        (240.0, (Color::Blue, Color::LightBlue)),
        (300.0, (Color::Magenta, Color::LightMagenta)),
    ]
    .into_iter()
    .min_by(|(a, _), (c, _)| {
        // Hue is circular: 354 degrees is 6 away from red, not 354.
        let dist = |h: f64| {
            let d = (h - hue).abs();
            d.min(360.0 - d)
        };
        dist(*a).total_cmp(&dist(*c))
    })
    .map(|(_, pair)| pair)
    .unwrap_or((Color::White, Color::White));

    if max >= BRIGHT { bright } else { dim }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps(color: ColorDepth) -> Caps {
        Caps {
            color,
            unicode: true,
            braille: true,
            width: 120,
            height: 40,
        }
    }

    #[test]
    fn truecolor_passes_the_theme_through_unchanged() {
        let theme = Theme::default();
        let p = Palette::resolve(&theme, &caps(ColorDepth::TrueColor));
        assert_eq!(p.main.fg, Some(Color::Rgb(0xe2, 0xb7, 0x14)));
    }

    #[test]
    fn no_color_still_distinguishes_every_semantic_role() {
        let p = Palette::resolve(&Theme::default(), &caps(ColorDepth::None));
        // Nothing may carry a foreground colour...
        for s in [p.main, p.sub, p.text, p.error, p.error_extra, p.missed] {
            assert_eq!(s.fg, None);
        }
        // ...and correct/incorrect/extra must still be told apart.
        assert_ne!(p.text, p.error);
        assert_ne!(p.error, p.error_extra);
        assert_ne!(p.text, p.sub);
    }

    #[test]
    fn greys_land_on_the_greyscale_ramp_not_the_color_cube() {
        // #323437 is the reference background: nearly neutral, and quantising
        // it into the 6x6x6 cube would visibly tint it.
        let idx = to_ansi256(Rgb::new(0x32, 0x34, 0x37));
        assert!((232..=255).contains(&idx), "got {idx}");
    }

    #[test]
    fn saturated_colors_land_in_the_color_cube() {
        let idx = to_ansi256(Rgb::new(0xe2, 0xb7, 0x14));
        assert!((16..232).contains(&idx), "got {idx}");
    }

    #[test]
    fn ansi16_keeps_the_hue_of_saturated_colors() {
        // Every one of these is a muted theme colour whose nearest neighbour by
        // plain RGB distance is a grey. Keeping the hue is the whole point: a
        // typing test whose error colour turns grey has stopped working.
        assert_eq!(to_ansi16(Rgb::new(0xca, 0x47, 0x54)), Color::LightRed);
        assert_eq!(to_ansi16(Rgb::new(0xe2, 0xb7, 0x14)), Color::LightYellow);
        assert_eq!(to_ansi16(Rgb::new(0x7e, 0x2a, 0x33)), Color::Red);
    }

    #[test]
    fn ansi16_sends_near_greys_along_the_grey_ramp() {
        assert_eq!(to_ansi16(Rgb::new(0x32, 0x34, 0x37)), Color::Black);
        assert_eq!(to_ansi16(Rgb::new(0x64, 0x66, 0x69)), Color::DarkGray);
        assert_eq!(to_ansi16(Rgb::new(0xd1, 0xd0, 0xc5)), Color::Gray);
        assert_eq!(to_ansi16(Rgb::new(0, 0, 0)), Color::Black);
        assert_eq!(to_ansi16(Rgb::new(255, 255, 255)), Color::White);
    }

    #[test]
    fn ansi16_covers_every_hue_family() {
        for (rgb, want) in [
            (Rgb::new(255, 0, 0), Color::LightRed),
            (Rgb::new(255, 255, 0), Color::LightYellow),
            (Rgb::new(0, 255, 0), Color::LightGreen),
            (Rgb::new(0, 255, 255), Color::LightCyan),
            (Rgb::new(0, 0, 255), Color::LightBlue),
            (Rgb::new(255, 0, 255), Color::LightMagenta),
            (Rgb::new(128, 0, 0), Color::Red),
            (Rgb::new(0, 0, 128), Color::Blue),
        ] {
            assert_eq!(to_ansi16(rgb), want, "{rgb}");
        }
    }

    #[test]
    fn the_reference_theme_stays_legible_at_every_depth() {
        // The three roles that carry meaning during a test must never collide,
        // at any colour depth. This is the property the tests above exist to
        // protect, stated directly.
        let theme = Theme::default();
        for depth in [
            ColorDepth::TrueColor,
            ColorDepth::Ansi256,
            ColorDepth::Ansi16,
        ] {
            let p = Palette::resolve(&theme, &caps(depth));
            assert_ne!(p.text.fg, p.error.fg, "{depth:?}");
            assert_ne!(p.text.fg, p.sub.fg, "{depth:?}");
            assert_ne!(p.error.fg, p.sub.fg, "{depth:?}");
        }
    }
}
