use symbolica::domains::SelfRing;

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext};
use crate::foundry::completion::frame::PhysicalFramePlan;
use crate::foundry::parametric::{ParametricExactReplayWitness, ParametricSourceRowContribution};
use crate::identity::ParametricRelation;

use super::super::ExactTargetCircuit;
use super::resource::{
    REPLAY_OPERATIONS, SELECTED_SOURCE_TERMS, check_limit, checked_add, try_vec,
};
use super::{ExactCircuitLoweringError, ExactCircuitLoweringLimits, ExactCircuitLoweringSeal};

pub(super) fn independently_replay_full_span(
    seal: &ExactCircuitLoweringSeal,
    context: &IndexedCoefficientContext,
    plan: &PhysicalFramePlan,
    circuit: &ExactTargetCircuit,
    selected_rows: &[usize],
    combination: &[ParametricSourceRowContribution],
    relations: &[ParametricRelation],
    limits: ExactCircuitLoweringLimits,
) -> Result<ParametricExactReplayWitness, ExactCircuitLoweringError> {
    let mut replayed: Vec<Option<IndexedCoefficient>> =
        try_vec("full-span replay accumulators", plan.columns().len())?;
    replayed.resize_with(plan.columns().len(), || None);
    let mut operations = 0usize;
    let mut source_terms = 0usize;
    for contribution in combination {
        let source_ordinal = contribution.source_ordinal();
        let relation =
            relations
                .get(source_ordinal)
                .ok_or(ExactCircuitLoweringError::Invariant(
                    "source combination refers outside compact source relations",
                ))?;
        let physical_row =
            *selected_rows
                .get(source_ordinal)
                .ok_or(ExactCircuitLoweringError::Invariant(
                    "source combination refers outside selected physical rows",
                ))?;
        let physical_columns = plan.column_indices_for_row(physical_row).ok_or(
            ExactCircuitLoweringError::Invariant("selected physical row has invalid CSR bounds"),
        )?;
        if physical_columns.len() != relation.terms().len() {
            return Err(ExactCircuitLoweringError::Invariant(
                "selected source relation disagrees with physical CSR arity",
            ));
        }
        for ((shift, coefficient), &physical_column) in
            relation.terms().iter().zip(physical_columns)
        {
            source_terms = checked_add(SELECTED_SOURCE_TERMS, source_terms, 1)?;
            operations = charge_replay(operations, limits)?;
            let physical_column = usize::try_from(physical_column).map_err(|_| {
                ExactCircuitLoweringError::Invariant(
                    "physical CSR column is not representable as usize",
                )
            })?;
            if plan
                .columns()
                .get(physical_column)
                .is_none_or(|physical| physical.values() != shift.values())
            {
                return Err(ExactCircuitLoweringError::Invariant(
                    "selected source term disagrees with its physical CSR column",
                ));
            }
            let (coefficient, _denominator_guard) = context.specialize_fixed_indices_sealed(
                coefficient,
                circuit.fixed_indices(),
                limits.parametric.indexed_algebra,
            )?;
            let product = context.mul_with_limits(
                contribution.coefficient(),
                &coefficient,
                limits.parametric.indexed_algebra.exact_algebra,
            )?;
            if let Some(accumulator) = replayed[physical_column].take() {
                operations = charge_replay(operations, limits)?;
                let sum = context.add_with_limits(
                    &accumulator,
                    &product,
                    limits.parametric.indexed_algebra.exact_algebra,
                )?;
                if !sum.is_zero() {
                    replayed[physical_column] = Some(sum);
                }
            } else if !product.is_zero() {
                replayed[physical_column] = Some(product);
            }
        }
    }
    for (column, actual) in replayed.into_iter().enumerate() {
        operations = charge_replay(operations, limits)?;
        let matches = if column == circuit.target_column() {
            actual.as_ref().is_some_and(|value| value.raw().is_one())
        } else if let Ok(ordinal) = circuit
            .residual_terms()
            .binary_search_by_key(&column, |term| term.physical_column())
        {
            actual.as_ref() == Some(circuit.residual_terms()[ordinal].coefficient())
        } else {
            actual.is_none() || actual.as_ref().is_some_and(IndexedCoefficient::is_zero)
        };
        if !matches {
            return Err(ExactCircuitLoweringError::ReplayMismatch {
                physical_column: column,
            });
        }
    }

    let exact = circuit.replay();
    if exact.source_contributions() != combination.len() {
        return Err(ExactCircuitLoweringError::ReplayWitnessMismatch(
            "source-contribution count",
        ));
    }
    if exact.source_terms() != source_terms {
        return Err(ExactCircuitLoweringError::ReplayWitnessMismatch(
            "source-term count",
        ));
    }
    if exact.physical_columns() != plan.columns().len() {
        return Err(ExactCircuitLoweringError::ReplayWitnessMismatch(
            "physical-column count",
        ));
    }
    if exact.exact_operations() != operations {
        return Err(ExactCircuitLoweringError::ReplayWitnessMismatch(
            "exact-operation count",
        ));
    }
    Ok(ParametricExactReplayWitness::from_exact_lowering(
        seal,
        combination.len(),
        plan.columns().len(),
        operations,
    ))
}

fn charge_replay(
    used: usize,
    limits: ExactCircuitLoweringLimits,
) -> Result<usize, ExactCircuitLoweringError> {
    let used = checked_add(REPLAY_OPERATIONS, used, 1)?;
    check_limit(
        REPLAY_OPERATIONS,
        used,
        limits.parametric.max_replay_exact_operations,
    )?;
    Ok(used)
}
