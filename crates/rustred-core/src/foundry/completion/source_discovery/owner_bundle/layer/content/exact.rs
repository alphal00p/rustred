use crate::foundry::completion::frame::PhysicalFramePlan;
use crate::foundry::completion::frame::exact::{ExactCircuitGuardOrigin, ExactTargetCircuit};
use crate::foundry::completion::stratum::StratumRegistryError;

use super::algebra::{
    append_coefficient, append_identity_condition_source, append_integral_shift, append_mask,
    append_monotone_descent, append_polynomial, append_proper_subsector_owner,
    append_translated_source,
};
use super::encoder::BoundedContentHasher;

/// Encode the physical source space itself, not discovery metadata which led
/// to it. CSR chronology is retained because circuit row and column ordinals
/// are interpreted against this exact plan.
pub(super) fn append_physical_plan(
    output: &mut BoundedContentHasher,
    plan: &PhysicalFramePlan,
) -> Result<(), StratumRegistryError> {
    output.text(plan.family_fingerprint())?;
    output.text(plan.context_fingerprint())?;
    append_mask(output, plan.sector())?;

    output.count(plan.columns().len())?;
    for column in plan.columns() {
        append_integral_shift(output, column)?;
    }
    output.count(plan.row_offsets().len())?;
    for &offset in plan.row_offsets() {
        output.u32(offset)?;
    }
    output.count(plan.column_indices().len())?;
    for &column in plan.column_indices() {
        output.u32(column)?;
    }

    output.count(plan.source_instances().len())?;
    for (row, source_instance) in plan.source_instances().iter().enumerate() {
        output.text(&source_instance.stable_string())?;
        let source = plan
            .source_for_row(row)
            .ok_or(StratumRegistryError::Invariant {
                detail: "published physical plan has no translated source for one row",
            })?;
        append_translated_source(output, source)?;
    }
    Ok(())
}

/// Encode exact characteristic-zero circuit content. Modular primes, sampled
/// coordinates, ranks, and live-plan pointer tokens are intentionally absent.
pub(super) fn append_exact_circuit(
    output: &mut BoundedContentHasher,
    circuit: &ExactTargetCircuit,
) -> Result<(), StratumRegistryError> {
    output.text(circuit.stratum_id().as_str())?;
    output.text(circuit.owner_snapshot_id().as_str())?;
    output.usize(circuit.target_column())?;
    append_integral_shift(output, circuit.target_shift())?;

    output.count(circuit.residual_terms().len())?;
    for term in circuit.residual_terms() {
        output.usize(term.physical_column())?;
        append_integral_shift(output, term.shift())?;
        append_coefficient(output, term.coefficient())?;
        append_monotone_descent(output, term.descent())?;
        output.count(term.proper_subsector_owners().len())?;
        for &owner in term.proper_subsector_owners() {
            append_proper_subsector_owner(output, owner)?;
        }
    }

    output.count(circuit.source_combination().len())?;
    for source in circuit.source_combination() {
        output.usize(source.frame_row_ordinal())?;
        output.text(&source.source_instance().stable_string())?;
        append_coefficient(output, source.coefficient())?;
    }

    output.count(circuit.pivot_guards().len())?;
    for guard in circuit.pivot_guards() {
        output.usize(guard.frame_row_ordinal())?;
        output.text(&guard.source_instance().stable_string())?;
        output.usize(guard.physical_pivot_column())?;
        append_coefficient(output, guard.coefficient())?;
        append_polynomial(output, guard.nonzero_polynomial())?;
    }

    output.count(circuit.nonzero_guards().len())?;
    for guard in circuit.nonzero_guards() {
        append_polynomial(output, guard.polynomial())?;
        output.count(guard.origins().len())?;
        for origin in guard.origins() {
            append_guard_origin(output, origin)?;
        }
    }

    let replay = circuit.replay();
    output.usize(replay.source_contributions())?;
    output.usize(replay.source_terms())?;
    output.usize(replay.physical_columns())?;
    output.usize(replay.exact_operations())
}

fn append_guard_origin(
    output: &mut BoundedContentHasher,
    origin: &ExactCircuitGuardOrigin,
) -> Result<(), StratumRegistryError> {
    match origin {
        ExactCircuitGuardOrigin::SourceCondition {
            frame_row_ordinal,
            source_instance,
            condition_ordinal,
            condition_sources,
        } => {
            output.tag(0)?;
            output.usize(*frame_row_ordinal)?;
            output.text(&source_instance.stable_string())?;
            output.usize(*condition_ordinal)?;
            output.count(condition_sources.len())?;
            for source in condition_sources {
                append_identity_condition_source(output, source)?;
            }
            Ok(())
        }
        ExactCircuitGuardOrigin::SourceCoefficientDenominator {
            frame_row_ordinal,
            source_instance,
            physical_column,
        } => {
            output.tag(1)?;
            output.usize(*frame_row_ordinal)?;
            output.text(&source_instance.stable_string())?;
            output.usize(*physical_column)
        }
        ExactCircuitGuardOrigin::ReducerPivotNumerator {
            frame_row_ordinal,
            source_instance,
            physical_pivot_column,
        } => {
            output.tag(2)?;
            output.usize(*frame_row_ordinal)?;
            output.text(&source_instance.stable_string())?;
            output.usize(*physical_pivot_column)
        }
        ExactCircuitGuardOrigin::ReducerPivotDenominator {
            frame_row_ordinal,
            source_instance,
            physical_pivot_column,
        } => {
            output.tag(3)?;
            output.usize(*frame_row_ordinal)?;
            output.text(&source_instance.stable_string())?;
            output.usize(*physical_pivot_column)
        }
        ExactCircuitGuardOrigin::SourceMultiplierDenominator {
            frame_row_ordinal,
            source_instance,
        } => {
            output.tag(4)?;
            output.usize(*frame_row_ordinal)?;
            output.text(&source_instance.stable_string())
        }
        ExactCircuitGuardOrigin::ResidualCoefficientDenominator { physical_column } => {
            output.tag(5)?;
            output.usize(*physical_column)
        }
    }
}
