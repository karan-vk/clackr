//! The input thread.
//!
//! Reading is decoupled from rendering by a channel. That is what guarantees no
//! dropped keystrokes: `event::read()` is a blocking OS-level read straight
//! from the terminal's input buffer, so a burst of fast typing queues in the
//! channel rather than waiting on a frame boundary. A slow draw costs us a
//! frame, never a keypress.

use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Instant;

use ratatui::crossterm::event::{self, Event, KeyEvent, KeyEventKind};

/// Everything the main loop reacts to.
pub enum Msg {
    /// A key press, stamped at the moment it was read.
    ///
    /// The timestamp is captured in the reader thread rather than at render
    /// time so the per-second buckets behind the results chart reflect when the
    /// user actually typed. A render stall shifts the graph by nothing.
    Key(KeyEvent, Instant),
    Resize(u16, u16),
    /// A bracketed paste arrived. The content is deliberately discarded; we
    /// only record that it happened.
    Paste,
    /// The reader thread ended, which means stdin closed.
    Closed,
}

/// Spawns the reader thread and returns the receiving end.
///
/// The thread exits when `event::read()` fails (stdin closed), which drops the
/// sender and lets the main loop see a disconnect.
pub fn spawn() -> Receiver<Msg> {
    let (tx, rx) = mpsc::channel();
    thread::Builder::new()
        .name("clackr-input".into())
        .spawn(move || {
            loop {
                let Ok(ev) = event::read() else {
                    let _ = tx.send(Msg::Closed);
                    break;
                };
                let at = Instant::now();

                let msg = match ev {
                    // Filtering to Press here, unconditionally on every
                    // platform, is the portable baseline: Windows reports
                    // press+release natively, and a kitty-protocol terminal
                    // could report repeats. Without this, every keystroke would
                    // register twice on Windows.
                    Event::Key(k) if k.kind == KeyEventKind::Press => Msg::Key(k, at),
                    Event::Resize(w, h) => Msg::Resize(w, h),
                    Event::Paste(_) => Msg::Paste,
                    _ => continue,
                };

                if tx.send(msg).is_err() {
                    break;
                }
            }
        })
        .expect("spawning the input thread");
    rx
}
