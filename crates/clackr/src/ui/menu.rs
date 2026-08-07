//! The menu: pick a mode, its parameter, and a theme.
//!
//! Three rows of horizontal option lists rather than a settings form, because
//! there are only three things to choose and every one of them is a small fixed
//! set. Everything the menu edits is written straight back into the live
//! `Config`, so changing the theme previews it immediately.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use clackr_core::config::Config;
use clackr_core::mode::{QuoteLength, TestMode};
use clackr_core::words;

use crate::term::caps::Fidelity;
use crate::ui::palette::Palette;

pub const TIMES: [u32; 4] = [15, 30, 60, 120];
pub const WORD_COUNTS: [u32; 4] = [10, 25, 50, 100];
pub const QUOTE_LENGTHS: [QuoteLength; 3] =
    [QuoteLength::Short, QuoteLength::Medium, QuoteLength::Long];

/// Which row the selection is on.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Row {
    #[default]
    Mode,
    /// The selected mode's parameter: duration, word count, or quote length.
    Amount,
    Theme,
}

impl Row {
    const ALL: [Row; 3] = [Row::Mode, Row::Amount, Row::Theme];
}

#[derive(Clone, Debug)]
pub struct MenuState {
    pub row: Row,
    mode: usize,
    time: usize,
    words: usize,
    quote: usize,
    theme: usize,
    themes: Vec<&'static str>,
}

impl MenuState {
    /// Seeds the selection from the config so the menu opens on whatever the
    /// user last chose (or asked for on the command line).
    pub fn from_config(config: &Config) -> Self {
        let themes: Vec<&'static str> = words::theme_names().collect();
        Self {
            row: Row::default(),
            mode: index_of(&TestMode::ALL, &config.test.mode),
            time: index_of(&TIMES, &config.test.time),
            words: index_of(&WORD_COUNTS, &config.test.words),
            quote: index_of(&QUOTE_LENGTHS, &config.test.quote),
            theme: themes
                .iter()
                .position(|t| *t == config.ui.theme)
                .unwrap_or(0),
            themes,
        }
    }

    pub fn mode(&self) -> TestMode {
        TestMode::ALL[self.mode]
    }

    /// Moves between rows, skipping the parameter row for zen — zen has no
    /// parameter, so stopping on an empty row would just feel broken.
    pub fn move_row(&mut self, delta: isize) {
        let mut row = self.row;
        for _ in 0..Row::ALL.len() {
            let i = index_of(&Row::ALL, &row) as isize;
            row = Row::ALL[wrap_index(i + delta, Row::ALL.len())];
            if row != Row::Amount || self.mode() != TestMode::Zen {
                break;
            }
        }
        self.row = row;
    }

    /// Moves within the current row. Lists wrap, which is what makes a four-way
    /// choice reachable in at most two presses.
    pub fn move_value(&mut self, delta: isize) {
        let (slot, len) = match self.row {
            Row::Mode => (&mut self.mode, TestMode::ALL.len()),
            Row::Amount => match self.mode() {
                TestMode::Time => (&mut self.time, TIMES.len()),
                TestMode::Words => (&mut self.words, WORD_COUNTS.len()),
                TestMode::Quote => (&mut self.quote, QUOTE_LENGTHS.len()),
                TestMode::Zen => return,
            },
            Row::Theme => {
                if self.themes.is_empty() {
                    return;
                }
                (&mut self.theme, self.themes.len())
            }
        };
        *slot = wrap_index(*slot as isize + delta, len);
    }

    /// Writes the selection back into the config. Called after every keypress,
    /// not just on Enter, so the theme preview is live.
    pub fn apply_to(&self, config: &mut Config) {
        config.test.mode = self.mode();
        config.test.time = TIMES[self.time];
        config.test.words = WORD_COUNTS[self.words];
        config.test.quote = QUOTE_LENGTHS[self.quote];
        if let Some(theme) = self.themes.get(self.theme) {
            config.ui.theme = (*theme).to_string();
        }
    }

    /// The options on one row, and which of them is selected.
    fn options(&self, row: Row) -> (Vec<String>, usize) {
        match row {
            Row::Mode => (
                TestMode::ALL
                    .iter()
                    .map(|m| m.as_str().to_string())
                    .collect(),
                self.mode,
            ),
            Row::Amount => match self.mode() {
                TestMode::Time => (TIMES.iter().map(u32::to_string).collect(), self.time),
                TestMode::Words => (WORD_COUNTS.iter().map(u32::to_string).collect(), self.words),
                TestMode::Quote => (
                    QUOTE_LENGTHS
                        .iter()
                        .map(|q| q.as_str().to_string())
                        .collect(),
                    self.quote,
                ),
                TestMode::Zen => (Vec::new(), 0),
            },
            Row::Theme => (
                self.themes.iter().map(|t| (*t).to_string()).collect(),
                self.theme,
            ),
        }
    }

    fn label(&self, row: Row) -> &'static str {
        match row {
            Row::Mode => "mode",
            Row::Amount => match self.mode() {
                TestMode::Time => "seconds",
                TestMode::Words => "words",
                TestMode::Quote => "length",
                TestMode::Zen => "",
            },
            Row::Theme => "theme",
        }
    }
}

pub fn draw(frame: &mut Frame, area: Rect, state: &MenuState, palette: &Palette, _f: Fidelity) {
    let mut lines = vec![
        Line::from(Span::styled(
            "clackr",
            palette.main.add_modifier(Modifier::BOLD),
        )),
        Line::default(),
    ];
    for row in Row::ALL {
        lines.push(option_row(state, row, palette));
    }
    lines.push(Line::default());
    lines.push(Line::from(vec![
        Span::styled("enter", palette.text),
        Span::styled("  start      ", palette.sub),
        Span::styled("arrows", palette.text),
        Span::styled("  choose      ", palette.sub),
        Span::styled("q", palette.text),
        Span::styled("  quit", palette.sub),
    ]));

    // Left-align inside a centred block: centring each line individually would
    // let the option columns drift out of alignment row to row.
    let width = lines
        .iter()
        .map(Line::width)
        .max()
        .unwrap_or(0)
        .min(area.width as usize) as u16;
    let height = (lines.len() as u16).min(area.height);
    let block = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    };
    frame.render_widget(Paragraph::new(lines), block);
}

/// One row: a cursor, a label, then the options with the selected one lit.
fn option_row<'a>(state: &MenuState, row: Row, palette: &Palette) -> Line<'a> {
    let active = state.row == row;
    let (options, selected) = state.options(row);

    // ASCII marker rather than a glyph: the menu has to read on a Linux VT.
    let mut spans = vec![
        Span::styled(if active { "> " } else { "  " }, palette.main),
        Span::styled(
            format!("{:<9}", state.label(row)),
            style_label(active, palette),
        ),
    ];
    for (i, option) in options.into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", palette.sub));
        }
        spans.push(Span::styled(
            option,
            style_option(i == selected, active, palette),
        ));
    }
    Line::from(spans)
}

fn style_label(active: bool, palette: &Palette) -> Style {
    if active { palette.text } else { palette.sub }
}

fn style_option(selected: bool, active: bool, palette: &Palette) -> Style {
    match (selected, active) {
        (true, true) => palette.main.add_modifier(Modifier::BOLD),
        (true, false) => palette.text,
        (false, _) => palette.sub,
    }
}

fn index_of<T: PartialEq>(haystack: &[T], needle: &T) -> usize {
    haystack.iter().position(|c| c == needle).unwrap_or(0)
}

/// Euclidean remainder, so stepping left off the start lands on the end.
fn wrap_index(i: isize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    i.rem_euclid(len as isize) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_menu_opens_on_whatever_the_config_says() {
        let mut config = Config::default();
        config.test.mode = TestMode::Words;
        config.test.words = 50;
        let state = MenuState::from_config(&config);
        assert_eq!(state.mode(), TestMode::Words);
        let (options, selected) = state.options(Row::Amount);
        assert_eq!(options[selected], "50");
    }

    #[test]
    fn moving_left_off_the_start_wraps_to_the_end() {
        let mut state = MenuState::from_config(&Config::default());
        state.move_value(-1);
        assert_eq!(state.mode(), TestMode::Zen);
        state.move_value(1);
        assert_eq!(state.mode(), TestMode::Time);
    }

    #[test]
    fn zen_has_no_parameter_row_to_land_on() {
        let mut config = Config::default();
        config.test.mode = TestMode::Zen;
        let mut state = MenuState::from_config(&config);
        state.move_row(1);
        assert_eq!(state.row, Row::Theme);
        state.move_row(-1);
        assert_eq!(state.row, Row::Mode);
    }

    #[test]
    fn selections_are_written_back_into_the_config() {
        let mut config = Config::default();
        let mut state = MenuState::from_config(&config);
        state.row = Row::Amount;
        state.move_value(1);
        state.apply_to(&mut config);
        assert_eq!(config.test.mode, TestMode::Time);
        assert_eq!(config.test.time, TIMES[2]);
    }

    #[test]
    fn the_theme_row_only_ever_offers_themes_that_exist() {
        let config = Config::default();
        let mut state = MenuState::from_config(&config);
        state.row = Row::Theme;
        for _ in 0..12 {
            state.move_value(1);
            let mut applied = config.clone();
            state.apply_to(&mut applied);
            assert!(
                words::theme(&applied.ui.theme).is_some(),
                "menu offered an unknown theme: {}",
                applied.ui.theme
            );
        }
    }
}
