//! The results screen: the big `wpm`/`acc` hero, the WPM/raw/errors chart, and
//! the bottom stat row.
//!
//! This is a deliberate reproduction of MonkeyType's results view, so the
//! layout constants below are design decisions rather than arbitrary numbers.
//! Everything here is a pure function of [`Summary`] plus the terminal's
//! [`Fidelity`], which is what lets the whole screen be snapshot-tested through
//! `TestBackend` without a TTY.

mod chart;
mod hero;
mod statrow;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use clackr_core::Summary;

use crate::term::caps::Fidelity;
use crate::ui::palette::Palette;

/// Rows the stat row occupies at `Full`/`Reduced`.
const STAT_ROWS: u16 = 4;
/// The blank separator row the design leaves under the stat row.
const FOOTER_ROWS: u16 = 1;
/// The keybinding hint pinned to the bottom of the screen.
const HINT_ROWS: u16 = 1;

/// Upper bound on the hero/chart row.
///
/// The plan pins this row at 11, which is what it works out to in a 16-row
/// area — but two 5-row glyph blocks plus their labels need 12, and the
/// vertical `Words per Minute` axis title needs a 16-row plot (20 rows once the
/// border and the x-label rows are taken out). So the row takes whatever
/// vertical space is going spare up to that ceiling, and anything beyond it is
/// left as bottom padding rather than stretching the chart into a shape the
/// reference never has.
const TOP_ROWS_MAX: u16 = 20;

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

    // At `Plain` the hero is two text lines and the chart is one, so the tall
    // row the other tiers want would just be a screenful of blank.
    let ceiling = match fidelity {
        Fidelity::Plain => 2,
        _ => TOP_ROWS_MAX,
    };
    // The hint is pinned to the very bottom, out of the centring calculation —
    // without it there is nothing on screen telling you how to run another test.
    let hint_rows = HINT_ROWS.min(area.height);
    let [body, hint] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(hint_rows)]).areas(area);

    let top_rows = body
        .height
        .saturating_sub(STAT_ROWS + FOOTER_ROWS)
        .min(ceiling);

    // Centre the block vertically rather than pinning it to the top. The chart
    // has a ceiling, so on a tall terminal a top-aligned layout leaves a screen
    // of dead space under the stats and the result reads as half-drawn.
    let content = top_rows + statrow::rows(fidelity) + FOOTER_ROWS;
    let pad_top = body.height.saturating_sub(content) / 2;

    let [_pad, top, stats, _footer, _rest] = Layout::vertical([
        Constraint::Length(pad_top),
        Constraint::Length(top_rows),
        Constraint::Length(statrow::rows(fidelity)),
        Constraint::Length(FOOTER_ROWS),
        Constraint::Min(0),
    ])
    .areas(body);

    let hero_width = hero::width(fidelity);
    // The chart is the first thing to go when space runs out: a plot squeezed
    // into twenty columns is noise, whereas the hero numbers *are* the result.
    let chart_fits = top.width.saturating_sub(hero_width) >= chart::MIN_WIDTH;

    if chart_fits && chart::visible(area, fidelity) {
        let [hero_area, chart_area] =
            Layout::horizontal([Constraint::Length(hero_width), Constraint::Min(0)]).areas(top);
        hero::draw(frame, hero_area, summary, palette, fidelity);
        chart::draw(frame, chart_area, summary, palette, fidelity);
    } else {
        hero::draw(frame, top, summary, palette, fidelity);
    }

    statrow::draw(frame, stats, summary, palette, fidelity);
    draw_hint(frame, hint, palette);
}

/// The keybinding hint. Two key/label pairs, keys in `text` and labels in `sub`
/// so the keys read first.
fn draw_hint(frame: &mut Frame, area: Rect, palette: &Palette) {
    if area.is_empty() {
        return;
    }
    let line = Line::from(vec![
        Span::styled("tab", palette.text),
        Span::styled("  next test", palette.sub),
        Span::styled("    esc", palette.text),
        Span::styled("  menu", palette.sub),
    ]);
    frame.render_widget(Paragraph::new(line).centered(), area);
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use crate::term::caps::{Caps, ColorDepth};

    /// `chart` is private to this module, so the fixture it owns is re-exported
    /// here for the snapshot harness.
    pub(crate) use super::chart::tests::fixture_with_errors;

    fn palette() -> Palette {
        Palette::resolve(
            &clackr_core::Theme::default(),
            &Caps {
                color: ColorDepth::TrueColor,
                unicode: true,
                braille: true,
                width: 120,
                height: 40,
            },
        )
    }

    /// Renders the screen into a throwaway `TestBackend` and returns the buffer
    /// as text. Panics and buffer overflows surface here rather than in CI.
    pub(crate) fn render(width: u16, height: u16, summary: &Summary, fidelity: Fidelity) -> String {
        let palette = palette();
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| draw(frame, frame.area(), summary, &palette, fidelity))
            .unwrap();
        let buf = terminal.backend().buffer();
        let mut out = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            // Trailing blanks carry no information and make the snapshots
            // needlessly brittle against a one-column shift in padding.
            while out.ends_with(' ') {
                out.pop();
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn every_size_from_tiny_to_huge_renders_without_panicking() {
        // Small-terminal panics are the classic TUI bug, and ratatui's `Chart`
        // carries debug assertions that fire on a degenerate plot area, so this
        // sweep is exhaustive rather than sampled. The fixture with errors is
        // the one used because it exercises the extra gutter and the
        // hand-placed labels as well.
        let summary = fixture_with_errors();
        for height in 8..=60u16 {
            for width in 20..=200u16 {
                for fidelity in [Fidelity::Plain, Fidelity::Reduced, Fidelity::Full] {
                    let text = render(width, height, &summary, fidelity);
                    assert_eq!(
                        text.lines().count(),
                        usize::from(height),
                        "{width}x{height} {fidelity:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_zero_sized_area_draws_nothing_at_all() {
        let palette = palette();
        let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
        terminal
            .draw(|frame| {
                draw(
                    frame,
                    Rect::new(0, 0, 0, 0),
                    &Summary::fixture(),
                    &palette,
                    Fidelity::Full,
                );
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        assert!(buf.content().iter().all(|c| c.symbol() == " "));
    }

    #[test]
    fn the_hero_survives_when_the_chart_is_dropped() {
        // A narrow terminal must still show the headline number; losing the
        // chart is fine, losing the result is not.
        let text = render(30, 24, &Summary::fixture(), Fidelity::Full);
        assert!(text.contains("wpm"), "{text}");
        assert!(text.contains('█'), "{text}");
    }

    #[test]
    fn the_stat_row_never_lands_on_the_hero() {
        // Two things are being pinned here. First, the stat row must sit
        // directly under the hero/chart block: if the trailing filler ever
        // regressed, ratatui's legacy flex would stretch the last `Length` and
        // drag the stat row down the screen. Second, the block must be
        // vertically centred — a top-aligned layout leaves a screenful of dead
        // space under the stats on a tall terminal and reads as half-drawn.
        let (w, h) = (120u16, 40u16);
        let text = render(w, h, &Summary::fixture(), Fidelity::Full);
        let row = text
            .lines()
            .position(|l| l.starts_with("test type"))
            .expect("stat row");

        let body = h - HINT_ROWS;
        let content = TOP_ROWS_MAX + STAT_ROWS + FOOTER_ROWS;
        let pad_top = (body - content) / 2;
        assert_eq!(row, usize::from(pad_top + TOP_ROWS_MAX), "{text}");

        // Blank-row symmetry is not a usable measure here, because the stat
        // block reserves a row it does not always ink. So state the defect
        // directly: there must be padding above, and the content must reach
        // past the vertical midpoint rather than huddling in the top half.
        let rows: Vec<&str> = text.lines().take(usize::from(body)).collect();
        let first_ink = rows.iter().position(|l| !l.trim().is_empty()).unwrap();
        let last_ink = rows.iter().rposition(|l| !l.trim().is_empty()).unwrap();
        assert!(first_ink > 0, "content is flush with the top\n{text}");
        assert!(
            last_ink > usize::from(body / 2),
            "content stops at row {last_ink} of {body} — the bottom half is dead space\n{text}"
        );
    }

    #[test]
    fn the_hint_is_pinned_to_the_bottom_row() {
        // Without this the results screen gives the user no way to discover how
        // to start another test.
        for (w, h) in [(120u16, 40u16), (80, 24), (60, 16)] {
            let text = render(w, h, &Summary::fixture(), Fidelity::Full);
            let last = text.lines().nth(usize::from(h) - 1).unwrap_or_default();
            assert!(last.contains("tab"), "{w}x{h}: {last:?}");
            assert!(last.contains("esc"), "{w}x{h}: {last:?}");
        }
    }
}
