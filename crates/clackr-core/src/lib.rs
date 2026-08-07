//! Terminal-free core of clackr: the typing engine, the stats math, word
//! sources, and the pure-data config/theme types.
//!
//! Nothing in this crate touches a terminal, the filesystem, or the clock.
//! That is enforced by the dependency list rather than by convention: callers
//! pass elapsed time in explicitly, so every timing-dependent behaviour is
//! deterministic and testable without sleeping.

pub mod config;
pub mod engine;
pub mod mode;
pub mod stats;
pub mod theme;
pub mod words;

pub use config::Config;
pub use engine::{Action, CellState, Engine, EngineEvent, Outcome};
pub use mode::{QuoteLength, TestMode, TestParams};
pub use stats::{CharCounts, SecondSample, Summary};
pub use theme::Theme;
