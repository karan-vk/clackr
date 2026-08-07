//! Line packing for the word display.
//!
//! Pure and independent of ratatui, which is what makes the packing rules
//! testable without a buffer. The subtlety it exists to handle: a word's
//! rendered width is not fixed. Typing past the end of a word adds `Extra`
//! cells, so the word genuinely gets wider as you type and the display has to
//! re-wrap underneath the caret — exactly what MonkeyType does.
//!
//! Only the *active* word can change width, so a repack never has to touch the
//! lines above the one the caret is on. That keeps a keystroke O(words on
//! screen) rather than O(words in the test), which matters: a 120-second run
//! accumulates several hundred words and the whole point of the drain-then-draw
//! loop is that a keystroke is cheap.

use std::cmp::Ordering;
use std::ops::Range;

use clackr_core::Engine;
use clackr_core::engine::WordView;
use unicode_width::UnicodeWidthStr;

/// Rendered width of one grapheme cluster.
///
/// Clamped to at least one column: a cluster that measures zero would occupy no
/// cell yet still needs somewhere for the caret to sit.
pub fn cell_width(cell: &str) -> u16 {
    UnicodeWidthStr::width(cell).clamp(1, u16::MAX as usize) as u16
}

/// Rendered width of a whole word, in terminal columns.
///
/// This walks the cells the widget will actually draw — typed text where the
/// user has typed, target text ahead of the caret — so wrapping and rendering
/// can never disagree about how wide a word is.
pub fn word_width(view: &WordView<'_>) -> u16 {
    (0..view.len())
        .map(|i| u32::from(cell_width(view.text(i))))
        .sum::<u32>()
        .min(u32::from(u16::MAX)) as u16
}

/// Greedy line packing: fill each line with as many words as fit, one space
/// between them.
///
/// `out` is replaced. Each emitted range is a half-open span of word indices.
pub fn pack(widths: &[u16], width: u16, out: &mut Vec<Range<usize>>) {
    out.clear();
    pack_from(widths, width, out, 0);
}

/// Packs `widths[from..]`, appending lines to `out`.
fn pack_from(widths: &[u16], width: u16, out: &mut Vec<Range<usize>>, from: usize) {
    let limit = u32::from(width);
    let mut i = from;
    while i < widths.len() {
        let start = i;
        // The first word of a line is placed unconditionally. A word wider than
        // the terminal then gets a line to itself and overflows it, which is
        // ugly but finite — testing `fits` here instead would spin forever.
        let mut used = u32::from(widths[i]);
        i += 1;
        while i < widths.len() {
            let grown = used + 1 + u32::from(widths[i]);
            if grown > limit {
                break;
            }
            used = grown;
            i += 1;
        }
        out.push(start..i);
    }
}

/// The packed layout of the current word list, kept incrementally.
#[derive(Clone, Debug, Default)]
pub struct WrapCache {
    width: u16,
    widths: Vec<u16>,
    lines: Vec<Range<usize>>,
    /// Lowest word index whose width changed since the last [`WrapCache::sync`].
    dirty_from: Option<usize>,
}

impl WrapCache {
    pub fn new(width: u16) -> Self {
        Self {
            width,
            ..Self::default()
        }
    }

    /// The column budget one line has.
    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn lines(&self) -> &[Range<usize>] {
        &self.lines
    }

    pub fn word_count(&self) -> usize {
        self.widths.len()
    }

    /// A resize invalidates everything; nothing about the old packing survives.
    pub fn set_width(&mut self, width: u16) {
        if self.width != width {
            self.width = width;
            self.dirty_from = Some(0);
        }
    }

    /// Rebuilds the width table from scratch. Used when the word list itself
    /// changes — time mode appending another batch, or zen growing a word.
    pub fn reset(&mut self, engine: &Engine) {
        self.widths.clear();
        self.widths.extend(engine.words().map(|w| word_width(&w)));
        self.dirty_from = Some(0);
    }

    /// Records a new width for word `i`, marking it for repacking if it moved.
    pub fn touch(&mut self, i: usize, width: u16) {
        if let Some(slot) = self.widths.get_mut(i)
            && *slot != width
        {
            *slot = width;
            self.dirty_from = Some(self.dirty_from.map_or(i, |d| d.min(i)));
        }
    }

    /// Repacks whatever is stale. Cheap and idempotent, so the caller may call
    /// it after every message without thinking about it.
    pub fn sync(&mut self) {
        let Some(from) = self.dirty_from.take() else {
            return;
        };
        // Repacking starts one line *above* the one holding `from`: a word that
        // got narrower — a backspace over an extra — may now fit at the end of
        // the previous line, and starting on its own line could never move it
        // there. Anything above that is packed from words that have not moved
        // and is still correct by construction.
        let line = self.line_of(from).saturating_sub(1);
        if line == 0 {
            pack(&self.widths, self.width, &mut self.lines);
            return;
        }
        let start = match self.lines.get(line) {
            Some(r) => r.start,
            None => self.lines.last().map_or(0, |r| r.end),
        };
        self.lines.truncate(line);
        pack_from(&self.widths, self.width, &mut self.lines, start);
    }

    /// Index of the line containing `word`, or the line count if it is past the
    /// end (which happens for one keystroke when the last word is submitted).
    pub fn line_of(&self, word: usize) -> usize {
        self.lines
            .binary_search_by(|r| {
                if word < r.start {
                    Ordering::Greater
                } else if word >= r.end {
                    Ordering::Less
                } else {
                    Ordering::Equal
                }
            })
            .unwrap_or_else(|i| i)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Packs and returns the lines, for tests that only care about the result.
    fn packed(widths: &[u16], width: u16) -> Vec<Range<usize>> {
        let mut out = Vec::new();
        pack(widths, width, &mut out);
        out
    }

    /// A cache primed with `widths`, as if the words had just been appended.
    fn cache(widths: &[u16], width: u16) -> WrapCache {
        let mut cache = WrapCache::new(width);
        cache.widths = widths.to_vec();
        cache.dirty_from = Some(0);
        cache.sync();
        cache
    }

    /// Width a line occupies once rendered, including the single spaces.
    fn line_width(widths: &[u16], line: &Range<usize>) -> u32 {
        widths[line.clone()]
            .iter()
            .map(|w| u32::from(*w) + 1)
            .sum::<u32>()
            .saturating_sub(1)
    }

    const SAMPLE: [u16; 16] = [3, 5, 5, 3, 4, 2, 7, 4, 4, 6, 3, 3, 8, 2, 5, 4];

    #[test]
    fn every_word_lands_on_exactly_one_line() {
        for width in 1..40u16 {
            let lines = packed(&SAMPLE, width);
            let mut next = 0;
            for line in &lines {
                assert_eq!(line.start, next, "gap or overlap at width {width}");
                assert!(line.end > line.start, "empty line at width {width}");
                next = line.end;
            }
            assert_eq!(next, SAMPLE.len(), "words lost at width {width}");
        }
    }

    #[test]
    fn no_line_exceeds_the_width() {
        let width = 20;
        for line in packed(&SAMPLE, width) {
            assert!(
                line_width(&SAMPLE, &line) <= u32::from(width),
                "{line:?} overflows"
            );
        }
    }

    #[test]
    fn packing_is_greedy_so_no_line_could_take_one_more_word() {
        let width = 20;
        let lines = packed(&SAMPLE, width);
        for line in &lines {
            let Some(next) = SAMPLE.get(line.end) else {
                continue;
            };
            let grown = line_width(&SAMPLE, line) + 1 + u32::from(*next);
            assert!(
                grown > u32::from(width),
                "{line:?} had room for word {}",
                line.end
            );
        }
    }

    #[test]
    fn a_word_wider_than_the_line_gets_a_line_to_itself() {
        let lines = packed(&[3, 40, 3], 10);
        assert_eq!(lines, vec![0..1, 1..2, 2..3]);
    }

    #[test]
    fn zero_width_never_loops_forever() {
        assert_eq!(packed(&[1, 2, 3], 0), vec![0..1, 1..2, 2..3]);
    }

    #[test]
    fn empty_input_packs_to_no_lines() {
        assert!(packed(&[], 40).is_empty());
    }

    #[test]
    fn repacking_from_a_dirty_word_matches_a_full_repack() {
        // The incremental path is the whole performance argument for this
        // module, so it has to be indistinguishable from the naive one.
        for touched in 0..SAMPLE.len() {
            for new_width in [1u16, 4, 9, 30] {
                let mut cache = cache(&SAMPLE, 20);
                cache.touch(touched, new_width);
                cache.sync();

                let mut expected = SAMPLE;
                expected[touched] = new_width;
                assert_eq!(
                    cache.lines(),
                    packed(&expected, 20),
                    "incremental repack from word {touched} (width {new_width}) diverged"
                );
            }
        }
    }

    #[test]
    fn touching_a_word_leaves_earlier_lines_untouched() {
        let mut cache = cache(&SAMPLE, 20);
        let before = cache.lines().to_vec();

        let last = SAMPLE.len() - 1;
        let line = cache.line_of(last);
        cache.touch(last, 19);
        cache.sync();

        assert_eq!(&cache.lines()[..line], &before[..line]);
    }

    #[test]
    fn a_resize_invalidates_the_whole_packing() {
        let mut cache = cache(&SAMPLE, 20);
        cache.set_width(40);
        cache.sync();
        assert_eq!(cache.lines(), packed(&SAMPLE, 40));
    }

    #[test]
    fn line_of_finds_the_containing_line_and_saturates_past_the_end() {
        let cache = cache(&SAMPLE, 20);
        for (i, line) in cache.lines().to_vec().iter().enumerate() {
            for word in line.clone() {
                assert_eq!(cache.line_of(word), i);
            }
        }
        assert_eq!(cache.line_of(SAMPLE.len()), cache.lines().len());
    }

    #[test]
    fn cell_width_counts_columns_not_characters() {
        assert_eq!(cell_width("a"), 1);
        // A CJK ideograph is two columns wide...
        assert_eq!(cell_width("日"), 2);
        // ...and a lone combining mark still needs a cell to live in.
        assert_eq!(cell_width("\u{0301}"), 1);
    }
}
