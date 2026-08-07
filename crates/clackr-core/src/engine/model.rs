//! The typing model: target words, what the user typed, and the *derived*
//! per-cell state.
//!
//! The load-bearing decision of the whole engine is here: nothing stores a
//! mutable [`CellState`]. We store only the target and the input and compute
//! state on demand from the two lengths plus a string compare. Backspace is
//! then a `pop()` with no invalidation logic to get wrong, and skip-ahead
//! (`Missed`) and overflow (`Extra`) fall out of the same match arm table
//! rather than needing their own bookkeeping.

use std::ops::Range;

use super::grapheme;

/// One word of the text the user is asked to type.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TargetWord {
    text: String,
    cells: Vec<Range<u16>>,
}

impl TargetWord {
    pub fn new(text: impl Into<String>) -> Self {
        let mut text: String = text.into();
        if text.len() > grapheme::MAX_WORD_BYTES {
            // Absurd input from a corrupted word list; truncate on a char
            // boundary rather than overflow the u16 cell ranges.
            let mut end = grapheme::MAX_WORD_BYTES;
            while end > 0 && !text.is_char_boundary(end) {
                end -= 1;
            }
            text.truncate(end);
        }
        let cells = grapheme::cell_ranges(&text);
        Self { text, cells }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// Number of rendered cells (grapheme clusters), not bytes or chars.
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// The `i`th grapheme cluster, or `""` past the end.
    pub fn cell(&self, i: usize) -> &str {
        cell_str(&self.text, &self.cells, i)
    }
}

/// What the user typed for one word.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WordEntry {
    typed: String,
    cells: Vec<Range<u16>>,
    submitted: bool,
}

impl WordEntry {
    pub fn typed(&self) -> &str {
        &self.typed
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// True once a space moved past this word. Only submitted words show
    /// `Missed` cells — an unfinished word in front of the caret is `Untyped`.
    pub fn submitted(&self) -> bool {
        self.submitted
    }

    pub fn cell(&self, i: usize) -> &str {
        cell_str(&self.typed, &self.cells, i)
    }

    pub(crate) fn set_submitted(&mut self, submitted: bool) {
        self.submitted = submitted;
    }

    pub(crate) fn push_char(&mut self, c: char) -> Option<grapheme::Push> {
        grapheme::push_char(&mut self.typed, &mut self.cells, c)
    }

    pub(crate) fn undo_push(&mut self, push: grapheme::Push) {
        grapheme::undo_push(&mut self.typed, &mut self.cells, push);
    }

    pub(crate) fn pop_cell(&mut self) -> bool {
        grapheme::pop_cell(&mut self.typed, &mut self.cells)
    }

    pub(crate) fn clear(&mut self) {
        self.typed.clear();
        self.cells.clear();
    }
}

fn cell_str<'a>(text: &'a str, cells: &[Range<u16>], i: usize) -> &'a str {
    match cells.get(i) {
        Some(r) => &text[r.start as usize..r.end as usize],
        None => "",
    }
}

/// Per-cell state, derived — never stored.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum CellState {
    /// Ahead of the caret in the current word.
    Untyped,
    Correct,
    Incorrect,
    /// Typed past the end of the target word.
    Extra,
    /// Skipped: the word was submitted before this cell was typed.
    Missed,
}

/// The whole correctness rule, in one table.
///
/// `i` must be `< max(target.len(), entry.len())`; the `(false, false)` arm is
/// unreachable for any in-range index.
pub fn cell_state(target: &TargetWord, entry: &WordEntry, i: usize) -> CellState {
    match (i < entry.len(), i < target.len()) {
        (true, true) => {
            if entry.cell(i) == target.cell(i) {
                CellState::Correct
            } else {
                CellState::Incorrect
            }
        }
        (true, false) => CellState::Extra,
        (false, true) => {
            if entry.submitted {
                CellState::Missed
            } else {
                CellState::Untyped
            }
        }
        (false, false) => unreachable!("cell index {i} is past both target and entry"),
    }
}

/// A target/entry pair plus the one bit of context the pair cannot carry: zen
/// mode has no target text at all, yet everything the user types is correct by
/// definition, so the derived rule needs an override.
#[derive(Copy, Clone, Debug)]
pub struct WordView<'a> {
    pub target: &'a TargetWord,
    pub entry: &'a WordEntry,
    zen: bool,
}

impl<'a> WordView<'a> {
    pub(crate) fn new(target: &'a TargetWord, entry: &'a WordEntry, zen: bool) -> Self {
        Self { target, entry, zen }
    }

    /// Rendered cell count: extras genuinely widen a word, which is why the
    /// wrapping layer must ask for this rather than the target width.
    pub fn len(&self) -> usize {
        self.target.len().max(self.entry.len())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn target_len(&self) -> usize {
        self.target.len()
    }

    pub fn entry_len(&self) -> usize {
        self.entry.len()
    }

    pub fn submitted(&self) -> bool {
        self.entry.submitted()
    }

    pub fn state(&self, i: usize) -> CellState {
        if self.zen {
            return if i < self.entry.len() {
                CellState::Correct
            } else {
                CellState::Untyped
            };
        }
        cell_state(self.target, self.entry, i)
    }

    /// The text to draw in cell `i`: what the user typed if they typed it,
    /// otherwise the target they still owe.
    pub fn text(&self, i: usize) -> &'a str {
        if i < self.entry.len() {
            self.entry.cell(i)
        } else {
            self.target.cell(i)
        }
    }

    pub fn cells(&self) -> impl ExactSizeIterator<Item = (&'a str, CellState)> + '_ {
        (0..self.len()).map(move |i| (self.text(i), self.state(i)))
    }

    /// Typed exactly right: no wrong cells, no extras, nothing missed. This is
    /// both the WPM credit test and the "may I backspace into it?" test.
    pub fn is_perfect(&self) -> bool {
        self.zen || self.entry.typed() == self.target.text()
    }
}
