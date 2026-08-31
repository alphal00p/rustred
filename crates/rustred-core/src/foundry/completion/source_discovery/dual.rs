use std::sync::Arc;

use symbolica::domains::Ring;
use symbolica::domains::finite_field::FiniteFieldElement;

use crate::foundry::completion::frame::PhysicalFramePlanIdentity;
use crate::foundry::completion::frame::modular::{
    ModularRightObstruction, ModularRightObstructionIdentity, ModularSampleFingerprint,
    ModularTargetQuery,
};
use crate::foundry::completion::stratum::{DecoratedStratumId, ImmutableOwnerSnapshotId};
use crate::identity::{IntegralShift, TranslatedSourceRequest};
use crate::sector::OrderingPolicy;

use super::model::IncidentNominationOrigin;
use super::nominate::{check_limit, checked_add, try_vec};
use super::{
    FreshTaskEpoch, FreshTaskQuery, IncidentTranslationNominations,
    NonzeroIncidentTranslationResiduals, OrdinarySourceIncidenceIndex, SourceDiscoveryError,
    SourceDiscoveryLimits,
};

mod admission;
mod error;

pub(crate) use error::SampledDeclaredModuleDualError;

#[cfg(test)]
mod tests;

const RESIDUAL_CANDIDATES: &str = "source-discovery residual candidates";
const RESIDUAL_SOURCE_TERMS: &str = "source-discovery residual exact-source terms";
const RESIDUAL_SUPPORT_COORDINATES: &str =
    "source-discovery residual obstruction-support coordinate cells";
const DUAL_REQUESTS: &str = "sampled-dual retained source requests";
const DUAL_REQUEST_COORDINATES: &str = "sampled-dual retained request coordinate cells";
const DUAL_OBSTRUCTION_ENTRIES: &str = "sampled-dual raw obstruction entries";
const DUAL_OBSTRUCTION_COORDINATES: &str = "sampled-dual raw obstruction coordinate cells";
const DUAL_SAMPLE_COORDINATES: &str = "sampled-dual retained sample coordinates";
const DUAL_DIAGNOSTIC_ORDINALS: &str = "sampled-dual inspected rank diagnostic ordinals";

/// One nonzero checked obstruction coefficient retained by raw integral key.
///
/// No physical or query-local column ordinal survives in this payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SampledDeclaredModuleDualObstructionEntry {
    shift: IntegralShift,
    coefficient: FiniteFieldElement<u64>,
    target: bool,
}

impl SampledDeclaredModuleDualObstructionEntry {
    pub(crate) const fn shift(&self) -> &IntegralShift {
        &self.shift
    }

    pub(crate) const fn coefficient(&self) -> &FiniteFieldElement<u64> {
        &self.coefficient
    }

    pub(crate) const fn is_target(&self) -> bool {
        self.target
    }
}

/// Exact finite counters retained from sampled-dual admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SampledDeclaredModuleDualCensus {
    declared_source_rows: usize,
    final_request_count: usize,
    raw_incidence_visits: usize,
    structurally_incident_rows: usize,
    evaluated_unseen_rows: usize,
    already_materialized_incident_rows: usize,
    evaluated_source_terms: usize,
    paired_source_terms: usize,
}

impl SampledDeclaredModuleDualCensus {
    pub(crate) const fn declared_source_rows(self) -> usize {
        self.declared_source_rows
    }

    pub(crate) const fn final_request_count(self) -> usize {
        self.final_request_count
    }

    pub(crate) const fn raw_incidence_visits(self) -> usize {
        self.raw_incidence_visits
    }

    pub(crate) const fn structurally_incident_rows(self) -> usize {
        self.structurally_incident_rows
    }

    pub(crate) const fn evaluated_unseen_rows(self) -> usize {
        self.evaluated_unseen_rows
    }

    pub(crate) const fn already_materialized_incident_rows(self) -> usize {
        self.already_materialized_incident_rows
    }

    pub(crate) const fn evaluated_source_terms(self) -> usize {
        self.evaluated_source_terms
    }

    pub(crate) const fn paired_source_terms(self) -> usize {
        self.paired_source_terms
    }
}

/// Plan-independent scalar summary of the two checked modular rank probes.
///
/// Physical column and source-row ordinals are validated while the fresh plan
/// is alive, then discarded. Retaining them after the plan is dropped would
/// make otherwise harmless telemetry look applicable to a rebuilt plan with
/// unrelated ordinals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SampledDeclaredModuleDualRankCensus {
    forbidden_columns: usize,
    forbidden_rank: usize,
    augmented_rank: usize,
    forbidden_pivot_columns: usize,
    augmented_pivot_columns: usize,
    forbidden_independent_source_rows: usize,
    augmented_independent_source_rows: usize,
    forbidden_input_nonzeros: usize,
    augmented_input_nonzeros: usize,
    forbidden_lower_pattern_nonzeros: usize,
    augmented_lower_pattern_nonzeros: usize,
    forbidden_upper_nonzeros: usize,
    augmented_upper_nonzeros: usize,
    forbidden_total_fill_nonzeros: usize,
    augmented_total_fill_nonzeros: usize,
}

#[allow(dead_code)] // Full counters feed the next outer-driver telemetry boundary.
impl SampledDeclaredModuleDualRankCensus {
    pub(crate) const fn forbidden_columns(self) -> usize {
        self.forbidden_columns
    }

    pub(crate) const fn forbidden_rank(self) -> usize {
        self.forbidden_rank
    }

    pub(crate) const fn augmented_rank(self) -> usize {
        self.augmented_rank
    }

    pub(crate) const fn forbidden_pivot_columns(self) -> usize {
        self.forbidden_pivot_columns
    }

    pub(crate) const fn augmented_pivot_columns(self) -> usize {
        self.augmented_pivot_columns
    }

    pub(crate) const fn forbidden_independent_source_rows(self) -> usize {
        self.forbidden_independent_source_rows
    }

    pub(crate) const fn augmented_independent_source_rows(self) -> usize {
        self.augmented_independent_source_rows
    }

    pub(crate) const fn forbidden_input_nonzeros(self) -> usize {
        self.forbidden_input_nonzeros
    }

    pub(crate) const fn augmented_input_nonzeros(self) -> usize {
        self.augmented_input_nonzeros
    }

    pub(crate) const fn forbidden_lower_pattern_nonzeros(self) -> usize {
        self.forbidden_lower_pattern_nonzeros
    }

    pub(crate) const fn augmented_lower_pattern_nonzeros(self) -> usize {
        self.augmented_lower_pattern_nonzeros
    }

    pub(crate) const fn forbidden_upper_nonzeros(self) -> usize {
        self.forbidden_upper_nonzeros
    }

    pub(crate) const fn augmented_upper_nonzeros(self) -> usize {
        self.augmented_upper_nonzeros
    }

    pub(crate) const fn forbidden_total_fill_nonzeros(self) -> usize {
        self.forbidden_total_fill_nonzeros
    }

    pub(crate) const fn augmented_total_fill_nonzeros(self) -> usize {
        self.augmented_total_fill_nonzeros
    }
}

/// Sealed fixed-sample evidence that one checked obstruction has no unseen
/// cutting row in the declared ordinary translated-source module.
///
/// This value is scoped to one exact fresh plan, modular sample, target
/// partition, decorated stratum, ordering policy, immutable-owner snapshot,
/// and ordinary-source incidence index.  It is modular discovery evidence
/// only.  It has no conversion to a rule, owner, terminal, closing artifact,
/// or exact no-relation claim.
#[derive(Debug)]
pub(crate) struct SampledDeclaredModuleDual {
    // Admission identities remain opaque and have no authority conversion.
    _plan_identity: PhysicalFramePlanIdentity,
    sample: Arc<ModularSampleFingerprint>,
    _obstruction_identity: ModularRightObstructionIdentity,
    _incidence_identity: Arc<()>,
    target_shift: IntegralShift,
    stratum_id: DecoratedStratumId,
    ordering: OrderingPolicy,
    snapshot_id: ImmutableOwnerSnapshotId,
    final_requests: Box<[TranslatedSourceRequest]>,
    obstruction: Box<[SampledDeclaredModuleDualObstructionEntry]>,
    rank_census: SampledDeclaredModuleDualRankCensus,
    census: SampledDeclaredModuleDualCensus,
}
