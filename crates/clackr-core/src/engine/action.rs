//! The engine's input alphabet and its reply.
//!
//! Deliberately tiny and terminal-agnostic: the binary maps crossterm key
//! events onto these five variants (Ctrl+Backspace, Alt+Backspace, Ctrl+W and
//! a bare `\u{17}` all collapse to [`Action::DeleteWord`], because which of
//! those a terminal emits is a coin flip), and the engine never learns that a
//! keyboard exists.

/// One user intent.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    /// A printable scalar. Zero-width scalars merge into the previous cell.
    Type(char),
    /// Submit the current word and move on.
    Space,
    /// Delete one grapheme cell, or step back into the previous word.
    Backspace,
    /// Delete the current word, or step back and delete that one.
    DeleteWord,
    /// End the test now (zen's only exit, and the manual stop elsewhere).
    EndTest,
}

/// Something the caller must react to, beyond redrawing.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum EngineEvent {
    /// The first counted keystroke landed: start the clock *now*. The engine
    /// has no clock of its own, so this is how `elapsed` gets its origin.
    Started,
    /// The test is over; call [`Engine::summary`](super::Engine::summary).
    Finished,
    /// Time mode is running low on target words — append at least this many.
    /// Level-triggered: re-emitted from `tick` until words actually arrive, so
    /// a caller that drops one event does not stall.
    NeedWords(usize),
}

/// The result of an [`Action`] or a tick.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// Something the user can see changed; the caller should redraw.
    pub dirty: bool,
    pub event: Option<EngineEvent>,
}

impl Outcome {
    /// Nothing happened — the common case for a tick between seconds, and for
    /// keystrokes the rules swallow entirely.
    pub const IDLE: Outcome = Outcome {
        dirty: false,
        event: None,
    };

    pub(crate) const fn event(event: EngineEvent) -> Self {
        Self {
            dirty: true,
            event: Some(event),
        }
    }
}
