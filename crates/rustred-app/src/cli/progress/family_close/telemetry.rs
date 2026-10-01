//! Versioned, bounded observations for external preparation presenters.
//!
//! These values never certify rules, checkpoints, output files or closure.
//! Serialization and publication belong to a presenter, never solver callbacks.

use serde::{Deserialize, Serialize};
use std::path::Path;

mod producer;
mod stage;
pub(crate) use producer::FamilyGenerationTelemetry;

pub(crate) const SCHEMA: &str = "rustred.family-generation-progress.v1";
pub(crate) const MAX_SNAPSHOT_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_ACTIVE_JOBS: usize = 256;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct NativeSnapshot {
    pub(crate) schema: String,
    pub(crate) pid: u32,
    pub(crate) process_start_ticks: Option<u64>,
    pub(crate) invocation_id: String,
    /// Increases on observations, not on repeated publication of the same data.
    pub(crate) revision: u64,
    pub(crate) state: NativeState,
    pub(crate) elapsed_seconds: f64,
    pub(crate) last_event_elapsed_seconds: Option<f64>,
    pub(crate) counts: NativeCounts,
    pub(crate) active_jobs: Vec<ActiveJob>,
    pub(crate) details_truncated: bool,
    pub(crate) last_failure: Option<NativeFailure>,
    pub(crate) checkpoint: CheckpointSemantics,
    pub(crate) family_closure_claim: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NativeState {
    Running,
    OutputWritten,
    Failed,
    Stopped,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct NativeCounts {
    pub(crate) sectors_total: Option<u64>,
    pub(crate) generated: u64,
    pub(crate) reused: u64,
    pub(crate) checkpointed_new: u64,
    /// Current invocation's generated sectors only; reused totals are unknown.
    pub(crate) rules_generated: u64,
    pub(crate) finite_residuals_generated: u64,
    /// RuleFound observations, deliberately separate from completed-sector rules.
    pub(crate) observed_rule_hits: u64,
    pub(crate) failures: u64,
    pub(crate) overflow: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ActiveJob {
    pub(crate) ordinal: u64,
    /// Decimal u64 string: bit i remains original denominator coordinate i.
    pub(crate) sector_mask: String,
    pub(crate) phase: String,
    pub(crate) phase_elapsed_seconds: f64,
    pub(crate) last_event_elapsed_seconds: f64,
    pub(crate) case_index: Option<u64>,
    pub(crate) frame: Option<FrameSnapshot>,
    pub(crate) detail: StageDetail,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct FrameSnapshot {
    pub(crate) index: Option<u64>,
    pub(crate) source_rows: u64,
    pub(crate) integral_columns: u64,
    pub(crate) target_column: u64,
    pub(crate) input_terms: u64,
    pub(crate) coefficient_variables: u64,
    pub(crate) active_variables: u64,
}

/// Typed scalar details, not a stringified solver object or coefficient payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum StageDetail {
    Case {
        pending: usize,
    },
    Discovery {
        depth: u32,
        seeds: usize,
        rows: usize,
    },
    ExactMaterialization,
    ExactFramePrepared {
        source_rows: usize,
        integral_columns: usize,
        target_column: usize,
        input_terms: usize,
        coefficient_variables: usize,
        active_variables: usize,
    },
    ExactDenseFractionFreeStarted {
        rows: usize,
        columns: usize,
        reduction_columns: usize,
        rational_coefficients: bool,
    },
    ExactDenseFractionFreeFinished {
        rank: usize,
    },
    ExactTargetBlockStarted {
        columns: usize,
    },
    ExactTargetWeightsStarted {
        rows: usize,
        lower_nonzeros: usize,
    },
    ExactTargetWeightsFinished {
        nonzero_weights: usize,
    },
    ExactTargetReconstructionStarted {
        rows: usize,
        columns: usize,
    },
    ExactTargetReconstructionFinished {
        output_terms: usize,
    },
    ExactSemiNumericalStarted {
        rows: usize,
        columns: usize,
        variables: usize,
    },
    ExactSemiNumericalCoefficient {
        column: usize,
        probes: usize,
        primes: usize,
    },
    ExactSemiNumericalExactReplayStarted {
        support_recovery: bool,
    },
    ExactSemiNumericalExactReplayFinished {
        output_terms: Option<usize>,
    },
    ExactSemiNumericalFinished {
        output_terms: usize,
    },
    ExactRowStarted {
        row: usize,
        input_nonzeros: usize,
        reducer_rows: usize,
        reducer_nonzeros: usize,
    },
    ExactRowFinished {
        row: usize,
        accepted_pivot: bool,
        reducer_rows: usize,
        reducer_nonzeros: usize,
    },
    Canonicalization,
    GuardExtraction,
    ExceptionalGeometry,
    FiniteRetention,
    RuleTrialFinished {
        trial: usize,
        outcome: String,
        search_seeds: usize,
        search_rows: usize,
        independent_rows: usize,
        exact_trace_rows: usize,
        exact_trace_terms: usize,
        exact_lifts: usize,
        guard_branches: usize,
        geometry_calls: usize,
        // Internally tagged serde values cannot roundtrip u128. Saturate only
        // this diagnostic wire representation and mark any lost range.
        search_us: u64,
        exact_materialization_us: u64,
        guard_extraction_us: u64,
        geometry_us: u64,
        duration_micros_saturated: bool,
    },
    RuleFound {
        pending: usize,
    },
    Numerical {
        cases: usize,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct NativeFailure {
    pub(crate) ordinal: u64,
    pub(crate) sector_mask: String,
    pub(crate) message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CheckpointSemantics {
    pub(crate) resume_granularity: String,
    pub(crate) in_sector_resume: bool,
}

impl Default for CheckpointSemantics {
    fn default() -> Self {
        Self {
            resume_granularity: "completed-sector".into(),
            in_sector_resume: false,
        }
    }
}

impl NativeSnapshot {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        if self.schema != SCHEMA {
            return Err("unsupported native progress schema");
        }
        if self.pid == 0 || self.invocation_id.is_empty() || self.invocation_id.len() > 128 {
            return Err("invalid native invocation identity");
        }
        if !finite_seconds(self.elapsed_seconds)
            || self
                .last_event_elapsed_seconds
                .is_some_and(|at| !finite_seconds(at) || at > self.elapsed_seconds)
            || self.active_jobs.len() > MAX_ACTIVE_JOBS
        {
            return Err("invalid native progress time or detail count");
        }
        if self.family_closure_claim
            || self.checkpoint.in_sector_resume
            || self.checkpoint.resume_granularity != "completed-sector"
        {
            return Err("unsupported native progress semantics");
        }
        if !self.counts.overflow {
            let completed = self
                .counts
                .generated
                .checked_add(self.counts.reused)
                .ok_or("native progress count overflow")?;
            if self
                .counts
                .sectors_total
                .is_some_and(|total| completed > total)
                || self.counts.checkpointed_new > self.counts.generated
            {
                return Err("inconsistent native sector counters");
            }
        }
        for job in &self.active_jobs {
            if job.sector_mask.parse::<u64>().is_err()
                || job.phase.len() > 128
                || !finite_seconds(job.phase_elapsed_seconds)
                || !finite_seconds(job.last_event_elapsed_seconds)
                || job.last_event_elapsed_seconds > self.elapsed_seconds
            {
                return Err("invalid native active-job observation");
            }
        }
        if self.last_failure.as_ref().is_some_and(|failure| {
            failure.message.len() > 1027 || failure.sector_mask.parse::<u64>().is_err()
        }) {
            return Err("invalid native failure detail");
        }
        Ok(())
    }

    /// Best effort caller-owned publication. No mathematical outcome depends on it.
    pub(crate) fn publish(&self, path: &Path) -> Result<(), String> {
        self.validate().map_err(str::to_owned)?;
        let bytes = serde_json::to_vec(self).map_err(|error| error.to_string())?;
        if bytes.len() > MAX_SNAPSHOT_BYTES {
            return Err("native progress snapshot exceeds size limit".into());
        }
        crate::application::atomic_file::write_file_atomically(path, &bytes, true)
    }
}

pub(crate) fn finite_seconds(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}

pub(crate) fn process_start_ticks() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        parse_process_start_ticks(&std::fs::read_to_string("/proc/self/stat").ok()?)
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

fn parse_process_start_ticks(stat: &str) -> Option<u64> {
    // comm may itself contain spaces and parentheses. Field3 starts after its
    // final ')'; starttime is field22, hence offset19 from that suffix.
    stat.rsplit_once(')')?
        .1
        .split_whitespace()
        .nth(19)?
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_identity_handles_spaces_and_parentheses_in_comm() {
        let tail = (3..=22)
            .map(|n| if n == 3 { "S".into() } else { n.to_string() })
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(
            parse_process_start_ticks(&format!("123 (weird ) (name) {tail}")),
            Some(22)
        );
        assert_eq!(parse_process_start_ticks("123 (truncated) S 4"), None);
    }

    #[test]
    fn stage_detail_is_structured_and_preserves_unknown_frame_size() {
        let detail = StageDetail::ExactSemiNumericalExactReplayFinished { output_terms: None };
        let value = serde_json::to_value(detail).unwrap();
        assert_eq!(value["kind"], "exact_semi_numerical_exact_replay_finished");
        assert!(value["output_terms"].is_null());
    }
}
