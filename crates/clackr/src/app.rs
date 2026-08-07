//! Application state and the main loop.
//!
//! The loop is deadline-driven rather than tick-driven. Outside a running test
//! there is no deadline at all, so the process blocks in `recv()` at zero CPU;
//! during a test the only deadline is the next one-second boundary, which is
//! all the live WPM readout and the per-second chart buckets need. The caret
//! does not need a timer because it is the terminal's own cursor, which the
//! terminal blinks for us.

use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use color_eyre::eyre::Result;
use rand::SeedableRng;
use rand::rngs::StdRng;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use clackr_core::config::QuickRestart;
use clackr_core::engine::WORD_LOOKAHEAD;
use clackr_core::mode::{TestMode, TestParams};
use clackr_core::words::{self, RandomWords, WordSource, quotes};
use clackr_core::{Action, Config, Engine, EngineEvent, Outcome, Summary};

use crate::term::caps::Caps;
use crate::term::input::Msg;
use crate::ui::menu::MenuState;
use crate::ui::test::wrap::{WrapCache, word_width};

/// Upper bound on how many queued messages one iteration will absorb before
/// drawing. Draining is what keeps fast typing responsive, but an unbounded
/// drain would let a paste storm starve rendering indefinitely.
const MAX_DRAIN: usize = 256;

/// Words to buffer ahead of the caret when a time-mode test starts. The engine
/// asks for more long before these run out.
const INITIAL_WORDS: usize = WORD_LOOKAHEAD * 2;

/// Which screen has focus. Transitions: Menu -> Test -> Results, with Esc
/// moving back up a level from anywhere.
///
/// Both payloads are boxed: a `TestState` carries an engine, an RNG and a wrap
/// cache, and an unboxed variant would make every `Screen` — including the menu
/// — that large.
pub enum Screen {
    Menu(MenuState),
    Test(Box<TestState>),
    Results(Box<Summary>),
}

/// Everything a running test owns.
pub struct TestState {
    pub engine: Engine,
    /// The instant the engine emitted `Started`, i.e. the first counted
    /// keystroke. `None` until then, which is exactly why the loop has no
    /// deadline while a test sits waiting for its first key.
    pub origin: Option<Instant>,
    /// Elapsed time as of the last message. Kept here rather than read from the
    /// clock during rendering so `ui::draw` stays a pure function of `App`.
    pub elapsed: Duration,
    pub wrap: WrapCache,
    /// Tab has armed a restart that Enter will confirm (`QuickRestart::Off`).
    pub restart_armed: bool,
    source: Box<dyn WordSource>,
    rng: StdRng,
}

impl TestState {
    /// Pulls `n` more words out of the source. Time mode's endless word supply
    /// comes through here, driven by `EngineEvent::NeedWords`.
    fn feed(&mut self, n: usize) {
        let Self { source, rng, .. } = self;
        let words: Vec<String> = (0..n).map(|_| source.next(rng)).collect();
        self.engine.append_words(words);
        self.wrap.reset(&self.engine);
    }

    /// Re-measures whatever the last keystroke could have changed.
    ///
    /// Only the active word's width can move, so this is O(1) — and the repack
    /// it triggers only touches the lines from the caret down.
    fn remeasure(&mut self) {
        if self.wrap.word_count() != self.engine.word_count() {
            // Zen grows a word every time you press space, so the width table
            // itself can be short.
            self.wrap.reset(&self.engine);
            return;
        }
        let i = self.engine.word_index();
        if let Some(view) = self.engine.word(i) {
            self.wrap.touch(i, word_width(&view));
        }
    }

    fn elapsed_at(&self, at: Instant) -> Duration {
        self.origin
            .map_or(Duration::ZERO, |o| at.saturating_duration_since(o))
    }
}

pub struct App {
    pub config: Config,
    pub caps: Caps,
    pub screen: Screen,
    pub quit: bool,
    /// Set when a bracketed paste is seen. Pasting is not typing, so the result
    /// is marked rather than silently accepted.
    pub flagged: bool,
    /// Fixed word-generation seed. Only tests set it; a real run draws from the
    /// OS so two tests in a row are not the same text.
    seed: Option<u64>,
    /// Where to write menu choices back to, and what was last written.
    ///
    /// `None` in tests, which is what keeps the suite from touching the real
    /// config file.
    persist: Option<Persist>,
}

/// Remembers the last config written to disk so we only write when something
/// actually changed, rather than once per test started.
struct Persist {
    path: Option<std::path::PathBuf>,
    last_written: Config,
}

impl App {
    pub fn new(config: Config, caps: Caps) -> Self {
        Self {
            screen: Screen::Menu(MenuState::from_config(&config)),
            config,
            caps,
            quit: false,
            flagged: false,
            seed: None,
            persist: None,
        }
    }

    /// Pins word generation to a fixed seed, so snapshot tests get the same
    /// text every run. Test-only for now; a `--seed` flag would make it public.
    #[cfg(test)]
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Persists menu choices back to the config file, so the mode and theme you
    /// picked last time are what you get next time.
    pub fn persisting_to(mut self, path: Option<std::path::PathBuf>) -> Self {
        self.persist = Some(Persist {
            path,
            last_written: self.config.clone(),
        });
        self
    }

    /// Writes the config back if the user changed anything since it was loaded.
    ///
    /// A failure here is deliberately not fatal: a read-only home directory is
    /// no reason to refuse to run a typing test. The setting simply does not
    /// stick, and the run continues.
    fn persist_config(&mut self) {
        let Some(persist) = &mut self.persist else {
            return;
        };
        if persist.last_written == self.config {
            return;
        }
        if crate::config_io::save(&self.config, persist.path.as_deref()).is_ok() {
            persist.last_written = self.config.clone();
        }
    }

    /// Runs until the user quits or stdin closes.
    pub fn run(&mut self, terminal: &mut DefaultTerminal, rx: &Receiver<Msg>) -> Result<()> {
        // Draw once up front: the first frame must appear before any input.
        terminal.draw(|f| crate::ui::draw(f, self))?;

        loop {
            let msg = match self.next_deadline() {
                Some(deadline) => {
                    let now = Instant::now();
                    let timeout = deadline.saturating_duration_since(now);
                    match rx.recv_timeout(timeout) {
                        Ok(m) => Some(m),
                        Err(RecvTimeoutError::Timeout) => None,
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                }
                // No deadline: block indefinitely. This is the zero-CPU idle
                // path that a fixed tick rate would cost us.
                None => match rx.recv() {
                    Ok(m) => Some(m),
                    Err(_) => break,
                },
            };

            let mut dirty = false;
            if let Some(m) = msg {
                dirty |= self.handle(m);
            }
            // Drain everything already queued before drawing, so a burst of
            // keystrokes costs one frame rather than one frame each.
            for m in rx.try_iter().take(MAX_DRAIN) {
                dirty |= self.handle(m);
            }
            dirty |= self.tick(Instant::now());

            if self.quit {
                break;
            }
            if dirty {
                terminal.draw(|f| crate::ui::draw(f, self))?;
            }
        }
        Ok(())
    }

    /// The next instant we must wake up, if any.
    ///
    /// `None` everywhere except inside a test that has actually started, which
    /// is what keeps the menu and the results screen at zero CPU.
    fn next_deadline(&self) -> Option<Instant> {
        let Screen::Test(test) = &self.screen else {
            return None;
        };
        let origin = test.origin?;
        if test.engine.is_finished() {
            return None;
        }
        Some(origin + Duration::from_secs(origin.elapsed().as_secs() + 1))
    }

    /// Advances the engine's clock. Idempotent, so it is safe to call on every
    /// loop iteration regardless of why we woke up.
    fn tick(&mut self, now: Instant) -> bool {
        let Screen::Test(test) = &mut self.screen else {
            return false;
        };
        let Some(origin) = test.origin else {
            return false;
        };
        let elapsed = now.saturating_duration_since(origin);
        test.elapsed = elapsed;
        let out = test.engine.tick(elapsed);
        self.settle(out, elapsed)
    }

    /// Reacts to one message. Visible to the crate so the UI snapshot tests can
    /// drive a real app through the real input path.
    pub(crate) fn handle(&mut self, msg: Msg) -> bool {
        match msg {
            Msg::Key(k, at) => self.on_key(k, at),
            Msg::Resize(w, h) => {
                self.caps = Caps::detect(w, h);
                if let Screen::Test(test) = &mut self.screen {
                    test.wrap.set_width(crate::ui::test::text_width(w));
                    test.wrap.sync();
                }
                true
            }
            // Discarding the payload is the point: it stops a 500-character
            // paste from reaching the engine at all.
            Msg::Paste => {
                self.flagged = true;
                true
            }
            Msg::Closed => {
                self.quit = true;
                false
            }
        }
    }

    fn on_key(&mut self, key: KeyEvent, at: Instant) -> bool {
        // Raw mode means the terminal no longer turns Ctrl+C into SIGINT, so it
        // has to be handled explicitly or the app becomes unquittable.
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('q'))
        {
            self.quit = true;
            return false;
        }

        if matches!(self.screen, Screen::Menu(_)) {
            return self.on_menu_key(key);
        }
        if matches!(self.screen, Screen::Test(_)) {
            return self.on_test_key(key, at);
        }
        match key.code {
            KeyCode::Esc => {
                self.open_menu();
                true
            }
            KeyCode::Tab | KeyCode::Enter => {
                self.start_test();
                true
            }
            // The results screen is a resting point, so plain `q` quits here
            // just as it does in the menu. During a test it must not, or a
            // mistyped word would end the run.
            KeyCode::Char('q') => {
                self.quit = true;
                false
            }
            _ => false,
        }
    }

    fn on_menu_key(&mut self, key: KeyEvent) -> bool {
        let mut start = false;
        {
            let Screen::Menu(menu) = &mut self.screen else {
                return false;
            };
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => menu.move_row(-1),
                KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => menu.move_row(1),
                KeyCode::Left | KeyCode::Char('h') => menu.move_value(-1),
                KeyCode::Right | KeyCode::Char('l') => menu.move_value(1),
                KeyCode::Enter => start = true,
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.quit = true;
                    return false;
                }
                _ => return false,
            }
            // Written back on every move, not just on Enter, so picking a theme
            // previews it straight away.
            menu.apply_to(&mut self.config);
        }
        if start {
            // Starting a test is the point at which the menu's choices count as
            // deliberate, so that is where they get written back.
            self.persist_config();
            self.start_test();
        }
        true
    }

    fn on_test_key(&mut self, key: KeyEvent, at: Instant) -> bool {
        let (zen, armed) = match &self.screen {
            Screen::Test(test) => (
                test.engine.params().mode == TestMode::Zen,
                test.restart_armed,
            ),
            _ => return false,
        };

        // An armed restart swallows exactly one keystroke: Enter confirms it,
        // anything else calls it off.
        if armed {
            self.set_armed(false);
            if key.code == KeyCode::Enter {
                self.start_test();
            }
            return true;
        }

        match key.code {
            // Zen has no end condition of its own, so Esc and Enter are its
            // only way out — and they finish the test rather than abandon it.
            KeyCode::Esc | KeyCode::Enter if zen => self.act(Action::EndTest, at),
            KeyCode::Esc => match self.config.keys.quick_restart {
                QuickRestart::Esc => {
                    self.start_test();
                    true
                }
                _ => {
                    self.open_menu();
                    true
                }
            },
            KeyCode::Tab => match self.config.keys.quick_restart {
                QuickRestart::Tab => {
                    self.start_test();
                    true
                }
                QuickRestart::Off => {
                    self.set_armed(true);
                    true
                }
                // With Esc restarting, Tab is what is left to reach the menu.
                QuickRestart::Esc => {
                    self.open_menu();
                    true
                }
            },
            _ => match action_for(key) {
                Some(action) => self.act(action, at),
                None => false,
            },
        }
    }

    /// Feeds one action to the engine, using the timestamp the *input thread*
    /// stamped on the key rather than the time we got round to handling it.
    /// A render stall must not move a keystroke into the next second bucket.
    fn act(&mut self, action: Action, at: Instant) -> bool {
        let Screen::Test(test) = &mut self.screen else {
            return false;
        };
        let elapsed = test.elapsed_at(at);
        let out = test.engine.apply(action, elapsed);
        if out.event == Some(EngineEvent::Started) {
            test.origin = Some(at);
        }
        test.elapsed = elapsed;
        test.remeasure();
        self.settle(out, elapsed)
    }

    /// Reacts to whatever the engine asked for and brings the wrap cache back
    /// into step. Shared by `act` and `tick` because `NeedWords` is
    /// level-triggered and can arrive from either.
    fn settle(&mut self, out: Outcome, elapsed: Duration) -> bool {
        let mut summary = None;
        if let Screen::Test(test) = &mut self.screen {
            match out.event {
                Some(EngineEvent::NeedWords(n)) => test.feed(n),
                Some(EngineEvent::Finished) => summary = Some(test.engine.summary(elapsed)),
                _ => {}
            }
            test.wrap.sync();
        }
        if let Some(summary) = summary {
            self.screen = Screen::Results(Box::new(summary));
            return true;
        }
        out.dirty
    }

    fn set_armed(&mut self, armed: bool) {
        if let Screen::Test(test) = &mut self.screen {
            test.restart_armed = armed;
        }
    }

    fn open_menu(&mut self) {
        self.screen = Screen::Menu(MenuState::from_config(&self.config));
    }

    fn start_test(&mut self) {
        let params = self.config.params();
        let mut rng = match self.seed {
            Some(seed) => StdRng::seed_from_u64(seed),
            None => StdRng::from_rng(&mut rand::rng()),
        };
        let (mut source, initial) = word_source(&params, &mut rng);

        let mut engine = Engine::new(params);
        let words: Vec<String> = (0..initial).map(|_| source.next(&mut rng)).collect();
        engine.append_words(words);

        let mut wrap = WrapCache::new(crate::ui::test::text_width(self.caps.width));
        wrap.reset(&engine);
        wrap.sync();

        self.flagged = false;
        self.screen = Screen::Test(Box::new(TestState {
            engine,
            origin: None,
            elapsed: Duration::ZERO,
            wrap,
            restart_armed: false,
            source,
            rng,
        }));
    }
}

/// Maps a key event onto an engine action, or `None` if the engine has no
/// opinion about that key.
///
/// The `DeleteWord` arms are first because which of Ctrl+Backspace,
/// Alt+Backspace, Ctrl+W and a bare `\u{17}` a terminal emits is a coin flip —
/// macOS terminals in particular disagree with everyone else.
fn action_for(key: KeyEvent) -> Option<Action> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    match key.code {
        KeyCode::Backspace if ctrl || alt => Some(Action::DeleteWord),
        KeyCode::Char('w') if ctrl => Some(Action::DeleteWord),
        KeyCode::Char('\u{17}') => Some(Action::DeleteWord),
        KeyCode::Backspace => Some(Action::Backspace),
        KeyCode::Char(' ') => Some(Action::Space),
        // Ctrl/Alt chords are commands, not text.
        KeyCode::Char(c) if !ctrl && !alt => Some(Action::Type(c)),
        _ => None,
    }
}

/// A finite word source: quote mode's fixed target text.
struct FixedWords(std::vec::IntoIter<String>);

impl WordSource for FixedWords {
    fn next(&mut self, _rng: &mut dyn rand::Rng) -> String {
        self.0.next().unwrap_or_default()
    }

    fn is_finite(&self) -> bool {
        true
    }
}

/// The word source for a test, plus how many words to buffer before the first
/// frame. Time mode streams; the others know their whole target up front.
fn word_source(params: &TestParams, rng: &mut StdRng) -> (Box<dyn WordSource>, usize) {
    match params.mode {
        TestMode::Zen => (Box::new(FixedWords(Vec::new().into_iter())), 0),
        TestMode::Quote => {
            let quote = quotes::pick(params.quote, rng)
                // An empty bucket must not leave the user staring at nothing.
                .or_else(|| quotes::pick(clackr_core::QuoteLength::All, rng));
            let words: Vec<String> = quote
                .map(|q| q.text.split_whitespace().map(str::to_string).collect())
                .unwrap_or_default();
            let n = words.len();
            (Box::new(FixedWords(words.into_iter())), n)
        }
        TestMode::Time | TestMode::Words => {
            // An unknown language would otherwise panic inside RandomWords;
            // falling back to the default keeps a typo in the config from
            // taking the whole app down.
            let pool = words::language(&params.language)
                .or_else(|| words::language("english1k"))
                .unwrap_or(&["the"]);
            let count = match params.mode {
                TestMode::Words => params.words as usize,
                _ => INITIAL_WORDS,
            };
            (Box::new(RandomWords::new(pool)), count)
        }
    }
}

#[cfg(test)]
mod tests {
    use rand::RngExt;

    use clackr_core::CellState;
    use clackr_core::mode::TestMode;

    use super::*;
    use crate::term::caps::ColorDepth;

    const SEED: u64 = 20_260_807;

    fn caps() -> Caps {
        Caps {
            color: ColorDepth::TrueColor,
            unicode: true,
            braille: true,
            width: 120,
            height: 40,
        }
    }

    fn new_app(mode: TestMode) -> App {
        let mut config = Config::default();
        config.test.mode = mode;
        config.test.words = 2000;
        App::new(config, caps()).with_seed(SEED)
    }

    fn test_state(app: &App) -> &TestState {
        match &app.screen {
            Screen::Test(t) => t,
            _ => panic!("expected the test screen"),
        }
    }

    /// The full rendered text of every word, which is the finest-grained view
    /// of engine state there is: it folds in the target, the input, and every
    /// derived cell state.
    fn transcript(engine: &Engine) -> Vec<(String, Vec<CellState>)> {
        engine
            .words()
            .map(|w| {
                (
                    (0..w.len()).map(|i| w.text(i)).collect::<String>(),
                    (0..w.len()).map(|i| w.state(i)).collect(),
                )
            })
            .collect()
    }

    /// The key event a user would press to produce `action`.
    fn key_for(action: Action) -> KeyEvent {
        let (code, mods) = match action {
            Action::Type(c) => (KeyCode::Char(c), KeyModifiers::NONE),
            Action::Space => (KeyCode::Char(' '), KeyModifiers::NONE),
            Action::Backspace => (KeyCode::Backspace, KeyModifiers::NONE),
            Action::DeleteWord => (KeyCode::Backspace, KeyModifiers::CONTROL),
            Action::EndTest => unreachable!("not produced by the script"),
        };
        KeyEvent::new(code, mods)
    }

    /// A long script of realistic typing: mostly letters, with the space,
    /// backspace and delete-word keys that make the engine's rules interesting.
    fn script(len: usize) -> Vec<Action> {
        let mut rng = StdRng::seed_from_u64(99);
        (0..len)
            .map(|_| match rng.random_range(0..100u32) {
                0..=64 => Action::Type((b'a' + rng.random_range(0..26u8)) as char),
                65..=84 => Action::Space,
                85..=96 => Action::Backspace,
                _ => Action::DeleteWord,
            })
            .collect()
    }

    #[test]
    fn the_key_mapping_layer_is_lossless() {
        // Five thousand keystrokes pushed through the real `Msg::Key` path must
        // land the engine in exactly the state the same actions produce when
        // applied directly. This is what proves the crossterm-to-`Action`
        // mapping neither drops nor invents input.
        let mut app = new_app(TestMode::Words);
        app.start_test();

        let mut reference = Engine::new(app.config.params());
        let mut rng = StdRng::seed_from_u64(SEED);
        let mut source = RandomWords::new(words::language("english1k").unwrap());
        let generated: Vec<String> = (0..2000).map(|_| source.next(&mut rng)).collect();
        reference.append_words(generated);

        let at = Instant::now();
        for action in script(5000) {
            app.handle(Msg::Key(key_for(action), at));
            reference.apply(action, Duration::ZERO);
        }

        let engine = &test_state(&app).engine;
        assert_eq!(engine.word_index(), reference.word_index());
        assert_eq!(engine.cursor_in_word(), reference.cursor_in_word());
        assert_eq!(engine.keys_correct(), reference.keys_correct());
        assert_eq!(engine.keys_incorrect(), reference.keys_incorrect());
        assert_eq!(engine.counts(), reference.counts());
        assert_eq!(transcript(engine), transcript(&reference));
    }

    #[test]
    fn every_delete_word_spelling_maps_to_the_same_action() {
        let ctrl = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL);
        for key in [
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::CONTROL),
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::ALT),
            ctrl('w'),
            KeyEvent::new(KeyCode::Char('\u{17}'), KeyModifiers::NONE),
        ] {
            assert_eq!(action_for(key), Some(Action::DeleteWord), "{key:?}");
        }
    }

    #[test]
    fn command_chords_are_not_text() {
        for key in [
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::ALT),
            KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE),
        ] {
            assert_eq!(action_for(key), None, "{key:?}");
        }
        // Shift is a different matter: it is how capitals are typed.
        assert_eq!(
            action_for(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT)),
            Some(Action::Type('A'))
        );
    }

    #[test]
    fn an_idle_screen_never_asks_to_be_woken_up() {
        // The zero-CPU idle property, asserted rather than trusted: no deadline
        // on the menu, none on the results screen, and none inside a test that
        // has not been typed into yet.
        let mut app = new_app(TestMode::Time);
        assert!(app.next_deadline().is_none(), "menu");

        app.start_test();
        assert!(app.next_deadline().is_none(), "test before the first key");

        let at = Instant::now();
        app.handle(Msg::Key(key_for(Action::Type('a')), at));
        assert!(app.next_deadline().is_some(), "test after the first key");

        app.screen = Screen::Results(Box::new(Summary::fixture()));
        assert!(app.next_deadline().is_none(), "results");
    }

    #[test]
    fn the_clock_starts_at_the_first_keystroke_not_at_the_menu() {
        let mut app = new_app(TestMode::Time);
        app.start_test();
        // Simulate a long pause between opening the test and typing: the key's
        // own timestamp is what the engine is given, so elapsed must be zero.
        let at = Instant::now() + Duration::from_secs(30);
        app.handle(Msg::Key(key_for(Action::Type('a')), at));
        assert_eq!(test_state(&app).elapsed, Duration::ZERO);
        assert_eq!(test_state(&app).origin, Some(at));
    }

    #[test]
    fn time_mode_keeps_being_fed_words() {
        let mut app = new_app(TestMode::Time);
        app.start_test();
        let before = test_state(&app).engine.word_count();

        // Submit enough words to eat into the lookahead buffer.
        let at = Instant::now();
        for _ in 0..INITIAL_WORDS {
            app.handle(Msg::Key(key_for(Action::Type('x')), at));
            app.handle(Msg::Key(key_for(Action::Space), at));
        }
        let test = test_state(&app);
        assert!(
            test.engine.word_count() > before,
            "the engine ran out of words"
        );
        assert_eq!(test.wrap.word_count(), test.engine.word_count());
    }

    #[test]
    fn finishing_a_words_test_lands_on_the_results_screen() {
        let mut config = Config::default();
        config.test.mode = TestMode::Words;
        config.test.words = 3;
        let mut app = App::new(config, caps()).with_seed(SEED);
        app.start_test();

        let at = Instant::now();
        for _ in 0..3 {
            app.handle(Msg::Key(key_for(Action::Type('z')), at));
            app.handle(Msg::Key(key_for(Action::Space), at));
        }
        assert!(matches!(app.screen, Screen::Results(_)));
    }

    #[test]
    fn zen_ends_on_esc_rather_than_abandoning_the_test() {
        let mut app = new_app(TestMode::Zen);
        app.start_test();
        let at = Instant::now();
        app.handle(Msg::Key(key_for(Action::Type('a')), at));
        app.handle(Msg::Key(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            at,
        ));
        assert!(matches!(app.screen, Screen::Results(_)));
    }

    #[test]
    fn esc_abandons_a_normal_test() {
        let mut app = new_app(TestMode::Time);
        app.start_test();
        let at = Instant::now();
        app.handle(Msg::Key(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            at,
        ));
        assert!(matches!(app.screen, Screen::Menu(_)));
    }

    #[test]
    fn tab_restarts_only_when_the_config_says_so() {
        let at = Instant::now();

        let mut app = new_app(TestMode::Time);
        app.start_test();
        app.handle(Msg::Key(key_for(Action::Type('a')), at));
        app.handle(Msg::Key(
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            at,
        ));
        assert!(!test_state(&app).engine.is_started(), "tab should restart");

        // With quick restart off, Tab only arms; Enter is what pulls the
        // trigger, and anything else calls it off.
        let mut app = new_app(TestMode::Time);
        app.config.keys.quick_restart = QuickRestart::Off;
        app.start_test();
        app.handle(Msg::Key(key_for(Action::Type('a')), at));
        app.handle(Msg::Key(
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            at,
        ));
        assert!(test_state(&app).restart_armed);
        app.handle(Msg::Key(key_for(Action::Type('b')), at));
        assert!(!test_state(&app).restart_armed);
        assert!(
            test_state(&app).engine.is_started(),
            "restart was cancelled"
        );

        app.handle(Msg::Key(
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            at,
        ));
        app.handle(Msg::Key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            at,
        ));
        assert!(
            !test_state(&app).engine.is_started(),
            "restart was confirmed"
        );
    }

    #[test]
    fn a_resize_repacks_the_word_display() {
        let mut app = new_app(TestMode::Time);
        app.start_test();
        let wide = test_state(&app).wrap.lines().len();
        app.handle(Msg::Resize(60, 20));
        let narrow = test_state(&app).wrap.lines().len();
        assert!(narrow > wide, "{narrow} lines at 60 cols vs {wide} at 120");
        assert_eq!(
            test_state(&app).wrap.width(),
            crate::ui::test::text_width(60)
        );
    }

    #[test]
    fn a_paste_is_flagged_and_discarded() {
        let mut app = new_app(TestMode::Time);
        app.start_test();
        app.handle(Msg::Paste);
        assert!(app.flagged);
        assert_eq!(test_state(&app).engine.cursor_in_word(), 0);
    }

    #[test]
    fn quote_mode_types_a_real_quote() {
        let mut app = new_app(TestMode::Quote);
        app.start_test();
        let test = test_state(&app);
        assert!(test.engine.word_count() > 5);
        assert!(!test.wrap.lines().is_empty());
    }
}
