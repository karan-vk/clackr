//! Command-line surface. Every flag here overrides the corresponding config
//! file setting for this run only; nothing written on the command line is
//! persisted.

use clap::{Parser, ValueEnum};

use clackr_core::config::{Config, FidelityPref};
use clackr_core::mode::{QuoteLength, TestMode};

#[derive(Parser, Debug)]
#[command(
    name = "clackr",
    version,
    about = "A MonkeyType-style typing test for your terminal.",
    long_about = None,
)]
pub struct Cli {
    /// Test mode.
    #[arg(short, long, value_enum)]
    pub mode: Option<ModeArg>,

    /// Seconds to type for, in time mode.
    #[arg(short, long, value_name = "SECONDS")]
    pub time: Option<u32>,

    /// Number of words, in words mode.
    #[arg(short, long, value_name = "COUNT")]
    pub words: Option<u32>,

    /// Quote length, in quote mode.
    #[arg(short, long, value_enum)]
    pub quote: Option<QuoteArg>,

    /// Word list to draw from.
    #[arg(short, long, value_name = "NAME")]
    pub language: Option<String>,

    /// Colour theme.
    #[arg(long, value_name = "NAME")]
    pub theme: Option<String>,

    /// Use the low-fidelity layout regardless of what the terminal supports.
    #[arg(long)]
    pub simple: bool,

    /// Draw with ASCII only, no Unicode box-drawing or braille.
    #[arg(long)]
    pub ascii: bool,

    /// Use this config file instead of the default location.
    #[arg(long, value_name = "PATH")]
    pub config: Option<std::path::PathBuf>,

    /// Print the available word lists and exit.
    #[arg(long)]
    pub list_languages: bool,

    /// Print the available themes and exit.
    #[arg(long)]
    pub list_themes: bool,

    /// Print the resolved config file path and exit.
    #[arg(long)]
    pub config_path: bool,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum ModeArg {
    Time,
    Words,
    Quote,
    Zen,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum QuoteArg {
    Short,
    Medium,
    Long,
    All,
}

impl Cli {
    /// Folds the command-line overrides into a config loaded from disk.
    ///
    /// Precedence is CLI > config file > defaults, and it is applied field by
    /// field so passing `--time 60` does not reset the user's theme.
    pub fn apply_to(&self, cfg: &mut Config) {
        if let Some(m) = self.mode {
            cfg.test.mode = match m {
                ModeArg::Time => TestMode::Time,
                ModeArg::Words => TestMode::Words,
                ModeArg::Quote => TestMode::Quote,
                ModeArg::Zen => TestMode::Zen,
            };
        }
        if let Some(t) = self.time {
            cfg.test.time = t;
            // Asking for a duration is an unambiguous request for time mode.
            if self.mode.is_none() {
                cfg.test.mode = TestMode::Time;
            }
        }
        if let Some(w) = self.words {
            cfg.test.words = w;
            if self.mode.is_none() && self.time.is_none() {
                cfg.test.mode = TestMode::Words;
            }
        }
        if let Some(q) = self.quote {
            cfg.test.quote = match q {
                QuoteArg::Short => QuoteLength::Short,
                QuoteArg::Medium => QuoteLength::Medium,
                QuoteArg::Long => QuoteLength::Long,
                QuoteArg::All => QuoteLength::All,
            };
            if self.mode.is_none() {
                cfg.test.mode = TestMode::Quote;
            }
        }
        if let Some(l) = &self.language {
            cfg.test.language = l.clone();
        }
        if let Some(t) = &self.theme {
            cfg.ui.theme = t.clone();
        }
        if self.simple {
            cfg.ui.fidelity = FidelityPref::Plain;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn time_flag_implies_time_mode() {
        let cli = Cli::try_parse_from(["clackr", "--time", "60"]).unwrap();
        let mut cfg = Config::default();
        cfg.test.mode = TestMode::Words;
        cli.apply_to(&mut cfg);
        assert_eq!(cfg.test.mode, TestMode::Time);
        assert_eq!(cfg.test.time, 60);
    }

    #[test]
    fn explicit_mode_wins_over_implication() {
        let cli = Cli::try_parse_from(["clackr", "--mode", "zen", "--time", "60"]).unwrap();
        let mut cfg = Config::default();
        cli.apply_to(&mut cfg);
        assert_eq!(cfg.test.mode, TestMode::Zen);
    }

    #[test]
    fn overrides_leave_unrelated_settings_alone() {
        let cli = Cli::try_parse_from(["clackr", "--words", "50"]).unwrap();
        let mut cfg = Config::default();
        cfg.ui.theme = "nord".into();
        cli.apply_to(&mut cfg);
        assert_eq!(cfg.ui.theme, "nord");
    }
}
