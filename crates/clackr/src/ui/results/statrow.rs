//! The bottom stat row: five `label` / `value` columns under the hero.
//!
//! Order matches the reference: test type, raw, characters, consistency, time.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use clackr_core::Summary;

use crate::term::caps::Fidelity;
use crate::ui::palette::Palette;

/// One cell of the row: a label, its value, and an optional second value line
/// (only `test type` uses it, for the language).
struct Stat {
    label: &'static str,
    value: String,
    detail: Option<String>,
}

/// Rows the stat row needs, by tier.
pub fn rows(fidelity: Fidelity) -> u16 {
    match fidelity {
        // Two `key: value` lines.
        Fidelity::Plain => 2,
        // Two stacked half-rows at `Reduced`, one label+value+detail at `Full`.
        _ => 4,
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
    let stats = stats(summary);

    match layout_for(fidelity, area.width) {
        Fidelity::Full => {
            let cells: [Rect; 5] = Layout::horizontal([Constraint::Ratio(1, 5); 5]).areas(area);
            for (rect, stat) in cells.into_iter().zip(&stats) {
                frame.render_widget(Paragraph::new(column(stat, palette, true)), rect);
            }
        }
        Fidelity::Reduced => {
            // The row wraps into two half-height bands of three. Both bands use
            // thirds, including the one that only has two entries, so the
            // columns line up vertically instead of straddling each other.
            let [upper, lower] =
                Layout::vertical([Constraint::Length(2), Constraint::Length(2)]).areas(area);
            let top: [Rect; 3] = Layout::horizontal([Constraint::Ratio(1, 3); 3]).areas(upper);
            let bottom: [Rect; 3] = Layout::horizontal([Constraint::Ratio(1, 3); 3]).areas(lower);
            for (rect, stat) in top.into_iter().chain(bottom).zip(&stats) {
                frame.render_widget(Paragraph::new(column(stat, palette, false)), rect);
            }
        }
        Fidelity::Plain => {
            let line = |picks: &[usize]| {
                let mut spans = Vec::new();
                for (i, &ix) in picks.iter().enumerate() {
                    if i > 0 {
                        spans.push(Span::styled("   ", palette.sub));
                    }
                    spans.push(Span::styled(format!("{}: ", stats[ix].label), palette.sub));
                    spans.push(Span::styled(stats[ix].value.clone(), palette.text));
                }
                Line::from(spans)
            };
            frame.render_widget(Paragraph::new(vec![line(&[0, 1, 4]), line(&[2, 3])]), area);
        }
    }
}

/// Columns are also chosen by how much room there actually is: `consistency` is
/// eleven characters, so five columns need about sixty and three need about
/// thirty-six. Below that, truncated headings ("test typ", "consiste") look
/// broken, and two `key: value` lines say the same thing honestly.
///
/// This only ever degrades, so the result always fits the rows [`rows`]
/// reserved for the tier.
fn layout_for(fidelity: Fidelity, width: u16) -> Fidelity {
    match fidelity {
        Fidelity::Full if width >= 60 => Fidelity::Full,
        Fidelity::Plain => Fidelity::Plain,
        _ if width >= 36 => Fidelity::Reduced,
        _ => Fidelity::Plain,
    }
}

fn column(stat: &Stat, palette: &Palette, with_detail: bool) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::styled(stat.label, palette.sub),
        Line::styled(stat.value.clone(), palette.text),
    ];
    if with_detail && let Some(detail) = &stat.detail {
        lines.push(Line::styled(detail.clone(), palette.text));
    }
    lines
}

fn stats(summary: &Summary) -> [Stat; 5] {
    let (correct, incorrect, extra, missed) = summary.characters();
    [
        Stat {
            label: "test type",
            value: summary.params.label(),
            detail: Some(summary.params.language.clone()),
        },
        Stat {
            label: "raw",
            value: format!("{:.0}", summary.raw),
            detail: None,
        },
        Stat {
            label: "characters",
            value: format!("{correct}/{incorrect}/{extra}/{missed}"),
            detail: None,
        },
        Stat {
            label: "consistency",
            value: format!("{:.0}%", summary.consistency),
            detail: None,
        },
        Stat {
            label: "time",
            value: format!("{}s", summary.duration.as_secs()),
            detail: None,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::ui::results::tests::render;

    #[test]
    fn full_fidelity_shows_all_five_labels_and_the_language() {
        let text = render(120, 40, &Summary::fixture(), Fidelity::Full);
        for label in ["test type", "raw", "characters", "consistency", "time"] {
            assert!(text.contains(label), "missing {label} in\n{text}");
        }
        assert!(text.contains("english1k"), "{text}");
        assert!(text.contains("42/0/0/0"), "{text}");
    }

    #[test]
    fn reduced_fidelity_drops_the_language_but_keeps_every_stat() {
        let text = render(100, 30, &Summary::fixture(), Fidelity::Reduced);
        for label in ["test type", "raw", "characters", "consistency", "time"] {
            assert!(text.contains(label), "missing {label} in\n{text}");
        }
        assert!(!text.contains("english1k"), "{text}");
    }

    #[test]
    fn plain_fidelity_is_two_key_value_lines() {
        let text = render(120, 40, &Summary::fixture(), Fidelity::Plain);
        assert!(text.contains("test type: time 30"), "{text}");
        assert!(text.contains("characters: 42/0/0/0"), "{text}");
    }

    #[test]
    fn a_narrow_terminal_drops_columns_rather_than_truncating_headings() {
        // "consiste" and "characte" read as corruption, not as a stat row.
        let text = render(40, 12, &Summary::fixture(), Fidelity::Full);
        assert!(!text.contains("consiste\n"), "{text}");
        assert!(text.contains("consistency"), "{text}");
    }

    #[test]
    fn the_chosen_layout_never_needs_more_rows_than_the_tier_reserved() {
        for fidelity in [Fidelity::Plain, Fidelity::Reduced, Fidelity::Full] {
            for width in 0..=200 {
                assert!(rows(layout_for(fidelity, width)) <= rows(fidelity));
            }
        }
    }
}
