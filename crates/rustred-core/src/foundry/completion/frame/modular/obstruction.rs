use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::domains::finite_field::Zp64;
use symbolica::domains::{Ring, RingOps};
use symbolica::tensors::sparse::{SparseMatrix, SparseRowReducer};

use super::sample::{check_limit, checked_add, checked_mul, try_vec};
use super::{
    ModularKernelError, ModularKernelLimits, ModularObstructionEntry, ModularPhysicalFrame,
    ModularRankDiagnostics, ModularRightObstruction,
};

const OBSTRUCTION_ENTRIES: &str = "modular right-obstruction nonzero entries";
const BACK_SUBSTITUTION_OUTPUT: &str = "modular obstruction back-substitution output entries";
const BACK_SUBSTITUTION_LIVE: &str = "modular obstruction back-substitution live entries";
const BACK_SUBSTITUTION_FILL_MULTIPLE: &str =
    "modular obstruction back-substitution fill-multiple entries";

#[allow(clippy::too_many_arguments)]
pub(super) fn finish_no_hit<'frame>(
    frame: &ModularPhysicalFrame<'frame>,
    diagnostics: ModularRankDiagnostics,
    logical_physical_columns: Vec<usize>,
    projected: &SparseMatrix<Zp64>,
    reducer: SparseRowReducer<Zp64>,
    projected_input_nonzeros: usize,
    limits: ModularKernelLimits,
) -> Result<ModularRightObstruction<'frame>, ModularKernelError> {
    validate_scope(
        frame,
        &diagnostics,
        &logical_physical_columns,
        projected,
        &reducer,
    )?;

    let entries = checked_entries(projected, reducer, projected_input_nonzeros, limits)?;
    Ok(ModularRightObstruction::from_checked_parts(
        frame.plan(),
        frame.sample_fingerprint().clone(),
        diagnostics,
        logical_physical_columns,
        entries,
    ))
}

pub(super) fn checked_entries(
    projected: &SparseMatrix<Zp64>,
    mut reducer: SparseRowReducer<Zp64>,
    projected_input_nonzeros: usize,
    limits: ModularKernelLimits,
) -> Result<Vec<ModularObstructionEntry>, ModularKernelError> {
    let logical_columns = projected.ncols() as usize;
    if logical_columns == 0
        || reducer.u().ncols() != projected.ncols()
        || reducer.pivots().len() != logical_columns
    {
        return Err(ModularKernelError::Invariant {
            detail: "modular obstruction extraction received an empty or mismatched projection",
        });
    }
    let target_logical_column = logical_columns - 1;
    if reducer.pivots()[target_logical_column].is_some() {
        return Err(ModularKernelError::Invariant {
            detail: "target-last no-hit projection made the target a forward pivot",
        });
    }

    let rank = reducer.u().nrows() as usize;
    let output_bound = checked_mul(BACK_SUBSTITUTION_OUTPUT, rank, logical_columns)?;
    check_limit(
        BACK_SUBSTITUTION_OUTPUT,
        output_bound,
        limits.max_reducer_dense_cells,
    )?;
    let retained_forward = checked_add(
        BACK_SUBSTITUTION_LIVE,
        reducer.u().nvalues(),
        reducer.l().col_idcs().len(),
    )?;
    // The original logical projection remains live for exact residual replay
    // while Symbolica owns its forward U/L data and builds a fresh reduced U.
    // Charge all three sparse representations before entering native code.
    let retained_projection_and_forward = checked_add(
        BACK_SUBSTITUTION_LIVE,
        projected_input_nonzeros,
        retained_forward,
    )?;
    let live_bound = checked_add(
        BACK_SUBSTITUTION_LIVE,
        retained_projection_and_forward,
        output_bound,
    )?;
    check_limit(
        BACK_SUBSTITUTION_LIVE,
        live_bound,
        limits.max_reducer_total_fill_entries,
    )?;

    call_native(
        "serially back-substituting the modular target obstruction",
        || reducer.back_substitute(),
    )?;
    postvalidate_reduced_obstruction(
        &reducer,
        target_logical_column,
        projected_input_nonzeros,
        output_bound,
        limits,
    )?;

    let entries = extract_normalized_entries(&reducer, target_logical_column, limits)?;
    verify_normalized_obstruction(projected, target_logical_column, &entries, limits)?;
    Ok(entries)
}

fn validate_scope(
    frame: &ModularPhysicalFrame<'_>,
    diagnostics: &ModularRankDiagnostics,
    logical_physical_columns: &[usize],
    projected: &SparseMatrix<Zp64>,
    reducer: &SparseRowReducer<Zp64>,
) -> Result<(), ModularKernelError> {
    let expected_columns = diagnostics.forbidden_columns.len().checked_add(1).ok_or(
        ModularKernelError::ResourceCountOverflow {
            resource: "modular obstruction logical columns",
        },
    )?;
    if logical_physical_columns.len() != expected_columns
        || logical_physical_columns[..expected_columns - 1] != *diagnostics.forbidden_columns
        || logical_physical_columns[expected_columns - 1] != diagnostics.target_column
        || diagnostics
            .forbidden_columns
            .binary_search(&diagnostics.target_column)
            .is_ok()
    {
        return Err(ModularKernelError::Invariant {
            detail: "modular obstruction logical columns disagree with the target partition",
        });
    }
    if diagnostics.augmented_rank != diagnostics.forbidden_rank {
        return Err(ModularKernelError::Invariant {
            detail: "a rank-separating query entered modular obstruction construction",
        });
    }
    if projected.nrows() != frame.matrix().nrows()
        || projected.ncols() as usize != expected_columns
        || reducer.u().ncols() as usize != expected_columns
        || reducer.pivots().len() != expected_columns
        || reducer.u().nrows() as usize != diagnostics.augmented_rank
        || reducer.pivots().iter().flatten().count() != diagnostics.augmented_rank
    {
        return Err(ModularKernelError::Invariant {
            detail: "modular obstruction reducer shape disagrees with its checked rank query",
        });
    }
    Ok(())
}

fn postvalidate_reduced_obstruction(
    reducer: &SparseRowReducer<Zp64>,
    target_logical_column: usize,
    projected_input_nonzeros: usize,
    output_bound: usize,
    limits: ModularKernelLimits,
) -> Result<(), ModularKernelError> {
    let logical_columns = target_logical_column + 1;
    if reducer.u().ncols() as usize != logical_columns
        || reducer.pivots().len() != logical_columns
        || reducer.l().nrows() != 0
        || reducer.l().ncols() != 0
        || reducer.l().nvalues() != 0
        || reducer.pivots()[target_logical_column].is_some()
    {
        return Err(ModularKernelError::Invariant {
            detail: "modular obstruction back substitution changed shape or target freedom",
        });
    }
    let rank = reducer.u().nrows() as usize;
    if reducer.pivots().iter().flatten().count() != rank {
        return Err(ModularKernelError::Invariant {
            detail: "modular obstruction RREF pivot map is not a row bijection",
        });
    }
    if reducer.u().nvalues() > output_bound {
        return Err(ModularKernelError::Invariant {
            detail: "modular obstruction RREF exceeded its admitted dense output bound",
        });
    }
    check_limit(
        BACK_SUBSTITUTION_OUTPUT,
        reducer.u().nvalues(),
        limits.max_reducer_total_fill_entries,
    )?;
    let fill_multiple_limit = checked_mul(
        BACK_SUBSTITUTION_FILL_MULTIPLE,
        projected_input_nonzeros,
        limits.max_reducer_fill_multiple,
    )?;
    check_limit(
        BACK_SUBSTITUTION_FILL_MULTIPLE,
        reducer.u().nvalues(),
        fill_multiple_limit,
    )?;

    for (pivot_column, pivot_row) in reducer.pivots().iter().enumerate() {
        let Some(pivot_row) = *pivot_row else {
            continue;
        };
        let row = pivot_row as usize;
        let start = *reducer
            .u()
            .row_ptrs()
            .get(row)
            .ok_or(ModularKernelError::Invariant {
                detail: "modular obstruction pivot row has no start pointer",
            })?;
        let end = *reducer
            .u()
            .row_ptrs()
            .get(row + 1)
            .ok_or(ModularKernelError::Invariant {
                detail: "modular obstruction pivot row has no end pointer",
            })?;
        let columns =
            reducer
                .u()
                .col_idcs()
                .get(start..end)
                .ok_or(ModularKernelError::Invariant {
                    detail: "modular obstruction pivot row has invalid sparse bounds",
                })?;
        let values = reducer
            .u()
            .values()
            .get(start..end)
            .ok_or(ModularKernelError::Invariant {
                detail: "modular obstruction pivot values have invalid sparse bounds",
            })?;
        if columns.first().copied().map(|column| column as usize) != Some(pivot_column)
            || values
                .first()
                .is_none_or(|value| !reducer.u().field().is_one(value))
        {
            return Err(ModularKernelError::Invariant {
                detail: "modular obstruction RREF row is not normalized at its declared pivot",
            });
        }
        for &column in columns.iter().skip(1) {
            if reducer.pivots()[column as usize].is_some() {
                return Err(ModularKernelError::Invariant {
                    detail: "modular obstruction RREF retained a distinct pivot column",
                });
            }
        }
    }
    Ok(())
}

fn extract_normalized_entries(
    reducer: &SparseRowReducer<Zp64>,
    target_logical_column: usize,
    limits: ModularKernelLimits,
) -> Result<Vec<ModularObstructionEntry>, ModularKernelError> {
    let logical_columns = target_logical_column + 1;
    check_limit(
        OBSTRUCTION_ENTRIES,
        logical_columns,
        limits.max_projected_columns,
    )?;
    let mut entries = try_vec(OBSTRUCTION_ENTRIES, logical_columns)?;
    for logical_column in 0..target_logical_column {
        let Some(row) = reducer.pivots()[logical_column] else {
            continue;
        };
        let row = row as usize;
        let start = reducer.u().row_ptrs()[row];
        let end = reducer.u().row_ptrs()[row + 1];
        let columns = &reducer.u().col_idcs()[start..end];
        let Ok(target_position) = columns.binary_search(&(target_logical_column as u32)) else {
            continue;
        };
        let coefficient = reducer
            .u()
            .field()
            .neg(&reducer.u().values()[start + target_position]);
        if !reducer.u().field().is_zero(&coefficient) {
            entries.push(ModularObstructionEntry::new(logical_column, coefficient));
        }
    }
    entries.push(ModularObstructionEntry::new(
        target_logical_column,
        reducer.u().field().one(),
    ));
    Ok(entries)
}

fn verify_normalized_obstruction(
    projected: &SparseMatrix<Zp64>,
    target_logical_column: usize,
    entries: &[ModularObstructionEntry],
    limits: ModularKernelLimits,
) -> Result<(), ModularKernelError> {
    check_limit(
        OBSTRUCTION_ENTRIES,
        entries.len(),
        limits.max_projected_columns,
    )?;
    if projected.ncols() as usize != target_logical_column + 1
        || entries.is_empty()
        || entries
            .windows(2)
            .any(|pair| pair[0].logical_column >= pair[1].logical_column)
        || entries.iter().any(|entry| {
            entry.logical_column > target_logical_column
                || projected.field().is_zero(&entry.coefficient)
        })
    {
        return Err(ModularKernelError::Invariant {
            detail: "modular right obstruction is not canonical sparse target-local data",
        });
    }
    let target = entries.last().ok_or(ModularKernelError::Invariant {
        detail: "modular right obstruction lost its target entry",
    })?;
    if target.logical_column != target_logical_column
        || !projected.field().is_one(&target.coefficient)
    {
        return Err(ModularKernelError::Invariant {
            detail: "modular right obstruction is not normalized to target coefficient one",
        });
    }

    for bounds in projected.row_ptrs().windows(2) {
        let columns = projected.col_idcs().get(bounds[0]..bounds[1]).ok_or(
            ModularKernelError::Invariant {
                detail: "modular obstruction verification found invalid projected row bounds",
            },
        )?;
        let values =
            projected
                .values()
                .get(bounds[0]..bounds[1])
                .ok_or(ModularKernelError::Invariant {
                    detail: "modular obstruction verification found invalid projected values",
                })?;
        let mut residual = projected.field().zero();
        for (&column, value) in columns.iter().zip(values) {
            let Ok(position) =
                entries.binary_search_by_key(&(column as usize), |entry| entry.logical_column)
            else {
                continue;
            };
            residual = projected.field().add(
                &residual,
                &projected.field().mul(value, &entries[position].coefficient),
            );
        }
        if !projected.field().is_zero(&residual) {
            return Err(ModularKernelError::Invariant {
                detail: "modular right obstruction failed exact finite-field residual replay",
            });
        }
    }
    Ok(())
}

fn call_native<T>(
    operation: &'static str,
    callback: impl FnOnce() -> T,
) -> Result<T, ModularKernelError> {
    catch_unwind(AssertUnwindSafe(callback))
        .map_err(|_| ModularKernelError::NativePanic { operation })
}

#[cfg(test)]
pub(super) fn verify_obstruction_for_test(
    projected: &SparseMatrix<Zp64>,
    target_logical_column: usize,
    entries: &[ModularObstructionEntry],
    limits: ModularKernelLimits,
) -> Result<(), ModularKernelError> {
    verify_normalized_obstruction(projected, target_logical_column, entries, limits)
}
