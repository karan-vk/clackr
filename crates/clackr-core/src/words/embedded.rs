//! The embedded asset tables, generated at build time by `build.rs` from
//! `assets/` (languages, themes, quotes). See `assets/LICENSES.md` for the
//! provenance of every embedded file.
//!
//! `build.rs` already validated every language file (non-empty, no blank
//! lines, no leading/trailing whitespace, no duplicate words) and every
//! theme file (parses, has all seven colour fields as valid hex). Nothing
//! in this module re-validates that -- a malformed asset fails `cargo
//! build`, not something at runtime.

use std::sync::OnceLock;

include!(concat!(env!("OUT_DIR"), "/assets.rs"));

/// `LANGUAGES` split into one `&'static str` word per line, computed once
/// on first access and cached for the life of the program.
fn split_languages() -> &'static [(&'static str, Vec<&'static str>)] {
    static CACHE: OnceLock<Vec<(&'static str, Vec<&'static str>)>> = OnceLock::new();
    CACHE.get_or_init(|| {
        LANGUAGES
            .iter()
            .map(|(id, raw)| (*id, raw.lines().collect()))
            .collect()
    })
}

/// The word pool for the embedded language `name`, in the file's own
/// (frequency-ranked) order.
pub(super) fn language_words(name: &str) -> Option<&'static [&'static str]> {
    split_languages()
        .iter()
        .find(|(id, _)| *id == name)
        .map(|(_, words)| words.as_slice())
}

/// All embedded language ids, alphabetical (the order `build.rs` sorts
/// them into).
pub(super) fn language_ids() -> impl Iterator<Item = &'static str> {
    LANGUAGES.iter().map(|(id, _)| *id)
}

/// The raw (unparsed) TOML for the embedded theme `name`.
pub(super) fn theme_raw(name: &str) -> Option<&'static str> {
    THEMES
        .iter()
        .find(|(id, _)| *id == name)
        .map(|(_, raw)| *raw)
}

/// All embedded theme ids, alphabetical.
pub(super) fn theme_ids() -> impl Iterator<Item = &'static str> {
    THEMES.iter().map(|(id, _)| *id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_is_non_empty_deduplicated_and_whitespace_clean() {
        assert!(!LANGUAGES.is_empty(), "no embedded languages");
        for (id, _) in LANGUAGES {
            let words = language_words(id).unwrap();
            assert!(!words.is_empty(), "language {id:?} has no words");

            let mut seen = std::collections::HashSet::new();
            for word in words {
                assert_eq!(
                    word.trim(),
                    *word,
                    "language {id:?} word {word:?} has whitespace"
                );
                assert!(!word.is_empty(), "language {id:?} has a blank entry");
                assert!(
                    seen.insert(*word),
                    "language {id:?} has duplicate word {word:?}"
                );
            }
        }
    }

    #[test]
    fn every_theme_is_non_empty() {
        assert!(!THEMES.is_empty(), "no embedded themes");
        for (id, raw) in THEMES {
            assert!(!raw.trim().is_empty(), "theme {id:?} is empty");
        }
    }

    #[test]
    fn language_and_theme_lookup_roundtrip_every_id() {
        for (id, _) in LANGUAGES {
            assert!(language_words(id).is_some());
        }
        assert!(language_words("does-not-exist").is_none());

        for (id, _) in THEMES {
            assert!(theme_raw(id).is_some());
        }
        assert!(theme_raw("does-not-exist").is_none());
    }
}
