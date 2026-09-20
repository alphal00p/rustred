//! Bounded presentation state, updated before any snapshots are coalesced.

use std::collections::BTreeMap;
use std::time::Instant;

use crate::{FamilyCloseGenerationStage, FamilyCloseProgress};

/// A presentation-only cap, not an execution-width limit. Unknown frame details
/// are preferable to retaining an unbounded number of caller-supplied job IDs.
pub(super) const MAX_TRACKED_JOBS: usize = 256;
const MAX_FAILURE_BYTES: usize = 1_024;

#[derive(Clone, Copy, Debug)]
pub(super) struct ExactFrame {
    pub(super) sequence: Option<usize>,
    pub(super) source_rows: usize,
    pub(super) integral_columns: usize,
    pub(super) target_column: usize,
    pub(super) input_terms: usize,
    pub(super) coefficient_variables: usize,
    pub(super) active_variables: usize,
}

#[derive(Clone, Debug)]
pub(super) struct ExactJob {
    pub(super) sequence: Option<usize>,
    pub(super) case: Option<usize>,
    pub(super) frame: Option<ExactFrame>,
}

impl Default for ExactJob {
    fn default() -> Self {
        Self {
            sequence: Some(0),
            case: Some(0),
            frame: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Counts {
    pub(super) total: Option<usize>,
    pub(super) generated: usize,
    pub(super) reused: usize,
    pub(super) checkpointed: usize,
    pub(super) rules: usize,
    pub(super) residuals: usize,
    pub(super) failures: usize,
    /// Observed RuleFound callbacks, not the sum of completed-sector rules.
    pub(super) observed_rules: usize,
    pub(super) overflow: bool,
}

impl Counts {
    fn add(overflow: &mut bool, slot: &mut usize, value: usize) {
        match slot.checked_add(value) {
            Some(sum) => *slot = sum,
            None => {
                *overflow = true;
                *slot = usize::MAX;
            }
        }
    }

    /// Only sector generation, never overall closure or per-case completion.
    pub(super) fn fraction(&self) -> Option<(usize, usize)> {
        let total = self.total?;
        let complete = self.generated.checked_add(self.reused)?;
        (!self.overflow && complete <= total).then_some((complete, total))
    }
}

#[derive(Clone, Debug)]
pub(super) struct Snapshot {
    pub(super) event: FamilyCloseProgress,
    pub(super) at: Instant,
    pub(super) frame: Option<ExactFrame>,
    pub(super) case: Option<usize>,
    pub(super) counts: Counts,
    pub(super) last_failure: Option<String>,
    pub(super) phase: &'static str,
    pub(super) phase_since: Instant,
}

#[derive(Default)]
pub(super) struct Tracker {
    pub(super) jobs: BTreeMap<(usize, u64), ExactJob>,
    pub(super) counts: Counts,
    last_failure: Option<String>,
    phase: Option<(Option<(usize, u64)>, &'static str, Instant)>,
}

impl Tracker {
    fn job(&mut self, key: (usize, u64)) -> Option<&mut ExactJob> {
        if !self.jobs.contains_key(&key) && self.jobs.len() >= MAX_TRACKED_JOBS {
            return None;
        }
        Some(self.jobs.entry(key).or_default())
    }

    pub(super) fn observe(&mut self, mut event: FamilyCloseProgress, at: Instant) -> Snapshot {
        use FamilyCloseGenerationStage::*;
        if let FamilyCloseProgress::FailedSector { message, .. } = &mut event {
            // Bound retained error text without splitting UTF-8. Escaping is
            // deferred to the presenter; no formatting happens in this callback.
            let mut end = message.len().min(MAX_FAILURE_BYTES);
            while !message.is_char_boundary(end) {
                end -= 1;
            }
            let mut bounded = message[..end].to_owned();
            if end != message.len() {
                bounded.push_str("...");
            }
            // `truncate` alone would retain an arbitrarily large allocation,
            // including a short message with a caller-reserved large capacity.
            *message = bounded;
            self.last_failure = Some(message.clone());
        }
        let phase = phase(&event);
        let phase_job = match &event {
            FamilyCloseProgress::Generating {
                ordinal, sector, ..
            } => Some((*ordinal, *sector)),
            _ => None,
        };
        let phase_since = match self.phase {
            Some((job, previous, since)) if previous == phase && job == phase_job => since,
            _ => at,
        };
        self.phase = Some((phase_job, phase, phase_since));
        let mut current = None;
        match &event {
            FamilyCloseProgress::Prepared { sectors, .. } => self.counts.total = Some(*sectors),
            FamilyCloseProgress::CheckpointPrepared { reused_sectors, .. } => {
                self.counts.reused = *reused_sectors;
            }
            FamilyCloseProgress::CheckpointedSector { .. } => {
                Counts::add(&mut self.counts.overflow, &mut self.counts.checkpointed, 1);
            }
            FamilyCloseProgress::GeneratedSector {
                ordinal,
                sector,
                rules,
                finite_residuals,
                ..
            } => {
                self.jobs.remove(&(*ordinal, *sector));
                Counts::add(&mut self.counts.overflow, &mut self.counts.generated, 1);
                Counts::add(&mut self.counts.overflow, &mut self.counts.rules, *rules);
                Counts::add(
                    &mut self.counts.overflow,
                    &mut self.counts.residuals,
                    *finite_residuals,
                );
            }
            FamilyCloseProgress::FailedSector {
                ordinal, sector, ..
            } => {
                self.jobs.remove(&(*ordinal, *sector));
                Counts::add(&mut self.counts.overflow, &mut self.counts.failures, 1);
            }
            FamilyCloseProgress::Generating {
                ordinal,
                sector,
                stage,
                ..
            } => {
                let key = (*ordinal, *sector);
                if matches!(stage, RuleFound { .. }) {
                    Counts::add(
                        &mut self.counts.overflow,
                        &mut self.counts.observed_rules,
                        1,
                    );
                }
                match *stage {
                    ExactFramePrepared {
                        source_rows,
                        integral_columns,
                        target_column,
                        input_terms,
                        coefficient_variables,
                        active_variables,
                    } => {
                        if let Some(job) = self.job(key) {
                            job.sequence = job.sequence.and_then(|n| n.checked_add(1));
                            job.frame = Some(ExactFrame {
                                sequence: job.sequence,
                                source_rows,
                                integral_columns,
                                target_column,
                                input_terms,
                                coefficient_variables,
                                active_variables,
                            });
                        }
                    }
                    Case { .. } => {
                        if let Some(job) = self.job(key) {
                            job.case = job.case.and_then(|n| n.checked_add(1));
                            job.frame = None;
                        }
                    }
                    ExactMaterialization
                    | Discovery { .. }
                    | Canonicalization
                    | GuardExtraction
                    | ExceptionalGeometry
                    | RuleFound { .. }
                    | Numerical { .. } => {
                        if let Some(job) = self.jobs.get_mut(&key) {
                            job.frame = None;
                        }
                    }
                    ExactDenseFractionFreeStarted { .. }
                    | ExactDenseFractionFreeFinished { .. }
                    | ExactTargetBlockStarted { .. }
                    | ExactTargetWeightsStarted { .. }
                    | ExactTargetWeightsFinished { .. }
                    | ExactTargetReconstructionStarted { .. }
                    | ExactTargetReconstructionFinished { .. }
                    | ExactSemiNumericalStarted { .. }
                    | ExactSemiNumericalCoefficient { .. }
                    | ExactSemiNumericalExactReplayStarted { .. }
                    | ExactSemiNumericalExactReplayFinished { .. }
                    | ExactSemiNumericalFinished { .. }
                    | ExactRowStarted { .. }
                    | ExactRowFinished { .. } => {}
                }
                current = self.jobs.get(&key);
            }
            _ => {}
        }
        Snapshot {
            frame: current.and_then(|job| job.frame),
            case: current.and_then(|job| job.case).filter(|n| *n != 0),
            event,
            at,
            counts: self.counts,
            last_failure: self.last_failure.clone(),
            phase,
            phase_since,
        }
    }
}

fn phase(event: &FamilyCloseProgress) -> &'static str {
    use FamilyCloseGenerationStage as G;
    use FamilyCloseProgress as P;
    match event {
        P::Preparing { .. } => "preparing",
        P::Prepared { .. } => "prepared",
        P::CheckpointPrepared { .. } => "checkpoint resume",
        P::CheckpointedSector { .. } => "checkpoint saved",
        P::Generating { stage, .. } => match stage {
            G::Case { .. } => "case selection",
            G::Discovery { .. } => "modular discovery",
            G::Canonicalization => "canonicalization",
            G::ExactMaterialization | G::ExactFramePrepared { .. } => "exact frame",
            G::ExactRowStarted { .. } | G::ExactRowFinished { .. } => "exact sparse elimination",
            G::ExactDenseFractionFreeStarted { .. } | G::ExactDenseFractionFreeFinished { .. } => {
                "exact fraction-free"
            }
            G::ExactTargetBlockStarted { .. } => "target block",
            G::ExactTargetWeightsStarted { .. } | G::ExactTargetWeightsFinished { .. } => {
                "source weights"
            }
            G::ExactTargetReconstructionStarted { .. }
            | G::ExactTargetReconstructionFinished { .. } => "target reconstruction",
            G::ExactSemiNumericalStarted { .. } | G::ExactSemiNumericalCoefficient { .. } => {
                "semi-numerical reconstruction"
            }
            G::ExactSemiNumericalExactReplayStarted { .. }
            | G::ExactSemiNumericalExactReplayFinished { .. } => "exact reconstruction replay",
            G::ExactSemiNumericalFinished { .. } => "reconstruction finished",
            G::GuardExtraction => "guard extraction",
            G::ExceptionalGeometry => "exceptional geometry",
            G::RuleFound { .. } => "rule found",
            G::Numerical { .. } => "numerical cases",
        },
        P::GeneratedSector { .. } => "sector generated",
        P::FailedSector { .. } => "sector failed",
        P::CheckingSector { .. } | P::CheckingRule { .. } | P::CheckedSector { .. } => {
            "source replay"
        }
        P::LoweringRule { .. } | P::LoweredSector { .. } => "owner lowering",
        P::SuccessorGeometry { .. } => "successor geometry",
        P::Installing { .. } | P::Installed { .. } => "installing",
        P::Encoding { .. } | P::Encoded { .. } => "encoding output",
    }
}
