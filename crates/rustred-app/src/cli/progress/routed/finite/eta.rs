//! Approximate input ETA, independent of the current descendant chase.
//!
//! A retained prefix on resume is not throughput. Capture it from the lossless
//! initial checkpoint milestone, never from the overwriteable heartbeat slot.
//! Only scalar telemetry is retained; no campaign traversal or algebra is done.
use serde_json::{Value, json};

#[derive(Default)]
pub(in crate::cli::progress::routed) struct InputEta {
    baseline: Option<Baseline>,
    milestone_current: Option<u64>,
    displayed_current: Option<u64>,
    previous_elapsed: Option<f64>,
    invalid: Option<&'static str>,
}

struct Baseline {
    initial: u64,
    total: String,
    checkpoint: String,
}

fn unsigned(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| digits(value)?.parse().ok())
}

fn digits(value: &Value) -> Option<String> {
    if let Some(number) = value.as_u64() {
        return Some(number.to_string());
    }
    let text = value.as_str()?;
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let trimmed = text.trim_start_matches('0');
    Some(if trimmed.is_empty() { "0" } else { trimmed }.into())
}

// Keep integer subtraction exact where machine arithmetic suffices, including
// adjacent counters above f64's exact integer range. Beyond u128, current is
// still a native u64 counter, so approximate telemetry subtraction is harmless.
fn remaining(total: &str, current: u64) -> Option<f64> {
    if let Ok(total) = total.parse::<u128>() {
        return total.checked_sub(u128::from(current)).map(|n| n as f64);
    }
    let total = total.parse::<f64>().ok()?;
    let remaining = total - current as f64;
    (total.is_finite() && remaining.is_finite() && remaining >= 0.).then_some(remaining)
}

impl InputEta {
    /// Call in FIFO control-message order, before sampling the latest slot.
    pub(in crate::cli::progress::routed) fn milestone(&mut self, event: &Value) {
        if event["operation"] != "finite_demand"
            || !matches!(
                event["event"].as_str(),
                Some("finite_checkpoint_saved" | "finite_checkpoint_restored")
            )
            || self.invalid.is_some()
        {
            return;
        }
        let Some(current) = unsigned(&event["committed"]["input_occurrences"]) else {
            self.invalid = Some("invalid_committed_count");
            return;
        };
        if self
            .milestone_current
            .is_some_and(|previous| current < previous)
        {
            self.invalid = Some("counter_regression");
            return;
        }
        self.milestone_current = Some(current);
        if let Some(baseline) = &self.baseline {
            if event["checkpoint"].as_str() != Some(baseline.checkpoint.as_str())
                || event
                    .get("input_occurrences_total")
                    .is_some_and(|total| digits(total).as_deref() != Some(baseline.total.as_str()))
            {
                self.invalid = Some("scope_changed");
            }
            return;
        }
        // No later window/result may impersonate the invocation's starting
        // checkpoint: that would count retained work as freshly solved work.
        if event["window"].is_object() {
            self.invalid = Some("missing_initial_checkpoint");
            return;
        }
        let Some(total) = digits(&event["input_occurrences_total"]) else {
            self.invalid = Some("invalid_total_count");
            return;
        };
        let Some(checkpoint) = event["checkpoint"].as_str() else {
            self.invalid = Some("missing_scope_identity");
            return;
        };
        self.baseline = Some(Baseline {
            initial: current,
            total,
            checkpoint: checkpoint.into(),
        });
    }

    pub(in crate::cli::progress::routed) fn annotate(&mut self, record: &mut Value) {
        if record["progress"]["operation"] != "finite_demand" {
            return;
        }
        let elapsed = record["elapsed_seconds"]
            .as_f64()
            .filter(|n| n.is_finite() && *n >= 0.);
        let event = &record["progress"];
        let current = unsigned(&event["finite_committed"]["input_occurrences"]);
        let mut result = json!({
            "status":"warming_up", "reason":"awaiting_initial_checkpoint",
            "remaining_seconds":null, "inputs_per_second":null,
            "initial_committed":self.baseline.as_ref().map(|b| b.initial),
            "current_committed":current,
            "total_inputs":self.baseline.as_ref().map(|b| &b.total),
            "elapsed_seconds":elapsed,
            "method":"whole_invocation_committed_input_average",
            "publication_excluded":true, "estimate_only":true,
        });
        let unavailable = |result: &mut Value, reason: &str| {
            result["status"] = json!("unavailable");
            result["reason"] = json!(reason);
        };
        if let Some(reason) = self.invalid {
            unavailable(&mut result, reason);
        } else if let Some(baseline) = &self.baseline {
            let calculation = (|| {
                let elapsed = elapsed.ok_or("invalid_elapsed")?;
                if self
                    .previous_elapsed
                    .is_some_and(|previous| elapsed < previous)
                {
                    return Err("elapsed_regression");
                }
                self.previous_elapsed = Some(elapsed);
                let current = current.ok_or("invalid_committed_count")?;
                if current < baseline.initial
                    || self
                        .displayed_current
                        .is_some_and(|previous| current < previous)
                {
                    return Err("counter_regression");
                }
                self.displayed_current = Some(current);
                if digits(&event["finite_totals"]["input_occurrences"]).as_deref()
                    != Some(baseline.total.as_str())
                {
                    return Err("scope_changed");
                }
                let left =
                    remaining(&baseline.total, current).ok_or("count_range_or_inconsistency")?;
                if left == 0. && event["finite_committed"]["required_complete"] == true {
                    result["status"] = json!("complete");
                    result["reason"] = Value::Null;
                    result["remaining_seconds"] = json!(0.);
                    return Ok(());
                }
                let newly_committed = current - baseline.initial;
                if newly_committed == 0 {
                    result["reason"] = json!("awaiting_committed_progress");
                    return Ok(());
                }
                if left == 0. {
                    result["reason"] = json!("awaiting_native_completion");
                    return Ok(());
                }
                if elapsed == 0. {
                    return Err("zero_elapsed");
                }
                let rate = newly_committed as f64 / elapsed;
                let seconds = left / rate;
                if !rate.is_finite() || rate <= 0. || !seconds.is_finite() || seconds <= 0. {
                    return Err("estimate_out_of_range");
                }
                result["status"] = json!("available");
                result["reason"] = Value::Null;
                result["inputs_per_second"] = json!(rate);
                result["remaining_seconds"] = json!(seconds);
                Ok(())
            })();
            if let Err(reason) = calculation {
                // Progress errors affect presentation only. They never change
                // the solver, its quotas, or publication/closure authority.
                self.invalid = Some(reason);
                unavailable(&mut result, reason);
            }
        }
        record["finite_input_eta"] = result;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checkpoint(initial: u64, total: Value, restored: bool) -> Value {
        json!({"event":if restored {"finite_checkpoint_restored"} else {"finite_checkpoint_saved"},
            "operation":"finite_demand", "checkpoint":"campaign/store",
            "committed":{"input_occurrences":initial},"input_occurrences_total":total})
    }

    fn heartbeat(current: u64, total: Value, elapsed: f64, complete: bool) -> Value {
        json!({"elapsed_seconds":elapsed,"progress":{"operation":"finite_demand",
            "finite_committed":{"input_occurrences":current,"required_complete":complete},
            "finite_totals":{"input_occurrences":total}}})
    }

    fn estimate(tracker: &mut InputEta, current: u64, total: Value, elapsed: f64) -> Value {
        let mut record = heartbeat(current, total, elapsed, false);
        tracker.annotate(&mut record);
        record["finite_input_eta"].take()
    }

    #[test]
    fn fresh_run_whole_elapsed_includes_preparation_and_long_chase() {
        let mut tracker = InputEta::default();
        tracker.milestone(&checkpoint(0, json!("100"), false));
        let first = estimate(&mut tracker, 25, json!("100"), 100.);
        assert_eq!(first["remaining_seconds"], 300.);
        assert_eq!(first["inputs_per_second"], 0.25);
        // No new commit during a chase: the whole-run average slows down.
        let later = estimate(&mut tracker, 25, json!("100"), 200.);
        assert_eq!(later["remaining_seconds"], 600.);
        assert_eq!(later["publication_excluded"], true);
    }

    #[test]
    fn resume_or_extension_never_counts_retained_prefix_as_new_throughput() {
        for restored in [false, true] {
            let mut tracker = InputEta::default();
            tracker.milestone(&checkpoint(100, json!("200"), restored));
            let idle = estimate(&mut tracker, 100, json!("200"), 60.);
            assert_eq!(idle["status"], "warming_up");
            let moving = estimate(&mut tracker, 120, json!("200"), 60.);
            assert_eq!(moving["remaining_seconds"], 240.);
            assert_eq!(moving["initial_committed"], 100);
        }
    }

    #[test]
    fn initial_lossless_milestone_survives_latest_first_window_race() {
        let mut tracker = InputEta::default();
        let initial = checkpoint(100, json!("200"), true);
        let mut next = checkpoint(120, Value::Null, false);
        next.as_object_mut()
            .unwrap()
            .remove("input_occurrences_total");
        next["window"] = json!({"row":0,"inputs":20});
        // The latest slot already points beyond initial when FIFO control
        // delivers it. Reading the latest counts must not set the baseline.
        tracker.milestone(&initial);
        assert_eq!(
            estimate(&mut tracker, 120, json!("200"), 60.)["remaining_seconds"],
            240.
        );
        tracker.milestone(&next);
        assert_eq!(
            estimate(&mut tracker, 120, json!("200"), 120.)["remaining_seconds"],
            480.
        );
    }

    #[test]
    fn no_progress_no_initial_or_no_time_never_invents_an_eta() {
        let mut tracker = InputEta::default();
        assert_eq!(
            estimate(&mut tracker, 20, json!("100"), 60.)["status"],
            "warming_up"
        );
        tracker.milestone(&checkpoint(0, json!("100"), false));
        assert_eq!(
            estimate(&mut tracker, 0, json!("100"), 60.)["remaining_seconds"],
            Value::Null
        );
        let mut zero_time = InputEta::default();
        zero_time.milestone(&checkpoint(0, json!("100"), false));
        assert_eq!(
            estimate(&mut zero_time, 1, json!("100"), 0.)["status"],
            "unavailable"
        );
    }

    #[test]
    fn completion_requires_native_scope_confirmation_even_at_zero_total() {
        for count in [0, 100] {
            let mut tracker = InputEta::default();
            tracker.milestone(&checkpoint(0, json!(count.to_string()), false));
            let total = json!(count.to_string());
            let pending = estimate(&mut tracker, count, total.clone(), 0.);
            assert!(pending["remaining_seconds"].is_null());
            let mut done = heartbeat(count, total, 0., true);
            tracker.annotate(&mut done);
            assert_eq!(done["finite_input_eta"]["status"], "complete");
            assert_eq!(done["finite_input_eta"]["remaining_seconds"], 0.);
        }
    }

    #[test]
    fn counts_above_float_exact_range_keep_small_integer_differences() {
        let mut tracker = InputEta::default();
        let total = json!((u128::from(u64::MAX) + 100).to_string());
        tracker.milestone(&checkpoint(u64::MAX - 10, total.clone(), true));
        let result = estimate(&mut tracker, u64::MAX, total, 10.);
        assert_eq!(result["remaining_seconds"], 100.);
        let mut huge = InputEta::default();
        let total = json!("1000000000000000000000000000000000000000000000000000000000000");
        huge.milestone(&checkpoint(0, total.clone(), false));
        assert_eq!(estimate(&mut huge, 1, total, 1.)["status"], "available");
        let mut overflow = InputEta::default();
        let total = json!(format!("1{}", "0".repeat(400)));
        overflow.milestone(&checkpoint(0, total.clone(), false));
        assert_eq!(
            estimate(&mut overflow, 1, total, 1.)["status"],
            "unavailable"
        );
    }

    #[test]
    fn invalid_backward_or_different_scope_stays_unavailable() {
        for invalid_total in [json!("abc"), json!("-1"), json!("1.5"), Value::Null] {
            let mut tracker = InputEta::default();
            tracker.milestone(&checkpoint(0, invalid_total.clone(), false));
            assert_eq!(
                estimate(&mut tracker, 1, invalid_total, 1.)["status"],
                "unavailable"
            );
        }
        for (current, total, elapsed) in [
            (9, "100", 20.),
            (20, "101", 20.),
            (20, "100", 5.),
            (101, "100", 20.),
        ] {
            let mut tracker = InputEta::default();
            tracker.milestone(&checkpoint(0, json!("100"), false));
            estimate(&mut tracker, 10, json!("100"), 10.);
            let result = estimate(&mut tracker, current, json!(total), elapsed);
            assert_eq!(result["status"], "unavailable");
        }
        let mut tracker = InputEta::default();
        tracker.milestone(&checkpoint(0, json!("100"), false));
        let mut changed = checkpoint(10, json!("100"), false);
        changed["checkpoint"] = json!("different/store");
        tracker.milestone(&changed);
        assert_eq!(
            estimate(&mut tracker, 10, json!("100"), 10.)["reason"],
            "scope_changed"
        );
    }
}
