use crate::foundry::completion::frame::PhysicalFramePlan;
use crate::foundry::parametric::{ParametricGuardOrigin, ParametricNonZeroGuard};
use crate::identity::{IdentityConditionSource, IndexShift, ParametricRelation};

use super::super::{ExactCircuitGuardOrigin, ExactTargetCircuit};
use super::resource::{
    GUARD_COORDINATES, GUARD_ORIGINS, GUARD_SOURCES, check_limit, checked_add, try_vec,
};
use super::source::{local_row, require_row_column, validate_row_instance};
use super::{ExactCircuitLoweringError, ExactCircuitLoweringLimits, ExactCircuitLoweringSeal};

pub(super) fn compile_guards(
    seal: &ExactCircuitLoweringSeal,
    plan: &PhysicalFramePlan,
    circuit: &ExactTargetCircuit,
    selected_rows: &[usize],
    relations: &[ParametricRelation],
    source_shift_columns: &[IndexShift],
) -> Result<Vec<ParametricNonZeroGuard>, ExactCircuitLoweringError> {
    let mut guards = try_vec("nonzero guards", circuit.nonzero_guards().len())?;
    for (guard_ordinal, guard) in circuit.nonzero_guards().iter().enumerate() {
        let mut origins = try_vec(GUARD_ORIGINS, guard.origins().len())?;
        for (origin_ordinal, origin) in guard.origins().iter().enumerate() {
            origins.push(map_guard_origin(
                plan,
                circuit,
                selected_rows,
                relations,
                source_shift_columns,
                guard_ordinal,
                origin_ordinal,
                origin,
            )?);
        }
        guards.push(ParametricNonZeroGuard::from_replayed_exact_parts(
            seal,
            guard.polynomial().clone(),
            origins,
        ));
    }
    Ok(guards)
}

#[allow(clippy::too_many_arguments)]
fn map_guard_origin(
    plan: &PhysicalFramePlan,
    circuit: &ExactTargetCircuit,
    selected_rows: &[usize],
    relations: &[ParametricRelation],
    source_shift_columns: &[IndexShift],
    guard: usize,
    origin: usize,
    exact: &ExactCircuitGuardOrigin,
) -> Result<ParametricGuardOrigin, ExactCircuitLoweringError> {
    let join_error = |detail| ExactCircuitLoweringError::GuardOriginJoin {
        guard,
        origin,
        detail,
    };
    let row_parts = |row, instance: &_| -> Result<(usize, _), ExactCircuitLoweringError> {
        validate_row_instance(plan, row, instance, "guard origin")?;
        let local =
            local_row(selected_rows, row).map_err(|_| join_error("row was not selected"))?;
        Ok((local, relations[local].row_id().clone()))
    };
    Ok(match exact {
        ExactCircuitGuardOrigin::SourceCondition {
            frame_row_ordinal,
            source_instance,
            condition_ordinal,
            condition_sources,
        } => {
            let (source_ordinal, row_id) = row_parts(*frame_row_ordinal, source_instance)?;
            let condition = relations[source_ordinal]
                .nonzero_conditions()
                .get(*condition_ordinal)
                .ok_or_else(|| join_error("condition ordinal is outside the source"))?;
            if !condition.sources().iter().eq(condition_sources.iter()) {
                return Err(join_error("condition-source provenance changed"));
            }
            ParametricGuardOrigin::SourceCondition {
                source_ordinal,
                row_id,
                condition_ordinal: *condition_ordinal,
                condition_sources: clone_condition_sources(condition_sources)?,
            }
        }
        ExactCircuitGuardOrigin::SourceCoefficientDenominator {
            frame_row_ordinal,
            source_instance,
            physical_column,
        } => {
            let (source_ordinal, row_id) = row_parts(*frame_row_ordinal, source_instance)?;
            require_row_column(plan, *frame_row_ordinal, *physical_column)
                .map_err(|_| join_error("coefficient column is absent from source row"))?;
            let physical_shift = plan
                .columns()
                .get(*physical_column)
                .ok_or_else(|| join_error("coefficient column is outside the plan"))?;
            let compact = source_shift_columns
                .binary_search_by(|candidate| candidate.values().cmp(physical_shift.values()))
                .map_err(|_| join_error("coefficient shift is absent from source-view columns"))?;
            ParametricGuardOrigin::SourceCoefficientDenominator {
                source_ordinal,
                row_id,
                shift: source_shift_columns[compact].clone(),
            }
        }
        ExactCircuitGuardOrigin::ReducerPivotNumerator {
            frame_row_ordinal,
            source_instance,
            physical_pivot_column,
        }
        | ExactCircuitGuardOrigin::ReducerPivotDenominator {
            frame_row_ordinal,
            source_instance,
            physical_pivot_column,
        } => {
            let (source_ordinal, row_id) = row_parts(*frame_row_ordinal, source_instance)?;
            let physical_shift = plan
                .columns()
                .get(*physical_pivot_column)
                .ok_or_else(|| join_error("pivot column is outside the plan"))?
                .values();
            let pivot_column = source_shift_columns
                .binary_search_by(|candidate| candidate.values().cmp(physical_shift))
                .map_err(|_| join_error("pivot shift is absent from source-view columns"))?;
            let pivot_shift = source_shift_columns[pivot_column].clone();
            if matches!(exact, ExactCircuitGuardOrigin::ReducerPivotNumerator { .. }) {
                ParametricGuardOrigin::ReducerPivotNumerator {
                    source_ordinal,
                    row_id,
                    pivot_column,
                    pivot_shift,
                }
            } else {
                ParametricGuardOrigin::ReducerPivotDenominator {
                    source_ordinal,
                    row_id,
                    pivot_column,
                    pivot_shift,
                }
            }
        }
        ExactCircuitGuardOrigin::SourceMultiplierDenominator {
            frame_row_ordinal,
            source_instance,
        } => {
            let (source_ordinal, row_id) = row_parts(*frame_row_ordinal, source_instance)?;
            ParametricGuardOrigin::SourceCombinationDenominator {
                source_ordinal,
                row_id,
            }
        }
        ExactCircuitGuardOrigin::ResidualCoefficientDenominator { physical_column } => {
            if circuit
                .residual_terms()
                .binary_search_by_key(physical_column, |term| term.physical_column())
                .is_err()
            {
                return Err(join_error("column is not a retained residual"));
            }
            let physical_shift = plan
                .columns()
                .get(*physical_column)
                .ok_or_else(|| join_error("residual column is outside the plan"))?
                .values();
            let compact = source_shift_columns
                .binary_search_by(|candidate| candidate.values().cmp(physical_shift))
                .map_err(|_| join_error("residual shift is absent from source-view columns"))?;
            let shift = source_shift_columns[compact].clone();
            ParametricGuardOrigin::RuleCoefficientDenominator { shift }
        }
    })
}

fn clone_condition_sources(
    sources: &[IdentityConditionSource],
) -> Result<Box<[IdentityConditionSource]>, ExactCircuitLoweringError> {
    let mut retained = try_vec(GUARD_SOURCES, sources.len())?;
    for source in sources {
        retained.push(clone_condition_source(source)?);
    }
    Ok(retained.into_boxed_slice())
}

fn clone_condition_source(
    source: &IdentityConditionSource,
) -> Result<IdentityConditionSource, ExactCircuitLoweringError> {
    Ok(match source {
        IdentityConditionSource::FamilyInputCoefficientDenominator { location } => {
            IdentityConditionSource::FamilyInputCoefficientDenominator {
                location: location.clone(),
            }
        }
        IdentityConditionSource::FamilyBasisDeterminantNumerator => {
            IdentityConditionSource::FamilyBasisDeterminantNumerator
        }
        IdentityConditionSource::RelationConditionAttached { row } => {
            IdentityConditionSource::RelationConditionAttached { row: row.clone() }
        }
        IdentityConditionSource::RelationInputTermDenominator { row, shift } => {
            IdentityConditionSource::RelationInputTermDenominator {
                row: row.clone(),
                shift: try_boxed_i64(shift)?,
            }
        }
        IdentityConditionSource::RelationCollectedTermDenominator { row, shift } => {
            IdentityConditionSource::RelationCollectedTermDenominator {
                row: row.clone(),
                shift: try_boxed_i64(shift)?,
            }
        }
        IdentityConditionSource::RelationScaleFactorDenominator {
            target_row,
            source_row,
        } => IdentityConditionSource::RelationScaleFactorDenominator {
            target_row: target_row.clone(),
            source_row: source_row.clone(),
        },
        IdentityConditionSource::RelationTranslation {
            source_row,
            target_row,
            offset,
        } => IdentityConditionSource::RelationTranslation {
            source_row: source_row.clone(),
            target_row: target_row.clone(),
            offset: try_boxed_i64(offset)?,
        },
        IdentityConditionSource::IndexTranslation { offset } => {
            IdentityConditionSource::IndexTranslation {
                offset: try_boxed_i64(offset)?,
            }
        }
    })
}

fn try_boxed_i64(values: &[i64]) -> Result<Box<[i64]>, ExactCircuitLoweringError> {
    let mut retained = try_vec(GUARD_COORDINATES, values.len())?;
    retained.extend_from_slice(values);
    Ok(retained.into_boxed_slice())
}

pub(super) fn preflight_guards(
    circuit: &ExactTargetCircuit,
    limits: ExactCircuitLoweringLimits,
) -> Result<(), ExactCircuitLoweringError> {
    check_limit(
        "nonzero guards",
        circuit.nonzero_guards().len(),
        limits.parametric.max_rule_guards,
    )?;
    let mut origins = 0usize;
    let mut sources = 0usize;
    let mut coordinate_cells = 0usize;
    for guard in circuit.nonzero_guards() {
        origins = checked_add(GUARD_ORIGINS, origins, guard.origins().len())?;
        for origin in guard.origins() {
            if let ExactCircuitGuardOrigin::SourceCondition {
                condition_sources, ..
            } = origin
            {
                sources = checked_add(GUARD_SOURCES, sources, condition_sources.len())?;
                for source in condition_sources.iter() {
                    coordinate_cells = checked_add(
                        GUARD_COORDINATES,
                        coordinate_cells,
                        source_coordinate_cells(source),
                    )?;
                }
            }
        }
    }
    check_limit(GUARD_ORIGINS, origins, limits.max_guard_origins)?;
    check_limit(
        "parametric rule guard origins",
        origins,
        limits.parametric.max_guard_origins,
    )?;
    check_limit(GUARD_SOURCES, sources, limits.max_guard_condition_sources)?;
    check_limit(
        "parametric guard provenance sources",
        sources,
        limits.parametric.max_guard_provenance_sources,
    )?;
    check_limit(
        GUARD_COORDINATES,
        coordinate_cells,
        limits.max_guard_provenance_coordinate_cells,
    )?;
    check_limit(
        "parametric guard provenance index cells",
        coordinate_cells,
        limits.parametric.max_guard_provenance_index_cells,
    )?;
    Ok(())
}

fn source_coordinate_cells(source: &IdentityConditionSource) -> usize {
    match source {
        IdentityConditionSource::RelationInputTermDenominator { shift, .. }
        | IdentityConditionSource::RelationCollectedTermDenominator { shift, .. } => shift.len(),
        IdentityConditionSource::RelationTranslation { offset, .. }
        | IdentityConditionSource::IndexTranslation { offset } => offset.len(),
        IdentityConditionSource::FamilyInputCoefficientDenominator { .. }
        | IdentityConditionSource::FamilyBasisDeterminantNumerator
        | IdentityConditionSource::RelationConditionAttached { .. }
        | IdentityConditionSource::RelationScaleFactorDenominator { .. } => 0,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::identity::RowId;

    use super::*;

    #[test]
    fn every_coordinate_bearing_guard_source_counts_its_full_vector() {
        let row = RowId::Derived {
            label: Arc::from("guard-coordinate-census"),
        };
        let sources = [
            IdentityConditionSource::RelationInputTermDenominator {
                row: row.clone(),
                shift: Box::new([1, 2]),
            },
            IdentityConditionSource::RelationCollectedTermDenominator {
                row: row.clone(),
                shift: Box::new([3, 4, 5]),
            },
            IdentityConditionSource::RelationTranslation {
                source_row: row.clone(),
                target_row: row,
                offset: Box::new([6, 7, 8, 9]),
            },
            IdentityConditionSource::IndexTranslation {
                offset: Box::new([10, 11, 12, 13, 14]),
            },
        ];
        assert_eq!(
            sources.map(|source| source_coordinate_cells(&source)),
            [2, 3, 4, 5]
        );
    }
}
