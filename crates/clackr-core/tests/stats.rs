//! Golden vectors for the stats math.
//!
//! Every expected number below is derived by hand from the transcript in the
//! comment above it, so a future reader can re-check the arithmetic without
//! running anything. These vectors *are* the specification: if the code and a
//! vector disagree, the code is wrong until someone redoes the arithmetic.

mod support;

use std::time::Duration;

use clackr_core::stats::{ConsistencyKind, consistency_of, kogasa};
use clackr_core::{Action, EngineEvent};
use support::{Rig, actions, time_params, words_params};

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

/// # Golden vector 1 — one wrong letter in a 12-second time test
///
/// Target: `the quick brown fox jumps`
/// Typed:  `the quixk brown fox jumps`   (no trailing space; the clock stops it)
///
/// 25 keystrokes: 21 letters and 4 spaces, laid down two per second for the
/// first 11 seconds and three in the twelfth.
///
/// `countChars`, word by word:
///
/// | word    | typed   | verdict                                        |
/// |---------|---------|------------------------------------------------|
/// | `the`   | `the`   | perfect: +3 correct-word, +3 all-correct, +1 correct space |
/// | `quick` | `quixk` | same length: 4 match, 1 mismatch (`c` vs `x`)  |
/// | `brown` | `brown` | perfect: +5, +5, +1 correct space              |
/// | `fox`   | `fox`   | perfect: +3, +3, +1 correct space              |
/// | `jumps` | `jumps` | perfect: +5, +5; last word, so no space counted |
///
/// * `correct_word_chars` = 3 + 5 + 3 + 5 = **16**
/// * `all_correct_chars`  = 3 + 4 + 5 + 3 + 5 = **20**
/// * `incorrect` = **1**, `extra` = **0**, `missed` = **0**
/// * `spaces` = 5 words − 1 = **4**, `correct_spaces` = **3**
///
/// * minutes = 12 / 60 = 0.2
/// * wpm = (16 + 3) / 5 / 0.2 = 19 / 5 / 0.2 = **19.0**
/// * raw = (20 + 1 + 0 + 4) / 5 / 0.2 = 25 / 5 / 0.2 = **25.0**
/// * keys correct = 20 letters + 3 spaces = 23; keys incorrect = 1 + 1 = 2
///   (the space after `quixk` submits a flawed word, so it is not credited)
/// * acc = 100 × 23 / 25 = **92.0**
///
/// Series: eleven seconds of 2 keys and one of 3, so instantaneous raw wpm is
/// eleven 24s and one 36.
/// mean = (11×24 + 36) / 12 = 300 / 12 = 25
/// variance = (11×1² + 1×11²) / 12 = 132 / 12 = 11, stddev = √11 = 3.31662479
/// cov = 3.31662479 / 25 = 0.13266499
/// kogasa = 100 × (1 − tanh(cov + cov³/3 + cov⁵/5)) = **86.73351113**
const GOLDEN1_TARGET: &str = "the quick brown fox jumps";
const GOLDEN1_KEYS: &str = "the_quixk_brown_fox_jumps";

fn golden1() -> Rig {
    let mut rig = Rig::new(time_params(12), GOLDEN1_TARGET);
    for (i, action) in actions(GOLDEN1_KEYS).into_iter().enumerate() {
        // Two keys per second, three in the last one.
        let secs = if i < 22 {
            (i / 2) as f64 + if i % 2 == 0 { 0.1 } else { 0.6 }
        } else {
            11.0 + 0.1 + 0.3 * (i - 22) as f64
        };
        rig.press_at(secs, action);
    }
    rig.tick_at(12.0);
    rig
}

#[test]
fn golden1_character_counts() {
    let c = golden1().engine.counts();
    assert_eq!(c.correct_word_chars, 16);
    assert_eq!(c.all_correct_chars, 20);
    assert_eq!(c.incorrect, 1);
    assert_eq!(c.extra, 0);
    assert_eq!(c.missed, 0);
    assert_eq!(c.spaces, 4);
    assert_eq!(c.correct_spaces, 3);
    assert_eq!(c.characters(), (20, 1, 0, 0));
}

#[test]
fn golden1_wpm_raw_and_accuracy() {
    let rig = golden1();
    assert!(rig.engine.is_finished());
    let s = rig.summary();
    assert_eq!(s.duration, Duration::from_secs(12));
    assert!(close(s.wpm, 19.0), "wpm = {}", s.wpm);
    assert!(close(s.raw, 25.0), "raw = {}", s.raw);
    assert!(close(s.acc, 92.0), "acc = {}", s.acc);
    // The gap between 19 and 25 is the point of the whole scheme: one wrong
    // letter costs the entire word on the net line and nothing on the raw one.
    assert!(s.raw > s.wpm);
}

#[test]
fn golden1_series_and_consistency() {
    let s = golden1().summary();
    assert_eq!(s.series.len(), 12);
    let raw: Vec<f64> = s.series.iter().map(|x| x.raw_wpm).collect();
    let mut expected = vec![24.0; 11];
    expected.push(36.0);
    assert_eq!(raw, expected);

    // The wrong "x" lands in second 3; the space that submits `quixk` lands
    // in second 4. Nothing else is an error.
    let errors: Vec<u32> = s.series.iter().map(|x| x.errors).collect();
    assert_eq!(errors, vec![0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0]);

    assert!(
        (s.consistency - 86.733_511_129_992_76).abs() < 1e-6,
        "consistency = {}",
        s.consistency
    );
    // The cumulative line must land on the headline number at the last second.
    assert!(close(s.series.last().unwrap().wpm, s.wpm));
}

/// # Golden vector 2 — every branch of `countChars` in one 10-second test
///
/// Target: `the quick brown fox`
/// Typed:  `the` · `quik` · `brownn` · `fix`   (the last word is not submitted)
///
/// | word    | typed    | branch                | contribution                       |
/// |---------|----------|-----------------------|------------------------------------|
/// | `the`   | `the`    | perfect               | +3 correct-word, +3 all, +1 correct space |
/// | `quick` | `quik`   | shorter than target   | q u i match (+3 all), `k`≠`c` (+1 incorrect), +1 missed |
/// | `brown` | `brownn` | longer than target    | +5 all, +1 extra                    |
/// | `fox`   | `fix`    | same length           | +2 all, +1 incorrect                |
///
/// * `correct_word_chars` = **3**, `all_correct_chars` = 3+3+5+2 = **13**
/// * `incorrect` = **2**, `extra` = **1**, `missed` = **1**
/// * `spaces` = 4 counted words − 1 = **3**, `correct_spaces` = **1**
/// * wpm = (3 + 1) / 5 / (10/60) = 4 × 6 / 5 = **4.8**
/// * raw = (13 + 2 + 1 + 3) / 5 / (10/60) = 19 × 6 / 5 = **22.8**
/// * keys: 16 letters (13 right, 3 wrong) + 3 spaces (1 right, 2 wrong)
///   → acc = 100 × 14 / 19 = **73.68421053**
#[test]
fn golden2_every_count_chars_branch() {
    let mut rig = Rig::new(words_params("the quick brown fox"), "the quick brown fox")
        .step(Duration::from_millis(500));
    rig.feed("the_quik_brownn_fix");
    let s = rig.engine.summary(Duration::from_secs(10));

    assert_eq!(s.counts.correct_word_chars, 3);
    assert_eq!(s.counts.all_correct_chars, 13);
    assert_eq!(s.counts.incorrect, 2);
    assert_eq!(s.counts.extra, 1);
    assert_eq!(s.counts.missed, 1);
    assert_eq!(s.counts.spaces, 3);
    assert_eq!(s.counts.correct_spaces, 1);

    assert!(close(s.wpm, 4.8), "wpm = {}", s.wpm);
    assert!(close(s.raw, 22.8), "raw = {}", s.raw);
    assert!(
        (s.acc - 73.684_210_526_315_8).abs() < 1e-9,
        "acc = {}",
        s.acc
    );
    assert_eq!(
        (rig.engine.keys_correct(), rig.engine.keys_incorrect()),
        (14, 5)
    );
}

/// # Golden vector 3 — a flawless 60-second test is its own arithmetic
///
/// 10 five-letter words typed perfectly in 60 seconds: 50 letters and 9
/// counted spaces, all correct. net = raw = (50 + 9) / 5 / 1 = **11.8**, and a
/// perfect test has nothing to separate the two lines. The final word is left
/// unsubmitted so the test does not end early and pin its own duration.
#[test]
fn golden3_a_perfect_test_has_wpm_equal_to_raw() {
    let target = "alpha bravo delta gamma sigma omega kappa lemma theta zebra";
    let keys = target.replace(' ', "_");
    let mut rig = Rig::new(words_params(target), target);
    rig.feed(&keys);
    let s = rig.engine.summary(Duration::from_secs(60));
    assert_eq!(s.counts.correct_word_chars, 50);
    assert_eq!(s.counts.correct_spaces, 9);
    assert!(close(s.wpm, 11.8), "wpm = {}", s.wpm);
    assert!(close(s.raw, s.wpm));
    assert_eq!(s.acc, 100.0);
    assert_eq!(s.characters(), (50, 0, 0, 0));
}

/// A stall must show up as zero-key seconds, because that is what makes
/// consistency punish it. Hand arithmetic: 4 keys in second 0, nothing for
/// seconds 1–3, 4 keys in second 4. raw series = [48, 0, 0, 0, 48], mean =
/// 19.2, variance = (2×829.44 + 3×368.64)/5 = 552.96, stddev = 23.51595204,
/// cov = 1.224789..., which kogasa maps well below 50.
#[test]
fn a_pause_shows_up_as_zero_key_seconds() {
    let mut rig = Rig::new(time_params(60), "abcd efgh ijkl");
    for (i, a) in actions("abcd").into_iter().enumerate() {
        rig.press_at(0.1 * i as f64, a);
    }
    for (i, a) in actions("_efg").into_iter().enumerate() {
        rig.press_at(4.0 + 0.1 * i as f64, a);
    }
    let s = rig.engine.summary(Duration::from_secs(5));
    let raw: Vec<f64> = s.series.iter().map(|x| x.raw_wpm).collect();
    assert_eq!(raw, vec![48.0, 0.0, 0.0, 0.0, 48.0]);
    assert!(s.consistency < 50.0, "consistency = {}", s.consistency);
}

#[test]
fn consistency_edge_cases() {
    assert_eq!(consistency_of(&[], ConsistencyKind::Kogasa), 100.0);
    assert_eq!(consistency_of(&[60.0], ConsistencyKind::Kogasa), 100.0);
    assert_eq!(consistency_of(&[0.0, 0.0], ConsistencyKind::Kogasa), 0.0);
    assert_eq!(
        consistency_of(&[60.0, 60.0], ConsistencyKind::Kogasa),
        100.0
    );
}

#[test]
fn kogasa_matches_hand_computed_values() {
    // kogasa(0) = 100 * (1 - tanh(0)) = 100
    assert!(close(kogasa(0.0), 100.0));
    // kogasa(1) = 100 * (1 - tanh(1 + 1/3 + 1/5)) = 100 * (1 - tanh(1.5333…))
    //           = 100 * (1 - 0.91099329…) = 8.90067105…
    assert!((kogasa(1.0) - 8.900_671_05).abs() < 1e-6, "{}", kogasa(1.0));
}

#[test]
fn an_untouched_test_reports_zeroes_not_nans() {
    let rig = Rig::new(time_params(30), "the quick brown");
    let s = rig.engine.summary(Duration::from_secs(30));
    assert_eq!(s.wpm, 0.0);
    assert_eq!(s.raw, 0.0);
    assert_eq!(s.acc, 0.0);
    assert!(s.wpm.is_finite() && s.raw.is_finite() && s.acc.is_finite());
    assert!(s.series.is_empty());
    assert_eq!(s.consistency, 100.0);
}

#[test]
fn ending_a_test_by_hand_stops_the_clock_there() {
    let mut rig = Rig::new(time_params(60), "the quick brown");
    rig.press_at(0.1, Action::Type('t'));
    rig.press_at(9.5, Action::EndTest);
    assert!(rig.events.contains(&EngineEvent::Finished));
    let s = rig.engine.summary(Duration::from_secs(99));
    assert_eq!(s.duration, Duration::from_secs_f64(9.5));
    // Seconds 0 through 8 are complete and sealed. The tenth second is a
    // partial one in which nothing was typed, and a partial empty bucket is
    // time that never really happened, so it is not emitted.
    assert_eq!(s.series.len(), 9);
}
