use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::domains::SelfRing;
use symbolica::prelude::Z;
use symbolica::tensors::sparse::{LuLMode, SparseRowReducer};

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext};
use crate::foundry::target_rref::{
    self, BackSubstitutionLimits, Error as TargetRrefError, ForwardReducerRowMeta, NativeField,
};

use super::error::ParametricRuleError;
use super::limits::ParametricRuleLimits;
use super::model::{ParametricReducerPivotGuard, ParametricSourceRowContribution};
use super::prepare::{PreparedProblem, check_limit, checked_add, try_vec};

pub(super) struct ReducedRuleRow {
    pub(super) pivot_column: usize,
    pub(super) shift_entries: Vec<(usize, IndexedCoefficient)>,
    pub(super) source_combination: Vec<ParametricSourceRowContribution>,
    pub(super) pivot_guards: Vec<ParametricReducerPivotGuard>,
}

type ReducerRowMeta = ForwardReducerRowMeta<IndexedCoefficient>;

#[derive(Clone, Copy)]
enum RowSelection {
    FirstDescending,
    Target { shift_column: usize },
}

pub(super) fn reduce_rows(
    context: &IndexedCoefficientContext,
    problem: &PreparedProblem,
    limits: ParametricRuleLimits,
) -> Result<ReducedRuleRow, ParametricRuleError> {
    reduce_rows_with_selection(context, problem, limits, RowSelection::FirstDescending)
}

pub(super) fn reduce_rows_for_target(
    context: &IndexedCoefficientContext,
    problem: &PreparedProblem,
    target_shift_column: usize,
    limits: ParametricRuleLimits,
) -> Result<ReducedRuleRow, ParametricRuleError> {
    reduce_rows_with_selection(
        context,
        problem,
        limits,
        RowSelection::Target {
            shift_column: target_shift_column,
        },
    )
}

fn reduce_rows_with_selection(
    context: &IndexedCoefficientContext,
    problem: &PreparedProblem,
    limits: ParametricRuleLimits,
    selection: RowSelection,
) -> Result<ReducedRuleRow, ParametricRuleError> {
    let shift_columns = problem.columns.len();
    let augmented_columns = checked_add(
        "parametric augmented columns",
        shift_columns,
        problem.sources.len(),
    )?;
    let native_columns =
        u32::try_from(augmented_columns).map_err(|_| ParametricRuleError::ReducerInvariant {
            detail: "admitted sparse column count does not fit u32",
        })?;
    let field = NativeField::new(Z);
    let mut reducer = call_native(
        "constructing Symbolica's indexed sparse row reducer",
        || SparseRowReducer::new(native_columns, field, LuLMode::Full),
    )?;
    let mut metadata: Vec<ReducerRowMeta> = try_vec(
        "parametric sparse reducer row metadata",
        problem.sources.len(),
    )?;
    let mut retained_pivot_dependency_entries = 0usize;

    for (source_ordinal, source) in problem.sources.iter().enumerate() {
        let row_weight = checked_add(
            "identity-augmented parametric source row entries",
            source.entries.len(),
            1,
        )?;
        let mut values = try_vec("identity-augmented indexed coefficients", row_weight)?;
        let mut columns = try_vec("identity-augmented parametric columns", row_weight)?;
        for (column, coefficient) in &source.entries {
            values.push(coefficient.raw().clone());
            columns.push(*column);
        }
        values.push(context.one().raw().clone());
        let provenance_column = checked_add(
            "parametric provenance column",
            shift_columns,
            source_ordinal,
        )?;
        columns.push(u32::try_from(provenance_column).map_err(|_| {
            ParametricRuleError::ReducerInvariant {
                detail: "admitted provenance column does not fit u32",
            }
        })?);

        let pivot = call_native(
            "adding an indexed row to Symbolica's sparse reducer",
            || reducer.add_row(&values, &columns),
        )?
        .ok_or(ParametricRuleError::ReducerRejectedChronologicalRow { source_ordinal })?;
        let (lower_row, lower_columns, lower_values) =
            reducer
                .l()
                .last_row()
                .ok_or(ParametricRuleError::ReducerInvariant {
                    detail: "L has no row after an accepted chronological input",
                })?;
        let reducer_row =
            reducer
                .u()
                .nrows()
                .checked_sub(1)
                .ok_or(ParametricRuleError::ReducerInvariant {
                    detail: "U has no row after an accepted chronological input",
                })?;
        let (_, upper_columns, _) =
            reducer
                .u()
                .last_row()
                .ok_or(ParametricRuleError::ReducerInvariant {
                    detail: "U has no last row after an accepted chronological input",
                })?;
        let has_trailing_physical_entry = upper_columns.iter().any(|&column| {
            let column = column as usize;
            column > pivot as usize && column < shift_columns
        });
        let native_source_ordinal =
            u32::try_from(source_ordinal).map_err(|_| ParametricRuleError::ReducerInvariant {
                detail: "an admitted source ordinal does not fit u32",
            })?;
        if lower_row != native_source_ordinal || lower_columns.last().copied() != Some(reducer_row)
        {
            return Err(ParametricRuleError::ReducerInvariant {
                detail: "L does not retain its chronological diagonal entry",
            });
        }
        let pivot_raw =
            lower_values
                .last()
                .cloned()
                .ok_or(ParametricRuleError::ReducerInvariant {
                    detail: "L chronological row has no pivot coefficient",
                })?;
        if pivot_raw.is_zero() {
            return Err(ParametricRuleError::ReducerInvariant {
                detail: "L retained an identically zero indexed pivot coefficient",
            });
        }
        let pivot_coefficient = context
            .admit_native_result_with_limits(pivot_raw, limits.indexed_algebra.exact_algebra)?;

        let dependency_capacity = lower_columns[..lower_columns.len() - 1].iter().try_fold(
            1usize,
            |capacity, dependency| {
                let dependency = usize::try_from(*dependency).map_err(|_| {
                    ParametricRuleError::ReducerInvariant {
                        detail: "an L dependency row does not fit usize",
                    }
                })?;
                let dependency =
                    metadata
                        .get(dependency)
                        .ok_or(ParametricRuleError::ReducerInvariant {
                            detail: "L refers to a reducer row outside the chronology",
                        })?;
                checked_add(
                    "parametric elimination pivot dependencies",
                    capacity,
                    dependency.pivot_dependencies.len(),
                )
            },
        )?;
        check_limit(
            "parametric elimination pivot dependencies",
            dependency_capacity,
            limits.max_elimination_pivots,
        )?;
        let mut pivot_dependencies = try_vec(
            "parametric elimination pivot dependencies",
            dependency_capacity,
        )?;
        for dependency in &lower_columns[..lower_columns.len() - 1] {
            let dependency = usize::try_from(*dependency).map_err(|_| {
                ParametricRuleError::ReducerInvariant {
                    detail: "an L dependency row does not fit usize",
                }
            })?;
            pivot_dependencies.extend_from_slice(&metadata[dependency].pivot_dependencies);
        }
        pivot_dependencies.sort_unstable();
        pivot_dependencies.dedup();
        pivot_dependencies.push(metadata.len());
        check_limit(
            "parametric elimination pivots",
            pivot_dependencies.len(),
            limits.max_elimination_pivots,
        )?;
        retained_pivot_dependency_entries = checked_add(
            "aggregate parametric elimination pivot dependencies",
            retained_pivot_dependency_entries,
            pivot_dependencies.len(),
        )?;
        check_limit(
            "aggregate parametric elimination pivot dependencies",
            retained_pivot_dependency_entries,
            limits.max_elimination_pivot_dependency_entries,
        )?;
        metadata.push(ReducerRowMeta {
            source_ordinal,
            reducer_row,
            pivot_column: pivot as usize,
            pivot_coefficient,
            pivot_dependencies,
            has_trailing_physical_entry,
        });

        let decomposition_nonzeros = checked_add(
            "Symbolica indexed sparse U/L nonzero entries",
            reducer.u().nvalues(),
            reducer.l().nvalues(),
        )?;
        check_limit(
            "Symbolica indexed sparse U/L nonzero entries",
            decomposition_nonzeros,
            limits.max_native_decomposition_nonzero_entries,
        )?;
    }

    let mut targeted_dependencies = None;
    let (candidate_pivot_column, candidate_reducer_row, dependency_owner, targeted) =
        match selection {
            RowSelection::FirstDescending => {
                let candidate = metadata
                    .iter()
                    .filter(|meta| {
                        meta.pivot_column < shift_columns && meta.has_trailing_physical_entry
                    })
                    .min_by_key(|meta| (meta.pivot_column, meta.source_ordinal))
                    .ok_or(ParametricRuleError::NoStrictlyDescendingRule)?;
                let candidate_reducer_row = candidate.reducer_row as usize;
                if candidate.pivot_dependencies.last().copied() != Some(candidate_reducer_row) {
                    return Err(ParametricRuleError::ReducerInvariant {
                        detail: "the chosen indexed pivot is not last in its dependency chronology",
                    });
                }
                (
                    candidate.pivot_column,
                    candidate_reducer_row,
                    Some(candidate_reducer_row),
                    false,
                )
            }
            RowSelection::Target { shift_column } => {
                let forward_row = reducer
                    .pivots()
                    .get(shift_column)
                    .copied()
                    .flatten()
                    .ok_or(ParametricRuleError::TargetShiftNotPivot)?
                    as usize;
                let forward_meta = metadata.get(forward_row).ok_or(
                    ParametricRuleError::ReducerInvariant {
                        detail: "a target indexed pivot refers to a reducer row outside the chronology",
                    },
                )?;
                if forward_meta.pivot_column != shift_column {
                    return Err(ParametricRuleError::ReducerInvariant {
                        detail: "the target indexed pivot map disagrees with reducer metadata",
                    });
                }
                targeted_dependencies = Some(
                    target_rref::pivot_dependencies(
                        &reducer,
                        &metadata,
                        forward_row,
                        shift_columns,
                        limits.max_elimination_pivots,
                    )
                    .map_err(map_target_rref_error)?,
                );
                let target_limits = BackSubstitutionLimits {
                    max_output_nonzero_entries: limits.max_back_substitution_output_nonzero_entries,
                    max_live_nonzero_entries: limits.max_back_substitution_live_nonzero_entries,
                };
                let (admission, physical_u, physical_pivots) =
                    target_rref::admit_back_substitution(
                        &reducer,
                        shift_columns,
                        augmented_columns,
                        target_limits,
                    )
                    .map_err(map_target_rref_error)?;
                let mut physical_reducer = call_native(
                    "constructing Symbolica's indexed physical target upper-triangular system",
                    || SparseRowReducer::from_upper_triangular_matrix(physical_u, physical_pivots),
                )?;
                call_native(
                    "serially back-substituting Symbolica's indexed physical sparse row reducer",
                    || physical_reducer.back_substitute(),
                )?;
                let output_nonzeros = target_rref::postvalidate_back_substitution(
                    &physical_reducer,
                    augmented_columns,
                    admission,
                    target_limits,
                )
                .map_err(map_target_rref_error)?;
                check_limit(
                    "Symbolica indexed sparse U/L nonzero entries",
                    output_nonzeros,
                    limits.max_native_decomposition_nonzero_entries,
                )?;
                let target_row = physical_reducer
                    .pivots()
                    .get(shift_column)
                    .copied()
                    .flatten()
                    .ok_or(ParametricRuleError::ReducerInvariant {
                        detail: "serial back-substitution lost the requested indexed target pivot",
                    })? as usize;
                reducer = physical_reducer;
                (shift_column, target_row, None, true)
            }
        };
    let candidate_dependencies = match (targeted_dependencies.as_deref(), dependency_owner) {
        (Some(dependencies), None) => dependencies,
        (None, Some(owner)) => metadata
            .get(owner)
            .ok_or(ParametricRuleError::ReducerInvariant {
                detail: "the chosen indexed dependency owner is outside the chronology",
            })?
            .pivot_dependencies
            .as_slice(),
        _ => {
            return Err(ParametricRuleError::ReducerInvariant {
                detail: "the indexed reducer selection has ambiguous dependency ownership",
            });
        }
    };

    let (_, columns, values) = reducer.u().row_iter().nth(candidate_reducer_row).ok_or(
        ParametricRuleError::ReducerInvariant {
            detail: "chosen indexed reducer row is absent from U",
        },
    )?;
    let mut shift_entries = try_vec("reduced parametric shift entries", columns.len())?;
    let mut source_combination = try_vec(
        "chronological indexed source-row combination",
        columns.len().min(problem.sources.len()),
    )?;
    for (&column, raw) in columns.iter().zip(values) {
        if raw.is_zero() {
            return Err(ParametricRuleError::ReducerInvariant {
                detail: "U exposes an explicit zero indexed sparse entry",
            });
        }
        let coefficient = context
            .admit_native_result_with_limits(raw.clone(), limits.indexed_algebra.exact_algebra)?;
        let column = column as usize;
        if targeted
            && column != candidate_pivot_column
            && reducer.pivots().get(column).copied().flatten().is_some()
        {
            return Err(ParametricRuleError::ReducerInvariant {
                detail: "the target indexed RREF row retains a distinct reducer pivot column",
            });
        }
        if column < shift_columns {
            if column < candidate_pivot_column {
                return Err(ParametricRuleError::ReducerInvariant {
                    detail: "U contains a shift left of its declared pivot",
                });
            }
            shift_entries.push((column, coefficient));
        } else {
            let source_ordinal = column - shift_columns;
            let source = problem.sources.get(source_ordinal).ok_or(
                ParametricRuleError::ReducerInvariant {
                    detail: "U provenance column is outside the source chronology",
                },
            )?;
            if source_combination.last().is_some_and(
                |previous: &ParametricSourceRowContribution| {
                    previous.source_ordinal() >= source_ordinal
                },
            ) {
                return Err(ParametricRuleError::ReducerInvariant {
                    detail: "U indexed provenance is not in source chronology",
                });
            }
            source_combination.push(ParametricSourceRowContribution::new(
                source_ordinal,
                source.row_id.clone(),
                coefficient,
            ));
        }
    }
    check_limit(
        "indexed source-row combination terms",
        source_combination.len(),
        limits.max_source_combination_terms,
    )?;
    if source_combination.is_empty() {
        return Err(ParametricRuleError::ReducerInvariant {
            detail: "a reduced indexed row has no source provenance",
        });
    }
    if shift_entries.first().map(|entry| entry.0) != Some(candidate_pivot_column)
        || !shift_entries[0].1.raw().is_one()
    {
        return Err(ParametricRuleError::ReducerInvariant {
            detail: "U indexed pivot is not the normalized leading unit",
        });
    }
    if shift_entries.len() < 2 {
        return Err(if targeted {
            ParametricRuleError::TargetHasNoUniformlyDescendingRule
        } else {
            ParametricRuleError::NoStrictlyDescendingRule
        });
    }

    let mut pivot_guards = try_vec(
        "parametric elimination pivot guards",
        candidate_dependencies.len(),
    )?;
    for &dependency in candidate_dependencies {
        let dependency = metadata
            .get(dependency)
            .ok_or(ParametricRuleError::ReducerInvariant {
                detail: "a chosen indexed pivot dependency is outside reducer metadata",
            })?;
        let pivot_shift = problem
            .columns
            .get(dependency.pivot_column)
            .ok_or(ParametricRuleError::ReducerInvariant {
                detail: "an indexed physical pivot is outside the ordered shift columns",
            })?
            .shift
            .clone();
        let pivot_coefficient = context.bind_sealed(&dependency.pivot_coefficient)?;
        let numerator = context.numerator_condition_from_bound(pivot_coefficient)?;
        pivot_guards.push(ParametricReducerPivotGuard::new(
            dependency.source_ordinal,
            problem.sources[dependency.source_ordinal].row_id.clone(),
            dependency.pivot_column,
            pivot_shift,
            dependency.pivot_coefficient.clone(),
            numerator,
        ));
    }

    Ok(ReducedRuleRow {
        pivot_column: candidate_pivot_column,
        shift_entries,
        source_combination,
        pivot_guards,
    })
}

fn call_native<T>(
    operation: &'static str,
    callback: impl FnOnce() -> T,
) -> Result<T, ParametricRuleError> {
    catch_unwind(AssertUnwindSafe(callback))
        .map_err(|_| ParametricRuleError::NativePanic { operation })
}

fn map_target_rref_error(error: TargetRrefError) -> ParametricRuleError {
    match error {
        TargetRrefError::ResourceCountOverflow { resource } => {
            ParametricRuleError::ResourceCountOverflow { resource }
        }
        TargetRrefError::ResourceLimit {
            resource,
            requested,
            limit,
        } => ParametricRuleError::ResourceLimit {
            resource,
            requested,
            limit,
        },
        TargetRrefError::AllocationFailure {
            resource,
            requested,
        } => ParametricRuleError::AllocationFailure {
            resource,
            requested,
        },
        TargetRrefError::Invariant { detail } => ParametricRuleError::ReducerInvariant { detail },
    }
}
