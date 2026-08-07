//! Walks `assets/` (at the workspace root, resolved via
//! `CARGO_MANIFEST_DIR` since this crate lives at `crates/clackr-core`),
//! validates every embedded language/theme/quote file, and emits an
//! `include_str!` table into `OUT_DIR/assets.rs` for `src/words/embedded.rs`
//! to `include!`.
//!
//! Validation happens here, at build time, on purpose: a malformed asset
//! should break `cargo build`, not silently ship and fail (or worse, not
//! fail) a test that a contributor might not think to run.

use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo"));
    let assets_dir = manifest_dir.join("..").join("..").join("assets");
    let assets_dir = assets_dir.canonicalize().unwrap_or_else(|e| {
        panic!(
            "clackr-core/build.rs: could not find assets/ at {} ({e}). \
             Expected it at the workspace root, two levels up from \
             CARGO_MANIFEST_DIR.",
            assets_dir.display()
        )
    });

    // Re-run if anything under assets/ changes, not just on a full clean
    // build. Cargo watches this path recursively.
    println!("cargo::rerun-if-changed={}", assets_dir.display());

    let languages_dir = assets_dir.join("languages");
    let themes_dir = assets_dir.join("themes");
    let quotes_path = assets_dir.join("quotes").join("english.toml");

    let languages = collect_languages(&languages_dir);
    if languages.is_empty() {
        panic!(
            "clackr-core/build.rs: no *.txt language files found in {}",
            languages_dir.display()
        );
    }

    let themes = collect_themes(&themes_dir);
    if themes.is_empty() {
        panic!(
            "clackr-core/build.rs: no *.toml theme files found in {}",
            themes_dir.display()
        );
    }

    validate_quotes(&quotes_path);

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let out_path = out_dir.join("assets.rs");
    let mut out = String::new();

    out.push_str("pub static LANGUAGES: &[(&str, &str)] = &[\n");
    for (id, path) in &languages {
        out.push_str(&format!(
            "    ({id:?}, include_str!({path:?})),\n",
            id = id,
            path = path.display()
        ));
    }
    out.push_str("];\n\n");

    out.push_str("pub static THEMES: &[(&str, &str)] = &[\n");
    for (id, path) in &themes {
        out.push_str(&format!(
            "    ({id:?}, include_str!({path:?})),\n",
            id = id,
            path = path.display()
        ));
    }
    out.push_str("];\n\n");

    out.push_str(&format!(
        "pub static QUOTES_EN: &str = include_str!({path:?});\n",
        path = quotes_path.display()
    ));

    fs::write(&out_path, &out).unwrap_or_else(|e| {
        panic!(
            "clackr-core/build.rs: failed to write {}: {e}",
            out_path.display()
        )
    });
}

/// Reads every `*.txt` file in `languages_dir`, validates it as a
/// newline-delimited word list, and returns `(id, absolute_path)` pairs
/// sorted by `id` for deterministic codegen. `id` is the file stem, e.g.
/// `english1k` for `english1k.txt`.
fn collect_languages(languages_dir: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    let entries = fs::read_dir(languages_dir).unwrap_or_else(|e| {
        panic!(
            "clackr-core/build.rs: could not read {}: {e}",
            languages_dir.display()
        )
    });
    for entry in entries {
        let entry = entry.expect("readdir entry");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("txt") {
            continue; // e.g. index.toml lives alongside the word lists.
        }
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_else(|| panic!("non-UTF8 filename: {}", path.display()))
            .to_string();

        let content = fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "clackr-core/build.rs: failed to read {}: {e}",
                path.display()
            )
        });
        validate_word_list(&id, &path, &content);

        out.push((id, path));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// A malformed asset breaks the build: empty list, blank lines,
/// leading/trailing whitespace on any line, or a duplicate word.
fn validate_word_list(id: &str, path: &Path, content: &str) {
    if content.trim().is_empty() {
        panic!(
            "clackr-core/build.rs: language {id:?} ({}) is empty",
            path.display()
        );
    }
    let mut seen = HashSet::new();
    for (i, line) in content.lines().enumerate() {
        let lineno = i + 1;
        if line.is_empty() {
            panic!(
                "clackr-core/build.rs: language {id:?} ({}) has a blank line at line {lineno}",
                path.display()
            );
        }
        if line.trim() != line {
            panic!(
                "clackr-core/build.rs: language {id:?} ({}) has leading/trailing \
                 whitespace on line {lineno}: {line:?}",
                path.display()
            );
        }
        if !seen.insert(line) {
            panic!(
                "clackr-core/build.rs: language {id:?} ({}) has a duplicate word \
                 {line:?} (first duplicate at line {lineno})",
                path.display()
            );
        }
    }
}

/// Reads every `*.toml` file in `themes_dir`, validates it against the
/// shape `theme::Theme` expects (without depending on that type -- build.rs
/// cannot depend on the crate it is building), and returns `(id,
/// absolute_path)` pairs sorted by `id`.
fn collect_themes(themes_dir: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    let entries = fs::read_dir(themes_dir).unwrap_or_else(|e| {
        panic!(
            "clackr-core/build.rs: could not read {}: {e}",
            themes_dir.display()
        )
    });
    for entry in entries {
        let entry = entry.expect("readdir entry");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_else(|| panic!("non-UTF8 filename: {}", path.display()))
            .to_string();

        let content = fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "clackr-core/build.rs: failed to read {}: {e}",
                path.display()
            )
        });
        validate_theme(&id, &path, &content);

        out.push((id, path));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

const THEME_COLOR_FIELDS: &[&str] = &[
    "bg",
    "main",
    "sub",
    "sub_alt",
    "text",
    "error",
    "error_extra",
];

fn validate_theme(id: &str, path: &Path, content: &str) {
    // `toml::Table::from_str` parses a full document (top-level `key =
    // value` pairs); `toml::Value::from_str` parses a single bare value and
    // would reject this file entirely, so it must not be used here.
    let table: toml::Table = content.parse().unwrap_or_else(|e| {
        panic!(
            "clackr-core/build.rs: theme {id:?} ({}) does not parse as TOML: {e}",
            path.display()
        )
    });

    match table.get("name").and_then(toml::Value::as_str) {
        Some(s) if !s.is_empty() => {}
        _ => panic!(
            "clackr-core/build.rs: theme {id:?} ({}) is missing a non-empty string `name`",
            path.display()
        ),
    }

    for field in THEME_COLOR_FIELDS {
        let value = table
            .get(*field)
            .and_then(toml::Value::as_str)
            .unwrap_or_else(|| {
                panic!(
                    "clackr-core/build.rs: theme {id:?} ({}) is missing string field `{field}`",
                    path.display()
                )
            });
        if !is_hex_color(value) {
            panic!(
                "clackr-core/build.rs: theme {id:?} ({}) field `{field}` = {value:?} \
                 is not a valid #rgb or #rrggbb colour",
                path.display()
            );
        }
    }
}

/// Mirrors the format `theme::Rgb::from_str` accepts: `#rgb` or `#rrggbb`,
/// hex digits only. Duplicated here (rather than shared) because build.rs
/// cannot depend on the crate it builds.
fn is_hex_color(s: &str) -> bool {
    let Some(hex) = s.strip_prefix('#') else {
        return false;
    };
    match hex.len() {
        3 | 6 => hex.chars().all(|c| c.is_ascii_hexdigit()),
        _ => false,
    }
}

/// Validates `assets/quotes/english.toml`: it must parse, contain a
/// non-empty `[[quote]]` array, and every entry must have the four string
/// fields `words::quotes::Quote` expects, with `length` one of
/// `short`/`medium`/`long`.
fn validate_quotes(path: &Path) {
    let content = fs::read_to_string(path).unwrap_or_else(|e| {
        panic!(
            "clackr-core/build.rs: failed to read {}: {e}",
            path.display()
        )
    });
    let table: toml::Table = content.parse().unwrap_or_else(|e| {
        panic!(
            "clackr-core/build.rs: quotes file ({}) does not parse as TOML: {e}",
            path.display()
        )
    });
    let quotes = table
        .get("quote")
        .and_then(toml::Value::as_array)
        .unwrap_or_else(|| {
            panic!(
                "clackr-core/build.rs: quotes file ({}) has no [[quote]] array",
                path.display()
            )
        });
    if quotes.is_empty() {
        panic!(
            "clackr-core/build.rs: quotes file ({}) has zero quotes",
            path.display()
        );
    }
    for (i, q) in quotes.iter().enumerate() {
        for field in ["text", "author", "source", "length"] {
            let value = q
                .get(field)
                .and_then(toml::Value::as_str)
                .unwrap_or_else(|| {
                    panic!(
                        "clackr-core/build.rs: quotes file ({}), entry {i}, is missing \
                     string field `{field}`",
                        path.display()
                    )
                });
            if field == "length" && !matches!(value, "short" | "medium" | "long") {
                panic!(
                    "clackr-core/build.rs: quotes file ({}), entry {i}, has \
                     length = {value:?}, expected short/medium/long",
                    path.display()
                );
            }
            if field == "text" && value.trim().is_empty() {
                panic!(
                    "clackr-core/build.rs: quotes file ({}), entry {i}, has empty text",
                    path.display()
                );
            }
        }
    }
}
