//! Terminal capability detection, and the fidelity tier it selects.
//!
//! clackr's full results screen wants truecolour, braille, and roughly 90x24.
//! Plenty of terminals have none of that — a Linux VT console, a CI log, an
//! 80x24 SSH session, `NO_COLOR` in the environment. Rather than looking broken
//! there, the UI drops to a simpler rendering of the same information.

use clackr_core::config::FidelityPref;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum ColorDepth {
    /// No colour at all: meaning is carried by DIM/REVERSED/UNDERLINE instead.
    None,
    #[default]
    Ansi16,
    Ansi256,
    TrueColor,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Caps {
    pub color: ColorDepth,
    pub unicode: bool,
    pub braille: bool,
    pub width: u16,
    pub height: u16,
}

/// How much of the full design we can actually draw.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Fidelity {
    /// Bare ASCII, no colour, minimal layout.
    Plain,
    /// Half-block chart, fewer columns, no vertical axis titles.
    Reduced,
    /// Braille chart, block digits, the full results layout.
    #[default]
    Full,
}

impl Caps {
    /// Reads the environment. `size` comes from the terminal and is refreshed
    /// on every resize event, so this is called again whenever the window
    /// changes.
    pub fn detect(width: u16, height: u16) -> Self {
        let env = |k: &str| std::env::var(k).unwrap_or_default();
        let term = env("TERM");

        // NO_COLOR is honoured for any value, per the no-color.org convention:
        // presence is the signal, not the content.
        let color = if std::env::var_os("NO_COLOR").is_some() || term == "dumb" || term.is_empty() {
            ColorDepth::None
        } else {
            let colorterm = env("COLORTERM");
            if colorterm.contains("truecolor") || colorterm.contains("24bit") {
                ColorDepth::TrueColor
            } else if term.contains("256color") {
                ColorDepth::Ansi256
            } else {
                ColorDepth::Ansi16
            }
        };

        let locale = format!("{} {} {}", env("LC_ALL"), env("LC_CTYPE"), env("LANG"));
        let unicode = locale.to_ascii_lowercase().contains("utf-8")
            || locale.to_ascii_lowercase().contains("utf8");

        // The Linux VT console font has no braille block, so braille there
        // renders as a wall of replacement glyphs even though the locale is
        // UTF-8.
        let braille = unicode && term != "linux";

        Self {
            color,
            unicode,
            braille,
            width,
            height,
        }
    }

    /// Applies the user's preference over the detected tier.
    pub fn fidelity(&self, pref: FidelityPref) -> Fidelity {
        match pref {
            FidelityPref::Full => Fidelity::Full,
            FidelityPref::Reduced => Fidelity::Reduced,
            FidelityPref::Plain => Fidelity::Plain,
            FidelityPref::Auto => self.auto_fidelity(),
        }
    }

    fn auto_fidelity(&self) -> Fidelity {
        if !self.unicode || self.color == ColorDepth::None || self.width < 60 || self.height < 16 {
            Fidelity::Plain
        } else if !self.braille
            || self.color <= ColorDepth::Ansi16
            || self.width < 90
            || self.height < 24
        {
            Fidelity::Reduced
        } else {
            Fidelity::Full
        }
    }

    /// Force the lowest tier regardless of what the terminal can do.
    pub fn downgrade_to_ascii(&mut self) {
        self.unicode = false;
        self.braille = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps(color: ColorDepth, braille: bool, w: u16, h: u16) -> Caps {
        Caps {
            color,
            unicode: true,
            braille,
            width: w,
            height: h,
        }
    }

    #[test]
    fn roomy_truecolor_terminal_gets_full_fidelity() {
        let c = caps(ColorDepth::TrueColor, true, 120, 40);
        assert_eq!(c.fidelity(FidelityPref::Auto), Fidelity::Full);
    }

    #[test]
    fn missing_braille_drops_to_reduced_even_when_large() {
        let c = caps(ColorDepth::TrueColor, false, 200, 60);
        assert_eq!(c.fidelity(FidelityPref::Auto), Fidelity::Reduced);
    }

    #[test]
    fn tiny_terminal_drops_to_plain() {
        let c = caps(ColorDepth::TrueColor, true, 59, 40);
        assert_eq!(c.fidelity(FidelityPref::Auto), Fidelity::Plain);
    }

    #[test]
    fn no_color_drops_to_plain_regardless_of_size() {
        let c = caps(ColorDepth::None, true, 200, 60);
        assert_eq!(c.fidelity(FidelityPref::Auto), Fidelity::Plain);
    }

    #[test]
    fn explicit_preference_overrides_detection() {
        // A user on a 40x12 terminal who asks for Full gets Full, and gets to
        // deal with the consequences themselves.
        let c = caps(ColorDepth::None, false, 40, 12);
        assert_eq!(c.fidelity(FidelityPref::Full), Fidelity::Full);
    }
}
