//! The only filesystem code in clackr.
//!
//! Config is the sole thing we persist — there is no result history and no
//! database, which is what keeps the binary a single static file.

use std::path::{Path, PathBuf};

use color_eyre::eyre::{Context, Result};

use clackr_core::Config;

/// Resolves the config file path.
///
/// XDG comes first even on macOS: someone running a terminal app expects
/// `~/.config/clackr/config.toml`, not `~/Library/Application Support`. We fall
/// back to the platform convention only when XDG is not configured.
pub fn config_path() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|s| !s.is_empty()) {
        return Some(Path::new(&xdg).join("clackr").join("config.toml"));
    }
    directories::ProjectDirs::from("dev", "", "clackr").map(|d| d.config_dir().join("config.toml"))
}

/// Loads the config, falling back to defaults when the file does not exist.
///
/// A malformed file is an error rather than a silent reset: silently
/// overwriting someone's settings because of one typo is worse than refusing to
/// start with a message naming the bad key.
pub fn load(explicit: Option<&Path>) -> Result<Config> {
    let path = match explicit {
        Some(p) => p.to_path_buf(),
        None => match config_path() {
            Some(p) => p,
            None => return Ok(Config::default()),
        },
    };

    match std::fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).wrap_err_with(|| format!("parsing {}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
        Err(e) => Err(e).wrap_err_with(|| format!("reading {}", path.display())),
    }
}

/// Writes the config back, creating the directory if needed.
pub fn save(cfg: &Config, explicit: Option<&Path>) -> Result<()> {
    let path = match explicit {
        Some(p) => p.to_path_buf(),
        None => config_path().ok_or_else(|| {
            color_eyre::eyre::eyre!("could not determine a config directory on this platform")
        })?,
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).wrap_err_with(|| format!("creating {}", dir.display()))?;
    }
    let text = toml::to_string_pretty(cfg).wrap_err("serialising config")?;
    std::fs::write(&path, text).wrap_err_with(|| format!("writing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_yields_defaults() {
        let missing = Path::new("/nonexistent/clackr/config.toml");
        assert_eq!(load(Some(missing)).unwrap(), Config::default());
    }

    #[test]
    fn malformed_file_is_an_error_not_a_silent_reset() {
        let dir = std::env::temp_dir().join("clackr-cfg-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.toml");
        std::fs::write(&path, "[ui]\nthemee = \"x\"\n").unwrap();

        let err = load(Some(&path)).unwrap_err();
        assert!(format!("{err:?}").contains("themee"), "{err:?}");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = std::env::temp_dir().join("clackr-cfg-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("round.toml");

        let mut cfg = Config::default();
        cfg.ui.theme = "gruvbox-dark".into();
        cfg.test.time = 120;
        save(&cfg, Some(&path)).unwrap();

        assert_eq!(load(Some(&path)).unwrap(), cfg);
        std::fs::remove_file(&path).ok();
    }
}
