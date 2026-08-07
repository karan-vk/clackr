//! Mounts the `crate::ui` modules the results screen depends on. See the sister
//! `term.rs` for why the indirection is needed.
//!
//! Deliberately *not* `ui/mod.rs`: that would drag in `app.rs`, the menu and
//! the test screen, and the point of this target is that the results screen can
//! be verified on its own.

#[path = "../../src/ui/bigtext.rs"]
pub mod bigtext;

#[path = "../../src/ui/palette.rs"]
pub mod palette;

#[path = "../../src/ui/results/mod.rs"]
pub mod results;
