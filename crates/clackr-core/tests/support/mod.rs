//! A keystroke-script DSL, so engine tests read like transcripts.
//!
//! ```text
//! '_'  space          '<'  backspace
//! '^'  delete word    '!'  end test
//! ```
//!
//! Everything else is `Action::Type`. `script("the quick", "the_qui_")` is the
//! whole transcript of a user who skipped two letters.

#![allow(dead_code)] // each test binary uses a different slice of this module

use std::time::Duration;

use clackr_core::mode::{StopOnError, TestMode, TestParams};
use clackr_core::{Action, Engine, EngineEvent};

/// Translate a script string into the actions it stands for.
pub fn actions(keys: &str) -> Vec<Action> {
    keys.chars()
        .map(|c| match c {
            '_' => Action::Space,
            '<' => Action::Backspace,
            '^' => Action::DeleteWord,
            '!' => Action::EndTest,
            c => Action::Type(c),
        })
        .collect()
}

/// Words-mode params sized to `target`.
pub fn words_params(target: &str) -> TestParams {
    TestParams {
        mode: TestMode::Words,
        words: target.split_whitespace().count() as u32,
        ..TestParams::default()
    }
}

pub fn time_params(seconds: u32) -> TestParams {
    TestParams {
        mode: TestMode::Time,
        time: seconds,
        ..TestParams::default()
    }
}

pub fn zen_params() -> TestParams {
    TestParams {
        mode: TestMode::Zen,
        ..TestParams::default()
    }
}

pub fn stop_on_error_params(target: &str, stop: StopOnError) -> TestParams {
    TestParams {
        stop_on_error: stop,
        ..words_params(target)
    }
}

/// An engine plus a fake clock, so timing is explicit and deterministic.
pub struct Rig {
    pub engine: Engine,
    pub events: Vec<EngineEvent>,
    clock: Duration,
    step: Duration,
}

impl Rig {
    pub fn new(params: TestParams, target: &str) -> Self {
        let mut engine = Engine::new(params);
        engine.append_words(target.split_whitespace().map(str::to_string));
        Self {
            engine,
            events: Vec::new(),
            clock: Duration::ZERO,
            step: Duration::ZERO,
        }
    }

    pub fn words(target: &str) -> Self {
        Self::new(words_params(target), target)
    }

    /// Advance the fake clock by `step` before each subsequent keystroke.
    pub fn step(mut self, step: Duration) -> Self {
        self.step = step;
        self
    }

    pub fn feed(&mut self, keys: &str) -> &mut Self {
        for action in actions(keys) {
            self.press(action);
        }
        self
    }

    pub fn press(&mut self, action: Action) -> &mut Self {
        let out = self.engine.apply(action, self.clock);
        self.events.extend(out.event);
        self.clock += self.step;
        self
    }

    /// Apply an action at an exact time, ignoring `step`.
    pub fn press_at(&mut self, secs: f64, action: Action) -> &mut Self {
        self.clock = Duration::from_secs_f64(secs);
        let out = self.engine.apply(action, self.clock);
        self.events.extend(out.event);
        self
    }

    pub fn tick_at(&mut self, secs: f64) -> &mut Self {
        self.clock = Duration::from_secs_f64(secs);
        let out = self.engine.tick(self.clock);
        self.events.extend(out.event);
        self
    }

    pub fn clock(&self) -> Duration {
        self.clock
    }

    pub fn summary(&self) -> clackr_core::Summary {
        self.engine.summary(self.clock)
    }
}

/// The one-liner form: run `keys` against `target` in words mode, at t = 0.
pub fn script(target: &str, keys: &str) -> Engine {
    let mut rig = Rig::words(target);
    rig.feed(keys);
    rig.engine
}

/// Same, but with explicit params (mode, stop-on-error, freedom, ...).
pub fn script_with(params: TestParams, target: &str, keys: &str) -> Engine {
    let mut rig = Rig::new(params, target);
    rig.feed(keys);
    rig.engine
}

/// Render one word the way the UI would, as `(text, state)` pairs collapsed
/// into a string of state initials: `c`orrect, `i`ncorrect, `e`xtra,
/// `m`issed, `.`untyped.
pub fn states(engine: &Engine, word: usize) -> String {
    use clackr_core::CellState::*;
    engine
        .word(word)
        .expect("word in range")
        .cells()
        .map(|(_, s)| match s {
            Correct => 'c',
            Incorrect => 'i',
            Extra => 'e',
            Missed => 'm',
            Untyped => '.',
        })
        .collect()
}

/// The typed text of one word.
pub fn typed(engine: &Engine, word: usize) -> String {
    engine
        .word(word)
        .expect("word in range")
        .entry
        .typed()
        .to_string()
}
