//! Parses the embedded quote corpus (`assets/quotes/english.toml`, see
//! `assets/LICENSES.md` for provenance) and selects quotes by
//! [`QuoteLength`].
//!
//! The corpus ships as TOML rather than JSON specifically to avoid adding
//! `serde_json` as a dependency of this crate -- the file is ours, its
//! shape is fixed, and `toml` is already a dependency for theme parsing.

use std::sync::OnceLock;

use rand::{Rng, RngExt};
use serde::Deserialize;

use super::embedded;
use crate::mode::QuoteLength;

/// One entry from the quote corpus.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Quote {
    pub text: String,
    pub author: String,
    pub source: String,
    /// The bucket this quote was hand-classified into when the corpus was
    /// assembled. Selection (`matching`/`pick`) does not trust this field
    /// on its own -- it re-checks `length.accepts(text.chars().count())`
    /// -- but a test in this module asserts the two always agree, so a
    /// future quote added with a wrong `length` fails loudly at test time
    /// rather than silently sorting into the wrong bucket at runtime.
    pub length: QuoteLength,
}

#[derive(Deserialize)]
struct QuotesFile {
    quote: Vec<Quote>,
}

fn all_quotes() -> &'static [Quote] {
    static QUOTES: OnceLock<Vec<Quote>> = OnceLock::new();
    QUOTES
        .get_or_init(|| {
            let file: QuotesFile = toml::from_str(embedded::QUOTES_EN).unwrap_or_else(|e| {
                panic!(
                    "embedded quotes TOML failed to parse at runtime even though \
                     build.rs validated it: {e}"
                )
            });
            assert!(
                !file.quote.is_empty(),
                "embedded quotes file has zero quotes"
            );
            file.quote
        })
        .as_slice()
}

/// Every embedded quote, in file order.
pub fn all() -> &'static [Quote] {
    all_quotes()
}

/// Every embedded quote whose *actual* character count falls in `length`'s
/// bucket (via [`QuoteLength::accepts`]), not merely the ones whose stored
/// `length` field says so.
pub fn matching(length: QuoteLength) -> impl Iterator<Item = &'static Quote> {
    all_quotes()
        .iter()
        .filter(move |q| length.accepts(q.text.chars().count()))
}

/// Picks one uniformly-random quote matching `length`, or `None` if the
/// corpus has no quote in that bucket.
pub fn pick(length: QuoteLength, rng: &mut dyn Rng) -> Option<&'static Quote> {
    let candidates: Vec<&Quote> = matching(length).collect();
    if candidates.is_empty() {
        return None;
    }
    Some(candidates[rng.random_range(0..candidates.len())])
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    use super::*;

    #[test]
    fn corpus_is_non_empty() {
        assert!(!all().is_empty());
    }

    #[test]
    fn every_quote_length_agrees_with_its_character_count() {
        for q in all() {
            let n = q.text.chars().count();
            assert!(
                q.length.accepts(n),
                "quote {:?} (from {}) declares length={:?} but has {n} chars",
                &q.text[..q.text.len().min(40)],
                q.source,
                q.length,
            );
        }
    }

    #[test]
    fn every_quote_is_pure_ascii_and_non_empty() {
        for q in all() {
            assert!(
                q.text.is_ascii(),
                "quote from {} is not pure ASCII: {:?}",
                q.source,
                q.text
            );
            assert!(
                !q.text.trim().is_empty(),
                "quote from {} is empty",
                q.source
            );
        }
    }

    #[test]
    fn matching_only_returns_quotes_in_bucket() {
        for length in [QuoteLength::Short, QuoteLength::Medium, QuoteLength::Long] {
            for q in matching(length) {
                assert!(length.accepts(q.text.chars().count()));
            }
        }
        // `All` accepts every quote in the corpus.
        assert_eq!(matching(QuoteLength::All).count(), all().len());
    }

    #[test]
    fn every_bucket_has_at_least_one_quote() {
        for length in [QuoteLength::Short, QuoteLength::Medium, QuoteLength::Long] {
            assert!(
                matching(length).next().is_some(),
                "no quotes in the {length:?} bucket"
            );
        }
    }

    #[test]
    fn pick_is_seedable_and_always_in_bucket() {
        let mut rng = StdRng::seed_from_u64(7);
        for _ in 0..50 {
            let q = pick(QuoteLength::Short, &mut rng).expect("short bucket is non-empty");
            assert!(QuoteLength::Short.accepts(q.text.chars().count()));
        }
    }
}
