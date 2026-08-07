//! Test modes and the parameters that define a single test run.

use serde::{Deserialize, Serialize};

/// Which kind of test is being run.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TestMode {
    /// Type for a fixed number of seconds; words are appended indefinitely.
    #[default]
    Time,
    /// Type a fixed number of words.
    Words,
    /// Type a single quote drawn from the quote corpus.
    Quote,
    /// Free typing with no target text; ends only when the user says so.
    Zen,
}

impl TestMode {
    pub const ALL: [TestMode; 4] = [Self::Time, Self::Words, Self::Quote, Self::Zen];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Time => "time",
            Self::Words => "words",
            Self::Quote => "quote",
            Self::Zen => "zen",
        }
    }
}

/// Quote length buckets, matched against the quote's character count.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QuoteLength {
    Short,
    #[default]
    Medium,
    Long,
    /// Any length.
    All,
}

impl QuoteLength {
    /// Character-count range this bucket accepts.
    pub fn accepts(self, chars: usize) -> bool {
        match self {
            Self::Short => chars < 100,
            Self::Medium => (100..=300).contains(&chars),
            Self::Long => chars > 300,
            Self::All => true,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Short => "short",
            Self::Medium => "medium",
            Self::Long => "long",
            Self::All => "all",
        }
    }
}

/// What happens when the user types a wrong character.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StopOnError {
    /// Wrong characters are accepted and shown as errors.
    #[default]
    Off,
    /// Wrong characters are rejected outright (but still count against accuracy).
    Letter,
    /// The word cannot be submitted until it is correct.
    Word,
}

/// Everything that defines one test run. Cheap to clone.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestParams {
    pub mode: TestMode,
    /// Duration in seconds, used when `mode == Time`.
    pub time: u32,
    /// Word count, used when `mode == Words`.
    pub words: u32,
    /// Quote bucket, used when `mode == Quote`.
    pub quote: QuoteLength,
    pub language: String,
    /// Allow backspacing back into words that were typed perfectly.
    pub freedom_mode: bool,
    pub stop_on_error: StopOnError,
    /// Hide per-character correctness feedback while typing.
    pub blind_mode: bool,
}

impl Default for TestParams {
    fn default() -> Self {
        Self {
            mode: TestMode::Time,
            time: 30,
            words: 25,
            quote: QuoteLength::Medium,
            language: "english1k".to_string(),
            freedom_mode: false,
            stop_on_error: StopOnError::Off,
            blind_mode: false,
        }
    }
}

impl TestParams {
    /// The label shown as "test type" on the results screen, e.g. `"time 30"`.
    pub fn label(&self) -> String {
        match self.mode {
            TestMode::Time => format!("time {}", self.time),
            TestMode::Words => format!("words {}", self.words),
            TestMode::Quote => format!("quote {}", self.quote.as_str()),
            TestMode::Zen => "zen".to_string(),
        }
    }

    /// Time modes stream words forever; the others have a fixed target.
    pub fn is_endless(&self) -> bool {
        matches!(self.mode, TestMode::Time | TestMode::Zen)
    }

    /// Hard time limit in seconds, if the mode has one.
    pub fn time_limit(&self) -> Option<u32> {
        (self.mode == TestMode::Time).then_some(self.time)
    }
}
