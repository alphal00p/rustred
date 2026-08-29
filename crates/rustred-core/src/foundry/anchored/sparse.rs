use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::domains::SelfRing;
use symbolica::domains::rational_polynomial::RationalPolynomialField;
use symbolica::prelude::{IntegerRing, Z};
use symbolica::tensors::sparse::{LuLMode, SparseRowReducer};

use crate::algebra::{Coefficient, IndexedCoefficientContext};

use super::error::AnchoredRuleError;
use super::limits::AnchoredRuleLimits;
use super::model::{ReducerPivotGuard, SourceRowContribution};
use super::prepare::{PreparedProblem, check_limit, checked_add, try_vec};

pub(super) struct ReducedRuleRow {
    pub(super) pivot_column: usize,
    pub(super) integral_entries: Vec<(usize, Coefficient)>,
    pub(super) source_combination: Vec<SourceRowContribution>,
    pub(super) pivot_guards: Vec<ReducerPivotGuard>,
}

struct ReducerRowMeta {
    source_ordinal: usize,
    reducer_row: u32,
    pivot_column: usize,
    pivot_coefficient: Coefficient,
    pivot_dependencies: Vec<usize>,
}

pub(super) fn reduce_rows(
    context: &IndexedCoefficientContext,
    problem: &PreparedProblem,
    limits: AnchoredRuleLimits,
) -> Result<ReducedRuleRow, AnchoredRuleError> {
    let integral_columns = problem.columns.len();
    let augmented_columns = checked_add(
        "anchored augmented columns",
        integral_columns,
        problem.sources.len(),
    )?;
    let native_columns =
        u32::try_from(augmented_columns).map_err(|_| AnchoredRuleError::ReducerInvariant {
            detail: "admitted sparse column count does not fit u32",
        })?;
    let field = RationalPolynomialField::<IntegerRing, u16>::new(Z);
    let mut reducer = call_native("constructing Symbolica's sparse row reducer", || {
        SparseRowReducer::new(native_columns, field, LuLMode::Full)
    })?;
    let mut metadata: Vec<ReducerRowMeta> =
        try_vec("sparse reducer row metadata", problem.sources.len())?;

    for (source_ordinal, source) in problem.sources.iter().enumerate() {
        let row_weight = checked_add(
            "identity-augmented source row entries",
            source.entries.len(),
            1,
        )?;
        let mut values = try_vec("identity-augmented source coefficients", row_weight)?;
        let mut columns = try_vec("identity-augmented source columns", row_weight)?;
        for (column, coefficient) in &source.entries {
            values.push(coefficient.clone());
            columns.push(*column);
        }
        values.push(context.base().one());
        let provenance_column = checked_add(
            "anchored provenance column",
            integral_columns,
            source_ordinal,
        )?;
        columns.push(u32::try_from(provenance_column).map_err(|_| {
            AnchoredRuleError::ReducerInvariant {
                detail: "admitted provenance column does not fit u32",
            }
        })?);

        let pivot = call_native("adding a row to Symbolica's sparse row reducer", || {
            reducer.add_row(&values, &columns)
        })?
        .ok_or(AnchoredRuleError::ReducerRejectedChronologicalRow { source_ordinal })?;
        let (lower_row, lower_columns, lower_values) =
            reducer
                .l()
                .last_row()
                .ok_or(AnchoredRuleError::ReducerInvariant {
                    detail: "L has no row after an accepted chronological input",
                })?;
        let reducer_row =
            reducer
                .u()
                .nrows()
                .checked_sub(1)
                .ok_or(AnchoredRuleError::ReducerInvariant {
                    detail: "U has no row after an accepted chronological input",
                })?;
        let native_source_ordinal =
            u32::try_from(source_ordinal).map_err(|_| AnchoredRuleError::ReducerInvariant {
                detail: "an admitted source ordinal does not fit u32",
            })?;
        if lower_row != native_source_ordinal || lower_columns.last().copied() != Some(reducer_row)
        {
            return Err(AnchoredRuleError::ReducerInvariant {
                detail: "L does not retain its chronological diagonal entry",
            });
        }
        let pivot_coefficient =
            lower_values
                .last()
                .cloned()
                .ok_or(AnchoredRuleError::ReducerInvariant {
                    detail: "L chronological row has no pivot coefficient",
                })?;
        if pivot_coefficient.is_zero() {
            return Err(AnchoredRuleError::ReducerInvariant {
                detail: "L retained an identically zero pivot coefficient",
            });
        }
        context
            .base()
            .validate_with_limits(&pivot_coefficient, limits.indexed_algebra.exact_algebra)?;
        let dependency_capacity = lower_columns[..lower_columns.len() - 1].iter().try_fold(
            1usize,
            |capacity, dependency| {
                let dependency = usize::try_from(*dependency).map_err(|_| {
                    AnchoredRuleError::ReducerInvariant {
                        detail: "an L dependency row does not fit usize",
                    }
                })?;
                let dependency =
                    metadata
                        .get(dependency)
                        .ok_or(AnchoredRuleError::ReducerInvariant {
                            detail: "L refers to a reducer row outside the chronology",
                        })?;
                checked_add(
                    "anchored elimination pivot dependencies",
                    capacity,
                    dependency.pivot_dependencies.len(),
                )
            },
        )?;
        check_limit(
            "anchored elimination pivot dependencies",
            dependency_capacity,
            limits.max_elimination_pivots,
        )?;
        let mut pivot_dependencies = try_vec(
            "anchored elimination pivot dependencies",
            dependency_capacity,
        )?;
        for dependency in &lower_columns[..lower_columns.len() - 1] {
            let dependency =
                usize::try_from(*dependency).map_err(|_| AnchoredRuleError::ReducerInvariant {
                    detail: "an L dependency row does not fit usize",
                })?;
            pivot_dependencies.extend_from_slice(&metadata[dependency].pivot_dependencies);
        }
        pivot_dependencies.sort_unstable();
        pivot_dependencies.dedup();
        pivot_dependencies.push(metadata.len());
        check_limit(
            "anchored elimination pivots",
            pivot_dependencies.len(),
            limits.max_elimination_pivots,
        )?;
        metadata.push(ReducerRowMeta {
            source_ordinal,
            reducer_row,
            pivot_column: pivot as usize,
            pivot_coefficient,
            pivot_dependencies,
        });

        let decomposition_nonzeros = checked_add(
            "Symbolica sparse U/L nonzero entries",
            reducer.u().nvalues(),
            reducer.l().nvalues(),
        )?;
        check_limit(
            "Symbolica sparse U/L nonzero entries",
            decomposition_nonzeros,
            limits.max_native_decomposition_nonzero_entries,
        )?;
    }

    let candidate = metadata
        .iter()
        .filter(|meta| {
            meta.pivot_column < integral_columns
                && reducer
                    .u()
                    .row_iter()
                    .nth(meta.reducer_row as usize)
                    .is_some_and(|(_, columns, _)| {
                        columns.iter().any(|&column| {
                            let column = column as usize;
                            column > meta.pivot_column && column < integral_columns
                        })
                    })
        })
        .min_by_key(|meta| (meta.pivot_column, meta.source_ordinal))
        .ok_or(AnchoredRuleError::NoStrictlyDescendingRule)?;
    if candidate.pivot_dependencies.last().copied() != Some(candidate.reducer_row as usize) {
        return Err(AnchoredRuleError::ReducerInvariant {
            detail: "the chosen reducer pivot is not last in its dependency chronology",
        });
    }

    let (_, columns, values) = reducer
        .u()
        .row_iter()
        .nth(candidate.reducer_row as usize)
        .ok_or(AnchoredRuleError::ReducerInvariant {
            detail: "chosen reducer row is absent from U",
        })?;
    let mut integral_entries = try_vec("reduced integral entries", columns.len())?;
    let mut source_combination = try_vec(
        "chronological source-row combination",
        columns.len().min(problem.sources.len()),
    )?;
    for (&column, coefficient) in columns.iter().zip(values) {
        if coefficient.is_zero() {
            return Err(AnchoredRuleError::ReducerInvariant {
                detail: "U exposes an explicit zero sparse entry",
            });
        }
        context
            .base()
            .validate_with_limits(coefficient, limits.indexed_algebra.exact_algebra)?;
        let column = column as usize;
        if column < integral_columns {
            if column < candidate.pivot_column {
                return Err(AnchoredRuleError::ReducerInvariant {
                    detail: "U contains an integral left of its declared pivot",
                });
            }
            integral_entries.push((column, coefficient.clone()));
        } else {
            let source_ordinal = column - integral_columns;
            let source =
                problem
                    .sources
                    .get(source_ordinal)
                    .ok_or(AnchoredRuleError::ReducerInvariant {
                        detail: "U provenance column is outside the source chronology",
                    })?;
            if source_combination
                .last()
                .is_some_and(|previous: &SourceRowContribution| {
                    previous.source_ordinal() >= source_ordinal
                })
            {
                return Err(AnchoredRuleError::ReducerInvariant {
                    detail: "U provenance is not in source chronology",
                });
            }
            source_combination.push(SourceRowContribution::new(
                source_ordinal,
                source.row_id.clone(),
                coefficient.clone(),
            ));
        }
    }
    check_limit(
        "source-row combination terms",
        source_combination.len(),
        limits.max_source_combination_terms,
    )?;
    if source_combination.is_empty() {
        return Err(AnchoredRuleError::ReducerInvariant {
            detail: "a reduced physical row has no source provenance",
        });
    }
    if integral_entries.first().map(|entry| entry.0) != Some(candidate.pivot_column)
        || !integral_entries[0].1.is_one()
    {
        return Err(AnchoredRuleError::ReducerInvariant {
            detail: "U physical pivot is not the normalized leading unit",
        });
    }
    if integral_entries.len() < 2 {
        return Err(AnchoredRuleError::NoStrictlyDescendingRule);
    }

    let mut pivot_guards = try_vec(
        "anchored elimination pivot guards",
        candidate.pivot_dependencies.len(),
    )?;
    for &dependency in &candidate.pivot_dependencies {
        let dependency = metadata
            .get(dependency)
            .ok_or(AnchoredRuleError::ReducerInvariant {
                detail: "a chosen pivot dependency is outside reducer metadata",
            })?;
        pivot_guards.push(ReducerPivotGuard::new(
            dependency.source_ordinal,
            problem.sources[dependency.source_ordinal].row_id.clone(),
            dependency.pivot_column,
            dependency.pivot_coefficient.clone(),
        ));
    }

    Ok(ReducedRuleRow {
        pivot_column: candidate.pivot_column,
        integral_entries,
        source_combination,
        pivot_guards,
    })
}

fn call_native<T>(
    operation: &'static str,
    callback: impl FnOnce() -> T,
) -> Result<T, AnchoredRuleError> {
    catch_unwind(AssertUnwindSafe(callback))
        .map_err(|_| AnchoredRuleError::NativePanic { operation })
}
