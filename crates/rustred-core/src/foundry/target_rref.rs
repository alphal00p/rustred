use symbolica::domains::rational_polynomial::RationalPolynomialField;
use symbolica::prelude::IntegerRing;
use symbolica::tensors::sparse::SparseRowReducer;

pub(super) type NativeField = RationalPolynomialField<IntegerRing, u16>;
pub(super) type NativeReducer = SparseRowReducer<NativeField>;

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
    ProvenancePivot {
        source_ordinal: usize,
        pivot_column: usize,
    },
}

/// Compute a sound pre-RREF `U` reachability envelope. Every forward pivot
/// used to construct a reachable row contributes its normalization guard.
///
/// A provenance-column pivot is rejected because the public foundry guard
/// models currently identify physical pivots only. Rejecting this boundary is
/// preferable to inventing a physical shift or silently omitting a guard.
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
            return Err(Error::ProvenancePivot {
                source_ordinal: meta.source_ordinal,
                pivot_column: meta.pivot_column,
            });
        }
        for &dependency in &meta.pivot_dependencies {
            let dependency_meta = metadata.get(dependency).ok_or(Error::Invariant {
                detail: "a reachable pivot dependency is outside reducer metadata",
            })?;
            if dependency_meta.pivot_column >= physical_columns {
                return Err(Error::ProvenancePivot {
                    source_ordinal: dependency_meta.source_ordinal,
                    pivot_column: dependency_meta.pivot_column,
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

/// Admit the dense structural upper bound before Symbolica owns a second `U`
/// matrix. The forward `L` entries remain charged because clearing their
/// vectors does not promise to release their allocated capacity.
pub(super) fn admit_back_substitution(
    reducer: &NativeReducer,
    augmented_columns: usize,
    limits: BackSubstitutionLimits,
) -> Result<BackSubstitutionAdmission, Error> {
    let output_bound = checked_mul(
        "Symbolica target back-substitution output nonzero entries",
        reducer.u().nrows() as usize,
        augmented_columns,
    )?;
    check_limit(
        "Symbolica target back-substitution output nonzero entries",
        output_bound,
        limits.max_output_nonzero_entries,
    )?;
    let forward_nonzeros = checked_add(
        "Symbolica target back-substitution live nonzero entries",
        reducer.u().nvalues(),
        reducer.l().nvalues(),
    )?;
    let live_bound = checked_add(
        "Symbolica target back-substitution live nonzero entries",
        forward_nonzeros,
        output_bound,
    )?;
    check_limit(
        "Symbolica target back-substitution live nonzero entries",
        live_bound,
        limits.max_live_nonzero_entries,
    )?;
    Ok(BackSubstitutionAdmission { output_bound })
}

/// Validate the serial Symbolica output before any algebra-specific entry is
/// admitted or exposed. Returns the actual `U` nonzero count so each foundry
/// can also enforce its existing decomposition budget.
pub(super) fn postvalidate_back_substitution(
    reducer: &NativeReducer,
    expected_rows: usize,
    augmented_columns: usize,
    admission: BackSubstitutionAdmission,
    limits: BackSubstitutionLimits,
) -> Result<usize, Error> {
    let mut pivot_count = 0usize;
    let mut pivot_rows_valid = true;
    for row in reducer.pivots().iter().flatten() {
        pivot_count += 1;
        pivot_rows_valid &= (*row as usize) < expected_rows;
    }
    if reducer.u().nrows() as usize != expected_rows
        || reducer.u().ncols() as usize != augmented_columns
        || reducer.pivots().len() != augmented_columns
        || pivot_count != expected_rows
        || !pivot_rows_valid
        || reducer.l().nrows() != 0
        || reducer.l().ncols() != 0
        || reducer.l().nvalues() != 0
    {
        return Err(Error::Invariant {
            detail: "serial back-substitution changed the admitted reducer shape or retained L",
        });
    }
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
