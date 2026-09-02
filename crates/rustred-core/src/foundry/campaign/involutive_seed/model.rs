use crate::foundry::completion::involutive::{
    InvolutiveLimits, OrdinaryChartLiftLimits, OreOrderingAdapter,
};
use crate::foundry::completion::source_discovery::{
    RequestedDomainSupportLimits, RequestedDomainSupportUnion,
};
use crate::foundry::completion::{CompletionGeometryLimits, LatticeCardinality};
use crate::identity::CompletedIbpSourceRows;
use crate::sector::{Mask, OrderingPolicy};

use super::InvolutiveSeedError;

/// Complete resource envelope for one lift, bounded Janet calculation, and
/// authority-minimal support conversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct InvolutiveSeedLimits {
    pub(crate) chart_lift: OrdinaryChartLiftLimits,
    pub(crate) geometry: CompletionGeometryLimits,
    pub(crate) requested_support: RequestedDomainSupportLimits,
    /// Maximum finite complement cardinality retained as scalar telemetry.
    pub(crate) max_finite_complement_points: usize,
}

impl Default for InvolutiveSeedLimits {
    fn default() -> Self {
        Self {
            chart_lift: OrdinaryChartLiftLimits::default(),
            geometry: CompletionGeometryLimits::default(),
            requested_support: RequestedDomainSupportLimits::default(),
            max_finite_complement_points: 16_777_216,
        }
    }
}

impl InvolutiveSeedLimits {
    pub(crate) const fn involutive(self) -> InvolutiveLimits {
        self.chart_lift.involutive
    }
}

/// Stable-value request scope plus the opaque action/source-chronology seal.
///
/// Constructing this program freezes the sector, persisted order, coefficient
/// localization, and exact completed-source owner.  Running it against an
/// equivalent-looking but independently completed source transcript fails at
/// chart-lift ingress.
#[derive(Debug)]
pub(crate) struct InvolutiveSeedProgram {
    pub(super) stable_scope_key: String,
    pub(super) ordering: OreOrderingAdapter,
    /// Stable diagnostic commitment to family/context and exact row-ID order.
    /// Opaque `OreOrderingAdapter` ownership, not these bytes, remains the
    /// live source-chronology authority.
    pub(super) source_chronology_digest: [u8; blake3::OUT_LEN],
}

impl InvolutiveSeedProgram {
    pub(crate) fn try_new(
        stable_scope_key: &str,
        sector: Mask,
        policy: OrderingPolicy,
        completed: &CompletedIbpSourceRows,
        limits: InvolutiveLimits,
    ) -> Result<Self, InvolutiveSeedError> {
        if stable_scope_key.is_empty() {
            return Err(InvolutiveSeedError::EmptyStableScopeKey);
        }
        let mut retained_scope = String::new();
        retained_scope
            .try_reserve_exact(stable_scope_key.len())
            .map_err(|_| InvolutiveSeedError::AllocationFailure {
                resource: "stable scope key bytes",
                requested: stable_scope_key.len(),
            })?;
        retained_scope.push_str(stable_scope_key);
        let ordering =
            OreOrderingAdapter::try_new_for_completed(policy, sector, completed, limits)?;
        let source_chronology_digest = try_source_chronology_digest(completed)?;
        Ok(Self {
            stable_scope_key: retained_scope,
            ordering,
            source_chronology_digest,
        })
    }

    pub(crate) fn stable_scope_key(&self) -> &str {
        self.stable_scope_key.as_str()
    }

    pub(crate) fn sector(&self) -> &Mask {
        self.ordering.sector()
    }

    pub(crate) fn ordering_policy(&self) -> OrderingPolicy {
        self.ordering.policy()
    }
}

fn try_source_chronology_digest(
    completed: &CompletedIbpSourceRows,
) -> Result<[u8; blake3::OUT_LEN], InvolutiveSeedError> {
    let mut hasher = blake3::Hasher::new();
    hash_segment(&mut hasher, b"rustred.involutive-seed-source-chronology.v1")?;
    hash_segment(&mut hasher, completed.family_fingerprint().as_bytes())?;
    hash_segment(&mut hasher, completed.context_fingerprint().as_bytes())?;
    let source_rows = u64::try_from(completed.source_row_count()).map_err(|_| {
        InvolutiveSeedError::ResourceCountOverflow {
            resource: "source chronology rows",
        }
    })?;
    hasher.update(&source_rows.to_le_bytes());
    for ordinal in 0..completed.source_row_count() {
        let row = completed
            .source_row_id(ordinal)
            .ok_or(InvolutiveSeedError::Invariant {
                detail: "completed source row count and chronology disagree",
            })?;
        hash_segment(&mut hasher, row.stable_string().as_bytes())?;
    }
    Ok(*hasher.finalize().as_bytes())
}

fn hash_segment(hasher: &mut blake3::Hasher, value: &[u8]) -> Result<(), InvolutiveSeedError> {
    let length =
        u64::try_from(value.len()).map_err(|_| InvolutiveSeedError::ResourceCountOverflow {
            resource: "source chronology bytes",
        })?;
    hasher.update(&length.to_le_bytes());
    hasher.update(value);
    Ok(())
}

/// Exact monomial-complement evidence from the final autoreduced epoch.
///
/// This remains guard-blind proposal telemetry.  Even a finite complement and
/// complete pure-power coverage do not authenticate an executable owner.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct InvolutiveSeedComplementDiagnostics {
    pub(super) cardinality: LatticeCardinality,
    pub(super) pure_power_exponents: Box<[Option<u64>]>,
}

impl InvolutiveSeedComplementDiagnostics {
    pub(crate) const fn cardinality(&self) -> LatticeCardinality {
        self.cardinality
    }

    pub(crate) fn is_finite(&self) -> bool {
        matches!(self.cardinality, LatticeCardinality::Finite(_))
    }

    pub(crate) fn pure_power_exponents(&self) -> &[Option<u64>] {
        &self.pure_power_exponents
    }

    pub(crate) fn has_complete_pure_power_coverage(&self) -> bool {
        self.pure_power_exponents.iter().all(Option::is_some)
    }
}

/// Scalar, bounded accounting for one proposal-only run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct InvolutiveSeedCensus {
    pub(super) lifted_source_rows: usize,
    pub(super) initial_retained_rows: usize,
    pub(super) initial_equal_head_eliminations: usize,
    pub(super) initial_zero_remainders: usize,
    pub(super) initial_nonzero_remainders: usize,
    pub(super) initial_cascading_collisions: usize,
    pub(super) initial_max_collision_chain: usize,
    pub(super) initial_max_head_class: usize,
    pub(super) basis_rows: usize,
    pub(super) basis_revision: u64,
    pub(super) prolongation_attempts: usize,
    pub(super) zero_remainders: usize,
    pub(super) nonzero_remainders: usize,
    pub(super) truncated_blind_priority_epochs: usize,
    pub(super) autoreduction_passes: usize,
    pub(super) autoreduction_normal_form_steps: usize,
    pub(super) autoreduction_dropped_rows: usize,
    pub(super) autoreduction_shared_rows: usize,
    pub(super) autoreduction_materialized_rows: usize,
    pub(super) proposed_support_domains: usize,
    pub(super) unique_support_domains: usize,
    pub(super) raw_support_entries: usize,
    pub(super) unique_support_entries: usize,
}

/// Canonical guard-union size for retained rows and discarded zero proofs.
/// Guard polynomials themselves remain inside the Ore proposal and do not
/// cross into the requested-domain report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct InvolutiveSeedLocalizationCensus {
    pub(super) guards: usize,
    pub(super) terms: usize,
    pub(super) exponent_cells: usize,
    pub(super) retained_bytes: usize,
}

impl InvolutiveSeedLocalizationCensus {
    pub(crate) const fn guards(self) -> usize {
        self.guards
    }

    pub(crate) const fn terms(self) -> usize {
        self.terms
    }

    pub(crate) const fn exponent_cells(self) -> usize {
        self.exponent_cells
    }

    pub(crate) const fn retained_bytes(self) -> usize {
        self.retained_bytes
    }
}

/// Cumulative logical work across every nested normal form, autoreduction,
/// and prolongation in the single bounded proposal calculation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct InvolutiveSeedWorkCensus {
    pub(super) divisor_index_build_operations: usize,
    pub(super) divisor_index_query_operations: usize,
    pub(super) normal_form_steps: usize,
    pub(super) normal_form_divisor_visits: usize,
    pub(super) normal_form_trace_bytes: usize,
    pub(super) autoreduction_passes: usize,
    pub(super) autoreduction_shared_rows: usize,
    pub(super) autoreduction_materialized_rows: usize,
    pub(super) completion_iterations: usize,
    pub(super) exact_coefficient_operations: usize,
}

impl InvolutiveSeedWorkCensus {
    pub(crate) const fn divisor_index_build_operations(self) -> usize {
        self.divisor_index_build_operations
    }

    pub(crate) const fn divisor_index_query_operations(self) -> usize {
        self.divisor_index_query_operations
    }

    pub(crate) const fn normal_form_steps(self) -> usize {
        self.normal_form_steps
    }

    pub(crate) const fn normal_form_divisor_visits(self) -> usize {
        self.normal_form_divisor_visits
    }

    pub(crate) const fn normal_form_trace_bytes(self) -> usize {
        self.normal_form_trace_bytes
    }

    pub(crate) const fn autoreduction_passes(self) -> usize {
        self.autoreduction_passes
    }

    pub(crate) const fn autoreduction_shared_rows(self) -> usize {
        self.autoreduction_shared_rows
    }

    pub(crate) const fn autoreduction_materialized_rows(self) -> usize {
        self.autoreduction_materialized_rows
    }

    pub(crate) const fn completion_iterations(self) -> usize {
        self.completion_iterations
    }

    pub(crate) const fn exact_coefficient_operations(self) -> usize {
        self.exact_coefficient_operations
    }
}

impl InvolutiveSeedCensus {
    pub(crate) const fn lifted_source_rows(self) -> usize {
        self.lifted_source_rows
    }

    pub(crate) const fn initial_retained_rows(self) -> usize {
        self.initial_retained_rows
    }

    pub(crate) const fn initial_equal_head_eliminations(self) -> usize {
        self.initial_equal_head_eliminations
    }

    pub(crate) const fn initial_zero_remainders(self) -> usize {
        self.initial_zero_remainders
    }

    pub(crate) const fn initial_nonzero_remainders(self) -> usize {
        self.initial_nonzero_remainders
    }

    pub(crate) const fn initial_cascading_collisions(self) -> usize {
        self.initial_cascading_collisions
    }

    pub(crate) const fn initial_max_collision_chain(self) -> usize {
        self.initial_max_collision_chain
    }

    pub(crate) const fn initial_max_head_class(self) -> usize {
        self.initial_max_head_class
    }

    pub(crate) const fn basis_rows(self) -> usize {
        self.basis_rows
    }

    pub(crate) const fn basis_revision(self) -> u64 {
        self.basis_revision
    }

    pub(crate) const fn prolongation_attempts(self) -> usize {
        self.prolongation_attempts
    }

    pub(crate) const fn zero_remainders(self) -> usize {
        self.zero_remainders
    }

    pub(crate) const fn nonzero_remainders(self) -> usize {
        self.nonzero_remainders
    }

    pub(crate) const fn truncated_blind_priority_epochs(self) -> usize {
        self.truncated_blind_priority_epochs
    }

    pub(crate) const fn autoreduction_passes(self) -> usize {
        self.autoreduction_passes
    }

    pub(crate) const fn autoreduction_normal_form_steps(self) -> usize {
        self.autoreduction_normal_form_steps
    }

    pub(crate) const fn autoreduction_dropped_rows(self) -> usize {
        self.autoreduction_dropped_rows
    }

    pub(crate) const fn autoreduction_shared_rows(self) -> usize {
        self.autoreduction_shared_rows
    }

    pub(crate) const fn autoreduction_materialized_rows(self) -> usize {
        self.autoreduction_materialized_rows
    }

    pub(crate) const fn proposed_support_domains(self) -> usize {
        self.proposed_support_domains
    }

    pub(crate) const fn unique_support_domains(self) -> usize {
        self.unique_support_domains
    }

    pub(crate) const fn raw_support_entries(self) -> usize {
        self.raw_support_entries
    }

    pub(crate) const fn unique_support_entries(self) -> usize {
        self.unique_support_entries
    }
}

/// Explicit successful terminal state of this proposal lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InvolutiveSeedStatus {
    /// Every mandatory Janet obligation in the final frozen Ore epoch reduced
    /// to zero. This is not exact compiler closure and grants no authority.
    JanetQueueExhaustedProposalOnly,
}

/// Authority-minimal output of one completed involutive seed calculation.
///
/// No coefficient, Ore row, guard, source-provenance expression, owner,
/// closure flag, or artifact data is retained here.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct InvolutiveSeedReport {
    pub(super) status: InvolutiveSeedStatus,
    pub(super) complement: InvolutiveSeedComplementDiagnostics,
    pub(super) census: InvolutiveSeedCensus,
    pub(super) localization: InvolutiveSeedLocalizationCensus,
    pub(super) work: InvolutiveSeedWorkCensus,
    pub(super) support: RequestedDomainSupportUnion,
}

impl InvolutiveSeedReport {
    pub(crate) const fn status(&self) -> InvolutiveSeedStatus {
        self.status
    }

    pub(crate) const fn complement(&self) -> &InvolutiveSeedComplementDiagnostics {
        &self.complement
    }

    pub(crate) const fn census(&self) -> InvolutiveSeedCensus {
        self.census
    }

    pub(crate) const fn localization(&self) -> InvolutiveSeedLocalizationCensus {
        self.localization
    }

    pub(crate) const fn work(&self) -> InvolutiveSeedWorkCensus {
        self.work
    }

    pub(crate) const fn support(&self) -> &RequestedDomainSupportUnion {
        &self.support
    }
}
