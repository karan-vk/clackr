//! Terminal lifecycle: the extra setup `ratatui::init()` does not do, plus the
//! guarantee that we always undo it.
//!
//! A typing test spends its whole life in raw mode on the alternate screen. If
//! we ever exit without restoring, the user is left with a terminal that does
//! not echo and does not respond to Ctrl-C — so restoration is wired up three
//! ways: a `Drop` guard for the normal path, a panic hook for the abnormal one,
//! and idempotence so it is harmless when both fire.

pub mod caps;
pub mod input;

use std::io::{self, Write};

use ratatui::crossterm::{
    cursor::SetCursorStyle,
    event::{
        DisableBracketedPaste, EnableBracketedPaste, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::supports_keyboard_enhancement,
};

use clackr_core::config::CaretStyle;

/// Owns the terminal modifications clackr layers on top of `ratatui::init()`.
/// Dropping it puts them all back.
pub struct TermGuard {
    kitty: bool,
}

impl TermGuard {
    /// Applies the optional enhancements. Every one of them is a bonus: if the
    /// terminal does not support it we simply skip it and the app is fully
    /// functional without.
    pub fn install(caret: CaretStyle) -> Self {
        let mut out = io::stdout();

        // Bracketed paste is enabled so we can *reject* pastes. Without it a
        // pasted block arrives as a burst of indistinguishable keystrokes.
        let _ = execute!(out, EnableBracketedPaste);

        // Only DISAMBIGUATE_ESCAPE_CODES. It gives us an unambiguous Esc (no
        // 50ms escape-timeout guess, which matters when Esc leaves a test
        // mid-word) and distinguishes Ctrl+Backspace. We deliberately do NOT
        // ask for REPORT_EVENT_TYPES: that would deliver Release and Repeat
        // events we would only have to filter back out.
        //
        // `supports_keyboard_enhancement` is hard-coded false on Windows, so
        // this whole block is a no-op there by construction.
        let kitty = supports_keyboard_enhancement().unwrap_or(false)
            && execute!(
                out,
                PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
            )
            .is_ok();

        let _ = execute!(out, caret_style(caret));
        let _ = out.flush();

        Self { kitty }
    }
}

impl Drop for TermGuard {
    fn drop(&mut self) {
        let mut out = io::stdout();
        if self.kitty {
            let _ = execute!(out, PopKeyboardEnhancementFlags);
        }
        let _ = execute!(out, DisableBracketedPaste, SetCursorStyle::DefaultUserShape);
        let _ = out.flush();
    }
}

fn caret_style(caret: CaretStyle) -> SetCursorStyle {
    match caret {
        // The terminal blinks its own cursor, which is why clackr needs no
        // blink timer and can idle at a 1Hz tick.
        CaretStyle::Bar => SetCursorStyle::BlinkingBar,
        CaretStyle::Block => SetCursorStyle::BlinkingBlock,
        CaretStyle::Underline => SetCursorStyle::BlinkingUnderScore,
        CaretStyle::Off => SetCursorStyle::DefaultUserShape,
    }
}

/// Undo everything, unconditionally and idempotently.
///
/// Safe to call from a panic hook, from `Drop`, and from both in either order —
/// every step is a terminal escape sequence that is harmless to repeat.
pub fn restore() {
    let mut out = io::stdout();
    let _ = execute!(
        out,
        PopKeyboardEnhancementFlags,
        DisableBracketedPaste,
        SetCursorStyle::DefaultUserShape
    );
    ratatui::restore();
    let _ = out.flush();
}

/// Chains terminal restoration in front of the existing panic hook.
///
/// Order matters: restore first, *then* let the previous hook print. Printing a
/// backtrace while still in raw mode on the alternate screen produces a
/// staircase of text that vanishes when the screen is swapped back.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        previous(info);
    }));
}
