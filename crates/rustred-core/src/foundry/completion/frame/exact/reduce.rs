use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::domains::SelfRing;
use symbolica::domains::rational_polynomial::RationalPolynomialField;
use symbolica::prelude::{IntegerRing, Z};
use symbolica::tensors::sparse::{LuLMode, SparseRowReducer};

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext};
use crate::foundry::completion::stratum::TargetColumnPartition;

use super::super::PhysicalFramePlan;
use super::super::modular::ModularHit;
use super::{
    ExactCircuitError, ExactCircuitLift, ExactCircuitLimits, ExactCircuitPivotGuard,
    ExactCircuitSupportDidNotLift, ExactFrameSourceContribution,
};

const SELECTED_ROWS: &str = "exact-circuit selected source rows";
const PROJECTED_PHYSICAL_COLUMNS: &str = "exact-circuit projected physical columns";
const AUGMENTED_COLUMNS: &str = "exact-circuit augmented columns";
const PROJECTED_INPUT_ENTRIES: &str = "exact-circuit projected input nonzero entries";
const NATIVE_DECOMPOSITION: &str = "exact-circuit native U/L nonzero entries";
const PIVOT_DEPENDENCIES: &str = "exact-circuit pivot dependency entries";
const REPLAY_SOURCE_TERMS: &str = "exact-circuit replay source terms";
const REPLAY_EXACT_OPERATIONS: &str = "exact-circuit replay exact operations";
const GUARD_ORIGINS: &str = "exact-circuit conservative guard origins";
const CONDITION_SOURCES: &str = "exact-circuit condition-source entries";
const DEPENDENCY_OWNER_WITNESSES: &str = "exact-circuit dependency owner witnesses";

type NativeField = RationalPolynomialField<IntegerRing, u16>;

#[derive(Debug)]
struct ExactForwardRowMeta {
    frame_row: usize,
    reducer_row: usize,
    pivot_projected_column: usize,
    pivot_coefficient: IndexedCoefficient,
    pivot_dependencies: Vec<usize>,
}

pub(super) struct ReducedExactCircuit {
    pub(super) source_combination: Vec<ExactFrameSourceContribution>,
    pub(super) pivot_guards: Vec<ExactCircuitPivotGuard>,
}

pub(crate) fn try_lift_exact_circuit<'frame>(
    context: &IndexedCoefficientContext,
    hit: &ModularHit<'frame>,
    partition: &TargetColumnPartition<'frame>,
    limits: ExactCircuitLimits,
) -> Result<ExactCircuitLift, ExactCircuitError> {
    let plan = partition.frame();
    validate_binding(context, hit, partition, plan)?;
    let selected = hit.diagnostics().augmented_independent_source_rows.as_ref();
    validate_modular_hit_shape(hit, plan, limits)?;
    try_lift_exact_circuit_with_selected_rows(context, hit, partition, selected, limits)
}

/// Bounded diagnostic fallback that asks Symbolica for an exact target pivot
/// over every row in the already materialized physical frame.
///
/// This does not expand the translation universe and it does not turn a miss
/// into negative evidence.  It is intentionally separate from the normal
/// modular-minor lift so high-loop callers must opt into, and resource-bound,
/// the potentially larger exact reduction.
pub(crate) fn try_lift_exact_circuit_over_complete_frame<'frame>(
    context: &IndexedCoefficientContext,
    hit: &ModularHit<'frame>,
    partition: &TargetColumnPartition<'frame>,
    limits: ExactCircuitLimits,
) -> Result<ExactCircuitLift, ExactCircuitError> {
    let plan = partition.frame();
    validate_binding(context, hit, partition, plan)?;
    validate_modular_hit_shape(hit, plan, limits)?;
    check_limit(SELECTED_ROWS, plan.row_count(), limits.max_selected_rows)?;
    let mut selected = try_vec(SELECTED_ROWS, plan.row_count())?;
    selected.extend(0..plan.row_count());
    try_lift_exact_circuit_with_selected_rows(context, hit, partition, &selected, limits)
}

fn try_lift_exact_circuit_with_selected_rows<'frame>(
    context: &IndexedCoefficientContext,
    hit: &ModularHit<'frame>,
    partition: &TargetColumnPartition<'frame>,
    selected: &[usize],
    limits: ExactCircuitLimits,
) -> Result<ExactCircuitLift, ExactCircuitError> {
    let plan = partition.frame();
    validate_selected_rows(plan, selected, limits)?;
    preflight(context, plan, partition, selected, limits)?;
    let fixed_indices = fixed_index_assignments(context, partition)?;

    let forbidden = partition.forbidden_columns();
    let projected_physical_columns = checked_add(PROJECTED_PHYSICAL_COLUMNS, forbidden.len(), 1)?;
    let augmented_columns = checked_add(
        AUGMENTED_COLUMNS,
        projected_physical_columns,
        selected.len(),
    )?;
    let native_columns = checked_u32(AUGMENTED_COLUMNS, augmented_columns)?;
    let field = NativeField::new(Z);
    let mut reducer = call_native(
        "constructing the exact target-circuit sparse reducer",
        || SparseRowReducer::new(native_columns, field, LuLMode::Full),
    )?;
    let mut metadata: Vec<ExactForwardRowMeta> = try_vec(SELECTED_ROWS, selected.len())?;
    let mut retained_dependency_entries = 0usize;

    for (local_row, &frame_row) in selected.iter().enumerate() {
        let source = plan.source_for_row(frame_row).ok_or(
            ExactCircuitError::SelectedSourceRowOutOfRange {
                row: frame_row,
                rows: plan.row_count(),
            },
        )?;
        let structural_columns =
            plan.column_indices_for_row(frame_row)
                .ok_or(ExactCircuitError::Invariant {
                    detail: "selected source row has invalid physical CSR bounds",
                })?;
        if structural_columns.len() != source.terms().len() {
            return Err(ExactCircuitError::Invariant {
                detail: "selected source terms disagree with physical CSR",
            });
        }

        let row_capacity = checked_add(
            PROJECTED_INPUT_ENTRIES,
            projected_entry_count(structural_columns, partition)?,
            1,
        )?;
        let mut values = try_vec(PROJECTED_INPUT_ENTRIES, row_capacity)?;
        let mut columns = try_vec(PROJECTED_INPUT_ENTRIES, row_capacity)?;
        let mut target_value = None;
        for ((shift, coefficient), &physical_column) in
            source.terms().iter().zip(structural_columns)
        {
            context
                .bind_sealed(coefficient)
                .map_err(|_| ExactCircuitError::WrongIndexedContext { row: frame_row })?;
            let physical_column =
                usize::try_from(physical_column).map_err(|_| ExactCircuitError::Invariant {
                    detail: "physical source column does not fit usize",
                })?;
            if plan
                .columns()
                .get(physical_column)
                .is_none_or(|column| column.values() != shift.values())
            {
                return Err(ExactCircuitError::Invariant {
                    detail: "selected source term differs from its physical column",
                });
            }
            let (coefficient, _denominator_guard) = context.specialize_fixed_indices_sealed(
                coefficient,
                &fixed_indices,
                limits.indexed_algebra,
            )?;
            if coefficient.is_zero() {
                continue;
            }
            if physical_column == partition.target_column() {
                if target_value.replace(coefficient.raw().clone()).is_some() {
                    return Err(ExactCircuitError::Invariant {
                        detail: "selected source row repeats the target column",
                    });
                }
            } else if let Ok(projected) = forbidden.binary_search(&physical_column) {
                values.push(coefficient.raw().clone());
                columns.push(checked_u32("exact-circuit projected column", projected)?);
            }
        }
        if let Some(value) = target_value {
            values.push(value);
            columns.push(checked_u32(
                "exact-circuit projected target column",
                forbidden.len(),
            )?);
        }
        values.push(context.one().raw().clone());
        columns.push(checked_u32(
            "exact-circuit provenance column",
            checked_add(AUGMENTED_COLUMNS, projected_physical_columns, local_row)?,
        )?);
        if values.len() > row_capacity || columns.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(ExactCircuitError::Invariant {
                detail: "assembled exact projected row has invalid sparse ordering",
            });
        }

        let pivot = call_native("adding an exact selected row to the sparse reducer", || {
            reducer.add_row(&values, &columns)
        })?
        .ok_or(ExactCircuitError::ReducerRejectedSelectedRow { frame_row })?;
        let reducer_row =
            reducer
                .u()
                .nrows()
                .checked_sub(1)
                .ok_or(ExactCircuitError::Invariant {
                    detail: "exact reducer has no U row after accepting a selected source",
                })? as usize;
        let (lower_row, lower_columns, lower_values) =
            reducer.l().last_row().ok_or(ExactCircuitError::Invariant {
                detail: "exact reducer has no L row after accepting a selected source",
            })?;
        if lower_row as usize != local_row
            || lower_columns.last().copied().map(|row| row as usize) != Some(reducer_row)
        {
            return Err(ExactCircuitError::Invariant {
                detail: "exact reducer L lost selected-row chronology or its diagonal",
            });
        }
        let raw_pivot = lower_values
            .last()
            .cloned()
            .ok_or(ExactCircuitError::Invariant {
                detail: "exact reducer L diagonal has no pivot coefficient",
            })?;
        if raw_pivot.is_zero() {
            return Err(ExactCircuitError::Invariant {
                detail: "exact reducer returned an identically zero pivot coefficient",
            });
        }
        let pivot_coefficient = context
            .admit_native_result_with_limits(raw_pivot, limits.indexed_algebra.exact_algebra)?;

        let mut dependency_capacity = 1usize;
        for &dependency in &lower_columns[..lower_columns.len() - 1] {
            let dependency =
                usize::try_from(dependency).map_err(|_| ExactCircuitError::Invariant {
                    detail: "exact reducer L dependency does not fit usize",
                })?;
            let dependency = metadata
                .get(dependency)
                .ok_or(ExactCircuitError::Invariant {
                    detail: "exact reducer L dependency is outside prior chronology",
                })?;
            dependency_capacity = checked_add(
                PIVOT_DEPENDENCIES,
                dependency_capacity,
                dependency.pivot_dependencies.len(),
            )?;
        }
        check_limit(
            PIVOT_DEPENDENCIES,
            dependency_capacity,
            limits.max_pivot_dependency_entries,
        )?;
        let mut pivot_dependencies = try_vec(PIVOT_DEPENDENCIES, dependency_capacity)?;
        for &dependency in &lower_columns[..lower_columns.len() - 1] {
            let dependency =
                usize::try_from(dependency).map_err(|_| ExactCircuitError::Invariant {
                    detail: "exact reducer L dependency does not fit usize",
                })?;
            pivot_dependencies.extend_from_slice(&metadata[dependency].pivot_dependencies);
        }
        pivot_dependencies.sort_unstable();
        pivot_dependencies.dedup();
        pivot_dependencies.push(metadata.len());
        retained_dependency_entries = checked_add(
            PIVOT_DEPENDENCIES,
            retained_dependency_entries,
            pivot_dependencies.len(),
        )?;
        check_limit(
            PIVOT_DEPENDENCIES,
            retained_dependency_entries,
            limits.max_pivot_dependency_entries,
        )?;
        metadata.push(ExactForwardRowMeta {
            frame_row,
            reducer_row,
            pivot_projected_column: pivot as usize,
            pivot_coefficient,
            pivot_dependencies,
        });

        let decomposition_nonzeros = checked_add(
            NATIVE_DECOMPOSITION,
            reducer.u().nvalues(),
            reducer.l().nvalues(),
        )?;
        check_limit(
            NATIVE_DECOMPOSITION,
            decomposition_nonzeros,
            limits.max_native_decomposition_nonzero_entries,
        )?;
    }

    validate_complete_reducer(&reducer, &metadata, selected.len(), augmented_columns)?;
    let target_projected_column = forbidden.len();
    let exact_forbidden_rank = reducer
        .pivots()
        .iter()
        .take(target_projected_column)
        .filter(|pivot| pivot.is_some())
        .count();
    let exact_augmented_rank = exact_forbidden_rank
        + usize::from(
            reducer
                .pivots()
                .get(target_projected_column)
                .is_some_and(Option::is_some),
        );
    let Some(target_reducer_row) = reducer
        .pivots()
        .get(target_projected_column)
        .copied()
        .flatten()
        .map(|row| row as usize)
    else {
        let mut instances = try_vec(SELECTED_ROWS, selected.len())?;
        for &row in selected {
            instances.push(
                plan.source_instances()
                    .get(row)
                    .ok_or(ExactCircuitError::SelectedSourceRowOutOfRange {
                        row,
                        rows: plan.row_count(),
                    })?
                    .clone(),
            );
        }
        return Ok(ExactCircuitLift::ModularSupportDidNotLift(
            ExactCircuitSupportDidNotLift::new(
                hit.sample_fingerprint().clone(),
                hit.diagnostics().clone(),
                instances,
                exact_forbidden_rank,
                exact_augmented_rank,
            ),
        ));
    };

    let target_meta = metadata
        .get(target_reducer_row)
        .ok_or(ExactCircuitError::Invariant {
            detail: "exact target pivot row is outside reducer metadata",
        })?;
    if target_meta.reducer_row != target_reducer_row
        || target_meta.pivot_projected_column != target_projected_column
    {
        return Err(ExactCircuitError::Invariant {
            detail: "exact target pivot map disagrees with reducer metadata",
        });
    }
    let (_, row_columns, row_values) =
        reducer
            .u()
            .row_iter()
            .nth(target_reducer_row)
            .ok_or(ExactCircuitError::Invariant {
                detail: "exact target pivot row is absent from U",
            })?;
    let mut source_combination = try_vec(
        "exact-circuit source combination",
        row_columns.len().min(selected.len()),
    )?;
    let mut saw_target = false;
    for (&column, raw) in row_columns.iter().zip(row_values) {
        if raw.is_zero() {
            return Err(ExactCircuitError::Invariant {
                detail: "exact target U row exposes an explicit zero",
            });
        }
        let column = column as usize;
        if column < projected_physical_columns {
            if column != target_projected_column || saw_target || !raw.is_one() {
                return Err(ExactCircuitError::Invariant {
                    detail: "forward target-pivot row did not cancel every forbidden column or normalize the target",
                });
            }
            saw_target = true;
            continue;
        }
        let local_source = column - projected_physical_columns;
        let &frame_row = selected
            .get(local_source)
            .ok_or(ExactCircuitError::Invariant {
                detail: "exact target U provenance is outside selected chronology",
            })?;
        let source_instance = plan
            .source_instances()
            .get(frame_row)
            .ok_or(ExactCircuitError::SelectedSourceRowOutOfRange {
                row: frame_row,
                rows: plan.row_count(),
            })?
            .clone();
        let coefficient = context
            .admit_native_result_with_limits(raw.clone(), limits.indexed_algebra.exact_algebra)?;
        source_combination.push(ExactFrameSourceContribution::new(
            frame_row,
            source_instance,
            coefficient,
        ));
    }
    if !saw_target || source_combination.is_empty() {
        return Err(ExactCircuitError::Invariant {
            detail: "exact target U row lost its target or all source provenance",
        });
    }
    check_limit(
        "exact-circuit source combination",
        source_combination.len(),
        limits.max_source_combination_terms,
    )?;
    if source_combination
        .windows(2)
        .any(|pair| pair[0].frame_row_ordinal() >= pair[1].frame_row_ordinal())
    {
        return Err(ExactCircuitError::Invariant {
            detail: "exact target U provenance is not in frame chronology",
        });
    }

    let mut pivot_guards = try_vec(
        "exact-circuit reducer pivot guards",
        target_meta.pivot_dependencies.len(),
    )?;
    for &dependency in &target_meta.pivot_dependencies {
        let dependency = metadata
            .get(dependency)
            .ok_or(ExactCircuitError::Invariant {
                detail: "exact target pivot dependency is outside reducer metadata",
            })?;
        if dependency.pivot_projected_column >= projected_physical_columns {
            return Err(ExactCircuitError::Invariant {
                detail: "exact physical target circuit depends on a provenance pivot",
            });
        }
        let physical_pivot_column =
            projected_to_physical(dependency.pivot_projected_column, partition)?;
        let bound = context.bind_sealed(&dependency.pivot_coefficient)?;
        let numerator = context.numerator_condition_from_bound(bound)?;
        pivot_guards.push(ExactCircuitPivotGuard::new(
            dependency.frame_row,
            plan.source_instances()[dependency.frame_row].clone(),
            physical_pivot_column,
            dependency.pivot_coefficient.clone(),
            numerator,
        ));
    }

    let reduced = ReducedExactCircuit {
        source_combination,
        pivot_guards,
    };
    super::replay::replay_exact_circuit(context, hit, partition, &fixed_indices, reduced, limits)
}

/// Canonical singleton coordinates of the exact decorated stratum.
///
/// These are semantic domain restrictions, not values sampled by the modular
/// probe.  A maximal sector stratum therefore normally returns an empty list;
/// only an explicitly tightened face/ray may enter the corresponding exact
/// Symbolica quotient.
pub(super) fn fixed_index_assignments(
    context: &IndexedCoefficientContext,
    partition: &TargetColumnPartition<'_>,
) -> Result<Vec<(usize, i64)>, ExactCircuitError> {
    if partition.stratum().domain().arity() != context.index_count() {
        return Err(ExactCircuitError::Invariant {
            detail: "decorated stratum arity differs from the indexed coefficient context",
        });
    }
    let mut fixed = try_vec(
        "exact-circuit fixed index assignments",
        context.index_count(),
    )?;
    fixed.extend(partition.stratum().singleton_index_assignments());
    Ok(fixed)
}

fn validate_binding(
    context: &IndexedCoefficientContext,
    hit: &ModularHit<'_>,
    partition: &TargetColumnPartition<'_>,
    plan: &PhysicalFramePlan,
) -> Result<(), ExactCircuitError> {
    if !std::ptr::eq(hit.plan(), plan) {
        return Err(ExactCircuitError::ForeignFrameHit);
    }
    if !partition
        .try_verify()
        .map_err(|_| ExactCircuitError::PartitionVerificationFailed)?
    {
        return Err(ExactCircuitError::PartitionVerificationFailed);
    }
    if plan.context_fingerprint() != context.fingerprint() {
        return Err(ExactCircuitError::WrongIndexedContext { row: 0 });
    }
    if hit.diagnostics().target_column != partition.target_column() {
        return Err(ExactCircuitError::TargetMismatch {
            hit: hit.diagnostics().target_column,
            partition: partition.target_column(),
        });
    }
    if hit.diagnostics().forbidden_columns.as_ref() != partition.forbidden_columns() {
        return Err(ExactCircuitError::ForbiddenColumnsMismatch);
    }
    Ok(())
}

fn validate_modular_hit_shape(
    hit: &ModularHit<'_>,
    plan: &PhysicalFramePlan,
    limits: ExactCircuitLimits,
) -> Result<(), ExactCircuitError> {
    let diagnostics = hit.diagnostics();
    let selected = diagnostics.augmented_independent_source_rows.as_ref();
    if diagnostics.augmented_rank != diagnostics.forbidden_rank.saturating_add(1)
        || selected.len() != diagnostics.augmented_rank
    {
        return Err(ExactCircuitError::InvalidModularHitRanks {
            forbidden_rank: diagnostics.forbidden_rank,
            augmented_rank: diagnostics.augmented_rank,
            selected_rows: selected.len(),
        });
    }
    validate_selected_rows(plan, selected, limits)
}

fn validate_selected_rows(
    plan: &PhysicalFramePlan,
    selected: &[usize],
    limits: ExactCircuitLimits,
) -> Result<(), ExactCircuitError> {
    check_limit(SELECTED_ROWS, selected.len(), limits.max_selected_rows)?;
    check_limit(
        "exact-circuit source combination",
        selected.len(),
        limits.max_source_combination_terms,
    )?;
    if selected.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ExactCircuitError::SelectedSourceRowsNotStrictlyIncreasing);
    }
    for &row in selected {
        if row >= plan.row_count() {
            return Err(ExactCircuitError::SelectedSourceRowOutOfRange {
                row,
                rows: plan.row_count(),
            });
        }
    }
    Ok(())
}

fn preflight(
    context: &IndexedCoefficientContext,
    plan: &PhysicalFramePlan,
    partition: &TargetColumnPartition<'_>,
    selected: &[usize],
    limits: ExactCircuitLimits,
) -> Result<(), ExactCircuitError> {
    check_limit(
        "exact-circuit physical columns",
        plan.columns().len(),
        limits.max_physical_columns,
    )?;
    let projected_physical = checked_add(
        PROJECTED_PHYSICAL_COLUMNS,
        partition.forbidden_columns().len(),
        1,
    )?;
    check_limit(
        PROJECTED_PHYSICAL_COLUMNS,
        projected_physical,
        limits.max_projected_physical_columns,
    )?;
    let augmented = checked_add(AUGMENTED_COLUMNS, projected_physical, selected.len())?;
    check_limit(AUGMENTED_COLUMNS, augmented, limits.max_augmented_columns)?;
    checked_u32(AUGMENTED_COLUMNS, augmented)?;

    // U retains at most R*(P+R) entries and full L at most R*R entries.
    let p_plus_two_r = checked_add(
        NATIVE_DECOMPOSITION,
        projected_physical,
        checked_mul(NATIVE_DECOMPOSITION, selected.len(), 2)?,
    )?;
    let native_bound = checked_mul(NATIVE_DECOMPOSITION, selected.len(), p_plus_two_r)?;
    check_limit(
        NATIVE_DECOMPOSITION,
        native_bound,
        limits.max_native_decomposition_nonzero_entries,
    )?;
    let dependency_bound = checked_mul(PIVOT_DEPENDENCIES, selected.len(), selected.len())?;
    check_limit(
        PIVOT_DEPENDENCIES,
        dependency_bound,
        limits.max_pivot_dependency_entries,
    )?;

    let mut projected_entries = selected.len(); // one provenance unit per row
    let mut replay_terms = 0usize;
    let mut source_conditions = 0usize;
    let mut condition_sources = 0usize;
    for &row in selected {
        let source =
            plan.source_for_row(row)
                .ok_or(ExactCircuitError::SelectedSourceRowOutOfRange {
                    row,
                    rows: plan.row_count(),
                })?;
        let structural = plan
            .column_indices_for_row(row)
            .ok_or(ExactCircuitError::Invariant {
                detail: "selected source has invalid physical CSR bounds during preflight",
            })?;
        if structural.len() != source.terms().len() {
            return Err(ExactCircuitError::Invariant {
                detail: "selected source terms disagree with physical CSR during preflight",
            });
        }
        projected_entries = checked_add(
            PROJECTED_INPUT_ENTRIES,
            projected_entries,
            projected_entry_count(structural, partition)?,
        )?;
        replay_terms = checked_add(REPLAY_SOURCE_TERMS, replay_terms, source.terms().len())?;
        source_conditions = checked_add(
            GUARD_ORIGINS,
            source_conditions,
            source.nonzero_conditions().len(),
        )?;
        for (condition, nonzero) in source.nonzero_conditions().iter().enumerate() {
            context
                .validate_polynomial_context(nonzero.polynomial())
                .map_err(|_| ExactCircuitError::WrongIndexedContext { row })?;
            context.validate_polynomial_with_limits(
                nonzero.polynomial(),
                limits.indexed_algebra.exact_algebra,
            )?;
            if nonzero.polynomial().is_zero() {
                return Err(ExactCircuitError::IdenticallyZeroSourceCondition { row, condition });
            }
            condition_sources = checked_add(
                CONDITION_SOURCES,
                condition_sources,
                nonzero.sources().len(),
            )?;
        }
        for coefficient in source.terms().values() {
            context
                .bind_sealed(coefficient)
                .map_err(|_| ExactCircuitError::WrongIndexedContext { row })?;
        }
    }
    check_limit(
        PROJECTED_INPUT_ENTRIES,
        projected_entries,
        limits.max_projected_input_nonzero_entries,
    )?;
    check_limit(
        REPLAY_SOURCE_TERMS,
        replay_terms,
        limits.max_replay_source_terms,
    )?;
    check_limit(
        CONDITION_SOURCES,
        condition_sources,
        limits.max_condition_source_entries,
    )?;
    let replay_operations = checked_add(
        REPLAY_EXACT_OPERATIONS,
        checked_mul(REPLAY_EXACT_OPERATIONS, replay_terms, 2)?,
        plan.columns().len(),
    )?;
    check_limit(
        REPLAY_EXACT_OPERATIONS,
        replay_operations,
        limits.max_replay_exact_operations,
    )?;
    check_limit(
        "exact-circuit residual terms",
        partition.allowed_columns().len(),
        limits.max_circuit_terms,
    )?;
    let mut dependency_owner_witnesses = 0usize;
    for descriptor in partition.allowed_columns() {
        dependency_owner_witnesses = checked_add(
            DEPENDENCY_OWNER_WITNESSES,
            dependency_owner_witnesses,
            descriptor.proper_subsector_owners().len(),
        )?;
    }
    check_limit(
        DEPENDENCY_OWNER_WITNESSES,
        dependency_owner_witnesses,
        limits.max_dependency_owner_witnesses,
    )?;
    let guard_origin_bound = checked_add(
        GUARD_ORIGINS,
        checked_add(GUARD_ORIGINS, source_conditions, replay_terms)?,
        checked_add(
            GUARD_ORIGINS,
            checked_mul(GUARD_ORIGINS, selected.len(), 3)?,
            partition.allowed_columns().len(),
        )?,
    )?;
    check_limit(GUARD_ORIGINS, guard_origin_bound, limits.max_guard_origins)?;
    check_limit(
        "exact-circuit conservative guards",
        guard_origin_bound,
        limits.max_guards,
    )
}

fn projected_entry_count(
    structural_columns: &[u32],
    partition: &TargetColumnPartition<'_>,
) -> Result<usize, ExactCircuitError> {
    let mut count = 0usize;
    for &column in structural_columns {
        let column = usize::try_from(column).map_err(|_| ExactCircuitError::Invariant {
            detail: "physical source column does not fit usize",
        })?;
        if column == partition.target_column()
            || partition.forbidden_columns().binary_search(&column).is_ok()
        {
            count = checked_add(PROJECTED_INPUT_ENTRIES, count, 1)?;
        }
    }
    Ok(count)
}

fn projected_to_physical(
    projected: usize,
    partition: &TargetColumnPartition<'_>,
) -> Result<usize, ExactCircuitError> {
    if projected < partition.forbidden_columns().len() {
        Ok(partition.forbidden_columns()[projected])
    } else if projected == partition.forbidden_columns().len() {
        Ok(partition.target_column())
    } else {
        Err(ExactCircuitError::Invariant {
            detail: "exact physical pivot is outside [F,target]",
        })
    }
}

fn validate_complete_reducer(
    reducer: &SparseRowReducer<NativeField>,
    metadata: &[ExactForwardRowMeta],
    selected_rows: usize,
    augmented_columns: usize,
) -> Result<(), ExactCircuitError> {
    let rank = reducer.u().nrows() as usize;
    let pivots = reducer.pivots().iter().flatten().count();
    if reducer.u().ncols() as usize != augmented_columns
        || reducer.pivots().len() != augmented_columns
        || rank != selected_rows
        || pivots != selected_rows
        || metadata.len() != selected_rows
        || metadata
            .iter()
            .enumerate()
            .any(|(row, meta)| meta.reducer_row != row)
    {
        return Err(ExactCircuitError::Invariant {
            detail: "exact reducer shape does not retain the full selected modular minor",
        });
    }
    Ok(())
}

fn call_native<T>(
    operation: &'static str,
    callback: impl FnOnce() -> T,
) -> Result<T, ExactCircuitError> {
    catch_unwind(AssertUnwindSafe(callback))
        .map_err(|_| ExactCircuitError::NativePanic { operation })
}

pub(super) fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ExactCircuitError> {
    left.checked_add(right)
        .ok_or(ExactCircuitError::ResourceCountOverflow { resource })
}

pub(super) fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ExactCircuitError> {
    left.checked_mul(right)
        .ok_or(ExactCircuitError::ResourceCountOverflow { resource })
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ExactCircuitError> {
    if requested > limit {
        Err(ExactCircuitError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

pub(super) fn checked_u32(resource: &'static str, value: usize) -> Result<u32, ExactCircuitError> {
    u32::try_from(value).map_err(|_| ExactCircuitError::U32NotRepresentable { resource, value })
}

pub(super) fn try_vec<T>(
    resource: &'static str,
    capacity: usize,
) -> Result<Vec<T>, ExactCircuitError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| ExactCircuitError::AllocationFailure {
            resource,
            requested: capacity,
        })?;
    Ok(values)
}
