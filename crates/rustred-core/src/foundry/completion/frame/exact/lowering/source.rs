use crate::algebra::IndexedCoefficientContext;
use crate::foundry::cell::SourceViewProvenance;
use crate::foundry::completion::frame::{PhysicalFramePlan, SourceInstanceId};
use crate::foundry::parametric::{ParametricReducerPivotGuard, ParametricSourceRowContribution};
use crate::identity::{IndexShift, ParametricRelation};

use super::super::ExactTargetCircuit;
use super::resource::{SELECTED_ROWS, SELECTED_SOURCE_TERMS, check_limit, checked_add, try_vec};
use super::{ExactCircuitLoweringError, ExactCircuitLoweringLimits, ExactCircuitLoweringSeal};

pub(super) fn collect_selected_rows(
    plan: &PhysicalFramePlan,
    circuit: &ExactTargetCircuit,
    limits: ExactCircuitLoweringLimits,
) -> Result<Vec<usize>, ExactCircuitLoweringError> {
    let requested = checked_add(
        SELECTED_ROWS,
        circuit.source_combination().len(),
        circuit.pivot_guards().len(),
    )?;
    let mut rows = try_vec(SELECTED_ROWS, requested)?;
    for contribution in circuit.source_combination() {
        validate_row_instance(
            plan,
            contribution.frame_row_ordinal(),
            contribution.source_instance(),
            "source combination",
        )?;
        rows.push(contribution.frame_row_ordinal());
    }
    for pivot in circuit.pivot_guards() {
        validate_row_instance(
            plan,
            pivot.frame_row_ordinal(),
            pivot.source_instance(),
            "pivot chronology",
        )?;
        rows.push(pivot.frame_row_ordinal());
    }
    rows.sort_unstable();
    rows.dedup();
    check_limit(SELECTED_ROWS, rows.len(), limits.max_selected_source_rows)?;
    check_limit(
        "parametric source rows",
        rows.len(),
        limits.parametric.max_source_rows,
    )?;
    Ok(rows)
}

pub(super) fn clone_selected_sources(
    seal: &ExactCircuitLoweringSeal,
    context: &IndexedCoefficientContext,
    plan: &PhysicalFramePlan,
    selected_rows: &[usize],
    limits: ExactCircuitLoweringLimits,
) -> Result<(Vec<ParametricRelation>, Vec<SourceViewProvenance>), ExactCircuitLoweringError> {
    let mut term_count = 0usize;
    for &row in selected_rows {
        let source = plan
            .source_for_row(row)
            .ok_or(ExactCircuitLoweringError::SourceJoin {
                row,
                detail: "selected row has no translated source",
            })?;
        term_count = checked_add(SELECTED_SOURCE_TERMS, term_count, source.terms().len())?;
    }
    check_limit(
        SELECTED_SOURCE_TERMS,
        term_count,
        limits.max_selected_source_terms,
    )?;
    let input_entries = checked_add(
        "parametric source nonzero entries",
        term_count,
        selected_rows.len(),
    )?;
    check_limit(
        "parametric source nonzero entries",
        input_entries,
        limits.parametric.max_input_nonzero_entries,
    )?;

    let mut relations = try_vec(SELECTED_ROWS, selected_rows.len())?;
    let mut provenance = try_vec(SELECTED_ROWS, selected_rows.len())?;
    for &row in selected_rows {
        let source = plan
            .source_for_row(row)
            .ok_or(ExactCircuitLoweringError::SourceJoin {
                row,
                detail: "selected row has no translated source",
            })?;
        for coefficient in source.terms().values() {
            context.validate_with_limits(
                coefficient,
                limits.parametric.indexed_algebra.exact_algebra,
            )?;
        }
        for condition in source.nonzero_conditions() {
            context.validate_polynomial_with_limits(
                condition.polynomial(),
                limits.parametric.indexed_algebra.exact_algebra,
            )?;
        }
        let (relation, translated) =
            source.cloned_foundry_parts_with_limits(context, limits.relation)?;
        if plan.source_instances()[row].provenance() != &translated {
            return Err(ExactCircuitLoweringError::SourceJoin {
                row,
                detail: "cloned provenance differs from physical source identity",
            });
        }
        relations.push(relation);
        provenance.push(SourceViewProvenance::from_exact_translation(
            seal, translated,
        ));
    }
    Ok((relations, provenance))
}

pub(super) fn canonical_source_shift(
    plan: &PhysicalFramePlan,
    source_shift_columns: &[IndexShift],
    physical_column: usize,
    detail: &'static str,
) -> Result<IndexShift, ExactCircuitLoweringError> {
    let physical = plan
        .columns()
        .get(physical_column)
        .ok_or(ExactCircuitLoweringError::TargetJoin(detail))?;
    let compact = source_shift_columns
        .binary_search_by(|candidate| candidate.values().cmp(physical.values()))
        .map_err(|_| ExactCircuitLoweringError::TargetJoin(detail))?;
    Ok(source_shift_columns[compact].clone())
}

pub(super) fn compile_source_shift_columns(
    relations: &[ParametricRelation],
    limits: ExactCircuitLoweringLimits,
) -> Result<Vec<IndexShift>, ExactCircuitLoweringError> {
    let term_count = relations.iter().try_fold(0usize, |count, relation| {
        checked_add(SELECTED_SOURCE_TERMS, count, relation.terms().len())
    })?;
    let mut columns = try_vec("source-view shift columns", term_count)?;
    columns.extend(
        relations
            .iter()
            .flat_map(|relation| relation.terms().keys().cloned()),
    );
    columns.sort_unstable();
    columns.dedup();
    check_limit(
        "source-view shift columns",
        columns.len(),
        limits.parametric.max_shift_columns,
    )?;
    Ok(columns)
}

pub(super) fn compile_source_combination(
    seal: &ExactCircuitLoweringSeal,
    plan: &PhysicalFramePlan,
    circuit: &ExactTargetCircuit,
    selected_rows: &[usize],
    relations: &[ParametricRelation],
) -> Result<Vec<ParametricSourceRowContribution>, ExactCircuitLoweringError> {
    let mut output = try_vec("source combination", circuit.source_combination().len())?;
    for contribution in circuit.source_combination() {
        let source_ordinal = local_row(selected_rows, contribution.frame_row_ordinal())?;
        let relation = &relations[source_ordinal];
        let physical_source = plan
            .source_for_row(contribution.frame_row_ordinal())
            .ok_or(ExactCircuitLoweringError::SourceJoin {
                row: contribution.frame_row_ordinal(),
                detail: "source combination row is absent from the physical plan",
            })?;
        if relation.row_id() != physical_source.row_id() {
            return Err(ExactCircuitLoweringError::SourceJoin {
                row: contribution.frame_row_ordinal(),
                detail: "row id changed during source remap",
            });
        }
        output.push(ParametricSourceRowContribution::from_exact_lowering(
            seal,
            source_ordinal,
            relation.row_id().clone(),
            contribution.coefficient().clone(),
        ));
    }
    Ok(output)
}

pub(super) fn compile_pivot_guards(
    seal: &ExactCircuitLoweringSeal,
    plan: &PhysicalFramePlan,
    circuit: &ExactTargetCircuit,
    selected_rows: &[usize],
    relations: &[ParametricRelation],
    source_shift_columns: &[IndexShift],
) -> Result<Vec<ParametricReducerPivotGuard>, ExactCircuitLoweringError> {
    let mut output = try_vec("pivot guards", circuit.pivot_guards().len())?;
    for (ordinal, pivot) in circuit.pivot_guards().iter().enumerate() {
        validate_row_instance(
            plan,
            pivot.frame_row_ordinal(),
            pivot.source_instance(),
            "pivot guard",
        )?;
        let source_ordinal = local_row(selected_rows, pivot.frame_row_ordinal())?;
        let physical_shift = plan.columns().get(pivot.physical_pivot_column()).ok_or(
            ExactCircuitLoweringError::PivotJoin {
                pivot: ordinal,
                detail: "physical pivot column is outside the plan",
            },
        )?;
        let pivot_column = source_shift_columns
            .binary_search_by(|candidate| candidate.values().cmp(physical_shift.values()))
            .map_err(|_| ExactCircuitLoweringError::PivotJoin {
                pivot: ordinal,
                detail: "pivot shift is absent from compact source-view columns",
            })?;
        let pivot_shift = source_shift_columns[pivot_column].clone();
        output.push(ParametricReducerPivotGuard::from_exact_lowering(
            seal,
            source_ordinal,
            relations[source_ordinal].row_id().clone(),
            pivot_column,
            pivot_shift,
            pivot.coefficient().clone(),
            pivot.nonzero_polynomial().clone(),
        ));
    }
    Ok(output)
}

pub(super) fn validate_row_instance(
    plan: &PhysicalFramePlan,
    row: usize,
    instance: &SourceInstanceId,
    detail: &'static str,
) -> Result<(), ExactCircuitLoweringError> {
    if plan.source_instances().get(row) == Some(instance) && plan.source_for_row(row).is_some() {
        Ok(())
    } else {
        Err(ExactCircuitLoweringError::SourceJoin { row, detail })
    }
}

pub(super) fn require_row_column(
    plan: &PhysicalFramePlan,
    row: usize,
    column: usize,
) -> Result<(), ExactCircuitLoweringError> {
    let columns =
        plan.column_indices_for_row(row)
            .ok_or(ExactCircuitLoweringError::SourceJoin {
                row,
                detail: "row has invalid CSR bounds",
            })?;
    let column = u32::try_from(column).map_err(|_| ExactCircuitLoweringError::SourceJoin {
        row,
        detail: "column does not fit physical CSR storage",
    })?;
    columns
        .binary_search(&column)
        .map(|_| ())
        .map_err(|_| ExactCircuitLoweringError::SourceJoin {
            row,
            detail: "column is absent from physical row",
        })
}

pub(super) fn local_row(
    selected_rows: &[usize],
    physical_row: usize,
) -> Result<usize, ExactCircuitLoweringError> {
    selected_rows
        .binary_search(&physical_row)
        .map_err(|_| ExactCircuitLoweringError::SourceJoin {
            row: physical_row,
            detail: "row was not retained in compact source view",
        })
}
