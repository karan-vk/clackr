# Changelog

All notable changes to clackr will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-08-07

First release.

### Added

- **Test modes**: `time` (15/30/60/120s), `words` (10/25/50/100), `quote`
  (short/medium/long) and `zen`. Time mode streams words indefinitely.
- **MonkeyType-accurate scoring**: net WPM credits only words typed 100%
  correctly, raw WPM counts everything the fingers produced, accuracy is
  counted at keypress time and never recovered by backspacing, and consistency
  uses the kogasa transform over the per-second raw series.
- **Results screen**: block-digit `wpm`/`acc` hero, an overlaid net-WPM and
  raw-WPM chart with an error scatter and a right-hand "Errors" axis, and a
  stat row of test type / raw / characters / consistency / time.
- **Typing behaviour** matching MonkeyType: pressing space early marks the rest
  of the word missed, characters typed past a word's end show as extras (capped
  at 20), backspace cannot re-enter a word already typed perfectly unless
  `freedom_mode` is set, and `Ctrl+Backspace`/`Alt+Backspace`/`Ctrl+W` delete a
  whole word. Combining marks merge into the preceding grapheme.
- **Six themes** (`amber-dark`, `nord`, `dracula`, `gruvbox-dark`,
  `catppuccin-mocha`, `mono`) with live preview in the menu, plus a config file
  under the XDG config directory that remembers your menu choices.
- **Three fidelity tiers.** The UI detects colour depth, Unicode and braille
  support and terminal size, and degrades from a braille chart and block digits
  through half-block rendering down to plain ASCII where meaning is carried by
  text attributes rather than colour. Forced with `--simple`, `--ascii` or
  `NO_COLOR`.
- **Prebuilt binaries** for Linux and Windows on x86_64 and arm64 and for Apple
  Silicon, with shell and PowerShell installers. Linux builds are static musl.

### Notes

- Word lists and quotes are original or public-domain throughout; nothing is
  taken from MonkeyType, which is AGPL-3.0. See `assets/LICENSES.md`.
- Results are not saved between runs — only configuration is persisted.

[Unreleased]: https://github.com/karan-vk/clackr/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/karan-vk/clackr/releases/tag/v0.1.0
