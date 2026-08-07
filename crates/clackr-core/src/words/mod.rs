//! Word sources: the embedded language/theme/quote assets baked in by
//! `build.rs` (see `assets/LICENSES.md` for provenance), plus the
//! RNG-driven word generator that turns a language's word pool into a
//! stream of words for `time`/`words` mode.

mod embedded;
pub mod generator;
pub mod quotes;

pub use generator::{Modifiers, RandomWords};
pub use quotes::Quote;

use crate::theme::Theme;

/// Something that hands out one word at a time.
///
/// `time`/`words` mode wraps a [`RandomWords`] (`is_finite() == false`);
/// `quote` mode wraps a single fixed target (`is_finite() == true`); `zen`
/// mode has no target text and never calls this trait at all.
///
/// Takes `&mut dyn rand::Rng` rather than a generic `R: rand::RngExt` type
/// parameter so the trait stays dyn-compatible -- `Box<dyn WordSource>` is
/// a reasonable thing for the binary crate's mode selection to want, and a
/// generic method would rule that out. `rand::Rng` (in rand 0.10, the base
/// object-safe trait -- what older `rand` versions called `RngCore`) has a
/// blanket `RngExt` impl over every `T: Rng + ?Sized`, so `&mut dyn
/// rand::Rng` still has every sampling method (`random_range` and friends)
/// available at the call site once `rand::RngExt` is in scope.
pub trait WordSource {
    fn next(&mut self, rng: &mut dyn rand::Rng) -> String;
    fn is_finite(&self) -> bool;
}

/// Looks up an embedded language's word pool by id (e.g. `"english1k"`),
/// in the file's own frequency-ranked order.
pub fn language(name: &str) -> Option<&'static [&'static str]> {
    embedded::language_words(name)
}

/// All embedded language ids, alphabetical.
pub fn language_names() -> impl Iterator<Item = &'static str> {
    embedded::language_ids()
}

/// Parses an embedded theme by id (e.g. `"amber-dark"`).
///
/// `build.rs` already validated every embedded theme parses and has valid
/// hex colours for all seven fields, so the only way this returns `None`
/// is an id that isn't embedded at all; a parse failure here would be a
/// build.rs/runtime validation mismatch, which is a bug, hence the
/// `expect` rather than folding it into the `Option`.
pub fn theme(name: &str) -> Option<Theme> {
    let raw = embedded::theme_raw(name)?;
    Some(toml::from_str(raw).unwrap_or_else(|e| {
        panic!(
            "embedded theme {name:?} failed to parse at runtime even though \
             build.rs validated it: {e}"
        )
    }))
}

/// All embedded theme ids, alphabetical.
pub fn theme_names() -> impl Iterator<Item = &'static str> {
    embedded::theme_ids()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_english1k_is_embedded() {
        let words = language("english1k").expect("english1k should be embedded");
        assert!(words.len() >= 1000);
        assert!(language_names().any(|n| n == "english1k"));
    }

    #[test]
    fn unknown_language_is_none() {
        assert!(language("no-such-language").is_none());
    }

    #[test]
    fn every_theme_name_parses_into_a_theme() {
        let names: Vec<_> = theme_names().collect();
        assert!(!names.is_empty());
        for name in names {
            let t = theme(name).unwrap_or_else(|| panic!("theme {name:?} should parse"));
            assert!(!t.name.is_empty());
        }
    }

    #[test]
    fn unknown_theme_is_none() {
        assert!(theme("no-such-theme").is_none());
    }
}
