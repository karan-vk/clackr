//! Engine rules, one named test each, written as keystroke transcripts.

mod support;

use std::time::Duration;

use clackr_core::mode::{StopOnError, TestParams};
use clackr_core::{Action, EngineEvent};
use support::{Rig, script, script_with, states, time_params, typed, words_params, zen_params};

// ---- rule 1: zero-width scalars merge into the previous cell --------------

#[test]
fn rule1_zero_width_marks_merge_into_the_previous_cell() {
    // The target is the *decomposed* form: c a f e + U+0301. crossterm hands
    // us one KeyCode::Char per scalar, so the accent arrives as its own event
    // and must extend the "e" cell instead of becoming a fifth cell.
    let e = script("cafe\u{301} x", "cafe\u{301}_");
    assert_eq!(e.word(0).unwrap().len(), 4);
    assert_eq!(states(&e, 0), "cccc");
    // Accuracy is sampled at keypress time and never repaired, so the bare "e"
    // was an error for the instant before the accent landed. That is
    // MonkeyType's rule working as designed, not a bug in the merge.
    assert_eq!(e.keys_incorrect(), 1);
    assert_eq!(e.keys_correct(), 5); // c, a, f, the accent, and the space
}

#[test]
fn rule1_combining_mark_with_no_base_is_dropped_and_not_counted() {
    let e = script("abc x", "\u{301}abc");
    assert_eq!(typed(&e, 0), "abc");
    assert_eq!(e.word(0).unwrap().len(), 3);
    assert_eq!((e.keys_correct(), e.keys_incorrect()), (3, 0));
}

#[test]
fn rule1_backspace_removes_the_whole_cluster() {
    let e = script("cafe\u{301} x", "cafe\u{301}<");
    assert_eq!(typed(&e, 0), "caf");
    assert_eq!(e.cursor_in_word(), 3);
}

// ---- rule 2: extras are capped -------------------------------------------

#[test]
fn rule2_extras_are_capped_at_twenty_per_word() {
    let keys = format!("ab{}", "x".repeat(30));
    let e = script("ab cd", &keys);
    // 2 target cells + 20 extras, and not one cell more, so line wrapping
    // cannot be blown up by a stuck key.
    assert_eq!(e.word(0).unwrap().len(), 22);
    assert_eq!(e.counts().extra, 20);
    // The keystrokes past the cap still happened and are still wrong.
    assert_eq!(e.keys_incorrect(), 30);
    assert_eq!(e.keys_correct(), 2);
}

// ---- rule 3: space --------------------------------------------------------

#[test]
fn rule3_space_on_an_empty_word_is_not_a_keypress() {
    // Leaning on the space bar between words must be free: no advance, no
    // accuracy cost, nothing to redraw.
    let e = script("the quick", "_the_");
    assert_eq!(e.word_index(), 1);
    assert_eq!((e.keys_correct(), e.keys_incorrect()), (4, 0));

    let mut rig = Rig::words("the quick");
    let out = rig.engine.apply(Action::Space, Duration::ZERO);
    assert_eq!(out, clackr_core::Outcome::IDLE);
}

#[test]
fn rule3_space_is_correct_only_after_a_perfect_word() {
    let e = script("the quick", "teh_");
    // t correct, e and h wrong, then a space that submits a flawed word.
    assert_eq!((e.keys_correct(), e.keys_incorrect()), (1, 3));
    assert_eq!(e.word_index(), 1);
}

#[test]
fn rule3_submitting_early_leaves_missed_cells() {
    let e = script("the quick brown", "the_qui_brown_");
    assert_eq!(e.counts().missed, 2);
    assert_eq!(states(&e, 1), "cccmm");
}

// ---- rule 4: backspace ----------------------------------------------------

#[test]
fn rule4_backspace_refuses_to_reenter_a_perfect_word() {
    // MonkeyType's rule: backspacing into a word you already got right can
    // only be an accident, and the accident is expensive.
    let e = script("the quick", "the_<");
    assert_eq!(e.word_index(), 1);
    assert_eq!(states(&e, 0), "ccc");
}

#[test]
fn rule4_backspace_reenters_a_flawed_word_at_the_end_of_the_input() {
    let e = script("the quick", "teh_<");
    assert_eq!(e.word_index(), 0);
    assert_eq!(e.cursor_in_word(), 3);
    assert_eq!(typed(&e, 0), "teh");
    // Re-entry clears `submitted`, so the trailing cells go back to Untyped
    // rather than showing as Missed.
    assert_eq!(states(&e, 1), ".....");
}

#[test]
fn rule4_freedom_mode_reenters_even_a_perfect_word() {
    let params = TestParams {
        freedom_mode: true,
        ..words_params("the quick")
    };
    let e = script_with(params, "the quick", "the_<");
    assert_eq!(e.word_index(), 0);
    assert_eq!(e.cursor_in_word(), 3);
}

#[test]
fn rule4_backspace_never_touches_accuracy() {
    let e = script("abc def", "abc<<<abc_");
    assert_eq!(typed(&e, 0), "abc");
    // Six letters and one space were typed correctly; the three backspaces
    // are invisible to the counters, so a fixed typo still costs you.
    assert_eq!((e.keys_correct(), e.keys_incorrect()), (7, 0));
    assert_eq!(e.counts().incorrect, 0);
}

#[test]
fn rule4_backspace_at_the_very_start_does_nothing() {
    let e = script("abc def", "<<<");
    assert_eq!(e.word_index(), 0);
    assert_eq!(e.cursor_in_word(), 0);
    assert!(!e.is_finished());
}

// ---- rule 5: delete word --------------------------------------------------

#[test]
fn rule5_delete_word_clears_the_current_word() {
    let e = script("the quick", "teh^");
    assert_eq!(typed(&e, 0), "");
    assert_eq!(e.word_index(), 0);
}

#[test]
fn rule5_delete_word_on_an_empty_word_steps_back_and_clears() {
    let e = script("the quick", "teh_^");
    assert_eq!(e.word_index(), 0);
    assert_eq!(typed(&e, 0), "");
    assert_eq!(states(&e, 0), "...");
}

#[test]
fn rule5_delete_word_honours_the_perfect_word_rule() {
    let e = script("the quick", "the_^");
    assert_eq!(e.word_index(), 1);
    assert_eq!(typed(&e, 0), "the");
}

// ---- rule 6: stop on error ------------------------------------------------

#[test]
fn rule6_stop_on_error_letter_rejects_the_keystroke_but_counts_it() {
    let params = TestParams {
        stop_on_error: StopOnError::Letter,
        ..words_params("cat dog")
    };
    let e = script_with(params, "cat dog", "cxat_");
    // The "x" never lands, so the word is still clean...
    assert_eq!(typed(&e, 0), "cat");
    assert_eq!(e.counts().incorrect, 0);
    // ...but the mistake is still on the accuracy ledger.
    assert_eq!((e.keys_correct(), e.keys_incorrect()), (4, 1));
}

#[test]
fn rule6_stop_on_error_word_blocks_the_space_until_the_word_is_right() {
    let params = TestParams {
        stop_on_error: StopOnError::Word,
        ..words_params("cat dog")
    };
    let mut rig = Rig::new(params, "cat dog");
    rig.feed("cta_");
    assert_eq!(
        rig.engine.word_index(),
        0,
        "space must not submit a bad word"
    );
    // A blocked space is swallowed whole, exactly like rule 3's empty space.
    assert_eq!(rig.engine.keys_correct() + rig.engine.keys_incorrect(), 3);
    rig.feed("<<<cat_");
    assert_eq!(rig.engine.word_index(), 1);
}

// ---- rule 7: zen ----------------------------------------------------------

#[test]
fn rule7_zen_mode_is_all_correct_grows_forever_and_ends_only_on_command() {
    let mut rig = Rig::new(zen_params(), "");
    rig.feed("hi_there");
    assert!(!rig.engine.is_finished());
    assert_eq!(rig.engine.word_count(), 2);
    assert_eq!(states(&rig.engine, 0), "cc");
    assert_eq!(states(&rig.engine, 1), "ccccc");

    rig.press(Action::EndTest);
    assert!(rig.engine.is_finished());
    let counts = rig.engine.counts();
    assert_eq!(counts.incorrect + counts.extra + counts.missed, 0);
    assert_eq!(counts.correct_word_chars, 7);
    assert_eq!(rig.summary().acc, 100.0);
}

#[test]
fn rule7_zen_ignores_appended_words() {
    let mut rig = Rig::new(zen_params(), "");
    rig.engine
        .append_words(["one".to_string(), "two".to_string()]);
    assert_eq!(rig.engine.word_count(), 1);
    assert_eq!(states(&rig.engine, 0), "");
}

#[test]
fn rule7_zen_backspaces_back_into_earlier_words() {
    // Every zen word is "perfect" by definition, so the perfect-word guard
    // would lock the user out of their own text without the zen exception.
    let mut rig = Rig::new(zen_params(), "");
    rig.feed("hi_<");
    assert_eq!(rig.engine.word_index(), 0);
    assert_eq!(rig.engine.cursor_in_word(), 2);
}

// ---- rule 8: time mode ----------------------------------------------------

#[test]
fn rule8_time_mode_finishes_when_the_clock_runs_out() {
    let mut rig = Rig::new(time_params(5), "the quick brown fox");
    rig.feed("the_");
    assert!(!rig.tick_at(4.9).engine.is_finished());
    rig.tick_at(5.0);
    assert!(rig.engine.is_finished());
    assert!(rig.events.contains(&EngineEvent::Finished));
    // The duration is clamped to the limit, so a late tick cannot dilute WPM.
    assert_eq!(
        rig.engine.summary(Duration::from_secs(9)).duration,
        Duration::from_secs(5)
    );
}

#[test]
fn rule8_time_mode_asks_for_more_words_before_it_runs_out() {
    let mut rig = Rig::new(time_params(60), "one two three");
    rig.feed("o");
    // The first keystroke reports Started; the word request is level-triggered
    // and arrives on the next tick, so nothing is lost by the collision.
    rig.tick_at(0.5);
    let need = rig
        .events
        .iter()
        .find_map(|e| match e {
            EngineEvent::NeedWords(n) => Some(*n),
            _ => None,
        })
        .expect("expected a NeedWords request");
    assert!(need >= 40, "asked for {need} words");

    // Once the words land the request stops repeating.
    rig.engine.append_words((0..200).map(|i| format!("w{i}")));
    rig.events.clear();
    rig.tick_at(1.5);
    assert!(rig.events.is_empty(), "{:?}", rig.events);
}

#[test]
fn rule8_unsubmitted_final_word_is_not_charged_for_missed_characters() {
    // The clock cut the user off mid-word; charging them for the tail of a
    // word they were still typing would be the test's fault, not theirs.
    let mut rig = Rig::new(time_params(30), "the quick brown");
    rig.feed("the_qui");
    assert_eq!(rig.engine.counts().missed, 0);
    assert_eq!(states(&rig.engine, 1), "ccc..");
}

// ---- rule 9: words / quote mode -------------------------------------------

#[test]
fn rule9_words_mode_finishes_when_the_last_word_is_submitted() {
    let mut rig = Rig::words("the quick brown");
    rig.feed("the_quick_");
    assert!(!rig.engine.is_finished());
    rig.feed("brown_");
    assert!(rig.engine.is_finished());
    assert!(rig.events.contains(&EngineEvent::Finished));
}

#[test]
fn a_finished_engine_ignores_everything() {
    let mut rig = Rig::words("hi");
    rig.feed("hi_");
    assert!(rig.engine.is_finished());
    let before = rig.engine.counts();
    let out = rig.engine.apply(Action::Type('x'), Duration::from_secs(1));
    assert_eq!(out, clackr_core::Outcome::IDLE);
    assert_eq!(rig.engine.counts(), before);
}

// ---- derived state --------------------------------------------------------

#[test]
fn extras_widen_a_word_and_render_past_the_target() {
    let e = script("ab cd", "abzz");
    assert_eq!(states(&e, 0), "ccee");
    assert_eq!(e.word(0).unwrap().len(), 4);
    assert_eq!(e.counts().extra, 2);
}

#[test]
fn untouched_words_ahead_of_the_caret_score_nothing() {
    let e = script("the quick brown fox", "the_");
    let c = e.counts();
    assert_eq!((c.missed, c.incorrect, c.extra), (0, 0, 0));
    // One word reached, so no space is counted after it either.
    assert_eq!(c.spaces, 0);
    assert_eq!(c.correct_word_chars, 3);
}

#[test]
fn started_fires_once_on_the_first_counted_keystroke() {
    let mut rig = Rig::words("abc def");
    // Backspace is not a keystroke: it must not start the clock.
    rig.press(Action::Backspace);
    assert!(rig.events.is_empty());
    assert!(!rig.engine.is_started());
    rig.feed("abc");
    assert_eq!(
        rig.events
            .iter()
            .filter(|e| **e == EngineEvent::Started)
            .count(),
        1
    );
}

#[test]
fn a_literal_space_character_is_the_same_as_the_space_action() {
    let mut rig = Rig::words("the quick");
    rig.feed("the");
    rig.press(Action::Type(' '));
    assert_eq!(rig.engine.word_index(), 1);
    assert_eq!(rig.engine.counts().extra, 0);
}

#[test]
fn tick_is_idempotent() {
    let mut rig = Rig::new(time_params(60), "the quick brown fox");
    rig.feed("the_qu");
    rig.tick_at(1.0);
    let a = rig.engine.summary(Duration::from_secs(1));
    rig.tick_at(1.0);
    rig.tick_at(1.0);
    let b = rig.engine.summary(Duration::from_secs(1));
    assert_eq!(a, b);
}

#[test]
fn typing_into_an_exhausted_word_list_is_swallowed_not_miscounted() {
    // Time mode with a caller that has not streamed more words yet.
    let mut rig = Rig::new(time_params(60), "hi");
    rig.feed("hi_xyz");
    assert_eq!(rig.engine.word_index(), 1);
    assert_eq!(
        (rig.engine.keys_correct(), rig.engine.keys_incorrect()),
        (3, 0)
    );
}
