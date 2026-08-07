//! [`RandomWords`]: a [`WordSource`] that draws uniformly at random from a
//! fixed word pool (an embedded language, typically), never repeating the
//! same word twice in a row.

use rand::{Rng, RngExt};

use super::WordSource;

/// Punctuation/numbers injection. Parsed from `config::TestConfig` and
/// plumbed all the way down to here so the CLI/config surface is stable,
/// but currently a no-op: punctuation and numbers modifiers are a deferred
/// v2 feature (see the project plan's "Deferred to GitHub issues" list).
// TODO(v2): make `RandomWords::next` actually inject punctuation/numbers
// into generated words instead of ignoring these flags.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub punctuation: bool,
    pub numbers: bool,
}

/// Draws words uniformly at random from a fixed `pool`, guaranteeing the
/// same word is never emitted twice back-to-back.
///
/// `RandomWords` itself holds no RNG state -- [`WordSource::next`] takes the
/// `Rng` as an argument, so determinism comes from seeding *that* RNG
/// (e.g. `rand::rngs::StdRng::seed_from_u64`), not from anything stored
/// here. This is what makes `RandomWords` itself trivially `Clone`/reusable
/// and keeps `clackr-core` clock- and randomness-source-free per its own
/// rule (see `lib.rs`): the crate never calls `rand::rng()` internally.
pub struct RandomWords {
    pool: &'static [&'static str],
    last: Option<&'static str>,
    #[allow(dead_code)] // TODO(v2): wire into word generation.
    modifiers: Modifiers,
}

impl RandomWords {
    /// # Panics
    /// Panics if `pool` is empty -- there would be no word to return.
    pub fn new(pool: &'static [&'static str]) -> Self {
        Self::with_modifiers(pool, Modifiers::default())
    }

    /// # Panics
    /// Panics if `pool` is empty -- there would be no word to return.
    pub fn with_modifiers(pool: &'static [&'static str], modifiers: Modifiers) -> Self {
        assert!(!pool.is_empty(), "RandomWords pool must not be empty");
        Self {
            pool,
            last: None,
            modifiers,
        }
    }
}

impl WordSource for RandomWords {
    fn next(&mut self, rng: &mut dyn Rng) -> String {
        let word = if self.pool.len() == 1 {
            // Only one word in the pool: the "never repeat" rule is
            // unsatisfiable, so fall back to just returning it rather than
            // looping forever.
            self.pool[0]
        } else {
            loop {
                let candidate = self.pool[rng.random_range(0..self.pool.len())];
                if self.last != Some(candidate) {
                    break candidate;
                }
            }
        };
        self.last = Some(word);
        word.to_string()
    }

    fn is_finite(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    use super::*;

    const POOL: &[&str] = &[
        "the", "quick", "brown", "fox", "jumps", "over", "lazy", "dog",
    ];

    #[test]
    fn never_repeats_back_to_back() {
        let mut rng = StdRng::seed_from_u64(1);
        let mut source = RandomWords::new(POOL);
        let mut prev: Option<String> = None;
        for _ in 0..2000 {
            let word = source.next(&mut rng);
            assert!(POOL.contains(&word.as_str()));
            if let Some(p) = &prev {
                assert_ne!(p, &word, "repeated word back-to-back");
            }
            prev = Some(word);
        }
    }

    #[test]
    fn single_word_pool_never_loops_forever() {
        let mut rng = StdRng::seed_from_u64(1);
        let mut source = RandomWords::new(&["only"]);
        for _ in 0..50 {
            assert_eq!(source.next(&mut rng), "only");
        }
    }

    #[test]
    fn is_not_finite() {
        assert!(!RandomWords::new(POOL).is_finite());
    }

    #[test]
    fn same_seed_is_deterministic() {
        let run = || {
            let mut rng = StdRng::seed_from_u64(42);
            let mut source = RandomWords::new(POOL);
            (0..20).map(|_| source.next(&mut rng)).collect::<Vec<_>>()
        };
        assert_eq!(run(), run());
    }

    /// A 200-word sequence from `StdRng::seed_from_u64(1234)` over `POOL`,
    /// captured once and hardcoded here. This is a true snapshot, not just
    /// a self-consistency check: if a future `rand` upgrade changes
    /// `StdRng`'s output for the same seed (a real possibility across major
    /// versions, though not across patch versions), this is what catches
    /// it -- silently reproducing a *different* sequence for the same seed
    /// would quietly break every other seeded/deterministic test in this
    /// crate along with reproducibility of bug reports built on a fixed
    /// seed.
    #[test]
    fn two_hundred_word_snapshot_is_deterministic() {
        #[rustfmt::skip]
        const EXPECTED: [&str; 200] = [
            "the", "dog", "quick", "brown", "jumps", "over", "fox", "over", "dog", "the",
            "dog", "fox", "lazy", "dog", "lazy", "the", "over", "dog", "brown", "lazy",
            "jumps", "the", "fox", "over", "brown", "fox", "lazy", "jumps", "over", "the",
            "lazy", "jumps", "brown", "quick", "dog", "over", "jumps", "over", "brown", "fox",
            "over", "quick", "brown", "lazy", "brown", "lazy", "over", "jumps", "the", "over",
            "lazy", "over", "dog", "the", "jumps", "quick", "the", "lazy", "jumps", "dog",
            "lazy", "brown", "fox", "the", "dog", "brown", "over", "brown", "quick", "fox",
            "lazy", "brown", "quick", "over", "jumps", "lazy", "the", "fox", "the", "quick",
            "the", "dog", "fox", "quick", "the", "lazy", "quick", "brown", "quick", "the",
            "quick", "the", "fox", "quick", "brown", "lazy", "over", "dog", "quick", "lazy",
            "quick", "over", "dog", "quick", "dog", "quick", "fox", "quick", "dog", "lazy",
            "over", "lazy", "fox", "jumps", "over", "lazy", "fox", "the", "brown", "quick",
            "jumps", "the", "fox", "over", "quick", "jumps", "brown", "lazy", "dog", "over",
            "dog", "fox", "over", "jumps", "fox", "quick", "brown", "over", "the", "over",
            "jumps", "fox", "the", "quick", "the", "fox", "quick", "over", "jumps", "over",
            "dog", "quick", "the", "over", "brown", "lazy", "brown", "lazy", "over", "brown",
            "the", "fox", "over", "brown", "jumps", "brown", "lazy", "the", "brown", "over",
            "the", "jumps", "quick", "lazy", "jumps", "fox", "jumps", "dog", "brown", "fox",
            "lazy", "quick", "lazy", "brown", "lazy", "fox", "brown", "jumps", "the", "brown",
            "fox", "quick", "dog", "the", "lazy", "fox", "jumps", "lazy", "brown", "fox",
        ];

        let mut rng = StdRng::seed_from_u64(1234);
        let mut source = RandomWords::new(POOL);
        let words: Vec<String> = (0..200).map(|_| source.next(&mut rng)).collect();
        let words: Vec<&str> = words.iter().map(String::as_str).collect();

        assert_eq!(
            words, EXPECTED,
            "seed 1234 must reproduce this exact 200-word sequence"
        );

        for pair in words.windows(2) {
            assert_ne!(
                pair[0], pair[1],
                "no back-to-back repeat in the snapshot run"
            );
        }
    }
}
