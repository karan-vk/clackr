//! The live test screen: a header of live figures, the word display, and a
//! one-line hint bar.

pub mod widget;
pub mod wrap;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use clackr_core::config::{CaretStyle, Config, QuickRestart};
use clackr_core::mode::TestMode;
use clackr_core::stats::accuracy;

use crate::app::TestState;
use crate::term::caps::Fidelity;
use crate::ui::palette::Palette;
use crate::ui::test::widget::{WordsState, WordsWidget};

/// Text never runs wider than this, however wide the terminal is. Long lines
/// are hard to read and harder to type from — the eye loses its place on the
/// return sweep.
pub const MAX_TEXT_WIDTH: u16 = 100;

/// Columns kept clear either side of the word display.
const SIDE_PADDING: u16 = 2;

/// The column budget the word display gets, given the full terminal width.
///
/// `app` needs this to keep the wrap cache in step with the layout without
/// rendering a frame, which is why it is a free function rather than something
/// derived inside `draw`.
pub fn text_width(total: u16) -> u16 {
    total.saturating_sub(SIDE_PADDING * 2).min(MAX_TEXT_WIDTH)
}

/// How many lines of words to show. Three is the design; a terminal that cannot
/// spare the rows, or is already rendering a reduced layout, gets fewer.
pub fn visible_lines(fidelity: Fidelity, height: u16) -> u16 {
    let wanted = match fidelity {
        Fidelity::Full => 3,
        Fidelity::Reduced => 2,
        Fidelity::Plain => 1,
    };
    // The header and hint bar own one row each.
    wanted.min(height.saturating_sub(2)).max(1)
}

pub fn draw(
    frame: &mut Frame,
    area: Rect,
    test: &TestState,
    config: &Config,
    palette: &Palette,
    fidelity: Fidelity,
) {
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);

    // The width comes from the wrap cache, not from the layout: rendering wider
    // than the packing assumed would put words past the end of their line.
    let width = test.wrap.width().min(area.width);
    let x = area.x + area.width.saturating_sub(width) / 2;
    let rows = visible_lines(fidelity, area.height).min(body.height);
    let text = Rect {
        x,
        y: body.y + body.height.saturating_sub(rows) / 2,
        width,
        height: rows,
    };

    // The header and hint bar share the text column, so the whole screen reads
    // as one block rather than three unrelated ones.
    let strip = |row: Rect| Rect { x, width, ..row };
    draw_header(frame, strip(header), test, config, palette);

    let mut state = WordsState::default();
    frame.render_stateful_widget(
        WordsWidget::new(&test.engine, &test.wrap, palette)
            .caret(config.ui.caret != CaretStyle::Off),
        text,
        &mut state,
    );
    if let Some(caret) = state.caret {
        frame.set_cursor_position(caret);
    }

    frame.render_widget(Paragraph::new(hint(test, config, palette)), strip(footer));
}

/// The live readouts. Each is a figure in the accent colour with a quiet label,
/// which keeps the eye off the header and on the words.
fn draw_header(
    frame: &mut Frame,
    area: Rect,
    test: &TestState,
    config: &Config,
    palette: &Palette,
) {
    let mut left: Vec<Span> = Vec::new();
    if config.ui.show_timer {
        left.push(Span::styled(progress(test), palette.main));
    }

    let mut right: Vec<Span> = Vec::new();
    if config.ui.show_live_wpm {
        right.push(Span::styled(
            format!("{:.0}", test.engine.live_wpm(test.elapsed)),
            palette.main,
        ));
        right.push(Span::styled(" wpm", palette.sub));
    }
    if config.ui.show_live_acc {
        if !right.is_empty() {
            right.push(Span::styled("   ", palette.sub));
        }
        let acc = accuracy(test.engine.keys_correct(), test.engine.keys_incorrect());
        right.push(Span::styled(format!("{acc:.0}"), palette.main));
        right.push(Span::styled("% acc", palette.sub));
    }

    let right_width = right
        .iter()
        .map(|s| s.content.chars().count())
        .sum::<usize>() as u16;
    let [left_area, right_area] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(right_width)]).areas(area);

    frame.render_widget(Paragraph::new(Line::from(left)), left_area);
    frame.render_widget(Paragraph::new(Line::from(right)), right_area);
}

/// The countdown, or how far through the word list we are.
fn progress(test: &TestState) -> String {
    let engine = &test.engine;
    match engine.params().mode {
        TestMode::Time => {
            let left = engine.time_left(test.elapsed).unwrap_or_default();
            // Ceiling, so a test starts by showing its full duration rather
            // than blinking to one second less the instant it begins.
            format!("{}", left.as_millis().div_ceil(1000))
        }
        TestMode::Words | TestMode::Quote => format!(
            "{}/{}",
            engine.word_index().min(engine.word_count()),
            engine.word_count()
        ),
        // Zen has nothing to count down to, so it counts up instead.
        TestMode::Zen => {
            let secs = test.elapsed.as_secs();
            format!("{}:{:02}", secs / 60, secs % 60)
        }
    }
}

fn hint<'a>(test: &TestState, config: &Config, palette: &'a Palette) -> Line<'a> {
    if test.restart_armed {
        return Line::styled("enter  restart      any key  cancel", palette.main);
    }
    // `esc` means different things depending on the quick-restart setting, and
    // the hint bar is the only place the user finds that out.
    let keys: &[(&str, &str)] = if test.engine.params().mode == TestMode::Zen {
        &[("esc", "finish"), ("enter", "finish")]
    } else {
        match config.keys.quick_restart {
            QuickRestart::Tab => &[("esc", "menu"), ("tab", "restart")],
            QuickRestart::Off => &[("esc", "menu"), ("tab", "arm restart")],
            QuickRestart::Esc => &[("esc", "restart"), ("tab", "menu")],
        }
    };

    let mut spans = Vec::new();
    for (key, what) in keys {
        if !spans.is_empty() {
            spans.push(Span::styled("      ", palette.sub));
        }
        spans.push(Span::styled(*key, palette.text));
        spans.push(Span::styled(format!("  {what}"), palette.sub));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_word_display_shrinks_with_the_fidelity_tier() {
        assert_eq!(visible_lines(Fidelity::Full, 40), 3);
        assert_eq!(visible_lines(Fidelity::Reduced, 40), 2);
        assert_eq!(visible_lines(Fidelity::Plain, 40), 1);
    }

    #[test]
    fn a_terminal_too_short_for_three_lines_still_gets_one() {
        // Two rows are always spent on the header and the hint bar, so the word
        // display gets whatever is left — but never nothing.
        assert_eq!(visible_lines(Fidelity::Full, 4), 2);
        for height in 0..=3 {
            assert_eq!(visible_lines(Fidelity::Full, height), 1, "height {height}");
        }
    }

    #[test]
    fn the_text_column_is_padded_and_capped() {
        assert_eq!(text_width(400), MAX_TEXT_WIDTH);
        assert_eq!(text_width(80), 76);
        // A terminal narrower than the padding leaves nothing rather than
        // wrapping around into a huge number.
        assert_eq!(text_width(3), 0);
    }
}
