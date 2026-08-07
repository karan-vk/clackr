<!--
Thanks for the contribution! A few things that'll make this easier to review:
-->

## What does this change?

<!-- A short description of the change and why it's needed. Link any related issue. -->

## How was this tested?

<!--
clackr's tests run on CI runners with no TTY, so terminal rendering is
exercised through `ratatui::backend::TestBackend`, not a real terminal.
If you touched rendering, mention which snapshot/size/fidelity combos you
checked. If you touched the engine or stats, mention which keystroke
scripts or golden vectors cover it.
-->

## Checklist

- [ ] `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, and `cargo test --workspace` pass locally
- [ ] I added or updated tests for the behavior changed
- [ ] I updated `CHANGELOG.md` under `## [Unreleased]` (skip for docs-only / CI-only changes)
- [ ] I have not committed a regenerated `demo.gif` (that's handled via the manual `demo.yml` workflow, see its comments)

<!--
By submitting this pull request, you agree that, unless you explicitly
state otherwise, any contribution intentionally submitted for inclusion in
this work, as defined in the Apache-2.0 license, shall be dual licensed as
above (MIT OR Apache-2.0), without any additional terms or conditions. See
CONTRIBUTING.md for details.
-->
