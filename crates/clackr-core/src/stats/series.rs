//! Per-second samples: the data behind the results chart.
//!
//! One bucket per wall-clock second, keyed off the `elapsed` the caller stamps
//! at *read* time, so a render stall cannot smear the graph. Buckets are sealed
//! lazily; a pause seals a run of zero-key buckets, which is correct and is
//! exactly what makes consistency punish stalls.

use serde::{Deserialize, Serialize};

/// Keys per second → words per minute: `keys / 5 * 60`.
const KEYS_TO_WPM: f64 = 12.0;

/// What the two lines on the chart mean.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum SeriesMode {
    /// MonkeyType: raw is instantaneous, wpm is cumulative net. The two are
    /// deliberately different shapes — the jagged raw line against the smooth
    /// net line is the chart's whole visual language.
    #[default]
    MonkeyType,
    /// Both lines instantaneous. Kept because it is the obvious alternative
    /// reading of the reference screenshot; switching is a one-line change.
    BothInstantaneous,
}

/// One second of the test.
#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SecondSample {
    /// Zero-based second index.
    pub second: u32,
    /// Keystrokes counted in this second (correct and incorrect).
    pub keys: u32,
    /// Of those, how many were wrong. Plotted as the scatter series.
    pub errors: u32,
    /// Instantaneous raw WPM: `keys * 12`. Consistency is computed from this
    /// series, so it must stay instantaneous — a cumulative series would
    /// smooth away exactly the variance we are trying to measure.
    pub raw_wpm: f64,
    /// Cumulative net WPM at the end of this second.
    pub wpm: f64,
}

/// Accumulates [`SecondSample`]s. Clock-free: the second index is supplied.
#[derive(Clone, Debug, Default)]
pub struct SecondSampler {
    mode: SeriesMode,
    samples: Vec<SecondSample>,
    /// The open bucket's second index.
    cur: u32,
    keys: u32,
    errors: u32,
    /// A bucket is open, i.e. at least one key has been recorded.
    active: bool,
}

impl SecondSampler {
    pub fn new(mode: SeriesMode) -> Self {
        Self {
            mode,
            ..Self::default()
        }
    }

    pub fn samples(&self) -> &[SecondSample] {
        &self.samples
    }

    /// Whether [`advance_to`](Self::advance_to) would seal anything. Lets the
    /// caller skip computing `net_chars`, which is O(words), on the hot path.
    pub fn needs_seal(&self, second: u32) -> bool {
        self.active && self.cur < second
    }

    /// Seal every bucket strictly before `second`, filling the gap with
    /// zero-key buckets. `net_chars` is the WPM numerator as of now; during a
    /// gap it has not changed, so using the current value for every sealed
    /// bucket is exact.
    pub fn advance_to(&mut self, second: u32, net_chars: u32) {
        if !self.active {
            return;
        }
        while self.cur < second {
            self.seal(net_chars);
        }
    }

    /// Count one keystroke into the bucket for `second`.
    ///
    /// The caller is expected to have sealed up to `second` first (see
    /// [`needs_seal`](Self::needs_seal)); the fallback below keeps the series
    /// monotonic if it did not.
    pub fn record(&mut self, second: u32, correct: bool) {
        if !self.active {
            self.active = true;
            self.cur = 0;
            self.keys = 0;
            self.errors = 0;
            // Any seconds before the first keystroke really were empty, and
            // the numerator really was zero, so this needs no counts.
            self.advance_to(second, 0);
        } else if self.cur < second {
            debug_assert!(false, "seal buckets before recording into a later one");
            self.advance_to(second, 0);
        }
        self.keys += 1;
        if !correct {
            self.errors += 1;
        }
    }

    /// Seal everything through the end of the test.
    ///
    /// The open bucket is only emitted if it saw keystrokes: when a test ends
    /// exactly on a second boundary the "current" bucket is an empty one that
    /// belongs to time that never happened.
    pub fn close(&mut self, second: u32, net_chars: u32) {
        self.advance_to(second, net_chars);
        if self.active && self.keys > 0 {
            self.seal(net_chars);
        }
        self.active = false;
    }

    /// The series as it would be if the test ended now — without mutating.
    /// `Engine::summary` takes `&self`, so it needs this.
    pub fn snapshot(&self, second: u32, net_chars: u32) -> Vec<SecondSample> {
        let mut clone = self.clone();
        clone.close(second, net_chars);
        clone.samples
    }

    fn seal(&mut self, net_chars: u32) {
        let raw_wpm = f64::from(self.keys) * KEYS_TO_WPM;
        let elapsed_secs = f64::from(self.cur) + 1.0;
        let wpm = match self.mode {
            // Cumulative: net chars so far spread over the time so far.
            SeriesMode::MonkeyType => f64::from(net_chars) * 60.0 / 5.0 / elapsed_secs,
            SeriesMode::BothInstantaneous => {
                f64::from(self.keys.saturating_sub(self.errors)) * KEYS_TO_WPM
            }
        };
        self.samples.push(SecondSample {
            second: self.cur,
            keys: self.keys,
            errors: self.errors,
            raw_wpm,
            wpm,
        });
        self.cur += 1;
        self.keys = 0;
        self.errors = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pause_seals_zero_key_buckets() {
        let mut s = SecondSampler::default();
        s.record(0, true);
        s.record(0, true);
        // Nothing for seconds 1 and 2, then a key in second 3.
        s.advance_to(3, 2);
        s.record(3, true);
        s.close(4, 3);
        let keys: Vec<u32> = s.samples().iter().map(|x| x.keys).collect();
        assert_eq!(keys, vec![2, 0, 0, 1]);
        assert_eq!(s.samples()[1].raw_wpm, 0.0);
    }

    #[test]
    fn closing_on_a_boundary_does_not_invent_an_empty_second() {
        let mut s = SecondSampler::default();
        s.record(0, true);
        s.close(1, 1);
        assert_eq!(s.samples().len(), 1);
    }

    #[test]
    fn closing_mid_second_keeps_the_partial_bucket() {
        let mut s = SecondSampler::default();
        s.record(0, true);
        s.advance_to(1, 1); // the seal the caller owes before recording later
        s.record(1, true);
        s.close(1, 2);
        assert_eq!(s.samples().len(), 2);
    }

    #[test]
    fn snapshot_does_not_mutate() {
        let mut s = SecondSampler::default();
        s.record(0, true);
        let snap = s.snapshot(1, 1);
        assert_eq!(snap.len(), 1);
        assert_eq!(s.samples().len(), 0);
    }

    #[test]
    fn raw_wpm_is_keys_times_twelve() {
        let mut s = SecondSampler::default();
        for _ in 0..5 {
            s.record(0, true);
        }
        s.close(1, 5);
        assert_eq!(s.samples()[0].raw_wpm, 60.0);
    }
}
