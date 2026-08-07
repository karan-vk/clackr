# Contributing to clackr

Thanks for your interest in contributing! This document covers how the repo
is organized, how to build and test it, and what CI expects.

By participating in this project you agree to abide by the
[Code of Conduct](CODE_OF_CONDUCT.md).

## Getting started

```bash
git clone https://github.com/karan-vk/clackr.git
cd clackr
cargo build
cargo run -- --mode words --words 25
```

Rust MSRV is **1.88** (set by `rust-version` in `Cargo.toml`, driven by
ratatui's own MSRV). If you're on an older toolchain, `rustup update` first.

### Workspace layout

```
crates/
├── clackr-core/   # pure engine, stats, word/quote sourcing — no I/O, no clock, no terminal deps
└── clackr/        # the binary: CLI, ratatui UI, config I/O
```

`clackr-core` is deliberately dependency-light (no ratatui, no clock, no
filesystem access) so its test suite stays fast and its behavior stays
deterministic. If you're fixing an engine or stats bug, you almost
certainly want to be working in `clackr-core`, not `clackr`.

## Before you open a PR

Run what CI runs:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo doc --workspace --no-deps --locked
```

If you're touching dependencies, also worth running locally if you have the
tools installed: `cargo deny check bans licenses sources advisories` and
`typos`.

**A note on tests and terminals**: CI runners have no TTY, so nothing outside
`crates/clackr/src/main.rs` may touch raw mode or open a real terminal.
Rendering is tested through `ratatui::backend::TestBackend`
(see the `insta` snapshot tests under `crates/clackr/src`) — write new UI
tests against that, not against a live terminal.

Update `CHANGELOG.md` under `## [Unreleased]` for any user-visible change
(new flag, behavior change, bug fix). This file is parsed by `dist` to
generate release notes, so keep entries terse and user-facing rather than
implementation detail.

## Commit / PR expectations

- Keep PRs focused; unrelated cleanups belong in their own PR.
- The required CI check is `ci-ok`, which gates on every other job (fmt,
  clippy, the full test matrix, msrv, docs, typos, deny). `coverage` is
  informational and non-blocking.
- New behavior needs a test: a keystroke-script case for engine changes, a
  golden vector for stats changes, a snapshot for UI changes.

## Scope note

Punctuation/numbers modifiers, additional language word lists, and result
history are intentionally out of scope for v1 (see the project's open
issues) — please open an issue to discuss scope before sending a PR that
adds one of these, so we're aligned before you invest the time.

## Licensing

clackr is dual-licensed under [MIT](LICENSE-MIT) OR [Apache-2.0](LICENSE-APACHE).

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.

If you're contributing new word lists, quotes, or other assets, please
also read `assets/LICENSES.md` — every asset needs a provenance entry, and
sources that are share-alike (e.g. CC BY-SA) or otherwise incompatible with
a permissively-licensed binary are not acceptable (see the architecture
notes on why MonkeyType's own word/quote files can't be reused: they're
AGPL-3.0).
