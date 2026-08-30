use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::domains::Ring;
use symbolica::domains::finite_field::Zp64;
use symbolica::tensors::sparse::{LuLMode, SparseMatrix, SparseRowReducer};

use super::sample::{check_limit, checked_add, checked_mul, checked_u32, try_vec};
use super::{
    ModularHit, ModularKernelError, ModularKernelLimits, ModularNoHit, ModularPhysicalFrame,
    ModularRankDiagnostics, ModularTargetQuery,
};

const PROJECTED_COLUMNS: &str = "modular projected columns";
const PROJECTED_ENTRIES: &str = "modular projected nonzero entries";
const PROJECTED_ROW_OFFSETS: &str = "modular projected CSR row offsets";
const REDUCER_DENSE_CELLS: &str = "modular reducer dense-fill cells";
const REDUCER_TOTAL_FILL: &str = "modular reducer total L-pattern plus U fill entries";
const REDUCER_FILL_MULTIPLE: &str = "modular reducer fill-multiple entries";

#[derive(Debug)]
struct RankSummary {
    rank: usize,
    pivot_columns: Box<[usize]>,
    input_nonzeros: usize,
    lower_pattern_nonzeros: usize,
    upper_nonzeros: usize,
    total_fill_nonzeros: usize,
    independent_source_rows: Box<[usize]>,
}

pub(super) fn query_target<'frame>(
    frame: &ModularPhysicalFrame<'frame>,
    target_column: usize,
    forbidden_columns: &[usize],
    limits: ModularKernelLimits,
) -> Result<ModularTargetQuery<'frame>, ModularKernelError> {
    let physical_columns = frame.matrix().ncols() as usize;
    if target_column >= physical_columns {
        return Err(ModularKernelError::TargetColumnOutOfRange {
            target: target_column,
            columns: physical_columns,
        });
    }

    let augmented_count = checked_add(PROJECTED_COLUMNS, forbidden_columns.len(), 1)?;
    check_limit(
        PROJECTED_COLUMNS,
        augmented_count,
        limits.max_projected_columns,
    )?;
    let mut forbidden = try_vec(PROJECTED_COLUMNS, forbidden_columns.len())?;
    forbidden.extend_from_slice(forbidden_columns);
    forbidden.sort_unstable();
    for pair in forbidden.windows(2) {
        if pair[0] == pair[1] {
            return Err(ModularKernelError::DuplicateForbiddenColumn { column: pair[0] });
        }
    }
    for &column in &forbidden {
        if column >= physical_columns {
            return Err(ModularKernelError::ForbiddenColumnOutOfRange {
                column,
                columns: physical_columns,
            });
        }
    }
    if forbidden.binary_search(&target_column).is_ok() {
        return Err(ModularKernelError::TargetIsForbidden {
            target: target_column,
        });
    }

    let forbidden_summary = rank_projection(frame.matrix(), &forbidden, limits)?;
    let mut augmented = try_vec(PROJECTED_COLUMNS, augmented_count)?;
    augmented.extend_from_slice(&forbidden);
    let insertion = augmented.binary_search(&target_column).unwrap_err();
    augmented.insert(insertion, target_column);
    let augmented_summary = rank_projection(frame.matrix(), &augmented, limits)?;

    if augmented_summary.rank < forbidden_summary.rank
        || augmented_summary.rank > forbidden_summary.rank.saturating_add(1)
    {
        return Err(ModularKernelError::Invariant {
            detail: "adding one target column changed rank by more than one",
        });
    }
    let diagnostics = ModularRankDiagnostics {
        target_column,
        forbidden_columns: forbidden.into_boxed_slice(),
        forbidden_rank: forbidden_summary.rank,
        augmented_rank: augmented_summary.rank,
        forbidden_pivot_columns: forbidden_summary.pivot_columns,
        augmented_pivot_columns: augmented_summary.pivot_columns,
        forbidden_independent_source_rows: forbidden_summary.independent_source_rows,
        augmented_independent_source_rows: augmented_summary.independent_source_rows,
        forbidden_input_nonzeros: forbidden_summary.input_nonzeros,
        augmented_input_nonzeros: augmented_summary.input_nonzeros,
        forbidden_lower_pattern_nonzeros: forbidden_summary.lower_pattern_nonzeros,
        augmented_lower_pattern_nonzeros: augmented_summary.lower_pattern_nonzeros,
        forbidden_upper_nonzeros: forbidden_summary.upper_nonzeros,
        augmented_upper_nonzeros: augmented_summary.upper_nonzeros,
        forbidden_total_fill_nonzeros: forbidden_summary.total_fill_nonzeros,
        augmented_total_fill_nonzeros: augmented_summary.total_fill_nonzeros,
    };
    if diagnostics.augmented_rank > diagnostics.forbidden_rank {
        Ok(ModularTargetQuery::Hit(ModularHit::new(
            frame.plan(),
            frame.sample_fingerprint().clone(),
            diagnostics,
        )))
    } else {
        Ok(ModularTargetQuery::ModularNoHit(ModularNoHit {
            diagnostics,
        }))
    }
}

fn rank_projection(
    matrix: &SparseMatrix<Zp64>,
    selected_columns: &[usize],
    limits: ModularKernelLimits,
) -> Result<RankSummary, ModularKernelError> {
    if selected_columns.is_empty() || matrix.nrows() == 0 {
        return Ok(RankSummary {
            rank: 0,
            pivot_columns: Box::new([]),
            input_nonzeros: 0,
            lower_pattern_nonzeros: 0,
            upper_nonzeros: 0,
            total_fill_nonzeros: 0,
            independent_source_rows: Box::new([]),
        });
    }
    if selected_columns.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ModularKernelError::Invariant {
            detail: "rank projection columns are not strictly sorted",
        });
    }
    let physical_columns = matrix.ncols() as usize;
    if selected_columns
        .last()
        .is_some_and(|&column| column >= physical_columns)
    {
        return Err(ModularKernelError::Invariant {
            detail: "rank projection column is outside the physical matrix",
        });
    }
    check_limit(
        PROJECTED_COLUMNS,
        selected_columns.len(),
        limits.max_projected_columns,
    )?;
    let dense_cells = checked_mul(
        REDUCER_DENSE_CELLS,
        matrix.nrows() as usize,
        selected_columns.len(),
    )?;
    check_limit(
        REDUCER_DENSE_CELLS,
        dense_cells,
        limits.max_reducer_dense_cells,
    )?;
    let rank_bound = usize::min(matrix.nrows() as usize, selected_columns.len());
    let upper_fill_bound = checked_mul(REDUCER_TOTAL_FILL, rank_bound, selected_columns.len())?;
    let lower_fill_bound = checked_mul(REDUCER_TOTAL_FILL, matrix.nrows() as usize, rank_bound)?;
    let total_fill_bound = checked_add(REDUCER_TOTAL_FILL, upper_fill_bound, lower_fill_bound)?;
    check_limit(
        REDUCER_TOTAL_FILL,
        total_fill_bound,
        limits.max_reducer_total_fill_entries,
    )?;

    let input_nonzeros = projected_entry_count(matrix, selected_columns)?;
    check_limit(
        PROJECTED_ENTRIES,
        input_nonzeros,
        limits.max_projected_entries,
    )?;
    let row_offset_count = checked_add(PROJECTED_ROW_OFFSETS, matrix.nrows() as usize, 1)?;
    check_limit(
        PROJECTED_ROW_OFFSETS,
        row_offset_count,
        limits.max_csr_row_offsets,
    )?;
    let mut values = try_vec(PROJECTED_ENTRIES, input_nonzeros)?;
    let mut column_indices = try_vec(PROJECTED_ENTRIES, input_nonzeros)?;
    let mut row_offsets = try_vec(PROJECTED_ROW_OFFSETS, row_offset_count)?;
    row_offsets.push(0usize);

    for bounds in matrix.row_ptrs().windows(2) {
        let source_columns =
            matrix
                .col_idcs()
                .get(bounds[0]..bounds[1])
                .ok_or(ModularKernelError::Invariant {
                    detail: "physical modular CSR has invalid row bounds during projection",
                })?;
        let source_values =
            matrix
                .values()
                .get(bounds[0]..bounds[1])
                .ok_or(ModularKernelError::Invariant {
                    detail: "physical modular CSR values have invalid row bounds during projection",
                })?;
        for (&column, value) in source_columns.iter().zip(source_values) {
            if let Ok(projected) = selected_columns.binary_search(&(column as usize)) {
                values.push(value.clone());
                column_indices.push(checked_u32("modular projected column index", projected)?);
            }
        }
        row_offsets.push(values.len());
    }

    let row_count = matrix.nrows();
    let column_count = checked_u32("modular projected matrix columns", selected_columns.len())?;
    validate_projected_csr(
        row_count,
        column_count,
        &values,
        &row_offsets,
        &column_indices,
        matrix.field(),
        input_nonzeros,
    )?;
    let projected = SparseMatrix::from_csr(
        row_count,
        column_count,
        values,
        row_offsets,
        column_indices,
        matrix.field().clone(),
    );
    let fill_multiple_limit = checked_mul(
        REDUCER_FILL_MULTIPLE,
        input_nonzeros,
        limits.max_reducer_fill_multiple,
    )?;
    let mut reducer = call_native("constructing the modular sparse row reducer", || {
        SparseRowReducer::new(column_count, projected.field().clone(), LuLMode::Pattern)
    })?;
    let mut independent_source_rows = try_vec(
        "modular independent source rows",
        usize::min(matrix.nrows() as usize, selected_columns.len()),
    )?;
    for (row, bounds) in projected.row_ptrs().windows(2).enumerate() {
        let columns = &projected.col_idcs()[bounds[0]..bounds[1]];
        if columns.is_empty() {
            continue;
        }
        let values = &projected.values()[bounds[0]..bounds[1]];
        let accepted = call_native("adding a row to the modular sparse reducer", || {
            reducer.add_row(values, columns)
        })?;
        if accepted.is_some() {
            independent_source_rows.push(row);
        }
        let current_total_fill = checked_add(
            REDUCER_TOTAL_FILL,
            reducer.u().nvalues(),
            reducer.l().col_idcs().len(),
        )?;
        check_limit(
            REDUCER_TOTAL_FILL,
            current_total_fill,
            limits.max_reducer_total_fill_entries,
        )?;
        check_limit(
            REDUCER_FILL_MULTIPLE,
            current_total_fill,
            fill_multiple_limit,
        )?;
    }
    let rank = reducer.u().nrows() as usize;
    let pivot_count = reducer.pivots().iter().flatten().count();
    if rank != pivot_count || rank > matrix.nrows() as usize || rank > selected_columns.len() {
        return Err(ModularKernelError::Invariant {
            detail: "Symbolica sparse reducer returned an invalid rank/pivot shape",
        });
    }
    if independent_source_rows.len() != rank
        || independent_source_rows
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(ModularKernelError::Invariant {
            detail: "Symbolica sparse reducer lost original independent-row chronology",
        });
    }
    let upper_nonzeros = reducer.u().nvalues();
    if upper_nonzeros > dense_cells {
        return Err(ModularKernelError::Invariant {
            detail: "Symbolica sparse reducer exceeded its admitted dense-fill bound",
        });
    }
    let lower_pattern_nonzeros = reducer.l().col_idcs().len();
    if lower_pattern_nonzeros > lower_fill_bound {
        return Err(ModularKernelError::Invariant {
            detail: "Symbolica sparse reducer exceeded its admitted L-pattern fill bound",
        });
    }
    let total_fill_nonzeros =
        checked_add(REDUCER_TOTAL_FILL, lower_pattern_nonzeros, upper_nonzeros)?;
    if total_fill_nonzeros > total_fill_bound {
        return Err(ModularKernelError::Invariant {
            detail: "Symbolica sparse reducer exceeded its admitted total fill bound",
        });
    }
    let mut pivot_columns = try_vec("modular reducer pivot columns", rank)?;
    for (projected_column, pivot) in reducer.pivots().iter().enumerate() {
        if pivot.is_some() {
            pivot_columns.push(selected_columns[projected_column]);
        }
    }

    Ok(RankSummary {
        rank,
        pivot_columns: pivot_columns.into_boxed_slice(),
        input_nonzeros,
        lower_pattern_nonzeros,
        upper_nonzeros,
        total_fill_nonzeros,
        independent_source_rows: independent_source_rows.into_boxed_slice(),
    })
}

fn call_native<T>(
    operation: &'static str,
    callback: impl FnOnce() -> T,
) -> Result<T, ModularKernelError> {
    catch_unwind(AssertUnwindSafe(callback))
        .map_err(|_| ModularKernelError::NativePanic { operation })
}

fn projected_entry_count(
    matrix: &SparseMatrix<Zp64>,
    selected_columns: &[usize],
) -> Result<usize, ModularKernelError> {
    let mut count = 0usize;
    for &column in matrix.col_idcs() {
        if selected_columns.binary_search(&(column as usize)).is_ok() {
            count = checked_add(PROJECTED_ENTRIES, count, 1)?;
        }
    }
    Ok(count)
}

#[allow(clippy::too_many_arguments)]
fn validate_projected_csr(
    row_count: u32,
    column_count: u32,
    values: &[symbolica::domains::finite_field::FiniteFieldElement<u64>],
    row_offsets: &[usize],
    column_indices: &[u32],
    field: &Zp64,
    expected_entries: usize,
) -> Result<(), ModularKernelError> {
    if values.len() != expected_entries
        || column_indices.len() != expected_entries
        || row_offsets.len() != row_count as usize + 1
        || row_offsets.first() != Some(&0)
        || row_offsets.last() != Some(&expected_entries)
        || row_offsets.windows(2).any(|pair| pair[0] > pair[1])
        || values.iter().any(|value| field.is_zero(value))
    {
        return Err(ModularKernelError::Invariant {
            detail: "projected modular CSR failed shape or nonzero validation",
        });
    }
    for bounds in row_offsets.windows(2) {
        let row_columns =
            column_indices
                .get(bounds[0]..bounds[1])
                .ok_or(ModularKernelError::Invariant {
                    detail: "projected modular CSR row bounds are invalid",
                })?;
        if row_columns.iter().any(|&column| column >= column_count)
            || row_columns.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(ModularKernelError::Invariant {
                detail: "projected modular CSR columns are unsorted or out of range",
            });
        }
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn rank_projection_for_test(
    matrix: &SparseMatrix<Zp64>,
    selected_columns: &[usize],
    limits: ModularKernelLimits,
) -> Result<(usize, usize), ModularKernelError> {
    rank_projection(matrix, selected_columns, limits)
        .map(|summary| (summary.rank, summary.total_fill_nonzeros))
}
