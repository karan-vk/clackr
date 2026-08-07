//! Consistency: how even the typing was, from 0 (erratic) to 100 (metronome).
//!
//! The obvious formula is `1 - stddev/mean`, and it is not what MonkeyType
//! uses. MonkeyType applies a transform its author named **kogasa**, which
//! feeds the coefficient of variation through the first three terms of
//! `atanh`'s series and then a `tanh`:
//!
//! ```text
//! kogasa(cov) = 100 * (1 - tanh(cov + cov^3/3 + cov^5/5))
//! ```
//!
//! Since `atanh(x) = x + x^3/3 + x^5/5 + …`, the inner sum is a truncated
//! `atanh`, so for small `cov` the whole thing is very nearly `100 * (1 - cov)`
//! — the naive formula — but it saturates smoothly instead of going negative
//! for a wildly uneven test. That is the point: it never needs clamping at the
//! bottom, and a small wobble is punished about as much as intuition says.

use super::SecondSample;

/// Which consistency formula to use. Behind an enum because the kogasa
/// constant is inferred from MonkeyType's behaviour rather than a spec; if a
/// golden vector ever disagrees, swapping this is a one-line change.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum ConsistencyKind {
    #[default]
    Kogasa,
    /// The naive `100 * (1 - cov)`, clamped.
    LinearCov,
}

/// MonkeyType's transform. Monotonically decreasing on `cov >= 0`.
pub fn kogasa(cov: f64) -> f64 {
    100.0 * (1.0 - (cov + cov.powi(3) / 3.0 + cov.powi(5) / 5.0).tanh())
}

/// Consistency of the **instantaneous raw WPM** series.
///
/// Instantaneous is load-bearing: a cumulative series converges by
/// construction, so its variance would measure nothing but the length of the
/// test.
pub fn consistency(samples: &[SecondSample], kind: ConsistencyKind) -> f64 {
    let values: Vec<f64> = samples.iter().map(|s| s.raw_wpm).collect();
    consistency_of(&values, kind)
}

/// The same computation over a bare series, so it can be unit-tested and
/// hand-checked without building samples.
pub fn consistency_of(values: &[f64], kind: ConsistencyKind) -> f64 {
    // A single second has no variation to measure; calling that 100% is both
    // MonkeyType's behaviour and the only non-arbitrary answer.
    if values.len() < 2 {
        return 100.0;
    }
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    if mean <= 0.0 {
        // Typed nothing at all: there is no consistency to speak of.
        return 0.0;
    }
    // Population stddev, not sample: these seconds are the whole test, not a
    // sample drawn from a larger population.
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    let cov = variance.sqrt() / mean;
    let raw = match kind {
        ConsistencyKind::Kogasa => kogasa(cov),
        ConsistencyKind::LinearCov => 100.0 * (1.0 - cov),
    };
    raw.clamp(0.0, 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfectly_even_typing_is_one_hundred() {
        assert_eq!(consistency_of(&[60.0; 10], ConsistencyKind::Kogasa), 100.0);
    }

    #[test]
    fn fewer_than_two_samples_is_one_hundred() {
        assert_eq!(consistency_of(&[], ConsistencyKind::Kogasa), 100.0);
        assert_eq!(consistency_of(&[42.0], ConsistencyKind::Kogasa), 100.0);
    }

    #[test]
    fn an_empty_test_is_zero() {
        assert_eq!(
            consistency_of(&[0.0, 0.0, 0.0], ConsistencyKind::Kogasa),
            0.0
        );
    }

    #[test]
    fn kogasa_is_gentler_than_the_naive_formula_when_it_matters() {
        // Small cov: the two agree to within a fraction of a point, because
        // the inner sum is a truncated atanh.
        let near = (kogasa(0.1) - 100.0 * (1.0 - 0.1)).abs();
        assert!(near < 0.5, "kogasa(0.1) = {}", kogasa(0.1));
        // Large cov: the naive formula goes negative (and would need clamping,
        // losing all resolution down there); kogasa stays positive.
        assert!(kogasa(2.0) > 0.0);
        assert!(consistency_of(&[0.0, 120.0, 0.0, 120.0], ConsistencyKind::LinearCov) == 0.0);
        assert!(consistency_of(&[0.0, 120.0, 0.0, 120.0], ConsistencyKind::Kogasa) > 0.0);
    }

    #[test]
    fn kogasa_decreases_monotonically() {
        // Strictly decreasing over any cov a real test can produce. Past
        // cov ~= 2.3 the inner sum exceeds the point where `tanh` rounds to
        // exactly 1.0 in f64 and the curve is pinned at 0 — still monotone,
        // just no longer strict, which is why the two ranges are checked
        // separately.
        let mut prev = f64::INFINITY;
        for i in 0..=100 {
            let v = kogasa(f64::from(i) / 50.0);
            assert!(
                v < prev,
                "not strictly decreasing at cov={}",
                f64::from(i) / 50.0
            );
            prev = v;
        }
        for i in 100..400 {
            let v = kogasa(f64::from(i) / 50.0);
            assert!(v <= prev, "not monotone at cov={}", f64::from(i) / 50.0);
            prev = v;
        }
    }
}
