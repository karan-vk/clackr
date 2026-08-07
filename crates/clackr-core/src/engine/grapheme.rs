//! Grapheme-cluster bookkeeping for typed and target text.
//!
//! A "cell" is one rendered column of the word display: an extended grapheme
//! cluster. Both the target text and the user's input are stored as a plain
//! `String` plus a `Vec<Range<u16>>` of byte ranges, one per cell.
//!
//! The reason the typed side cannot simply be re-segmented after every
//! keystroke is that the terminal hands us one `KeyCode::Char` per *Unicode
//! scalar*, not per cluster. Re-segmenting would be correct but quadratic and,
//! worse, it would let a combining mark retroactively merge two cells the user
//! already saw as separate. Instead we apply the one rule that matters in
//! practice: a zero-width scalar extends the previous cell.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthChar;

/// Cell ranges are `u16` to keep `TargetWord`/`WordEntry` small, so no single
/// word may exceed 64 KiB of UTF-8. Corpus words are a handful of bytes; only a
/// zen-mode marathon on a single unbroken word could ever approach this, and
/// there we drop the overflowing keystrokes rather than panic.
pub(crate) const MAX_WORD_BYTES: usize = u16::MAX as usize;

/// Segment `text` into grapheme-cluster byte ranges.
pub(crate) fn cell_ranges(text: &str) -> Vec<Range<u16>> {
    text.grapheme_indices(true)
        .map(|(at, g)| (at as u16)..((at + g.len()) as u16))
        .collect()
}

/// A scalar that occupies no column of its own: combining marks, ZWJ, ZWSP,
/// variation selectors. These attach to whatever precedes them.
pub(crate) fn is_zero_width(c: char) -> bool {
    UnicodeWidthChar::width(c) == Some(0)
}

/// Control characters (and anything else without a defined width) are not text
/// and never reach the model. The input layer should already have filtered
/// these; this is the backstop.
pub(crate) fn is_typeable(c: char) -> bool {
    UnicodeWidthChar::width(c).is_some()
}

/// Undo token for [`push_char`], so a rejected keystroke (`stop_on_error =
/// letter`) can be rolled back without re-deriving anything.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Push {
    /// The keystroke extended the previous cell instead of creating a new one.
    merged: bool,
    /// Byte length of the text before the push.
    prev_bytes: u16,
}

/// Append `c`, either as a new cell or merged into the previous one.
///
/// Returns `None` when the character was dropped: it is not typeable, it is a
/// zero-width mark with nothing to attach to, or the word is already at
/// [`MAX_WORD_BYTES`].
pub(crate) fn push_char(text: &mut String, cells: &mut Vec<Range<u16>>, c: char) -> Option<Push> {
    if !is_typeable(c) {
        return None;
    }
    let prev_bytes = text.len();
    if prev_bytes + c.len_utf8() > MAX_WORD_BYTES {
        return None;
    }
    let merged = is_zero_width(c);
    if merged && cells.is_empty() {
        // Rule 1's tail: a combining mark with no base is not a cell of its
        // own, so there is nothing sensible to do but drop it.
        return None;
    }
    text.push(c);
    let end = text.len() as u16;
    if merged {
        // Extend the previous cluster. `unwrap` is safe: `cells` is non-empty.
        cells.last_mut().expect("checked non-empty").end = end;
    } else {
        cells.push(prev_bytes as u16..end);
    }
    Some(Push {
        merged,
        prev_bytes: prev_bytes as u16,
    })
}

/// Reverse a [`push_char`].
pub(crate) fn undo_push(text: &mut String, cells: &mut Vec<Range<u16>>, push: Push) {
    text.truncate(push.prev_bytes as usize);
    if push.merged {
        if let Some(last) = cells.last_mut() {
            last.end = push.prev_bytes;
        }
    } else {
        cells.pop();
    }
}

/// Remove the last cell — the whole cluster, combining marks included. One
/// backspace deletes one thing the user can see.
pub(crate) fn pop_cell(text: &mut String, cells: &mut Vec<Range<u16>>) -> bool {
    match cells.pop() {
        Some(range) => {
            text.truncate(range.start as usize);
            true
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(s: &str) -> (String, Vec<Range<u16>>) {
        let mut text = String::new();
        let mut cells = Vec::new();
        for c in s.chars() {
            push_char(&mut text, &mut cells, c);
        }
        (text, cells)
    }

    #[test]
    fn combining_mark_extends_the_previous_cell() {
        let (text, cells) = build("e\u{301}a");
        assert_eq!(text, "e\u{301}a");
        assert_eq!(cells.len(), 2);
        assert_eq!(
            &text[cells[0].start as usize..cells[0].end as usize],
            "e\u{301}"
        );
    }

    #[test]
    fn leading_combining_mark_is_dropped() {
        let (text, cells) = build("\u{301}x");
        assert_eq!(text, "x");
        assert_eq!(cells.len(), 1);
    }

    #[test]
    fn pop_removes_the_whole_cluster() {
        let (mut text, mut cells) = build("e\u{301}");
        assert!(pop_cell(&mut text, &mut cells));
        assert!(text.is_empty());
        assert!(cells.is_empty());
        assert!(!pop_cell(&mut text, &mut cells));
    }

    #[test]
    fn undo_restores_both_merged_and_fresh_cells() {
        let mut text = String::new();
        let mut cells = Vec::new();
        let p1 = push_char(&mut text, &mut cells, 'e').unwrap();
        let p2 = push_char(&mut text, &mut cells, '\u{301}').unwrap();
        undo_push(&mut text, &mut cells, p2);
        assert_eq!(text, "e");
        assert_eq!(cells.len(), 1);
        undo_push(&mut text, &mut cells, p1);
        assert!(text.is_empty());
        assert!(cells.is_empty());
    }

    #[test]
    fn control_characters_are_not_typeable() {
        assert!(!is_typeable('\t'));
        assert!(!is_typeable('\u{7}'));
        assert!(is_typeable(' '));
        assert!(is_typeable('a'));
    }

    #[test]
    fn cell_ranges_match_incremental_pushes() {
        let s = "na\u{308}ive";
        let (_, incremental) = build(s);
        assert_eq!(cell_ranges(s), incremental);
    }
}
