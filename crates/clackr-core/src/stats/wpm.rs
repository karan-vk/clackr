//! Words per minute, raw words per minute, accuracy.
//!
//! "A word" is five characters — the standard convention, and the reason every
//! formula here divides by five rather than counting spaces.

use std::time::Duration;

/// Characters per word, by convention.
pub const CHARS_PER_WORD: f64 = 5.0;

pub fn minutes(elapsed: Duration) -> f64 {
    elapsed.as_secs_f64() / 60.0
}

/// `chars / 5 / minutes`, guarded so a zero-length test yields `0.0` rather
/// than an infinity that would poison the chart's y-axis.
pub fn wpm(chars: u32, elapsed: Duration) -> f64 {
    let minutes = minutes(elapsed);
    if minutes <= 0.0 {
        return 0.0;
    }
    f64::from(chars) / CHARS_PER_WORD / minutes
}

/// Accuracy from the keypress counters, which are only ever incremented — a
/// typo you backspaced away still happened, so accuracy cannot be repaired.
pub fn accuracy(correct: u32, incorrect: u32) -> f64 {
    let total = correct + incorrect;
    if total == 0 {
        return 0.0;
    }
    100.0 * f64::from(correct) / f64::from(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_duration_does_not_produce_infinity() {
        assert_eq!(wpm(100, Duration::ZERO), 0.0);
        assert!(wpm(100, Duration::ZERO).is_finite());
    }

    #[test]
    fn sixty_seconds_of_five_char_words() {
        // 60 words * 5 chars = 300 chars in one minute is 60 wpm by definition.
        assert_eq!(wpm(300, Duration::from_secs(60)), 60.0);
        // The same text in half the time is twice the speed.
        assert_eq!(wpm(300, Duration::from_secs(30)), 120.0);
    }

    #[test]
    fn accuracy_of_nothing_is_zero() {
        assert_eq!(accuracy(0, 0), 0.0);
        assert_eq!(accuracy(23, 2), 92.0);
        assert_eq!(accuracy(1, 0), 100.0);
    }
}
