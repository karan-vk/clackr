//! The typing engine: a pure state machine over [`Action`]s.
//!
//! It has no clock. Every mutating method takes the caller's `elapsed`, which
//! is what makes timing behaviour (second buckets, the time limit, the whole
//! WPM series) deterministic and testable without a single `sleep`. The caller
//! starts its clock when the engine emits [`EngineEvent::Started`].

mod action;
mod grapheme;
mod model;

use std::time::Duration;

pub use action::{Action, EngineEvent, Outcome};
pub use model::{CellState, TargetWord, WordEntry, WordView, cell_state};

use crate::mode::{StopOnError, TestMode, TestParams};
use crate::stats::{CharCounts, ConsistencyKind, SecondSampler, Summary, consistency, count_chars};

/// Extras a single word may accumulate before further keystrokes stop being
/// rendered. A stuck key would otherwise widen one word without bound and take
/// the line-wrapping layer with it. MonkeyType uses the same trick.
pub const MAX_EXTRA_CELLS: usize = 20;

/// How close to the end of the word list time mode gets before asking for more.
/// Three lines of a wide terminal is well under 40 words, so the user never
/// sees the seam.
pub const WORD_LOOKAHEAD: usize = 40;

/// What a keystroke did, before the shared accounting is applied.
#[derive(Copy, Clone, Debug, Default)]
struct Key {
    /// `Some(correct)` if this counts toward accuracy. `None` means the rules
    /// swallowed the keystroke entirely — see rule 3 (space on an empty word).
    counted: Option<bool>,
    /// The visible state changed.
    dirty: bool,
}

impl Key {
    const IGNORED: Key = Key {
        counted: None,
        dirty: false,
    };

    const fn typed(correct: bool) -> Key {
        Key {
            counted: Some(correct),
            dirty: true,
        }
    }

    /// Counted against accuracy but with no visible effect: a keystroke
    /// rejected by `stop_on_error`, or one past the extras cap.
    const fn rejected() -> Key {
        Key {
            counted: Some(false),
            dirty: false,
        }
    }
}

/// The state of one test run.
#[derive(Clone, Debug)]
pub struct Engine {
    params: TestParams,
    targets: Vec<TargetWord>,
    /// Always the same length as `targets`; index `i` is what the user typed
    /// for `targets[i]`.
    entries: Vec<WordEntry>,
    word_ix: usize,
    started: bool,
    finished: bool,
    finished_at: Option<Duration>,
    /// Accuracy is counted at keypress time and is *never* decremented — a
    /// typo you fixed still happened. This is the whole reason backspace does
    /// not touch these two fields.
    keys_correct: u32,
    keys_incorrect: u32,
    sampler: SecondSampler,
    consistency_kind: ConsistencyKind,
    /// `targets.len()` at the last `NeedWords`, so the level-triggered request
    /// stops repeating until words actually arrive.
    requested_at: Option<usize>,
}

impl Engine {
    pub fn new(params: TestParams) -> Self {
        let zen = params.mode == TestMode::Zen;
        let mut engine = Self {
            params,
            targets: Vec::new(),
            entries: Vec::new(),
            word_ix: 0,
            started: false,
            finished: false,
            finished_at: None,
            keys_correct: 0,
            keys_incorrect: 0,
            sampler: SecondSampler::default(),
            consistency_kind: ConsistencyKind::default(),
            requested_at: None,
        };
        if zen {
            // Zen has no word source; it grows its own empty words.
            engine.push_word(TargetWord::default());
        }
        engine
    }

    /// Override the consistency formula. Present so a disagreement with a
    /// golden vector is a one-line change rather than a refactor.
    pub fn with_consistency(mut self, kind: ConsistencyKind) -> Self {
        self.consistency_kind = kind;
        self
    }

    /// Append target words. Time mode calls this again whenever the engine
    /// asks via [`EngineEvent::NeedWords`]; zen mode ignores it.
    pub fn append_words(&mut self, words: impl IntoIterator<Item = String>) {
        if self.is_zen() {
            return;
        }
        for w in words {
            self.push_word(TargetWord::new(w));
        }
    }

    fn push_word(&mut self, word: TargetWord) {
        self.targets.push(word);
        self.entries.push(WordEntry::default());
    }

    // ---- read-only views -------------------------------------------------

    pub fn params(&self) -> &TestParams {
        &self.params
    }

    pub fn is_started(&self) -> bool {
        self.started
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    pub fn word_index(&self) -> usize {
        self.word_ix
    }

    /// Caret position within the current word, in cells.
    pub fn cursor_in_word(&self) -> usize {
        self.entries.get(self.word_ix).map_or(0, WordEntry::len)
    }

    pub fn word_count(&self) -> usize {
        self.targets.len()
    }

    /// Every word, as a target/entry pair ready for rendering.
    pub fn words(&self) -> impl ExactSizeIterator<Item = WordView<'_>> + '_ {
        let zen = self.is_zen();
        self.targets
            .iter()
            .zip(self.entries.iter())
            .map(move |(t, e)| WordView::new(t, e, zen))
    }

    pub fn word(&self, i: usize) -> Option<WordView<'_>> {
        let zen = self.is_zen();
        Some(WordView::new(
            self.targets.get(i)?,
            self.entries.get(i)?,
            zen,
        ))
    }

    pub fn keys_correct(&self) -> u32 {
        self.keys_correct
    }

    pub fn keys_incorrect(&self) -> u32 {
        self.keys_incorrect
    }

    /// MonkeyType's `countChars()` over every word the user has reached.
    pub fn counts(&self) -> CharCounts {
        count_chars(self.words().take(self.counted_words()))
    }

    /// Seconds remaining in time mode.
    pub fn time_left(&self, elapsed: Duration) -> Option<Duration> {
        let limit = Duration::from_secs(u64::from(self.params.time_limit()?));
        Some(limit.saturating_sub(elapsed))
    }

    /// Net WPM as of `elapsed` — what the test screen shows live.
    pub fn live_wpm(&self, elapsed: Duration) -> f64 {
        crate::stats::wpm(self.counts().net_chars(), self.effective_duration(elapsed))
    }

    /// Everything the results screen needs.
    pub fn summary(&self, elapsed: Duration) -> Summary {
        let duration = self.effective_duration(elapsed);
        let counts = self.counts();
        let series = self
            .sampler
            .snapshot(duration.as_secs() as u32, counts.net_chars());
        Summary {
            params: self.params.clone(),
            wpm: crate::stats::wpm(counts.net_chars(), duration),
            raw: crate::stats::wpm(counts.raw_chars(), duration),
            acc: crate::stats::accuracy(self.keys_correct, self.keys_incorrect),
            consistency: consistency(&series, self.consistency_kind),
            counts,
            duration,
            series,
        }
    }

    // ---- driving ---------------------------------------------------------

    /// Apply one keystroke. `elapsed` is measured from the moment the engine
    /// emitted [`EngineEvent::Started`].
    pub fn apply(&mut self, action: Action, elapsed: Duration) -> Outcome {
        if self.finished {
            return Outcome::IDLE;
        }
        // Target words never contain whitespace, so a literal space can only
        // ever have meant `Space`. Normalising here keeps a caller that forgot
        // to special-case the space bar from scoring a word full of extras.
        let action = match action {
            Action::Type(' ') => Action::Space,
            other => other,
        };
        let mut out = Outcome::IDLE;
        match action {
            Action::EndTest => {
                self.finish(elapsed);
                return Outcome::event(EngineEvent::Finished);
            }
            // Backspacing is not a keystroke as far as the test is concerned:
            // it neither starts the clock nor moves the accuracy counters.
            Action::Backspace => {
                out.dirty = self.backspace();
                return out;
            }
            Action::DeleteWord => {
                out.dirty = self.delete_word();
                return out;
            }
            Action::Type(_) | Action::Space => {}
        }

        // Seal any second boundaries the keystroke crossed *before* mutating,
        // so a bucket's cumulative WPM reflects the counts as they stood when
        // that second ended.
        let second = elapsed.as_secs() as u32;
        self.seal_to(second);

        let key = match action {
            Action::Type(c) => self.type_char(c),
            Action::Space => self.space(),
            _ => unreachable!("handled above"),
        };

        out.dirty |= key.dirty;
        if let Some(correct) = key.counted {
            out.dirty = true;
            if !self.started {
                self.started = true;
                out.event = Some(EngineEvent::Started);
            }
            if correct {
                self.keys_correct += 1;
            } else {
                self.keys_incorrect += 1;
            }
            self.sampler.record(second, correct);
        }

        // Words and quote mode end the moment the last word is submitted.
        if !self.params.is_endless()
            && !self.targets.is_empty()
            && self.word_ix >= self.targets.len()
        {
            self.finish(elapsed);
            return Outcome::event(EngineEvent::Finished);
        }
        if out.event.is_none() {
            out.event = self.word_request();
        }
        out
    }

    /// Advance time without input. Idempotent: calling it twice with the same
    /// `elapsed` is a no-op, so the caller may drive it from any loop.
    pub fn tick(&mut self, elapsed: Duration) -> Outcome {
        if self.finished || !self.started {
            return Outcome::IDLE;
        }
        if let Some(limit) = self.params.time_limit() {
            let limit = Duration::from_secs(u64::from(limit));
            if elapsed >= limit {
                // Clamp to the limit exactly: a 30 s test divides by 30 s, not
                // by however late the tick happened to arrive.
                self.finish(limit);
                return Outcome::event(EngineEvent::Finished);
            }
        }
        let second = elapsed.as_secs() as u32;
        let mut out = Outcome::IDLE;
        out.dirty = self.seal_to(second);
        out.event = self.word_request();
        out
    }

    // ---- keystroke rules -------------------------------------------------

    fn type_char(&mut self, c: char) -> Key {
        if !grapheme::is_typeable(c) {
            return Key::IGNORED;
        }
        if self.is_zen() {
            // Rule 7: there is no target, so everything is correct.
            let entry = &mut self.entries[self.word_ix];
            return match entry.push_char(c) {
                Some(_) => Key::typed(true),
                None => Key::IGNORED,
            };
        }
        let Some(ix) = self.current_ix() else {
            // Time mode ran out of buffered words; the caller owes us an
            // `append_words`. Swallow the keystroke rather than mis-score it.
            return Key::IGNORED;
        };
        let stop_on_error = self.params.stop_on_error;
        let target = &self.targets[ix];
        let entry = &mut self.entries[ix];

        // Rule 2: a zero-width mark merges instead of adding a cell, so it is
        // never what pushes a word over the extras cap.
        let adds_cell = !grapheme::is_zero_width(c) || entry.is_empty();
        if adds_cell && entry.len() >= target.len() + MAX_EXTRA_CELLS {
            return Key::rejected();
        }

        let Some(push) = entry.push_char(c) else {
            return Key::IGNORED;
        };
        let i = entry.len() - 1;
        let correct = model::cell_state(target, entry, i) == CellState::Correct;
        if !correct && stop_on_error == StopOnError::Letter {
            // Rule 6: the character never lands, but the mistake still counts.
            entry.undo_push(push);
            return Key::rejected();
        }
        Key::typed(correct)
    }

    fn space(&mut self) -> Key {
        if self.is_zen() {
            self.entries[self.word_ix].set_submitted(true);
            self.word_ix += 1;
            if self.word_ix >= self.entries.len() {
                self.push_word(TargetWord::default());
            }
            return Key::typed(true);
        }
        let Some(ix) = self.current_ix() else {
            return Key::IGNORED;
        };
        // Rule 3: a space on an empty word is not a keystroke at all. Leaning
        // on the space bar between words must not cost accuracy.
        if self.entries[ix].is_empty() {
            return Key::IGNORED;
        }
        let exact = self.entries[ix].typed() == self.targets[ix].text();
        if self.params.stop_on_error == StopOnError::Word && !exact {
            return Key::IGNORED;
        }
        self.entries[ix].set_submitted(true);
        self.word_ix += 1;
        // Rule 3's tail: the space is credited only if the word it submits was
        // perfect, which is what makes `correct_spaces` a WPM input.
        Key::typed(exact)
    }

    fn backspace(&mut self) -> bool {
        if let Some(ix) = self.current_ix()
            && self.entries[ix].pop_cell()
        {
            return true;
        }
        self.step_back()
    }

    fn delete_word(&mut self) -> bool {
        if let Some(ix) = self.current_ix()
            && !self.entries[ix].is_empty()
        {
            self.entries[ix].clear();
            return true;
        }
        if self.step_back() {
            self.entries[self.word_ix].clear();
            return true;
        }
        false
    }

    /// Rule 4: you may only reopen the previous word if you got it wrong.
    /// MonkeyType's reasoning is that backspacing into a perfect word can only
    /// be an accident, and the cost of the accident (retyping a whole word) is
    /// much larger than the cost of refusing.
    fn step_back(&mut self) -> bool {
        if self.word_ix == 0 {
            return false;
        }
        let prev = self.word_ix - 1;
        let perfect = self.entries[prev].typed() == self.targets[prev].text();
        if perfect && !self.params.freedom_mode && !self.is_zen() {
            return false;
        }
        self.word_ix = prev;
        self.entries[prev].set_submitted(false);
        true
    }

    // ---- plumbing --------------------------------------------------------

    fn is_zen(&self) -> bool {
        self.params.mode == TestMode::Zen
    }

    fn current_ix(&self) -> Option<usize> {
        (self.word_ix < self.targets.len()).then_some(self.word_ix)
    }

    /// Words the user has reached: everything submitted, plus the one under
    /// the caret if it has any input. Untouched words ahead of the caret must
    /// not be scored as missed.
    fn counted_words(&self) -> usize {
        let n = self.word_ix.min(self.targets.len());
        match self.entries.get(n) {
            Some(e) if !e.is_empty() => n + 1,
            _ => n,
        }
    }

    /// Seal completed second buckets. Returns whether anything was sealed.
    ///
    /// `counts()` is O(words), so it is only paid on an actual second
    /// boundary — roughly once per 12 keystrokes at 150 wpm.
    fn seal_to(&mut self, second: u32) -> bool {
        if !self.sampler.needs_seal(second) {
            return false;
        }
        let net = self.counts().net_chars();
        self.sampler.advance_to(second, net);
        true
    }

    fn finish(&mut self, elapsed: Duration) {
        if self.finished {
            return;
        }
        self.finished = true;
        self.finished_at = Some(elapsed);
        let net = self.counts().net_chars();
        self.sampler.close(elapsed.as_secs() as u32, net);
    }

    fn effective_duration(&self, elapsed: Duration) -> Duration {
        self.finished_at.unwrap_or(elapsed)
    }

    /// Rule 8. Level-triggered on purpose: `tick` re-asks until the words show
    /// up, so a dropped event cannot strand the test.
    fn word_request(&mut self) -> Option<EngineEvent> {
        if self.finished || self.params.mode != TestMode::Time {
            return None;
        }
        let remaining = self.targets.len().saturating_sub(self.word_ix);
        if remaining >= WORD_LOOKAHEAD || self.requested_at == Some(self.targets.len()) {
            return None;
        }
        self.requested_at = Some(self.targets.len());
        Some(EngineEvent::NeedWords(WORD_LOOKAHEAD * 2 - remaining))
    }
}
