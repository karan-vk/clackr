//! A faithful port of MonkeyType's `countChars()`.
//!
//! The rules look arbitrary until you notice they encode two different ideas
//! at once:
//!
//! * `correct_word_chars` credits a word only if it was typed **perfectly**.
//!   It is the WPM numerator, and it is why one typo costs you the whole word
//!   while the raw line does not move. Reproducing that divergence is the
//!   entire point of the results chart.
//! * `all_correct_chars` credits every character that matched, even inside a
//!   word you fumbled. It is what the results screen reports as "characters".

use serde::{Deserialize, Serialize};

use crate::engine::{CellState, WordView};

/// Character tallies for a whole test.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharCounts {
    /// Characters belonging to words typed 100% correctly — the WPM numerator.
    pub correct_word_chars: u32,
    /// Every character that matched its target, wrong words included.
    pub all_correct_chars: u32,
    pub incorrect: u32,
    pub extra: u32,
    pub missed: u32,
    /// One per word except the last, typed or not.
    pub spaces: u32,
    /// Spaces that followed a perfectly typed word.
    pub correct_spaces: u32,
}

impl CharCounts {
    /// The results-screen "characters" quadruple, in display order.
    pub fn characters(&self) -> (u32, u32, u32, u32) {
        (
            self.all_correct_chars,
            self.incorrect,
            self.extra,
            self.missed,
        )
    }

    /// WPM numerator: only perfect words and the spaces that followed them.
    pub fn net_chars(&self) -> u32 {
        self.correct_word_chars + self.correct_spaces
    }

    /// Raw-WPM numerator: everything the fingers actually produced.
    pub fn raw_chars(&self) -> u32 {
        self.all_correct_chars + self.incorrect + self.extra + self.spaces
    }
}

/// Tally `words` — which must be exactly the words the user reached, in order.
///
/// `ExactSizeIterator` is required because several rules hinge on "is this the
/// last word?": no space is counted after it, and a word the clock cut off
/// mid-way must not be charged for the characters it never got to type.
pub fn count_chars<'a, I>(words: I) -> CharCounts
where
    I: IntoIterator<Item = WordView<'a>>,
    I::IntoIter: ExactSizeIterator,
{
    let words = words.into_iter();
    let n = words.len();
    let mut c = CharCounts::default();

    for (i, w) in words.enumerate() {
        let last = i + 1 == n;
        let target = w.target_len() as u32;
        let typed = w.entry_len() as u32;
        if !last {
            c.spaces += 1;
        }

        if w.is_perfect() {
            c.correct_word_chars += typed;
            c.all_correct_chars += typed;
            if !last {
                c.correct_spaces += 1;
            }
            continue;
        }

        // Not perfect: score cell by cell. `state` already distinguishes a
        // mismatch from an overflow, so the "typed longer than target" and
        // "typed shorter than target" branches of the original share this loop.
        for cell in 0..typed as usize {
            match w.state(cell) {
                CellState::Correct => c.all_correct_chars += 1,
                CellState::Incorrect => c.incorrect += 1,
                CellState::Extra => c.extra += 1,
                CellState::Untyped | CellState::Missed => {}
            }
        }

        if typed < target {
            // Skipped ahead: the rest of the word is missed — unless the word
            // was never submitted, i.e. the test ended in the middle of it.
            // Charging a time-mode test for the tail of the word the clock cut
            // off would punish the user for the test's own boundary.
            if w.submitted() {
                c.missed += target - typed;
            }
        }
    }
    c
}
