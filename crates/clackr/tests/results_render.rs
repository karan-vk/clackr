//! Snapshot coverage for the results screen.
//!
//! `clackr` is a binary crate, so there is no library for an integration test
//! to link against. Rather than wait for one, this target mounts exactly the
//! modules the results screen needs via `#[path]`, mirroring their real
//! positions in the crate so every `crate::…` path inside them still resolves.
//! That also keeps this test independent of `app.rs` and `ui/mod.rs`: the
//! results screen can be verified on its own.
//!
//! Mounting the sources also pulls in their `#[cfg(test)]` unit tests, so this
//! one target runs the whole results suite — the geometry pin included.
//!
//! Nothing here touches a real terminal: `TestBackend` is a plain in-memory
//! buffer, which is what CI (no TTY) needs.

// The mounted modules are the crate's public surface, most of which this target
// does not call.
#![allow(dead_code)]

#[path = "results_render/term.rs"]
mod term;

#[path = "results_render/ui.rs"]
mod ui;

use clackr_core::Summary;

use crate::term::caps::Fidelity;
use crate::ui::results::tests::{fixture_with_errors, render};

/// The sizes the plan pins: a roomy terminal, a typical one, the classic 80x24,
/// and two that are too small for the full design.
const SIZES: [(u16, u16); 5] = [(120, 40), (100, 30), (80, 24), (60, 16), (40, 12)];

const TIERS: [(&str, Fidelity); 3] = [
    ("full", Fidelity::Full),
    ("reduced", Fidelity::Reduced),
    ("plain", Fidelity::Plain),
];

#[test]
fn flawless_result_across_every_size_and_tier() {
    // `Summary::fixture` is 100% accurate, which is precisely the case where the
    // error series and its whole right-hand gutter must collapse away.
    for (w, h) in SIZES {
        for (tier, fidelity) in TIERS {
            insta::assert_snapshot!(
                format!("flawless_{tier}_{w}x{h}"),
                render(w, h, &Summary::fixture(), fidelity)
            );
        }
    }
}

#[test]
fn result_with_errors_across_every_size_and_tier() {
    let summary = fixture_with_errors();
    for (w, h) in SIZES {
        for (tier, fidelity) in TIERS {
            insta::assert_snapshot!(
                format!("errors_{tier}_{w}x{h}"),
                render(w, h, &summary, fidelity)
            );
        }
    }
}
