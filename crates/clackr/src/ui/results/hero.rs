//! The hero block: `wpm` and `acc` at headline size.
//!
//! This is the one thing a user actually looks at, so it is the last thing to
//! degrade — the chart and the stat row give up their space to it first.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use clackr_core::Summary;

use crate::term::caps::Fidelity;
use crate::ui::bigtext;
use crate::ui::palette::Palette;

/// Widest thing the block font ever has to draw here: `100%`.
const WIDEST: &str = "100%";

/// Rows a label-plus-glyph-block pair needs.
const BLOCK_ROWS: u16 = 1 + bigtext::HEIGHT as u16;

/// Columns to reserve for the hero, by tier.
pub fn width(fidelity: Fidelity) -> u16 {
    match fidelity {
        // `wpm 61` / `acc 100%` plus a column of breathing room.
        Fidelity::Plain => 10,
        _ => bigtext::width(WIDEST) as u16 + 1,
    }
}

pub fn draw(
    frame: &mut Frame,
    area: Rect,
    summary: &Summary,
    palette: &Palette,
    fidelity: Fidelity,
) {
    if area.is_empty() {
        return;
    }

    // MonkeyType shows the headline numbers rounded; the exact values live in
    // the stat row underneath.
    let wpm = format!("{:.0}", summary.wpm);
    let acc = format!("{:.0}%", summary.acc);

    let wide_enough = fidelity > Fidelity::Plain
        && area.width >= bigtext::width(&acc).max(bigtext::width(&wpm)) as u16;

    // Three tiers, because a clipped glyph block is worse than no glyph block.
    // The middle tier is what an 11-row hero row gets: WPM is the number people
    // came for, so it keeps the block font and accuracy drops to text.
    let mut lines = Vec::with_capacity(2 * BLOCK_ROWS as usize);
    if wide_enough && area.height >= 2 * BLOCK_ROWS {
        lines.push(Line::styled("wpm", palette.sub));
        lines.extend(bigtext::render(&wpm, palette.main));
        lines.push(Line::styled("acc", palette.sub));
        lines.extend(bigtext::render(&acc, palette.main));
    } else if wide_enough && area.height > BLOCK_ROWS {
        lines.push(Line::styled("wpm", palette.sub));
        lines.extend(bigtext::render(&wpm, palette.main));
        lines.push(labelled("acc", &acc, palette));
    } else {
        lines.push(labelled("wpm", &wpm, palette));
        lines.push(labelled("acc", &acc, palette));
    }

    frame.render_widget(Paragraph::new(lines), area);
}

fn labelled(label: &'static str, value: &str, palette: &Palette) -> Line<'static> {
    Line::from(vec![
        Span::styled(label, palette.sub),
        Span::styled(" ", palette.sub),
        Span::styled(value.to_string(), palette.main),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::term::caps::Fidelity;
    use crate::ui::results::tests::render;

    #[test]
    fn full_fidelity_uses_the_block_font() {
        let text = render(120, 40, &Summary::fixture(), Fidelity::Full);
        // "61" in the block font is two rows of solid block on the top line.
        assert!(text.contains("███ ███"), "{text}");
    }

    #[test]
    fn plain_fidelity_prints_the_numbers_at_normal_size() {
        let text = render(120, 40, &Summary::fixture(), Fidelity::Plain);
        assert!(text.contains("wpm 61"), "{text}");
        assert!(text.contains("acc 100%"), "{text}");
        assert!(!text.contains('█'), "{text}");
    }

    #[test]
    fn a_short_area_keeps_the_headline_and_demotes_accuracy() {
        // Half a glyph block is worse than no glyph block, so accuracy drops to
        // text before WPM does.
        let text = render(120, 14, &Summary::fixture(), Fidelity::Full);
        assert!(text.contains("███ ███"), "{text}");
        assert!(text.contains("acc 100%"), "{text}");
    }

    #[test]
    fn a_very_short_area_falls_back_to_two_text_lines() {
        let text = render(120, 11, &Summary::fixture(), Fidelity::Full);
        assert!(text.contains("wpm 61"), "{text}");
        assert!(text.contains("acc 100%"), "{text}");
    }

    #[test]
    fn the_reserved_width_fits_the_widest_number_it_will_ever_draw() {
        assert!(bigtext::width(WIDEST) as u16 <= width(Fidelity::Full));
        assert!(bigtext::width("999") as u16 <= width(Fidelity::Full));
    }
}
