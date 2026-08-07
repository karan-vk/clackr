//! The word display: three lines of target text, coloured per character.

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::widgets::StatefulWidget;

use clackr_core::{CellState, Engine};

use crate::ui::palette::Palette;
use crate::ui::test::wrap::{WrapCache, cell_width};

/// What the widget reports back to the caller.
///
/// Only the caret, and only because the caret is the terminal's own cursor:
/// `Frame::set_cursor_position` has to be called on the frame, not on the
/// buffer, so the position has to travel back out of `render`. Using the real
/// cursor is what buys us a blinking caret with no blink timer, and therefore
/// an event loop that ticks at 1 Hz instead of 30.
#[derive(Clone, Debug, Default)]
pub struct WordsState {
    pub caret: Option<Position>,
}

pub struct WordsWidget<'a> {
    engine: &'a Engine,
    wrap: &'a WrapCache,
    palette: &'a Palette,
    /// Blind mode: no correctness feedback at all while typing.
    blind: bool,
    /// False when the user turned the caret off.
    caret: bool,
}

impl<'a> WordsWidget<'a> {
    pub fn new(engine: &'a Engine, wrap: &'a WrapCache, palette: &'a Palette) -> Self {
        Self {
            engine,
            wrap,
            palette,
            blind: engine.params().blind_mode,
            caret: true,
        }
    }

    pub fn caret(mut self, caret: bool) -> Self {
        self.caret = caret;
        self
    }

    fn cell_style(&self, state: CellState) -> Style {
        if self.blind {
            // Blind mode's entire point is that you cannot tell yet, so an
            // incorrect character has to look exactly like a correct one.
            return match state {
                CellState::Untyped | CellState::Missed => self.palette.sub,
                _ => self.palette.text,
            };
        }
        match state {
            CellState::Untyped => self.palette.sub,
            CellState::Correct => self.palette.text,
            CellState::Incorrect => self.palette.error,
            CellState::Extra => self.palette.error_extra,
            CellState::Missed => self.palette.missed,
        }
    }
}

impl StatefulWidget for WordsWidget<'_> {
    type State = WordsState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut WordsState) {
        state.caret = None;
        if area.is_empty() {
            return;
        }

        let active = self.engine.word_index();
        let cursor = self.engine.cursor_in_word();
        let lines = self.wrap.lines();
        // One line of context above the caret, so finishing a line scrolls the
        // display up by exactly one row and the active line stays in the middle.
        let top = self.wrap.line_of(active).saturating_sub(1);

        for (row, line) in lines
            .iter()
            .skip(top)
            .take(area.height as usize)
            .enumerate()
        {
            let y = area.y + row as u16;
            let mut x = area.x;

            for w in line.clone() {
                let Some(view) = self.engine.word(w) else {
                    continue;
                };
                for i in 0..view.len() {
                    if self.caret && w == active && i == cursor {
                        state.caret = Some(Position::new(x, y));
                    }
                    let text = view.text(i);
                    let width = cell_width(text);
                    if x + width > area.right() {
                        break;
                    }
                    buf.set_string(x, y, text, self.cell_style(view.state(i)));
                    x += width;
                }
                // The caret sits past the last cell once the word is full —
                // either finished or overflowing into extras.
                if self.caret && w == active && cursor >= view.len() && x < area.right() {
                    state.caret = Some(Position::new(x, y));
                }
                // The inter-word space. Nothing is written into it: leaving the
                // cell blank means the background style shows through.
                x += 1;
                if x >= area.right() {
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use ratatui::layout::Rect;

    use clackr_core::Action;
    use clackr_core::mode::TestParams;
    use clackr_core::theme::Theme;

    use super::*;
    use crate::term::caps::{Caps, ColorDepth};

    const AREA: Rect = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 3,
    };

    fn palette() -> Palette {
        Palette::resolve(
            &Theme::default(),
            &Caps {
                color: ColorDepth::TrueColor,
                unicode: true,
                braille: true,
                width: 40,
                height: 3,
            },
        )
    }

    /// "the cat", with `the` mistyped as `txe`.
    fn typed(blind: bool) -> (Engine, WrapCache) {
        let mut engine = Engine::new(TestParams {
            blind_mode: blind,
            ..TestParams::default()
        });
        engine.append_words(["the".to_string(), "cat".to_string()]);
        for c in ['t', 'x'] {
            engine.apply(Action::Type(c), Duration::ZERO);
        }
        let mut wrap = WrapCache::new(AREA.width);
        wrap.reset(&engine);
        wrap.sync();
        (engine, wrap)
    }

    fn render(blind: bool, caret: bool) -> (Buffer, WordsState) {
        let (engine, wrap) = typed(blind);
        let palette = palette();
        let mut buffer = Buffer::empty(AREA);
        let mut state = WordsState::default();
        WordsWidget::new(&engine, &wrap, &palette)
            .caret(caret)
            .render(AREA, &mut buffer, &mut state);
        (buffer, state)
    }

    #[test]
    fn a_wrong_character_is_drawn_in_the_error_colour() {
        let (buffer, _) = render(false, true);
        assert_eq!(buffer[(1u16, 0u16)].symbol(), "x");
        assert_eq!(buffer[(1u16, 0u16)].fg, palette().error.fg.unwrap());
        // The untyped remainder is still dimmed.
        assert_eq!(buffer[(2u16, 0u16)].fg, palette().sub.fg.unwrap());
    }

    #[test]
    fn blind_mode_makes_a_mistake_look_like_a_correct_character() {
        let (buffer, _) = render(true, true);
        assert_eq!(buffer[(0u16, 0u16)].fg, palette().text.fg.unwrap());
        assert_eq!(buffer[(1u16, 0u16)].fg, palette().text.fg.unwrap());
    }

    #[test]
    fn the_caret_lands_on_the_next_cell_to_type() {
        let (_, state) = render(false, true);
        assert_eq!(state.caret, Some(Position::new(2, 0)));
    }

    #[test]
    fn no_caret_is_reported_when_it_is_switched_off() {
        let (_, state) = render(false, false);
        assert_eq!(state.caret, None);
    }
}
