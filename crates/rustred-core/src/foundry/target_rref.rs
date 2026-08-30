use symbolica::domains::{Ring, rational_polynomial::RationalPolynomialField};
use symbolica::prelude::IntegerRing;
use symbolica::tensors::sparse::{SparseMatrix, SparseRowReducer};

pub(super) type NativeField = RationalPolynomialField<IntegerRing, u16>;
pub(super) type NativeReducer = SparseRowReducer<NativeField>;
pub(super) type NativeMatrix = SparseMatrix<NativeField>;

pub(super) struct ForwardReducerRowMeta<C> {
    pub(super) source_ordinal: usize,
    pub(super) reducer_row: u32,
    pub(super) pivot_column: usize,
    pub(super) pivot_coefficient: C,
    pub(super) pivot_dependencies: Vec<usize>,
    pub(super) has_trailing_physical_entry: bool,
}

#[derive(Clone, Copy)]
pub(super) struct BackSubstitutionLimits {
    pub(super) max_output_nonzero_entries: usize,
    pub(super) max_live_nonzero_entries: usize,
}

#[derive(Clone, Copy)]
pub(super) struct BackSubstitutionAdmission {
    output_bound: usize,
    expected_rows: usize,
    physical_columns: usize,
}

pub(super) enum Error {
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    Invariant {
        detail: &'static str,
    },
}

/// Compute a sound pre-RREF `U` reachability envelope. Every forward pivot
/// used to construct a reachable row contributes its normalization guard.
///
/// Reachability follows physical pivots only. Provenance columns are free
/// right-hand-side columns in the target system and therefore never introduce
/// a normalization guard of their own. Every physical forward pivot used to
/// construct a reachable row remains represented by `pivot_dependencies`.
pub(super) fn pivot_dependencies<C>(
    reducer: &NativeReducer,
    metadata: &[ForwardReducerRowMeta<C>],
    target_row: usize,
    physical_columns: usize,
    max_dependencies: usize,
) -> Result<Vec<usize>, Error> {
    let mut reachable = try_vec("target back-substitution reachable rows", metadata.len())?;
    reachable.resize(metadata.len(), false);
    let mut guard_membership = try_vec(
        "target back-substitution pivot-guard membership",
        metadata.len(),
    )?;
    guard_membership.resize(metadata.len(), false);
    let mut stack = try_vec(
        "target back-substitution reachability stack",
        metadata.len(),
    )?;
    let target_slot = reachable.get_mut(target_row).ok_or(Error::Invariant {
        detail: "the target reducer row is outside reachability storage",
    })?;
    *target_slot = true;
    stack.push(target_row);

    while let Some(row) = stack.pop() {
        let meta = metadata.get(row).ok_or(Error::Invariant {
            detail: "target back-substitution reaches a row outside reducer metadata",
        })?;
        if meta.reducer_row as usize != row {
            return Err(Error::Invariant {
                detail: "reducer metadata no longer follows native row chronology",
            });
        }
        if meta.pivot_column >= physical_columns {
            return Err(Error::Invariant {
                detail: "physical target reachability entered a provenance-pivot row",
            });
        }
        for &dependency in &meta.pivot_dependencies {
            let dependency_meta = metadata.get(dependency).ok_or(Error::Invariant {
                detail: "a reachable pivot dependency is outside reducer metadata",
            })?;
            if dependency_meta.pivot_column >= physical_columns {
                return Err(Error::Invariant {
                    detail: "a physical forward pivot depends on a provenance-pivot row",
                });
            }
            let membership = guard_membership
                .get_mut(dependency)
                .ok_or(Error::Invariant {
                    detail: "a reachable pivot dependency is outside reducer metadata",
                })?;
            *membership = true;
        }

        let row_start = *reducer.u().row_ptrs().get(row).ok_or(Error::Invariant {
            detail: "a reachable reducer row has no sparse-row start pointer",
        })?;
        let row_end = *reducer
            .u()
            .row_ptrs()
            .get(row + 1)
            .ok_or(Error::Invariant {
                detail: "a reachable reducer row has no sparse-row end pointer",
            })?;
        let row_columns =
            reducer
                .u()
                .col_idcs()
                .get(row_start..row_end)
                .ok_or(Error::Invariant {
                    detail: "a reachable reducer row has invalid sparse-column bounds",
                })?;
        for &column in row_columns {
            if column as usize >= physical_columns {
                continue;
            }
            let next_row = reducer.pivots().get(column as usize).copied().flatten();
            let Some(next_row) = next_row else {
                continue;
            };
            let next_row = next_row as usize;
            if next_row == row {
                continue;
            }
            let next_meta = metadata.get(next_row).ok_or(Error::Invariant {
                detail: "a target U edge reaches a row outside reducer metadata",
            })?;
            if next_meta.pivot_column <= meta.pivot_column {
                return Err(Error::Invariant {
                    detail: "target U reachability is not strictly upper triangular",
                });
            }
            let next_reachable = reachable.get_mut(next_row).ok_or(Error::Invariant {
                detail: "a target U edge is outside reachability storage",
            })?;
            if !*next_reachable {
                *next_reachable = true;
                stack.push(next_row);
            }
        }
    }

    let dependency_count = guard_membership.iter().filter(|&&present| present).count();
    check_limit(
        "target back-substitution pivot dependencies",
        dependency_count,
        max_dependencies,
    )?;
    let mut dependencies = try_vec(
        "target back-substitution pivot dependencies",
        dependency_count,
    )?;
    dependencies.extend(
        guard_membership
            .iter()
            .enumerate()
            .filter_map(|(ordinal, &present)| present.then_some(ordinal)),
    );
    Ok(dependencies)
}

/// Admit and construct the physical-only upper-triangular system before
/// Symbolica owns its back-substitution output.
///
/// Only physical-pivot rows are copied, making the returned matrix and pivot
/// map an honest bijection for
/// [`SparseRowReducer::from_upper_triangular_matrix`]. The original forward
/// `U`/`L` remains alive while this physical `U` is reduced, so the live bound
/// charges both matrices, retained `L` capacity, and the complete prospective
/// output. Provenance columns remain free right-hand-side columns.
pub(super) fn admit_back_substitution(
    reducer: &NativeReducer,
    physical_columns: usize,
    augmented_columns: usize,
    limits: BackSubstitutionLimits,
) -> Result<(BackSubstitutionAdmission, NativeMatrix, Vec<Option<u32>>), Error> {
    if physical_columns > augmented_columns
        || reducer.u().ncols() as usize != augmented_columns
        || reducer.pivots().len() != augmented_columns
    {
        return Err(Error::Invariant {
            detail: "forward reducer shape disagrees with admitted target columns",
        });
    }
    let forward_pivot_columns = pivot_columns_by_row(
        reducer,
        augmented_columns,
        reducer.u().nrows() as usize,
        "forward target pivot row membership",
        "forward target pivot map is not a bijection with U rows",
    )?;
    validate_normalized_pivot_rows(
        reducer,
        &forward_pivot_columns,
        augmented_columns,
        "forward target U is not a normalized sparse upper-triangular matrix",
    )?;
    let mut physical_pivot_count = 0usize;
    let mut physical_u_nonzeros = 0usize;
    for (row, pivot_column) in forward_pivot_columns.iter().enumerate() {
        let Some(pivot_column) = *pivot_column else {
            return Err(Error::Invariant {
                detail: "forward target pivot map lost a row after validation",
            });
        };
        if pivot_column >= physical_columns {
            continue;
        }
        physical_pivot_count = checked_add(
            "physical target back-substitution pivots",
            physical_pivot_count,
            1,
        )?;
        let start = *reducer.u().row_ptrs().get(row).ok_or(Error::Invariant {
            detail: "a physical target row has no sparse-row start pointer",
        })?;
        let end = *reducer
            .u()
            .row_ptrs()
            .get(row + 1)
            .ok_or(Error::Invariant {
                detail: "a physical target row has no sparse-row end pointer",
            })?;
        physical_u_nonzeros = checked_add(
            "physical target upper-triangular nonzero entries",
            physical_u_nonzeros,
            end - start,
        )?;
    }
    if physical_pivot_count == 0 {
        return Err(Error::Invariant {
            detail: "forward reducer rows and admitted physical pivots are inconsistent",
        });
    }
    let output_bound = checked_mul(
        "Symbolica target back-substitution output nonzero entries",
        physical_pivot_count,
        augmented_columns,
    )?;
    check_limit(
        "Symbolica target back-substitution output nonzero entries",
        output_bound,
        limits.max_output_nonzero_entries,
    )?;
    let retained_forward = checked_add(
        "Symbolica target back-substitution live nonzero entries",
        reducer.u().nvalues(),
        reducer.l().nvalues(),
    )?;
    let with_physical_u = checked_add(
        "Symbolica target back-substitution live nonzero entries",
        retained_forward,
        physical_u_nonzeros,
    )?;
    let live_bound = checked_add(
        "Symbolica target back-substitution live nonzero entries",
        with_physical_u,
        output_bound,
    )?;
    check_limit(
        "Symbolica target back-substitution live nonzero entries",
        live_bound,
        limits.max_live_nonzero_entries,
    )?;
    let mut physical_pivots = try_vec(
        "physical-only target back-substitution pivot map",
        augmented_columns,
    )?;
    physical_pivots.resize(augmented_columns, None);
    let mut physical_values = try_vec(
        "physical target upper-triangular values",
        physical_u_nonzeros,
    )?;
    let mut physical_columns_storage = try_vec(
        "physical target upper-triangular column indices",
        physical_u_nonzeros,
    )?;
    let row_pointer_count = checked_add(
        "physical target upper-triangular row pointers",
        physical_pivot_count,
        1,
    )?;
    let mut physical_row_pointers = try_vec(
        "physical target upper-triangular row pointers",
        row_pointer_count,
    )?;
    physical_row_pointers.push(0);
    for (row, pivot_column) in forward_pivot_columns.iter().enumerate() {
        let Some(pivot_column) = *pivot_column else {
            return Err(Error::Invariant {
                detail: "forward target pivot map lost a row while copying physical U",
            });
        };
        if pivot_column >= physical_columns {
            continue;
        }
        let start = reducer.u().row_ptrs()[row];
        let end = reducer.u().row_ptrs()[row + 1];
        physical_values.extend(reducer.u().values()[start..end].iter().cloned());
        physical_columns_storage.extend_from_slice(&reducer.u().col_idcs()[start..end]);
        let physical_row =
            u32::try_from(physical_row_pointers.len() - 1).map_err(|_| Error::Invariant {
                detail: "physical target row index does not fit Symbolica's pivot map",
            })?;
        let pivot_slot = physical_pivots
            .get_mut(pivot_column)
            .ok_or(Error::Invariant {
                detail: "physical target pivot column is outside its admitted map",
            })?;
        if pivot_slot.replace(physical_row).is_some() {
            return Err(Error::Invariant {
                detail: "physical target pivot column is represented by multiple rows",
            });
        }
        physical_row_pointers.push(physical_values.len());
    }
    if physical_values.len() != physical_u_nonzeros
        || physical_columns_storage.len() != physical_u_nonzeros
        || physical_row_pointers.len() != row_pointer_count
    {
        return Err(Error::Invariant {
            detail: "physical target upper-triangular copy disagrees with its admission",
        });
    }
    let physical_row_count = u32::try_from(physical_pivot_count).map_err(|_| Error::Invariant {
        detail: "physical target row count does not fit Symbolica's matrix shape",
    })?;
    let physical_u = SparseMatrix::from_csr(
        physical_row_count,
        reducer.u().ncols(),
        physical_values,
        physical_row_pointers,
        physical_columns_storage,
        reducer.u().field().clone(),
    );
    Ok((
        BackSubstitutionAdmission {
            output_bound,
            expected_rows: physical_pivot_count,
            physical_columns,
        },
        physical_u,
        physical_pivots,
    ))
}

/// Validate the serial Symbolica output before any algebra-specific entry is
/// admitted or exposed. Returns the actual `U` nonzero count so each foundry
/// can also enforce its existing decomposition budget.
pub(super) fn postvalidate_back_substitution(
    reducer: &NativeReducer,
    augmented_columns: usize,
    admission: BackSubstitutionAdmission,
    limits: BackSubstitutionLimits,
) -> Result<usize, Error> {
    if reducer.u().nrows() as usize != admission.expected_rows
        || reducer.u().ncols() as usize != augmented_columns
        || reducer.pivots().len() != augmented_columns
        || reducer.l().nrows() != 0
        || reducer.l().ncols() != 0
        || reducer.l().nvalues() != 0
    {
        return Err(Error::Invariant {
            detail: "physical-only back-substitution changed the admitted shape, pivot map, or retained L",
        });
    }
    let pivot_columns = pivot_columns_by_row(
        reducer,
        admission.physical_columns,
        admission.expected_rows,
        "physical target pivot row membership",
        "physical-only back-substitution pivot map is not a bijection with output rows",
    )?;
    validate_normalized_pivot_rows(
        reducer,
        &pivot_columns,
        augmented_columns,
        "physical-only back-substitution returned a malformed normalized sparse row",
    )?;
    let output_nonzeros = reducer.u().nvalues();
    if output_nonzeros > admission.output_bound {
        return Err(Error::Invariant {
            detail: "serial back-substitution exceeded its dense output bound",
        });
    }
    check_limit(
        "Symbolica target back-substitution output nonzero entries",
        output_nonzeros,
        limits.max_output_nonzero_entries,
    )?;
    Ok(output_nonzeros)
}

fn pivot_columns_by_row(
    reducer: &NativeReducer,
    allowed_pivot_columns: usize,
    expected_rows: usize,
    resource: &'static str,
    invariant: &'static str,
) -> Result<Vec<Option<usize>>, Error> {
    let mut pivot_columns = try_vec(resource, expected_rows)?;
    pivot_columns.resize(expected_rows, None);
    for (column, &pivot) in reducer.pivots().iter().enumerate() {
        let Some(row) = pivot else {
            continue;
        };
        if column >= allowed_pivot_columns {
            return Err(Error::Invariant { detail: invariant });
        }
        let slot = pivot_columns
            .get_mut(row as usize)
            .ok_or(Error::Invariant { detail: invariant })?;
        if slot.replace(column).is_some() {
            return Err(Error::Invariant { detail: invariant });
        }
    }
    if pivot_columns.iter().any(Option::is_none) {
        return Err(Error::Invariant { detail: invariant });
    }
    Ok(pivot_columns)
}

fn validate_normalized_pivot_rows(
    reducer: &NativeReducer,
    pivot_columns: &[Option<usize>],
    expected_columns: usize,
    invariant: &'static str,
) -> Result<(), Error> {
    let matrix = reducer.u();
    if matrix.nrows() as usize != pivot_columns.len()
        || matrix.ncols() as usize != expected_columns
        || matrix.row_ptrs().len() != pivot_columns.len() + 1
        || matrix.col_idcs().len() != matrix.values().len()
        || matrix.row_ptrs().first().copied() != Some(0)
        || matrix.row_ptrs().last().copied() != Some(matrix.nvalues())
    {
        return Err(Error::Invariant { detail: invariant });
    }
    for (row, pivot_column) in pivot_columns.iter().enumerate() {
        let Some(pivot_column) = *pivot_column else {
            return Err(Error::Invariant { detail: invariant });
        };
        let start = *matrix
            .row_ptrs()
            .get(row)
            .ok_or(Error::Invariant { detail: invariant })?;
        let end = *matrix
            .row_ptrs()
            .get(row + 1)
            .ok_or(Error::Invariant { detail: invariant })?;
        if start >= end || end > matrix.nvalues() {
            return Err(Error::Invariant { detail: invariant });
        }
        let columns = matrix
            .col_idcs()
            .get(start..end)
            .ok_or(Error::Invariant { detail: invariant })?;
        let values = matrix
            .values()
            .get(start..end)
            .ok_or(Error::Invariant { detail: invariant })?;
        if columns.first().copied().map(|column| column as usize) != Some(pivot_column)
            || !matrix.field().is_one(
                values
                    .first()
                    .ok_or(Error::Invariant { detail: invariant })?,
            )
        {
            return Err(Error::Invariant { detail: invariant });
        }
        let mut previous_column = None;
        for (&column, value) in columns.iter().zip(values) {
            let column = column as usize;
            if column >= expected_columns
                || previous_column.is_some_and(|previous| previous >= column)
                || matrix.field().is_zero(value)
            {
                return Err(Error::Invariant { detail: invariant });
            }
            previous_column = Some(column);
        }
    }
    Ok(())
}

fn checked_add(resource: &'static str, left: usize, right: usize) -> Result<usize, Error> {
    left.checked_add(right)
        .ok_or(Error::ResourceCountOverflow { resource })
}

fn checked_mul(resource: &'static str, left: usize, right: usize) -> Result<usize, Error> {
    left.checked_mul(right)
        .ok_or(Error::ResourceCountOverflow { resource })
}

fn check_limit(resource: &'static str, requested: usize, limit: usize) -> Result<(), Error> {
    if requested > limit {
        Err(Error::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn try_vec<T>(resource: &'static str, capacity: usize) -> Result<Vec<T>, Error> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| Error::AllocationFailure {
            resource,
            requested: capacity,
        })?;
    Ok(values)
}
