//! Theme as pure data. Turning these into terminal styles (including
//! downsampling for 256- and 16-colour terminals) is the binary's job.

use serde::{Deserialize, Serialize};

/// A 24-bit colour.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Relative luminance, used to decide light/dark contrast fallbacks.
    pub fn luminance(self) -> f64 {
        let f = |c: u8| {
            let c = c as f64 / 255.0;
            if c <= 0.039_28 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(self.r) + 0.7152 * f(self.g) + 0.0722 * f(self.b)
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("invalid hex colour {0:?}: expected #rgb or #rrggbb")]
pub struct ParseRgbError(String);

impl std::str::FromStr for Rgb {
    type Err = ParseRgbError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let hex = s.strip_prefix('#').unwrap_or(s);
        let err = || ParseRgbError(s.to_string());
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| err());
        match hex.len() {
            // #rgb is shorthand for #rrggbb
            3 => {
                let nib = |i: usize| {
                    u8::from_str_radix(&hex[i..i + 1], 16)
                        .map(|n| n * 17)
                        .map_err(|_| err())
                };
                Ok(Self::new(nib(0)?, nib(1)?, nib(2)?))
            }
            6 => Ok(Self::new(byte(0)?, byte(2)?, byte(4)?)),
            _ => Err(err()),
        }
    }
}

impl TryFrom<String> for Rgb {
    type Error = ParseRgbError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<Rgb> for String {
    fn from(c: Rgb) -> Self {
        format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
    }
}

impl std::fmt::Display for Rgb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

/// The seven colours every clackr theme defines. The names follow the
/// vocabulary typing-test themes generally use, so ported palettes are easy to
/// read: `main` is the accent, `sub` is de-emphasised chrome, `text` is
/// correctly typed characters.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub bg: Rgb,
    /// Accent: hero numbers, caret, the WPM series.
    pub main: Rgb,
    /// Untyped text, axis labels, the raw-WPM series.
    pub sub: Rgb,
    /// Slightly lifted background for panels and inactive chrome.
    pub sub_alt: Rgb,
    /// Correctly typed characters.
    pub text: Rgb,
    pub error: Rgb,
    /// Characters typed past the end of a word.
    pub error_extra: Rgb,
}

impl Default for Theme {
    /// `amber-dark`, the palette used by the reference results screen.
    fn default() -> Self {
        Self {
            name: "amber dark".to_string(),
            bg: Rgb::new(0x32, 0x34, 0x37),
            main: Rgb::new(0xe2, 0xb7, 0x14),
            sub: Rgb::new(0x64, 0x66, 0x69),
            sub_alt: Rgb::new(0x2c, 0x2e, 0x31),
            text: Rgb::new(0xd1, 0xd0, 0xc5),
            error: Rgb::new(0xca, 0x47, 0x54),
            error_extra: Rgb::new(0x7e, 0x2a, 0x33),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_both_hex_forms() {
        assert_eq!(
            "#e2b714".parse::<Rgb>().unwrap(),
            Rgb::new(0xe2, 0xb7, 0x14)
        );
        assert_eq!("e2b714".parse::<Rgb>().unwrap(), Rgb::new(0xe2, 0xb7, 0x14));
        assert_eq!("#fff".parse::<Rgb>().unwrap(), Rgb::new(255, 255, 255));
    }

    #[test]
    fn rejects_malformed_hex() {
        for s in ["#gg0011", "#12345", "", "#"] {
            assert!(s.parse::<Rgb>().is_err(), "{s:?} should not parse");
        }
    }

    #[test]
    fn round_trips_through_string() {
        let c = Rgb::new(0x0a, 0xb1, 0xff);
        assert_eq!(String::from(c).parse::<Rgb>().unwrap(), c);
    }
}
