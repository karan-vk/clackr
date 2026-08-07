//! Rendering. Every widget here is a pure function of `App`, which is what
//! lets the whole UI be snapshot-tested through ratatui's `TestBackend` without
//! a real terminal — important because CI runners have no TTY.

pub mod bigtext;
pub mod menu;
pub mod palette;
pub mod results;
pub mod test;

use ratatui::Frame;

use clackr_core::words;

use crate::app::{App, Screen};
use crate::ui::palette::Palette;

pub fn draw(frame: &mut Frame, app: &App) {
    // An unknown theme name is not worth an error dialog: the config may have
    // been written by a newer clackr, or simply contain a typo.
    let theme = words::theme(&app.config.ui.theme).unwrap_or_default();
    let palette = Palette::resolve(&theme, &app.caps);
    let fidelity = app.caps.fidelity(app.config.ui.fidelity);

    let area = frame.area();
    // Paint the theme's background once, underneath everything. Widgets then
    // only ever set foreground styles.
    frame.buffer_mut().set_style(area, palette.bg);

    match &app.screen {
        Screen::Menu(state) => menu::draw(frame, area, state, &palette, fidelity),
        Screen::Test(state) => test::draw(frame, area, state, &app.config, &palette, fidelity),
        Screen::Results(summary) => results::draw(frame, area, summary, &palette, fidelity),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use clackr_core::Config;
    use clackr_core::mode::TestMode;

    use super::*;
    use crate::app::App;
    use crate::term::caps::{Caps, ColorDepth};
    use crate::term::input::Msg;

    /// A seed chosen once and pinned: the snapshots below are only stable
    /// because word generation is.
    const SEED: u64 = 20_260_807;

    fn caps(width: u16, height: u16) -> Caps {
        Caps {
            color: ColorDepth::TrueColor,
            unicode: true,
            braille: true,
            width,
            height,
        }
    }

    /// Renders an app at `width x height` and returns the buffer as plain text.
    ///
    /// `TestBackend`'s own `Display` quotes every row and appends a diff note,
    /// which makes for noisy snapshots; the symbols alone read like a
    /// screenshot.
    fn render(app: &App, width: u16, height: u16) -> String {
        let (text, _) = render_with_caret(app, width, height);
        text
    }

    fn render_with_caret(app: &App, width: u16, height: u16) -> (String, (u16, u16)) {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        let buffer = terminal.backend().buffer();
        let text = (0..buffer.area.height)
            .map(|y| {
                let row: String = (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect();
                row.trim_end().to_string()
            })
            .collect::<Vec<_>>()
            .join("\n");
        // The backend's cursor starts at the origin and only moves if the frame
        // asked for it, so (0, 0) means "no caret was placed".
        let caret = terminal.get_cursor_position().unwrap();
        (text, (caret.x, caret.y))
    }

    fn menu_app(width: u16, height: u16) -> App {
        App::new(Config::default(), caps(width, height)).with_seed(SEED)
    }

    /// A test screen with a few words typed, one of them wrong, so the snapshot
    /// covers every cell state the widget can draw.
    fn typed_app(mode: TestMode, width: u16, height: u16) -> App {
        let mut config = Config::default();
        config.test.mode = mode;
        config.test.words = 25;
        let mut app = App::new(config, caps(width, height)).with_seed(SEED);
        press(&mut app, KeyCode::Enter);
        // A wrong character, a skipped one and an extra, so the snapshot pins
        // every cell state the widget can produce.
        for c in "the quick brwn foxx ".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        app
    }

    fn press(app: &mut App, code: KeyCode) {
        app.handle(Msg::Key(
            KeyEvent::new(code, KeyModifiers::NONE),
            Instant::now(),
        ));
    }

    #[test]
    fn menu_snapshots() {
        for (w, h) in [(120u16, 40u16), (100, 30), (80, 24)] {
            insta::assert_snapshot!(format!("menu_{w}x{h}"), render(&menu_app(w, h), w, h));
        }
    }

    #[test]
    fn test_screen_snapshots() {
        for (w, h) in [(120u16, 40u16), (100, 30), (80, 24)] {
            let app = typed_app(TestMode::Words, w, h);
            insta::assert_snapshot!(format!("test_{w}x{h}"), render(&app, w, h));
        }
    }

    /// The text of the word under the caret.
    fn active_word(app: &App) -> String {
        match &app.screen {
            Screen::Test(test) => {
                let view = test.engine.word(test.engine.word_index()).unwrap();
                (0..view.len()).map(|i| view.text(i)).collect()
            }
            _ => unreachable!("expected the test screen"),
        }
    }

    /// The rendered word display, without the header and hint bar.
    fn word_rows(rendered: &str) -> Vec<String> {
        let lines: Vec<&str> = rendered.lines().collect();
        lines[1..lines.len() - 1]
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.trim().to_string())
            .collect()
    }

    #[test]
    fn the_word_display_scrolls_by_exactly_one_line() {
        // Finishing a line must move the text up by one row, not by a screenful:
        // the active line ends up in the middle and the line you just typed
        // stays visible above it.
        let mut app = App::new(Config::default(), caps(120, 40)).with_seed(SEED);
        press(&mut app, KeyCode::Enter);

        let before = word_rows(&render(&app, 120, 40));
        let second_line = before[1].clone();

        // Submit every word on the first two lines, so the caret lands on the
        // third and the window has to scroll once.
        let words = match &app.screen {
            Screen::Test(test) => test.wrap.lines()[..2]
                .iter()
                .map(|l| l.len())
                .sum::<usize>(),
            _ => unreachable!(),
        };
        // Typed exactly right, so the rendered text is unchanged and the only
        // thing that can have moved is the window.
        for _ in 0..words {
            for c in active_word(&app).chars() {
                press(&mut app, KeyCode::Char(c));
            }
            press(&mut app, KeyCode::Char(' '));
        }

        let after = word_rows(&render(&app, 120, 40));
        assert_eq!(
            after[0], second_line,
            "the second line should have scrolled up into first place"
        );
    }

    #[test]
    fn time_mode_shows_a_countdown_rather_than_a_word_count() {
        let app = typed_app(TestMode::Time, 120, 40);
        let rendered = render(&app, 120, 40);
        let header = rendered.lines().next().unwrap().trim_start();
        // Nothing has been ticked yet, so the countdown still reads the full
        // duration rather than one second less.
        assert!(header.starts_with("30"), "{header:?}");
        assert!(header.contains("wpm"), "{header:?}");
    }

    #[test]
    fn words_mode_counts_words_instead() {
        let app = typed_app(TestMode::Words, 120, 40);
        let header = render(&app, 120, 40)
            .lines()
            .next()
            .unwrap()
            .trim_start()
            .to_string();
        assert!(header.starts_with("4/25"), "{header:?}");
    }

    #[test]
    fn the_caret_sits_on_the_next_character_to_type() {
        // The caret is the terminal's own cursor, so this is the only way to
        // observe it: there is nothing in the buffer to look at.
        let app = typed_app(TestMode::Words, 120, 40);
        let (text, caret) = render_with_caret(&app, 120, 40);
        assert_ne!(caret, (0, 0), "no caret was placed");
        let row = text.lines().nth(caret.1 as usize).unwrap();
        assert!(
            row.chars().nth(caret.0 as usize).is_some(),
            "caret at {caret:?} is off the end of {row:?}"
        );
    }

    #[test]
    fn the_caret_can_be_turned_off() {
        let mut app = typed_app(TestMode::Words, 120, 40);
        app.config.ui.caret = clackr_core::config::CaretStyle::Off;
        assert_eq!(render_with_caret(&app, 120, 40).1, (0, 0));
    }

    #[test]
    fn every_mode_renders_at_every_size_without_panicking() {
        // The small sizes are the interesting ones: they push the layout into
        // one-line and zero-width territory, where an off-by-one turns into a
        // panic inside ratatui rather than a wrong pixel.
        for mode in TestMode::ALL {
            for (w, h) in [(120u16, 40u16), (80, 24), (60, 16), (40, 12), (20, 5)] {
                let app = typed_app(mode, w, h);
                render(&app, w, h);
            }
        }
    }

    #[test]
    fn an_unknown_theme_falls_back_instead_of_panicking() {
        let mut config = Config::default();
        config.ui.theme = "no-such-theme".to_string();
        let app = App::new(config, caps(80, 24)).with_seed(SEED);
        // Rendering at all is the assertion; `Palette::resolve` would otherwise
        // never be reached.
        assert!(!render(&app, 80, 24).is_empty());
    }
}
