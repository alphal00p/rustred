use crate::algebra::IndexedCoefficientContext;

use super::super::{
    CertifiedSupportBatch, ExactMaterializationBatch, ExactMaterializationBudget,
    ExactMaterializationCensus, ModularCoefficientDag, ModularGuideError, ModularProbeCensus,
    RejectedProbeReport, try_materialize_exact_batch,
};
use super::{ExactLazyError, ExactLazyOwner, ExactLazySupportLimits};

const CLASSIFICATION_ATTEMPTS: &str = "exact-lazy support-classification attempts";
const CLASSIFICATION_ROOTS: &str = "exact-lazy support-classification roots";
const SCHEDULED_PROBES: &str = "exact-lazy scheduled support probes";
const SUCCESSFUL_PROBES: &str = "exact-lazy successful support probes";
const REJECTED_PROBES: &str = "exact-lazy rejected support probes";
const PROBE_QUERIES: &str = "exact-lazy cumulative probe queries";
const PROBE_DELTA_COMPOSITIONS: &str = "exact-lazy cumulative probe delta compositions";
const PROBE_DELTA_COORDINATE_OPERATIONS: &str =
    "exact-lazy cumulative probe delta-coordinate operations";
const PROBE_EVALUATION_STEPS: &str = "exact-lazy cumulative probe evaluation steps";
const PROBE_FRAME_PUSHES: &str = "exact-lazy cumulative probe frame pushes";
const PROBE_CACHE_HITS: &str = "exact-lazy cumulative probe cache hits";
const PROBE_EXACT_LEAF_EVALUATIONS: &str = "exact-lazy cumulative probe exact-leaf evaluations";
const PROBE_EXACT_LEAF_TERMS: &str = "exact-lazy cumulative probe exact-leaf terms";
const PROBE_EXACT_LEAF_EXPONENT_CELLS: &str =
    "exact-lazy cumulative probe exact-leaf exponent cells";
const FALLBACK_BATCHES: &str = "exact-lazy exact-support fallback batches";
const FALLBACK_ROOTS: &str = "exact-lazy exact-support fallback roots";

/// Monotone work evidence for one caller-held support-classification budget.
/// Failed probes and failed exact batches remain visible here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ExactLazySupportCensus {
    classification_attempts: usize,
    classification_roots: usize,
    scheduled_probes: usize,
    successful_probes: usize,
    rejected_probes: usize,
    probe: ModularProbeCensus,
    exact_fallback_batches: usize,
    exact_fallback_roots: usize,
}

impl ExactLazySupportCensus {
    pub(super) const fn classification_attempts(self) -> usize {
        self.classification_attempts
    }

    pub(super) const fn classification_roots(self) -> usize {
        self.classification_roots
    }

    pub(super) const fn scheduled_probes(self) -> usize {
        self.scheduled_probes
    }

    pub(super) const fn successful_probes(self) -> usize {
        self.successful_probes
    }

    pub(super) const fn rejected_probes(self) -> usize {
        self.rejected_probes
    }

    pub(super) const fn probe(self) -> ModularProbeCensus {
        self.probe
    }

    pub(super) const fn exact_fallback_batches(self) -> usize {
        self.exact_fallback_batches
    }

    pub(super) const fn exact_fallback_roots(self) -> usize {
        self.exact_fallback_roots
    }
}

/// Only accounted outcomes cross from the modular theorem seam into ELC1.
pub(super) enum AccountedProbeOutcome {
    Complete(CertifiedSupportBatch),
    Rejected(ModularGuideError),
}

/// Caller-owned cumulative accounting gate for support classification.
///
/// It is bound to the exact-lazy owner (and therefore its immutable
/// `ExactLazyLimits`) and owns the only exact-fallback budget used by this
/// classification campaign. There is no method that unwraps an unaccounted
/// rejected report or successful batch.
#[derive(Debug)]
pub(super) struct ExactLazySupportBudget {
    owner: ExactLazyOwner,
    limits: ExactLazySupportLimits,
    census: ExactLazySupportCensus,
    exact_fallback: ExactMaterializationBudget,
}

impl ExactLazySupportBudget {
    pub(super) fn new(owner: &ExactLazyOwner) -> Self {
        let limits = owner.limits().support;
        Self {
            owner: owner.clone(),
            limits,
            census: ExactLazySupportCensus::default(),
            exact_fallback: ExactMaterializationBudget::new(limits.exact_fallback),
        }
    }

    pub(super) const fn census(&self) -> ExactLazySupportCensus {
        self.census
    }

    pub(super) const fn exact_fallback_census(&self) -> ExactMaterializationCensus {
        self.exact_fallback.census()
    }

    pub(super) const fn exact_fallback_attempts(&self) -> usize {
        self.exact_fallback.attempts()
    }

    pub(super) fn require_owner(&self, owner: &ExactLazyOwner) -> Result<(), ExactLazyError> {
        if self.owner.belongs_to(owner) && self.limits == owner.limits().support {
            Ok(())
        } else {
            Err(ExactLazyError::WrongLimitsContract)
        }
    }

    pub(super) fn try_start_classification(
        &mut self,
        owner: &ExactLazyOwner,
        roots: usize,
    ) -> Result<(), ExactLazyError> {
        self.require_owner(owner)?;
        let attempts = add(
            CLASSIFICATION_ATTEMPTS,
            self.census.classification_attempts,
            1,
        )?;
        let total_roots = add(
            CLASSIFICATION_ROOTS,
            self.census.classification_roots,
            roots,
        )?;
        // Charge first. If a cap is crossed, reuse of this budget remains
        // observably exhausted instead of erasing the attempted request.
        self.census.classification_attempts = attempts;
        self.census.classification_roots = total_roots;
        cap(
            CLASSIFICATION_ATTEMPTS,
            attempts,
            self.limits.max_classification_attempts,
        )?;
        cap(
            CLASSIFICATION_ROOTS,
            roots,
            self.limits.max_roots_per_classification,
        )?;
        cap(
            CLASSIFICATION_ROOTS,
            total_roots,
            self.limits.max_total_classification_roots,
        )
    }

    /// Mandatory accounting gate for a consumed probe. Both branches are
    /// charged in full before either the batch or its typed error can escape.
    pub(super) fn try_account_probe(
        &mut self,
        owner: &ExactLazyOwner,
        result: Result<CertifiedSupportBatch, RejectedProbeReport>,
    ) -> Result<AccountedProbeOutcome, ExactLazyError> {
        self.require_owner(owner)?;
        let (census, successful) = match &result {
            Ok(batch) => (batch.census(), true),
            Err(report) => (report.census(), false),
        };
        self.charge_probe(census, successful)?;
        Ok(match result {
            Ok(batch) => AccountedProbeOutcome::Complete(batch),
            Err(report) => AccountedProbeOutcome::Rejected(report.into_error()),
        })
    }

    pub(super) fn try_materialize_fallback(
        &mut self,
        owner: &ExactLazyOwner,
        dag: &ModularCoefficientDag,
        context: &IndexedCoefficientContext,
        roots: &[super::super::CoeffRef],
    ) -> Result<ExactMaterializationBatch, ExactLazyError> {
        self.require_owner(owner)?;
        if !owner.owns_dag(dag.owner()) {
            return Err(ExactLazyError::WrongSessionOwner);
        }
        let batches = add(FALLBACK_BATCHES, self.census.exact_fallback_batches, 1)?;
        let total_roots = add(
            FALLBACK_ROOTS,
            self.census.exact_fallback_roots,
            roots.len(),
        )?;
        self.census.exact_fallback_batches = batches;
        self.census.exact_fallback_roots = total_roots;
        cap(
            FALLBACK_BATCHES,
            batches,
            self.limits.max_exact_fallback_batches,
        )?;
        cap(
            FALLBACK_ROOTS,
            roots.len(),
            self.limits.max_exact_fallback_roots_per_batch,
        )?;
        cap(
            FALLBACK_ROOTS,
            total_roots,
            self.limits.max_total_exact_fallback_roots,
        )?;
        try_materialize_exact_batch(dag, context, roots, &mut self.exact_fallback)
            .map_err(ExactLazyError::from)
    }

    fn charge_probe(
        &mut self,
        census: ModularProbeCensus,
        successful: bool,
    ) -> Result<(), ExactLazyError> {
        let scheduled = add(SCHEDULED_PROBES, self.census.scheduled_probes, 1)?;
        let successes = add(
            SUCCESSFUL_PROBES,
            self.census.successful_probes,
            usize::from(successful),
        )?;
        let rejections = add(
            REJECTED_PROBES,
            self.census.rejected_probes,
            usize::from(!successful),
        )?;
        let probe = add_probe_census(self.census.probe, census)?;
        self.census.scheduled_probes = scheduled;
        self.census.successful_probes = successes;
        self.census.rejected_probes = rejections;
        self.census.probe = probe;

        cap(
            SCHEDULED_PROBES,
            scheduled,
            self.limits.max_total_scheduled_probes,
        )?;
        cap(
            SUCCESSFUL_PROBES,
            successes,
            self.limits.max_total_successful_probes,
        )?;
        cap(
            REJECTED_PROBES,
            rejections,
            self.limits.max_total_rejected_probes,
        )?;
        cap(
            PROBE_QUERIES,
            probe.queries,
            self.limits.max_total_probe_queries,
        )?;
        cap(
            PROBE_DELTA_COMPOSITIONS,
            probe.delta_compositions,
            self.limits.max_total_probe_delta_compositions,
        )?;
        cap(
            PROBE_DELTA_COORDINATE_OPERATIONS,
            probe.delta_coordinate_operations,
            self.limits.max_total_probe_delta_coordinate_operations,
        )?;
        cap(
            PROBE_EVALUATION_STEPS,
            probe.evaluation_steps,
            self.limits.max_total_probe_evaluation_steps,
        )?;
        cap(
            PROBE_FRAME_PUSHES,
            probe.evaluation_frame_pushes,
            self.limits.max_total_probe_evaluation_frame_pushes,
        )?;
        cap(
            "exact-lazy peak probe live evaluation frames",
            probe.peak_live_evaluation_frames,
            self.limits.max_peak_probe_live_evaluation_frames,
        )?;
        cap(
            "exact-lazy peak probe live evaluation values",
            probe.peak_live_evaluation_values,
            self.limits.max_peak_probe_live_evaluation_values,
        )?;
        cap(
            PROBE_CACHE_HITS,
            probe.cache_hits,
            self.limits.max_total_probe_cache_hits,
        )?;
        cap(
            PROBE_EXACT_LEAF_EVALUATIONS,
            probe.exact_leaf_evaluations,
            self.limits.max_total_probe_exact_leaf_evaluations,
        )?;
        cap(
            PROBE_EXACT_LEAF_TERMS,
            probe.exact_leaf_terms_evaluated,
            self.limits.max_total_probe_exact_leaf_terms_evaluated,
        )?;
        cap(
            PROBE_EXACT_LEAF_EXPONENT_CELLS,
            probe.exact_leaf_exponent_cells_evaluated,
            self.limits
                .max_total_probe_exact_leaf_exponent_cells_evaluated,
        )
    }
}

fn add_probe_census(
    left: ModularProbeCensus,
    right: ModularProbeCensus,
) -> Result<ModularProbeCensus, ExactLazyError> {
    Ok(ModularProbeCensus {
        queries: add(PROBE_QUERIES, left.queries, right.queries)?,
        delta_compositions: add(
            PROBE_DELTA_COMPOSITIONS,
            left.delta_compositions,
            right.delta_compositions,
        )?,
        delta_coordinate_operations: add(
            PROBE_DELTA_COORDINATE_OPERATIONS,
            left.delta_coordinate_operations,
            right.delta_coordinate_operations,
        )?,
        evaluation_steps: add(
            PROBE_EVALUATION_STEPS,
            left.evaluation_steps,
            right.evaluation_steps,
        )?,
        evaluation_frame_pushes: add(
            PROBE_FRAME_PUSHES,
            left.evaluation_frame_pushes,
            right.evaluation_frame_pushes,
        )?,
        peak_live_evaluation_frames: left
            .peak_live_evaluation_frames
            .max(right.peak_live_evaluation_frames),
        peak_live_evaluation_values: left
            .peak_live_evaluation_values
            .max(right.peak_live_evaluation_values),
        cache_hits: add(PROBE_CACHE_HITS, left.cache_hits, right.cache_hits)?,
        exact_leaf_evaluations: add(
            PROBE_EXACT_LEAF_EVALUATIONS,
            left.exact_leaf_evaluations,
            right.exact_leaf_evaluations,
        )?,
        exact_leaf_terms_evaluated: add(
            PROBE_EXACT_LEAF_TERMS,
            left.exact_leaf_terms_evaluated,
            right.exact_leaf_terms_evaluated,
        )?,
        exact_leaf_exponent_cells_evaluated: add(
            PROBE_EXACT_LEAF_EXPONENT_CELLS,
            left.exact_leaf_exponent_cells_evaluated,
            right.exact_leaf_exponent_cells_evaluated,
        )?,
    })
}

fn add(resource: &'static str, left: usize, right: usize) -> Result<usize, ExactLazyError> {
    left.checked_add(right)
        .ok_or(ExactLazyError::ResourceCountOverflow { resource })
}

fn cap(resource: &'static str, value: usize, limit: usize) -> Result<(), ExactLazyError> {
    if value > limit {
        Err(ExactLazyError::ResourceLimit {
            resource,
            requested: value,
            limit,
        })
    } else {
        Ok(())
    }
}
