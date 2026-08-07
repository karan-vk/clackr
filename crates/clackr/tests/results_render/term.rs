//! Mounts `crate::term::caps` at the position the results screen expects.
//!
//! This shim exists only because a `#[path]` inside an inline `mod` block is
//! resolved through a directory named after that block, and `tests/term/` does
//! not exist — so the `..` in the path would have nothing to climb out of.
//! A real file in a real directory sidesteps that.

#[path = "../../src/term/caps.rs"]
pub mod caps;
