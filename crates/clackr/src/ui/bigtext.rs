//! A tiny 3x5 block font for the results-screen hero numbers.
//!
//! The reference design renders the headline WPM and accuracy several rows
//! tall. Rather than pull in a figlet-style dependency and its font files, we
//! only ever need digits, `%` and `.` — so a hand-drawn table is smaller,
//! faster, and guaranteed to line up on a character grid.
//!
//! Glyphs use `█` only, so they survive any terminal that can render a full
//! block. Terminals that cannot are already on the `Plain` fidelity tier, which
//! prints the numbers at normal size instead.

use ratatui::style::Style;
use ratatui::text::{Line, Span};

/// Rows per glyph.
pub const HEIGHT: usize = 5;
/// Columns per glyph, before the inter-glyph gap.
pub const WIDTH: usize = 3;
/// Blank columns between glyphs.
const GAP: usize = 1;

const BLANK: [&str; HEIGHT] = ["   ", "   ", "   ", "   ", "   "];

#[rustfmt::skip]
const DIGITS: [[&str; HEIGHT]; 10] = [
    ["███", "█ █", "█ █", "█ █", "███"], // 0
    ["██ ", " █ ", " █ ", " █ ", "███"], // 1
    ["███", "  █", "███", "█  ", "███"], // 2
    ["███", "  █", "███", "  █", "███"], // 3
    ["█ █", "█ █", "███", "  █", "  █"], // 4
    ["███", "█  ", "███", "  █", "███"], // 5
    ["███", "█  ", "███", "█ █", "███"], // 6
    ["███", "  █", "  █", "  █", "  █"], // 7
    ["███", "█ █", "███", "█ █", "███"], // 8
    ["███", "█ █", "███", "  █", "███"], // 9
];

#[rustfmt::skip]
const PERCENT: [&str; HEIGHT] = ["█ █", "  █", " █ ", "█  ", "█ █"];
#[rustfmt::skip]
const PERIOD:  [&str; HEIGHT] = ["   ", "   ", "   ", "   ", " █ "];

fn glyph(c: char) -> [&'static str; HEIGHT] {
    match c {
        '0'..='9' => DIGITS[c as usize - '0' as usize],
        '%' => PERCENT,
        '.' => PERIOD,
        _ => BLANK,
    }
}

/// Display width of `s` when rendered as block text.
pub fn width(s: &str) -> usize {
    match s.chars().count() {
        0 => 0,
        n => n * WIDTH + (n - 1) * GAP,
    }
}

/// Renders `s` as `HEIGHT` styled lines. Characters with no glyph become
/// blanks, so an unexpected input degrades to a gap rather than a panic.
pub fn render(s: &str, style: Style) -> Vec<Line<'static>> {
    let glyphs: Vec<_> = s.chars().map(glyph).collect();
    (0..HEIGHT)
        .map(|row| {
            let mut text = String::with_capacity(width(s));
            for (i, g) in glyphs.iter().enumerate() {
                if i > 0 {
                    text.push(' ');
                }
                text.push_str(g[row]);
            }
            Line::from(Span::styled(text, style))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_the_expected_number_of_rows() {
        assert_eq!(render("61", Style::default()).len(), HEIGHT);
    }

    #[test]
    fn every_row_has_the_same_width() {
        for s in ["0", "61", "100%", "99.5"] {
            let lines = render(s, Style::default());
            let widths: Vec<usize> = lines
                .iter()
                .map(|l| l.to_string().chars().count())
                .collect();
            assert!(
                widths.iter().all(|w| *w == width(s)),
                "{s}: rows {widths:?} but width() says {}",
                width(s)
            );
        }
    }

    #[test]
    fn every_glyph_row_is_exactly_three_columns() {
        // A ragged table would silently skew every number rendered after it.
        for (i, d) in DIGITS.iter().enumerate() {
            for (r, row) in d.iter().enumerate() {
                assert_eq!(row.chars().count(), WIDTH, "digit {i} row {r}");
            }
        }
        for row in PERCENT.iter().chain(PERIOD.iter()).chain(BLANK.iter()) {
            assert_eq!(row.chars().count(), WIDTH);
        }
    }

    #[test]
    fn unknown_characters_render_as_blanks_not_panics() {
        let lines = render("6a1", Style::default());
        assert_eq!(lines.len(), HEIGHT);
        assert_eq!(lines[0].to_string().chars().count(), width("6a1"));
    }

    #[test]
    fn empty_input_has_zero_width() {
        assert_eq!(width(""), 0);
        assert_eq!(render("", Style::default())[0].to_string(), "");
    }
}
