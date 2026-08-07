//! The stats math: character tallies, WPM, accuracy, consistency, and the
//! per-second series behind the results chart.
//!
//! Every formula here is MonkeyType's, including the ones that look wrong at
//! first glance. Where the choice is genuinely inferred rather than documented
//! it sits behind an enum ([`ConsistencyKind`], [`SeriesMode`]) so a golden
//! vector can overrule it without a refactor.

mod consistency;
mod counts;
mod series;
mod wpm;

use std::time::Duration;

use serde::{Deserialize, Serialize};

pub use consistency::{ConsistencyKind, consistency, consistency_of, kogasa};
pub use counts::{CharCounts, count_chars};
pub use series::{SecondSample, SecondSampler, SeriesMode};
pub use wpm::{CHARS_PER_WORD, accuracy, minutes, wpm};

use crate::mode::TestParams;

/// Everything the results screen needs, and nothing it does not.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    pub params: TestParams,
    /// Net WPM: only 100%-correct words count.
    pub wpm: f64,
    /// Raw WPM: everything the fingers produced.
    pub raw: f64,
    pub acc: f64,
    pub consistency: f64,
    pub counts: CharCounts,
    pub duration: Duration,
    pub series: Vec<SecondSample>,
}

impl Summary {
    /// The results-screen "characters" quadruple.
    pub fn characters(&self) -> (u32, u32, u32, u32) {
        self.counts.characters()
    }

    /// A realistic, fully self-consistent result for snapshot tests and design
    /// work: ten seconds, ten perfect words, 61.2 wpm, 100% accuracy.
    ///
    /// Everything is derived from `KEYS_PER_SECOND` below rather than pasted
    /// in, so the numbers cannot drift apart from each other.
    pub fn fixture() -> Summary {
        // Ten words totalling 42 characters, plus the 9 spaces between them:
        // 51 keystrokes, spread with one slightly faster second so the chart
        // and the consistency number have something to show.
        const KEYS_PER_SECOND: [u32; 10] = [5, 5, 6, 5, 5, 5, 5, 5, 5, 5];
        const CHARS: u32 = 42;
        const WORDS: u32 = 10;

        let spaces = WORDS - 1;
        let counts = CharCounts {
            correct_word_chars: CHARS,
            all_correct_chars: CHARS,
            incorrect: 0,
            extra: 0,
            missed: 0,
            spaces,
            correct_spaces: spaces,
        };
        let duration = Duration::from_secs(KEYS_PER_SECOND.len() as u64);

        let mut cumulative = 0u32;
        let series: Vec<SecondSample> = KEYS_PER_SECOND
            .iter()
            .enumerate()
            .map(|(i, &keys)| {
                cumulative += keys;
                let second = i as u32;
                SecondSample {
                    second,
                    keys,
                    errors: 0,
                    raw_wpm: f64::from(keys) * 12.0,
                    wpm: f64::from(cumulative) * 12.0 / f64::from(second + 1),
                }
            })
            .collect();
        debug_assert_eq!(cumulative, counts.net_chars());

        Summary {
            params: TestParams::default(),
            wpm: wpm(counts.net_chars(), duration),
            raw: wpm(counts.raw_chars(), duration),
            acc: accuracy(cumulative, 0),
            consistency: consistency(&series, ConsistencyKind::default()),
            counts,
            duration,
            series,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_is_internally_consistent() {
        let s = Summary::fixture();
        assert_eq!(s.series.len(), 10);
        assert_eq!(s.duration, Duration::from_secs(10));
        // 51 net chars / 5 / (10/60 min) = 61.2
        assert!((s.wpm - 61.2).abs() < 1e-9, "{}", s.wpm);
        // A flawless test has nothing between net and raw.
        assert!((s.raw - s.wpm).abs() < 1e-9);
        assert_eq!(s.acc, 100.0);
        assert_eq!(s.characters(), (42, 0, 0, 0));
        // The last cumulative sample must land on the headline number.
        assert!((s.series.last().unwrap().wpm - s.wpm).abs() < 1e-9);
        assert!(s.consistency > 90.0 && s.consistency < 100.0);
    }

    #[test]
    fn summary_round_trips_through_toml() {
        // Proves the serde remote derive for TestParams actually works in both
        // directions; the UI crate persists nothing, but the shape is public.
        let s = Summary::fixture();
        let text = toml::to_string(&s).unwrap();
        let back: Summary = toml::from_str(&text).unwrap();
        assert_eq!(back, s);
    }
}
