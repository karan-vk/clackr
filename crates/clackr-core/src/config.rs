//! Config as pure data plus its serde shape. Locating and reading the file on
//! disk is the binary's job (`clackr::config_io`).

use serde::{Deserialize, Serialize};

use crate::mode::{QuoteLength, StopOnError, TestMode, TestParams};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub test: TestConfig,
    pub ui: UiConfig,
    pub keys: KeyConfig,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TestConfig {
    pub mode: TestMode,
    pub time: u32,
    pub words: u32,
    pub quote: QuoteLength,
    pub language: String,
    pub freedom_mode: bool,
    pub stop_on_error: StopOnError,
    pub blind_mode: bool,
    /// Reserved for a future release; parsed but currently inert.
    pub punctuation: bool,
    /// Reserved for a future release; parsed but currently inert.
    pub numbers: bool,
}

impl Default for TestConfig {
    fn default() -> Self {
        let p = TestParams::default();
        Self {
            mode: p.mode,
            time: p.time,
            words: p.words,
            quote: p.quote,
            language: p.language,
            freedom_mode: p.freedom_mode,
            stop_on_error: p.stop_on_error,
            blind_mode: p.blind_mode,
            punctuation: false,
            numbers: false,
        }
    }
}

/// Rendering fidelity. `Auto` inspects the terminal's capabilities and size.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FidelityPref {
    #[default]
    Auto,
    Full,
    Reduced,
    Plain,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CaretStyle {
    #[default]
    Bar,
    Block,
    Underline,
    Off,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UiConfig {
    pub theme: String,
    pub caret: CaretStyle,
    pub fidelity: FidelityPref,
    pub show_live_wpm: bool,
    pub show_live_acc: bool,
    pub show_timer: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "amber-dark".to_string(),
            caret: CaretStyle::Bar,
            fidelity: FidelityPref::Auto,
            show_live_wpm: true,
            show_live_acc: false,
            show_timer: true,
        }
    }
}

/// How Tab behaves during a test.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QuickRestart {
    /// Tab restarts immediately.
    #[default]
    Tab,
    /// Esc restarts; Tab is inert.
    Esc,
    /// Tab arms a restart that Enter confirms.
    Off,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct KeyConfig {
    pub quick_restart: QuickRestart,
}

impl Config {
    /// The test parameters this config describes.
    pub fn params(&self) -> TestParams {
        TestParams {
            mode: self.test.mode,
            time: self.test.time,
            words: self.test.words,
            quote: self.test.quote,
            language: self.test.language.clone(),
            freedom_mode: self.test.freedom_mode,
            stop_on_error: self.test.stop_on_error,
            blind_mode: self.test.blind_mode,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_round_trip_through_toml() {
        let cfg = Config::default();
        let text = toml::to_string(&cfg).unwrap();
        assert_eq!(toml::from_str::<Config>(&text).unwrap(), cfg);
    }

    #[test]
    fn partial_config_fills_in_defaults() {
        let cfg: Config = toml::from_str("[ui]\ntheme = \"nord\"\n").unwrap();
        assert_eq!(cfg.ui.theme, "nord");
        assert_eq!(cfg.test, TestConfig::default());
        assert_eq!(cfg.ui.caret, CaretStyle::Bar);
    }

    #[test]
    fn unknown_keys_are_rejected_loudly() {
        // deny_unknown_fields turns a typo in the config file into a message
        // naming the bad key rather than a silently ignored setting.
        let err = toml::from_str::<Config>("[ui]\nthemee = \"nord\"\n").unwrap_err();
        assert!(err.to_string().contains("themee"), "{err}");
    }
}
