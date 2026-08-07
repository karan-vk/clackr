//! Property tests. These are what catch the backspace edge cases: the
//! invariants below are the ones every hand-written transcript quietly
//! assumes, and an arbitrary action sequence is much better than a human at
//! finding the sequence that breaks one.

mod support;

use std::time::Duration;

use clackr_core::mode::{StopOnError, TestMode, TestParams};
use clackr_core::stats::{ConsistencyKind, consistency_of, kogasa};
use clackr_core::{Action, CellState, Engine};
use proptest::prelude::*;
use support::words_params;

const TARGET: &str = "the quick brown fox jumps over a lazy dog while it rains";

fn action_strategy() -> impl Strategy<Value = Action> {
    prop_oneof![
        // Weighted so that typing dominates, but every rule still gets
        // exercised thousands of times across a run.
        10 => prop::sample::select(vec!['t', 'h', 'e', 'q', 'x', 'é', '\u{301}'])
            .prop_map(Action::Type),
        3 => Just(Action::Space),
        3 => Just(Action::Backspace),
        1 => Just(Action::DeleteWord),
    ]
}

fn params_strategy() -> impl Strategy<Value = TestParams> {
    (
        prop::sample::select(vec![
            TestMode::Time,
            TestMode::Words,
            TestMode::Quote,
            TestMode::Zen,
        ]),
        any::<bool>(),
        prop::sample::select(vec![
            StopOnError::Off,
            StopOnError::Letter,
            StopOnError::Word,
        ]),
    )
        .prop_map(|(mode, freedom_mode, stop_on_error)| TestParams {
            mode,
            time: 120,
            freedom_mode,
            stop_on_error,
            ..words_params(TARGET)
        })
}

/// Everything the rest of the crate — and the UI — is allowed to assume.
fn check_invariants(engine: &Engine) {
    assert!(
        engine.word_index() <= engine.word_count(),
        "word_ix {} out of range (of {})",
        engine.word_index(),
        engine.word_count()
    );
    let words: Vec<_> = engine.words().collect();
    assert_eq!(
        words.len(),
        engine.word_count(),
        "targets and entries in step"
    );

    for (i, w) in words.iter().enumerate() {
        // Every rendered cell must resolve to text and a state without
        // panicking: this is what proves no cell range can go out of bounds.
        let mut cells = 0;
        for (text, state) in w.cells() {
            cells += 1;
            assert!(
                !text.is_empty() || state == CellState::Untyped,
                "empty cell text in word {i}"
            );
            if i > engine.word_index() {
                assert_eq!(state, CellState::Untyped, "word {i} is ahead of the caret");
            }
        }
        assert_eq!(cells, w.len());
        assert_eq!(w.len(), w.target_len().max(w.entry_len()));
        // Words behind the caret are submitted; the current one is not, and
        // words ahead of it were never touched.
        match i.cmp(&engine.word_index()) {
            std::cmp::Ordering::Less => assert!(w.submitted(), "word {i} behind the caret"),
            std::cmp::Ordering::Equal => assert!(!w.submitted(), "the current word is open"),
            std::cmp::Ordering::Greater => {
                assert!(!w.submitted() && w.entry_len() == 0, "word {i} untouched")
            }
        }
    }
    assert_eq!(
        engine.cursor_in_word(),
        engine
            .word(engine.word_index())
            .map_or(0, |w| w.entry_len())
    );
}

proptest! {
    /// Any sequence of actions, in any mode, leaves the model coherent.
    #[test]
    fn arbitrary_input_preserves_the_invariants(
        params in params_strategy(),
        script in prop::collection::vec(action_strategy(), 0..250),
    ) {
        let mut engine = Engine::new(params);
        engine.append_words(TARGET.split_whitespace().map(str::to_string));
        for (i, action) in script.into_iter().enumerate() {
            engine.apply(action, Duration::from_millis(80 * i as u64));
            engine.tick(Duration::from_millis(80 * i as u64));
            check_invariants(&engine);
        }
        let s = engine.summary(Duration::from_secs(20));
        prop_assert!(s.wpm.is_finite() && s.raw.is_finite());
        prop_assert!((0.0..=100.0).contains(&s.acc), "acc {}", s.acc);
        prop_assert!((0.0..=100.0).contains(&s.consistency), "consistency {}", s.consistency);
        prop_assert!(s.wpm <= s.raw + 1e-9, "net {} exceeded raw {}", s.wpm, s.raw);
    }

    /// Typing the target exactly is the fixed point of the whole scheme.
    #[test]
    fn typing_the_target_exactly_is_flawless(
        words in prop::collection::vec("[a-z]{1,9}", 1..12),
        secs in 5u64..90,
    ) {
        let target = words.join(" ");
        let mut engine = Engine::new(words_params(&target));
        engine.append_words(words.iter().cloned());
        let mut clock = Duration::ZERO;
        let step = Duration::from_millis(120);
        for (i, w) in words.iter().enumerate() {
            for c in w.chars() {
                engine.apply(Action::Type(c), clock);
                clock += step;
            }
            if i + 1 < words.len() {
                engine.apply(Action::Space, clock);
                clock += step;
            }
        }
        let s = engine.summary(Duration::from_secs(secs));
        prop_assert_eq!(s.acc, 100.0);
        prop_assert_eq!((s.counts.incorrect, s.counts.extra, s.counts.missed), (0, 0, 0));
        // Net credits `correct_word_chars + correct_spaces`; raw credits
        // `all_correct_chars + spaces`. With nothing wrong, those are the same
        // two numbers, so the two lines coincide exactly.
        prop_assert!((s.wpm - s.raw).abs() < 1e-9, "{} vs {}", s.wpm, s.raw);
    }

    /// Consistency is a percentage, whatever you feed it.
    #[test]
    fn consistency_is_always_a_percentage(values in prop::collection::vec(0.0f64..400.0, 0..200)) {
        for kind in [ConsistencyKind::Kogasa, ConsistencyKind::LinearCov] {
            let c = consistency_of(&values, kind);
            prop_assert!((0.0..=100.0).contains(&c), "{c} for {kind:?}");
        }
    }

    /// Kogasa must be monotone: more variation can never mean more consistency.
    #[test]
    fn kogasa_never_increases_with_cov(a in 0.0f64..4.0, b in 0.0f64..4.0) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        prop_assert!(kogasa(lo) >= kogasa(hi), "kogasa({lo}) < kogasa({hi})");
    }
}
