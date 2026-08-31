use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::domains::finite_field::Zp64;
use symbolica::domains::{Ring, RingOps};
use symbolica::tensors::sparse::{SparseMatrix, SparseRowReducer};

use super::sample::{check_limit, checked_add, checked_mul, try_vec};
use super::{
    ModularKernelError, ModularKernelLimits, ModularObstructionBlock, ModularObstructionDirection,
    ModularObstructionEntry, ModularPhysicalFrame, ModularRankDiagnostics, ModularRightObstruction,
};

const OBSTRUCTION_ENTRIES: &str = "modular right-obstruction nonzero entries";
const BACK_SUBSTITUTION_OUTPUT: &str = "modular obstruction back-substitution output entries";
const BACK_SUBSTITUTION_LIVE: &str = "modular obstruction back-substitution live entries";
const BACK_SUBSTITUTION_FILL_MULTIPLE: &str =
    "modular obstruction back-substitution fill-multiple entries";
const BLOCK_DIRECTIONS: &str = "modular obstruction-block directions";
const BLOCK_ENTRIES: &str = "modular obstruction-block nonzero entries";
const BLOCK_CONSTRUCTION: &str = "modular obstruction-block construction operations";
const BLOCK_REPLAY: &str = "modular obstruction-block replay operations";

#[allow(clippy::too_many_arguments)]
pub(super) fn finish_no_hit<'frame>(
    frame: &ModularPhysicalFrame<'frame>,
    diagnostics: ModularRankDiagnostics,
    logical_physical_columns: Vec<usize>,
    projected: &SparseMatrix<Zp64>,
    reducer: SparseRowReducer<Zp64>,
    projected_input_nonzeros: usize,
    obstruction_rotation: usize,
    limits: ModularKernelLimits,
) -> Result<ModularRightObstruction<'frame>, ModularKernelError> {
    validate_scope(
        frame,
        &diagnostics,
        &logical_physical_columns,
        projected,
        &reducer,
    )?;

    let (entries, block) = checked_obstruction_data(
        projected,
        reducer,
        projected_input_nonzeros,
        obstruction_rotation,
        limits,
    )?;
    Ok(ModularRightObstruction::from_checked_parts(
        frame.plan(),
        frame.sample_fingerprint().clone(),
        diagnostics,
        logical_physical_columns,
        entries,
        block,
    ))
}

pub(super) fn checked_entries(
    projected: &SparseMatrix<Zp64>,
    reducer: SparseRowReducer<Zp64>,
    projected_input_nonzeros: usize,
    limits: ModularKernelLimits,
) -> Result<Vec<ModularObstructionEntry>, ModularKernelError> {
    let (primary, _) =
        checked_obstruction_data(projected, reducer, projected_input_nonzeros, 0, limits)?;
    Ok(primary.iter().cloned().collect())
}

fn checked_obstruction_data(
    projected: &SparseMatrix<Zp64>,
    mut reducer: SparseRowReducer<Zp64>,
    projected_input_nonzeros: usize,
    obstruction_rotation: usize,
    limits: ModularKernelLimits,
) -> Result<(Arc<[ModularObstructionEntry]>, ModularObstructionBlock), ModularKernelError> {
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

    let (primary, block) = extract_checked_block(
        projected,
        &reducer,
        target_logical_column,
        obstruction_rotation,
        limits,
    )?;
    Ok((primary, block))
}

fn extract_checked_block(
    projected: &SparseMatrix<Zp64>,
    reducer: &SparseRowReducer<Zp64>,
    target_logical_column: usize,
    rotation: usize,
    limits: ModularKernelLimits,
) -> Result<(Arc<[ModularObstructionEntry]>, ModularObstructionBlock), ModularKernelError> {
    let configured_width = limits.max_obstruction_block_directions.min(4);
    if configured_width == 0 {
        return Err(ModularKernelError::ResourceLimit {
            resource: BLOCK_DIRECTIONS,
            requested: 1,
            limit: 0,
        });
    }
    let logical_columns = reducer.pivots().len();
    let direction_upper = configured_width.min(logical_columns);
    let construction_factor = checked_add(BLOCK_CONSTRUCTION, direction_upper, 1)?;
    let construction_per_half =
        checked_mul(BLOCK_CONSTRUCTION, logical_columns, construction_factor)?;
    // Two initial coordinate scans (pivot/free discovery and auxiliary-free
    // collection) plus extraction and independent verification for every
    // potentially retained direction. Admit all of it before the first scan.
    let construction = checked_mul(BLOCK_CONSTRUCTION, construction_per_half, 2)?;
    check_limit(
        BLOCK_CONSTRUCTION,
        construction,
        limits.max_obstruction_block_construction_operations,
    )?;
    if target_logical_column >= logical_columns || reducer.pivots()[target_logical_column].is_some()
    {
        return Err(ModularKernelError::Invariant {
            detail: "target logical column disappeared from the obstruction free columns",
        });
    }
    let mut free_columns = try_vec(BLOCK_DIRECTIONS, logical_columns)?;
    for (column, pivot) in reducer.pivots().iter().enumerate() {
        if pivot.is_none() {
            free_columns.push(column);
        }
    }
    let direction_count = free_columns.len().min(configured_width);
    check_limit(
        BLOCK_DIRECTIONS,
        direction_count,
        limits.max_obstruction_block_directions,
    )?;
    let retained_entry_upper = checked_mul(BLOCK_ENTRIES, reducer.pivots().len(), direction_count)?;
    check_limit(
        BLOCK_ENTRIES,
        retained_entry_upper,
        limits.max_obstruction_block_entries,
    )?;
    let replay = checked_mul(BLOCK_REPLAY, projected.nvalues(), direction_count)?;
    check_limit(
        BLOCK_REPLAY,
        replay,
        limits.max_obstruction_block_replay_operations,
    )?;

    let primary_vec =
        extract_target_normalized_direction(reducer, target_logical_column, None, limits)?;
    verify_free_direction(
        projected,
        reducer.pivots(),
        target_logical_column,
        target_logical_column,
        &primary_vec,
        limits,
    )?;
    let primary: Arc<[ModularObstructionEntry]> = Arc::from(primary_vec);
    let mut directions = try_vec(BLOCK_DIRECTIONS, direction_count)?;
    directions.push(ModularObstructionDirection::from_checked_parts(
        target_logical_column,
        primary.clone(),
    ));

    let mut auxiliary_free = try_vec(BLOCK_DIRECTIONS, free_columns.len().saturating_sub(1))?;
    for &column in &free_columns {
        if column != target_logical_column {
            auxiliary_free.push(column);
        }
    }
    if !auxiliary_free.is_empty() {
        let start = rotation % auxiliary_free.len();
        for offset in 0..direction_count.saturating_sub(1) {
            let designated = auxiliary_free[(start + offset) % auxiliary_free.len()];
            let entries = extract_target_normalized_direction(
                reducer,
                target_logical_column,
                Some(designated),
                limits,
            )?;
            verify_free_direction(
                projected,
                reducer.pivots(),
                target_logical_column,
                designated,
                &entries,
                limits,
            )?;
            directions.push(ModularObstructionDirection::from_checked_parts(
                designated,
                Arc::from(entries),
            ));
        }
    }
    if directions.len() != direction_count {
        return Err(ModularKernelError::Invariant {
            detail: "obstruction-block extraction changed its admitted direction count",
        });
    }
    let total_entries = directions.iter().try_fold(0usize, |total, direction| {
        checked_add(BLOCK_ENTRIES, total, direction.entries().len())
    })?;
    check_limit(
        BLOCK_ENTRIES,
        total_entries,
        limits.max_obstruction_block_entries,
    )?;
    Ok((
        primary,
        ModularObstructionBlock::from_checked_parts(rotation, directions),
    ))
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

fn extract_target_normalized_direction(
    reducer: &SparseRowReducer<Zp64>,
    target_logical_column: usize,
    auxiliary_free_logical_column: Option<usize>,
    limits: ModularKernelLimits,
) -> Result<Vec<ModularObstructionEntry>, ModularKernelError> {
    let logical_columns = reducer.pivots().len();
    if target_logical_column >= logical_columns
        || reducer.pivots()[target_logical_column].is_some()
        || auxiliary_free_logical_column.is_some_and(|column| {
            column >= logical_columns
                || column == target_logical_column
                || reducer.pivots()[column].is_some()
        })
    {
        return Err(ModularKernelError::Invariant {
            detail: "obstruction-block target or auxiliary column is not a distinct checked RREF free column",
        });
    }
    check_limit(
        OBSTRUCTION_ENTRIES,
        logical_columns,
        limits.max_projected_columns,
    )?;
    let mut entries = try_vec(OBSTRUCTION_ENTRIES, logical_columns)?;
    for logical_column in 0..logical_columns {
        let Some(row) = reducer.pivots()[logical_column] else {
            continue;
        };
        let row = row as usize;
        let start = reducer.u().row_ptrs()[row];
        let end = reducer.u().row_ptrs()[row + 1];
        let columns = &reducer.u().col_idcs()[start..end];
        let mut free_sum = reducer.u().field().zero();
        if let Ok(position) = columns.binary_search(&(target_logical_column as u32)) {
            free_sum = reducer
                .u()
                .field()
                .add(&free_sum, &reducer.u().values()[start + position]);
        }
        if let Some(auxiliary) = auxiliary_free_logical_column {
            if let Ok(position) = columns.binary_search(&(auxiliary as u32)) {
                free_sum = reducer
                    .u()
                    .field()
                    .add(&free_sum, &reducer.u().values()[start + position]);
            }
        }
        let coefficient = reducer.u().field().neg(&free_sum);
        if !reducer.u().field().is_zero(&coefficient) {
            entries.push(ModularObstructionEntry::new(logical_column, coefficient));
        }
    }
    let insertion = entries
        .binary_search_by_key(&target_logical_column, |entry| entry.logical_column)
        .expect_err("a free column cannot already have a pivot-derived coefficient");
    entries.insert(
        insertion,
        ModularObstructionEntry::new(target_logical_column, reducer.u().field().one()),
    );
    if let Some(auxiliary) = auxiliary_free_logical_column {
        let insertion = entries
            .binary_search_by_key(&auxiliary, |entry| entry.logical_column)
            .expect_err("an auxiliary free column cannot already have a pivot-derived coefficient");
        entries.insert(
            insertion,
            ModularObstructionEntry::new(auxiliary, reducer.u().field().one()),
        );
    }
    Ok(entries)
}

fn verify_free_direction(
    projected: &SparseMatrix<Zp64>,
    pivots: &[Option<u32>],
    target_logical_column: usize,
    designated_free_logical_column: usize,
    entries: &[ModularObstructionEntry],
    limits: ModularKernelLimits,
) -> Result<(), ModularKernelError> {
    check_limit(
        OBSTRUCTION_ENTRIES,
        entries.len(),
        limits.max_obstruction_block_entries,
    )?;
    if projected.ncols() as usize != pivots.len()
        || target_logical_column >= pivots.len()
        || designated_free_logical_column >= pivots.len()
        || pivots[target_logical_column].is_some()
        || pivots[designated_free_logical_column].is_some()
        || entries.is_empty()
        || entries
            .windows(2)
            .any(|pair| pair[0].logical_column >= pair[1].logical_column)
        || entries.iter().any(|entry| {
            entry.logical_column >= pivots.len() || projected.field().is_zero(&entry.coefficient)
        })
    {
        return Err(ModularKernelError::Invariant {
            detail: "modular obstruction-block direction is not canonical sparse data",
        });
    }
    for (column, pivot) in pivots.iter().enumerate() {
        if pivot.is_some() {
            continue;
        }
        let coefficient = entries
            .binary_search_by_key(&column, |entry| entry.logical_column)
            .ok()
            .map(|position| &entries[position].coefficient);
        if column == target_logical_column || column == designated_free_logical_column {
            if coefficient.is_none_or(|value| !projected.field().is_one(value)) {
                return Err(ModularKernelError::Invariant {
                    detail: "modular obstruction-block direction lost its target-normalized free identity",
                });
            }
        } else if coefficient.is_some() {
            return Err(ModularKernelError::Invariant {
                detail: "modular obstruction-block direction retained a distinct free coordinate",
            });
        }
    }

    for bounds in projected.row_ptrs().windows(2) {
        let columns = projected.col_idcs().get(bounds[0]..bounds[1]).ok_or(
            ModularKernelError::Invariant {
                detail: "modular obstruction-block replay found invalid projected row bounds",
            },
        )?;
        let values =
            projected
                .values()
                .get(bounds[0]..bounds[1])
                .ok_or(ModularKernelError::Invariant {
                    detail: "modular obstruction-block replay found invalid projected values",
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
                detail: "modular obstruction-block direction failed exact finite-field residual replay",
            });
        }
    }
    Ok(())
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

#[cfg(test)]
pub(super) fn checked_obstruction_data_for_test(
    projected: &SparseMatrix<Zp64>,
    reducer: SparseRowReducer<Zp64>,
    projected_input_nonzeros: usize,
    rotation: usize,
    limits: ModularKernelLimits,
) -> Result<(Arc<[ModularObstructionEntry]>, ModularObstructionBlock), ModularKernelError> {
    checked_obstruction_data(
        projected,
        reducer,
        projected_input_nonzeros,
        rotation,
        limits,
    )
}

#[cfg(test)]
pub(super) fn verify_block_direction_for_test(
    projected: &SparseMatrix<Zp64>,
    pivots: &[Option<u32>],
    target_logical_column: usize,
    designated_free_logical_column: usize,
    entries: &[ModularObstructionEntry],
    limits: ModularKernelLimits,
) -> Result<(), ModularKernelError> {
    verify_free_direction(
        projected,
        pivots,
        target_logical_column,
        designated_free_logical_column,
        entries,
        limits,
    )
}
