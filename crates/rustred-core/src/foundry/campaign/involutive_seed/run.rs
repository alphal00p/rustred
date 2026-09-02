use std::fmt::Write;

use crate::algebra::IndexedCoefficientContext;
use crate::foundry::completion::involutive::{
    InvolutiveError, JanetBasisEpoch, OreOrderingAdapter,
    try_complete_janet_proposal_from_consequences, try_lift_completed_ordinary_sources,
};
use crate::foundry::completion::source_discovery::{
    RequestedDomainSupportBatchPreflight, RequestedDomainSupportBatchShape,
    RequestedDomainSupportLimits, RequestedDomainSupportProposal, RequestedSupportProposalOrigin,
    RequestedSupportProposalProvenanceInput, try_preflight_requested_domain_support_batch,
    try_union_requested_domain_support,
};
use crate::identity::{CompletedIbpSourceRows, IntegralShift};

use crate::foundry::completion::CompletionGeometryError;

use super::{
    InvolutiveSeedCensus, InvolutiveSeedComplementDiagnostics, InvolutiveSeedError,
    InvolutiveSeedLimits, InvolutiveSeedLocalizationCensus, InvolutiveSeedProgram,
    InvolutiveSeedReport, InvolutiveSeedStatus, InvolutiveSeedWorkCensus,
};

const PROPOSAL_SCHEMA_REVISION: u32 = 1;
const ALGORITHM_REVISION: u32 = 1;
const OBLIGATION_KEY_CAPACITY: usize = 192;
const OBLIGATION_KEY_PREFIX: &str = "rustred.involutive-basis-leader.v1;source=";
const OBLIGATION_BASIS_PREFIX: &str = ";basis=";
const OBLIGATION_ORDINAL_PREFIX: &str = ";ordinal=";

impl InvolutiveSeedProgram {
    /// Lift the exact completed ordinary module, compute one bounded Janet
    /// fixed point, and detach only requested geometry and physical monomial
    /// support from its final autoreduced basis.
    pub(crate) fn try_run(
        &self,
        completed: &CompletedIbpSourceRows,
        context: &IndexedCoefficientContext,
        limits: InvolutiveSeedLimits,
    ) -> Result<InvolutiveSeedReport, InvolutiveSeedError> {
        let lifted = try_lift_completed_ordinary_sources(
            completed,
            &self.ordering,
            context,
            limits.chart_lift,
        )?;
        let lifted_source_rows = lifted.len();
        #[cfg(test)]
        crate::foundry::completion::involutive::diagnostics::record_lifted_rows(lifted_source_rows);
        let consequences = lifted.try_into_consequences(
            completed,
            &self.ordering,
            context,
            limits.involutive(),
        )?;
        let completion = try_complete_janet_proposal_from_consequences(
            consequences.into_vec(),
            &self.ordering,
            context,
            limits.involutive(),
            limits.geometry,
        )?;
        let epoch = completion.epoch();
        let cardinality = epoch.try_uncovered_cardinality(limits.max_finite_complement_points)?;
        let mut pure_power_exponents = Vec::new();
        pure_power_exponents
            .try_reserve_exact(epoch.arity())
            .map_err(|_| InvolutiveSeedError::AllocationFailure {
                resource: "pure-power diagnostic axes",
                requested: epoch.arity(),
            })?;
        pure_power_exponents.extend(
            (0..epoch.arity()).map(|position| epoch.pure_power_coverage().exponent(position)),
        );

        let (proposals, support_preflight) = try_convert_basis_leaders(
            self.stable_scope_key(),
            epoch,
            &self.ordering,
            &self.source_chronology_digest,
            limits.requested_support,
        )?;
        let proposed_support_domains = proposals.len();
        let support = try_union_requested_domain_support(proposals, limits.requested_support)?;
        if support.census() != support_preflight.union_census() {
            return Err(InvolutiveSeedError::Invariant {
                detail: "requested-support batch preflight and canonical union census disagree",
            });
        }
        let completion_census = completion.census();
        let initial_reduction = completion_census.initial_reduction();
        let autoreduction = completion_census.autoreduction();
        let localization = completion.localization_witness().census();
        let work = completion.work_census();
        let support_census = support.census();
        Ok(InvolutiveSeedReport {
            status: InvolutiveSeedStatus::JanetQueueExhaustedProposalOnly,
            complement: InvolutiveSeedComplementDiagnostics {
                cardinality,
                pure_power_exponents: pure_power_exponents.into_boxed_slice(),
            },
            census: InvolutiveSeedCensus {
                lifted_source_rows,
                initial_retained_rows: initial_reduction.retained_rows(),
                initial_equal_head_eliminations: initial_reduction.equal_head_eliminations(),
                initial_zero_remainders: initial_reduction.zero_remainders(),
                initial_nonzero_remainders: initial_reduction.nonzero_remainders(),
                initial_cascading_collisions: initial_reduction.cascading_collisions(),
                initial_max_collision_chain: initial_reduction.max_collision_chain(),
                initial_max_head_class: initial_reduction.max_head_class(),
                basis_rows: epoch.elements().len(),
                basis_revision: epoch.epoch().revision(),
                prolongation_attempts: completion_census.attempted_prolongations(),
                zero_remainders: completion_census.zero_remainders(),
                nonzero_remainders: completion_census.inserted_remainders(),
                truncated_blind_priority_epochs: completion_census
                    .truncated_blind_priority_epochs(),
                autoreduction_passes: autoreduction.passes(),
                autoreduction_normal_form_steps: autoreduction.normal_form_steps(),
                autoreduction_dropped_rows: autoreduction.dropped_rows(),
                autoreduction_shared_rows: autoreduction.shared_rows(),
                autoreduction_materialized_rows: autoreduction.materialized_rows(),
                proposed_support_domains,
                unique_support_domains: support_census.unique_domains(),
                raw_support_entries: support_census.raw_support_entries(),
                unique_support_entries: support_census.unique_support_entries(),
            },
            localization: InvolutiveSeedLocalizationCensus {
                guards: localization.count(),
                terms: localization.terms(),
                exponent_cells: localization.exponent_cells(),
                retained_bytes: localization.retained_bytes(),
            },
            work: InvolutiveSeedWorkCensus {
                divisor_index_build_operations: work.divisor_index_build_operations(),
                divisor_index_query_operations: work.divisor_index_query_operations(),
                normal_form_steps: work.normal_form_steps(),
                normal_form_divisor_visits: work.normal_form_divisor_visits(),
                normal_form_trace_bytes: work.normal_form_trace_bytes(),
                autoreduction_passes: work.autoreduction_passes(),
                autoreduction_shared_rows: work.autoreduction_shared_rows(),
                autoreduction_materialized_rows: work.autoreduction_materialized_rows(),
                completion_iterations: work.completion_iterations(),
                exact_coefficient_operations: work.exact_coefficient_operations(),
            },
            support,
        })
    }
}

fn try_convert_basis_leaders(
    stable_scope_key: &str,
    epoch: &JanetBasisEpoch,
    ordering: &OreOrderingAdapter,
    source_chronology_digest: &[u8; blake3::OUT_LEN],
    limits: RequestedDomainSupportLimits,
) -> Result<
    (
        Vec<RequestedDomainSupportProposal>,
        RequestedDomainSupportBatchPreflight,
    ),
    InvolutiveSeedError,
> {
    let arity = epoch.arity();
    let ordering_key = ordering.policy().stable_id();
    let basis_revision = epoch.epoch().revision();
    // This is the first operation in conversion and is allocation-free. The
    // final Janet epoch has distinct canonical leaders, canonical unique row
    // monomials, and exactly one distinct provenance record per proposal, so
    // the trusted batch-shape seam computes the exact eventual union census.
    let support_preflight = try_preflight_requested_domain_support_batch(
        epoch.elements().iter().map(|element| {
            RequestedDomainSupportBatchShape::new(
                stable_scope_key.len(),
                arity,
                arity,
                element.consequence().row().terms().len(),
                ordering_key.as_str().len(),
                obligation_key_len(basis_revision, element.ordinal()),
            )
        }),
        limits,
    )?;
    require_leaders_inside_sector_chart_carrier(epoch, ordering)?;
    let mut symbolic_axes = Vec::new();
    symbolic_axes
        .try_reserve_exact(arity)
        .map_err(|_| InvolutiveSeedError::AllocationFailure {
            resource: "generic interior symbolic axes",
            requested: arity,
        })?;
    symbolic_axes.extend(0..arity);
    let mut proposals = Vec::new();
    proposals
        .try_reserve_exact(epoch.elements().len())
        .map_err(|_| InvolutiveSeedError::AllocationFailure {
            resource: "basis-leader support proposals",
            requested: epoch.elements().len(),
        })?;
    for element in epoch.elements() {
        let mut parent_support = Vec::new();
        parent_support
            .try_reserve_exact(element.consequence().row().terms().len())
            .map_err(|_| InvolutiveSeedError::AllocationFailure {
                resource: "basis-row physical parent support",
                requested: element.consequence().row().terms().len(),
            })?;
        for term in element.consequence().row().terms() {
            parent_support.push(IntegralShift::try_new(
                ordering.try_physical_translation(term.shift())?,
            )?);
        }
        parent_support.sort_unstable();
        parent_support.dedup();
        let obligation_key = try_obligation_key(
            source_chronology_digest,
            epoch.epoch().revision(),
            element.ordinal(),
        )?;
        proposals.push(RequestedDomainSupportProposal::try_new(
            stable_scope_key,
            ordering.sector(),
            element.leading_shift().values(),
            &symbolic_axes,
            &parent_support,
            RequestedSupportProposalProvenanceInput::new(
                PROPOSAL_SCHEMA_REVISION,
                ALGORITHM_REVISION,
                epoch.epoch().revision(),
                ordering_key.as_str(),
                &obligation_key,
                RequestedSupportProposalOrigin::InvolutiveBasisLeader,
            ),
            limits,
        )?);
    }
    Ok((proposals, support_preflight))
}

fn require_leaders_inside_sector_chart_carrier(
    epoch: &JanetBasisEpoch,
    ordering: &OreOrderingAdapter,
) -> Result<(), InvolutiveSeedError> {
    // An active chart coordinate `u` reconstructs the physical power `u + 1`,
    // so `i64::MAX` itself is the sole ForwardShift coordinate admitted by
    // Ore translation but not representable as an active requested point.
    let maximum_active_coordinate = i64::MAX as u64 - 1;
    for element in epoch.elements() {
        for (position, (&coordinate, &active)) in element
            .leading_shift()
            .values()
            .iter()
            .zip(ordering.sector().active_bits())
            .enumerate()
        {
            if active && coordinate > maximum_active_coordinate {
                return Err(InvolutiveError::Geometry(
                    CompletionGeometryError::CoordinateNotRepresentable {
                        position,
                        coordinate,
                        active,
                    },
                )
                .into());
            }
        }
    }
    Ok(())
}

fn try_obligation_key(
    source_chronology_digest: &[u8; blake3::OUT_LEN],
    basis_revision: u64,
    basis_ordinal: usize,
) -> Result<String, InvolutiveSeedError> {
    let exact_length = obligation_key_len(basis_revision, basis_ordinal);
    if exact_length > OBLIGATION_KEY_CAPACITY {
        return Err(InvolutiveSeedError::Invariant {
            detail: "basis-leader obligation key exceeded its fixed schema capacity",
        });
    }
    let mut key = String::new();
    key.try_reserve_exact(exact_length)
        .map_err(|_| InvolutiveSeedError::AllocationFailure {
            resource: "basis-leader obligation key bytes",
            requested: exact_length,
        })?;
    key.push_str(OBLIGATION_KEY_PREFIX);
    for byte in source_chronology_digest {
        write!(&mut key, "{byte:02x}")
            .expect("the fixed-capacity basis-leader key can encode one digest");
    }
    key.push_str(OBLIGATION_BASIS_PREFIX);
    write!(&mut key, "{basis_revision}")
        .expect("the fixed-capacity basis-leader key can encode one revision");
    key.push_str(OBLIGATION_ORDINAL_PREFIX);
    write!(&mut key, "{basis_ordinal}")
        .expect("the fixed-capacity basis-leader key can encode two machine integers");
    if key.len() != exact_length {
        return Err(InvolutiveSeedError::Invariant {
            detail: "basis-leader obligation key length preflight disagreed with rendering",
        });
    }
    Ok(key)
}

fn obligation_key_len(basis_revision: u64, basis_ordinal: usize) -> usize {
    OBLIGATION_KEY_PREFIX.len()
        + blake3::OUT_LEN * 2
        + OBLIGATION_BASIS_PREFIX.len()
        + decimal_digits_u64(basis_revision)
        + OBLIGATION_ORDINAL_PREFIX.len()
        + decimal_digits_usize(basis_ordinal)
}

fn decimal_digits_u64(mut value: u64) -> usize {
    let mut digits = 1usize;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

fn decimal_digits_usize(mut value: usize) -> usize {
    let mut digits = 1usize;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

#[cfg(test)]
pub(super) fn try_convert_basis_leaders_for_test(
    stable_scope_key: &str,
    epoch: &JanetBasisEpoch,
    ordering: &OreOrderingAdapter,
    source_chronology_digest: &[u8; blake3::OUT_LEN],
    limits: RequestedDomainSupportLimits,
) -> Result<Vec<RequestedDomainSupportProposal>, InvolutiveSeedError> {
    try_convert_basis_leaders(
        stable_scope_key,
        epoch,
        ordering,
        source_chronology_digest,
        limits,
    )
    .map(|(proposals, _)| proposals)
}
