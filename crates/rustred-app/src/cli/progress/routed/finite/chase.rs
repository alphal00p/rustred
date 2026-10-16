//! Bounded, display-only history for one native finite window. Sampling happens
//! in the heartbeat thread, never in the solver or its observation callback.
use serde_json::{Value, json};
use std::collections::VecDeque;

const LONG_WINDOW: f64 = 1800.;
const MAX_SAMPLES: usize = 1803;
// Heartbeats normally arrive once a second. A missing reference this much
// farther back is reported as a history gap, not as a five/thirty-minute trend.
const SAMPLE_TOLERANCE: f64 = 5.;

#[derive(Clone, Copy)]
struct Sample {
    elapsed: f64,
    discovered: u64,
    expanded: u64,
}

impl Sample {
    fn gap(self) -> u64 {
        // Inputs are checked before constructing a sample.
        self.discovered - self.expanded
    }
}

#[derive(Default)]
pub(in crate::cli::progress::routed) struct ChaseProgress {
    window: Option<(u64, u64)>,
    samples: VecDeque<Sample>,
    latest: Option<Sample>,
    fresh_at: Option<f64>,
}

impl ChaseProgress {
    pub(in crate::cli::progress::routed) fn annotate(&mut self, record: &mut Value) {
        let event = &record["progress"];
        let Some(window) = super::window_identity(event) else {
            *self = Self::default();
            record["finite_chase"] = json!({"status":"inactive"});
            return;
        };
        let local = &event["finite_local"]["snapshot"];
        let read = || {
            let elapsed = local["elapsed_seconds"].as_f64()?;
            let discovered = local["scheduled_nodes"].as_u64()?;
            let expanded = local["completed_nodes"].as_u64()?;
            (elapsed.is_finite() && elapsed >= 0. && expanded <= discovered).then_some(Sample {
                elapsed,
                discovered,
                expanded,
            })
        };
        let Some(sample) = read() else {
            *self = Self::default();
            record["finite_chase"] = json!({"status":"invalid_snapshot"});
            return;
        };
        let now = record["elapsed_seconds"].as_f64().unwrap_or(0.);
        let reset = self.window != Some(window)
            || self.latest.is_some_and(|old| {
                sample.elapsed < old.elapsed
                    || sample.discovered < old.discovered
                    || sample.expanded < old.expanded
            });
        if reset {
            *self = Self::default();
            self.window = Some(window);
        }
        // Re-rendering an old snapshot must not manufacture observations. The
        // window's native elapsed clock, not heartbeat wall time, dates samples.
        if self.latest.is_none_or(|old| sample.elapsed > old.elapsed) {
            self.fresh_at = Some(
                now - record["progress_age_seconds"]
                    .as_f64()
                    .unwrap_or(0.)
                    .max(0.),
            );
            self.latest = Some(sample);
            if self
                .samples
                .back()
                .is_some_and(|old| old.elapsed.floor() == sample.elapsed.floor())
            {
                self.samples.pop_back();
            }
            self.samples.push_back(sample);
            let cutoff = sample.elapsed - LONG_WINDOW;
            while self.samples.len() > 1 && self.samples.get(1).is_some_and(|s| s.elapsed <= cutoff)
            {
                self.samples.pop_front();
            }
            while self.samples.len() > MAX_SAMPLES {
                self.samples.pop_front();
            }
        }
        let sample = self.latest.unwrap();
        let age = self.fresh_at.map_or(0., |time| (now - time).max(0.));
        record["finite_chase"] = json!({
            "status":if local["finished"] == true {"finished"} else {"active"},
            "row":window.0,"window":window.1,
            "elapsed_seconds":sample.elapsed,
            "elapsed_source":"native_current_window_snapshot",
            "gap":sample.gap(),"gap_definition":"scheduled_nodes - completed_nodes",
            "snapshot_age_seconds":age,"snapshot_stale":age > SAMPLE_TOLERANCE,
            "five_minutes":self.convergence(sample, 300.),
            "thirty_minutes":self.convergence(sample, LONG_WINDOW),
            "negative_is_improving":true,"closure_or_eta_claim":false,
        });
    }

    fn convergence(&self, current: Sample, seconds: f64) -> Value {
        let cutoff = current.elapsed - seconds;
        let Some(previous) = self.samples.iter().rev().find(|s| s.elapsed <= cutoff) else {
            return json!({"status":"warming_up","window_seconds":seconds,"value":null});
        };
        let interval = current.elapsed - previous.elapsed;
        if interval > seconds + SAMPLE_TOLERANCE {
            return json!({"status":"history_gap","window_seconds":seconds,
                "comparison_seconds":interval,"value":null});
        }
        let gap = current.gap();
        let previous_gap = previous.gap();
        let denominator = u128::from(gap) + u128::from(previous_gap);
        let value = if denominator == 0 {
            0.
        } else {
            (i128::from(gap) - i128::from(previous_gap)) as f64 / denominator as f64
        };
        json!({"status":"available","window_seconds":seconds,
            "comparison_seconds":interval,"reference_gap":previous_gap,"value":value})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(row: u64, window: u64, elapsed: f64, discovered: u64, expanded: u64) -> Value {
        json!({"elapsed_seconds":elapsed,"progress_age_seconds":0.,
            "progress":{"operation":"finite_demand","finite_local":{
                "row":row,"window":window,"snapshot":{"elapsed_seconds":elapsed,
                    "scheduled_nodes":discovered,"completed_nodes":expanded}}}})
    }

    #[test]
    fn rolling_chase_convergence_has_real_five_and_thirty_minute_history() {
        let mut progress = ChaseProgress::default();
        for (time, discovered, expanded) in [(0., 100, 0), (299., 200, 150), (300., 200, 150)] {
            let mut event = sample(1, 2, time, discovered, expanded);
            progress.annotate(&mut event);
            let five = &event["finite_chase"]["five_minutes"];
            if time < 300. {
                assert_eq!(five["status"], "warming_up");
            } else {
                assert_eq!(five["value"], json!(-1. / 3.));
            }
            assert_eq!(
                event["finite_chase"]["thirty_minutes"]["status"],
                "warming_up"
            );
        }
        let mut event = sample(1, 2, 1800., 1000, 900);
        progress.annotate(&mut event);
        assert_eq!(event["finite_chase"]["thirty_minutes"]["value"], 0.);
        assert_eq!(
            event["finite_chase"]["five_minutes"]["status"],
            "history_gap"
        );
        assert_eq!(event["finite_chase"]["elapsed_seconds"], 1800.);
    }

    #[test]
    fn zero_gap_and_u64_extremes_do_not_overflow() {
        for (start, finish, expected) in [(0, 0, 0.), (0, u64::MAX, 1.), (u64::MAX, u64::MAX, 0.)] {
            let mut progress = ChaseProgress::default();
            let mut event = sample(0, 0, 0., start, 0);
            progress.annotate(&mut event);
            let mut event = sample(0, 0, 300., finish, 0);
            progress.annotate(&mut event);
            assert_eq!(event["finite_chase"]["five_minutes"]["value"], expected);
        }
    }

    #[test]
    fn chase_reset_and_inactive_states_never_reuse_another_windows_history() {
        let mut progress = ChaseProgress::default();
        let mut event = sample(1, 2, 0., 10, 0);
        progress.annotate(&mut event);
        let mut event = sample(1, 2, 300., 20, 10);
        progress.annotate(&mut event);
        assert_eq!(event["finite_chase"]["five_minutes"]["value"], 0.);
        for (row, window, elapsed, found, done) in [
            (1, 3, 301., 20, 10), // next window, even if clock did not restart
            (2, 3, 302., 20, 10), // next row
            (2, 3, 1., 20, 10),   // native clock reset
            (2, 3, 302., 20, 9),  // expanded counter reset
            (2, 3, 303., 19, 9),  // discovered counter reset
        ] {
            let mut event = sample(row, window, elapsed, found, done);
            progress.annotate(&mut event);
            assert_eq!(
                event["finite_chase"]["five_minutes"]["status"],
                "warming_up"
            );
        }
        let mut event = json!({"progress":{"operation":"finite_demand"}});
        progress.annotate(&mut event);
        assert_eq!(event["finite_chase"]["status"], "inactive");
        assert!(progress.samples.is_empty());
        let mut event = sample(2, 3, 600., 20, 10);
        progress.annotate(&mut event);
        assert_eq!(
            event["finite_chase"]["five_minutes"]["status"],
            "warming_up"
        );
        let mut event = sample(2, 3, 700., 10, 20);
        progress.annotate(&mut event);
        assert_eq!(event["finite_chase"]["status"], "invalid_snapshot");
    }

    #[test]
    fn stale_snapshots_do_not_create_zero_growth_or_advance_chase_elapsed() {
        let mut progress = ChaseProgress::default();
        let mut event = sample(0, 0, 0., 10, 0);
        progress.annotate(&mut event);
        let mut event = sample(0, 0, 300., 20, 15);
        progress.annotate(&mut event);
        let value = event["finite_chase"]["five_minutes"]["value"].clone();
        event["elapsed_seconds"] = json!(2100.);
        progress.annotate(&mut event);
        assert_eq!(event["finite_chase"]["elapsed_seconds"], 300.);
        assert_eq!(event["finite_chase"]["snapshot_age_seconds"], 1800.);
        assert_eq!(event["finite_chase"]["snapshot_stale"], true);
        assert_eq!(event["finite_chase"]["five_minutes"]["value"], value);
        assert_eq!(
            event["finite_chase"]["thirty_minutes"]["status"],
            "warming_up"
        );
        assert_eq!(progress.samples.len(), 2);
    }

    #[test]
    fn chase_history_is_bounded_even_with_subsecond_heartbeat_bursts() {
        let mut progress = ChaseProgress::default();
        for tick in 0..40000 {
            let mut event = sample(0, 0, tick as f64 / 10., tick, tick / 2);
            progress.annotate(&mut event);
            assert!(progress.samples.len() <= MAX_SAMPLES);
        }
        let mut event = sample(0, 0, 4000., 40000, 20000);
        progress.annotate(&mut event);
        for field in ["five_minutes", "thirty_minutes"] {
            assert_eq!(event["finite_chase"][field]["status"], "available");
        }
    }
}
