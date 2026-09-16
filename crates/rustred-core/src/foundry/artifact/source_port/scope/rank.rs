//! Exact negative-index-degree entry geometry, not closure evidence.
//!
//! `sum(max(-n_i, 0)) <= D` is a scalar-index condition. Only an explicitly
//! admitted quadratic-vacuum representation may interpret `2 * D` as a
//! maximal numerator momentum degree. It is not generic tensor rank.
//!
//! These finite unions retain infinite positive-power rays. Neither the
//! caller's degree bound nor these boxes imply closure under an IBP RHS.

use crate::foundry::artifact::ArtifactError;
use crate::foundry::completion::{
    CompletionGeometryError, CompletionGeometryLimits, LatticeBox, SimplexSupportError,
    try_build_simplex_offsets, try_simplex_sample_count,
};

/// One shared preflight owner for all sectors of a requested scope. Failed
/// allocation/enumeration attempts retain their charges; no partial scope is
/// returned. This is not a fresh allowance for every negative-index slice.
pub(in crate::foundry::artifact) struct RankScopeBudget {
    limits: CompletionGeometryLimits,
    boxes: usize,
    coordinate_cells: usize,
    structural_work: usize,
}

impl RankScopeBudget {
    pub(in crate::foundry::artifact) fn new(limits: CompletionGeometryLimits) -> Self {
        Self {
            limits,
            boxes: 0,
            coordinate_cells: 0,
            structural_work: 0,
        }
    }

    /// Build the complete entry simplex for this sector in the existing local
    /// orthant coordinates. The degree is not restricted to compact search
    /// powers. Excessively large enumerations fail before allocation.
    pub(in crate::foundry::artifact) fn slices(
        &mut self,
        sector: &[bool],
        max_negative_degree: u64,
    ) -> Result<Vec<LatticeBox>, ArtifactError> {
        if sector.is_empty() {
            return Err(invalid("rank scope needs a nonempty coordinate space"));
        }
        check("rank-scope arity", sector.len(), self.limits.max_arity)?;
        let inactive = sector.iter().filter(|&&active| !active).count();
        // There is one empty assignment when every coordinate is active,
        // independent of the numerical degree bound (including u64::MAX).
        let degree = if inactive == 0 {
            0
        } else {
            usize::try_from(max_negative_degree).map_err(|_| overflow("rank-scope degree"))?
        };
        let count = try_simplex_sample_count(inactive, degree).map_err(simplex_error)?;
        let boxes = add("rank-scope boxes", self.boxes, count)?;
        check("rank-scope boxes", boxes, self.limits.max_requested_boxes)?;
        // Charge all offset coordinates plus the final lower/upper arrays,
        // including peak stars-and-bars workspace. Cumulative
        // accounting is intentionally conservative across sector calls.
        let per_box = add(
            "rank-scope coordinates per box",
            mul("rank-scope box endpoints", sector.len(), 2)?,
            inactive,
        )?;
        let workspace = inactive.saturating_sub(1);
        let coordinates = add(
            "rank-scope coordinate cells",
            mul("rank-scope coordinate cells", count, per_box)?,
            workspace,
        )?;
        let coordinate_cells = add(
            "rank-scope aggregate coordinate cells",
            self.coordinate_cells,
            coordinates,
        )?;
        check(
            "rank-scope coordinate cells",
            coordinate_cells,
            self.limits.max_requested_box_coordinate_cells,
        )?;
        // The existing nonrecursive enumerator visits each composition once.
        // This conservative structural bound covers enumeration, local-axis
        // transfer, and endpoint construction; it charges no native algebra.
        let per_box_work = add(
            "rank-scope structural work",
            add(
                "rank-scope structural work",
                mul("rank-scope structural work", sector.len(), 3)?,
                mul("rank-scope structural work", inactive, 4)?,
            )?,
            4,
        )?;
        let structural_work = add(
            "rank-scope structural work",
            self.structural_work,
            add(
                "rank-scope structural work",
                mul("rank-scope structural work", count, per_box_work)?,
                inactive,
            )?,
        )?;
        check(
            "rank-scope structural work",
            structural_work,
            self.limits.max_split_operations,
        )?;
        self.boxes = boxes;
        self.coordinate_cells = coordinate_cells;
        self.structural_work = structural_work;

        let offsets = try_build_simplex_offsets(inactive, degree, count).map_err(simplex_error)?;
        let mut result = Vec::new();
        reserve(&mut result, count, "rank-scope slices")?;
        for offset in offsets {
            // Linear transfer preserves the original coordinate map without
            // an arity-sized scratch array or polynomial manipulation.
            let local = || {
                let mut values = offset.iter();
                sector.iter().map(move |&active| {
                    if active {
                        None
                    } else {
                        Some(*values.next().expect("complete simplex offset dimension"))
                    }
                })
            };
            result.push(
                LatticeBox::try_new(local().map(|value| value.unwrap_or(0)), local())
                    .map_err(geometry_error)?,
            );
        }
        Ok(result)
    }
}

/// Exact starting-integral membership, independent of a future proof envelope.
/// The root mask continues to mean a sector downset; it is not zero evidence.
pub(in crate::foundry::artifact) fn entry_contains(
    root: &[bool],
    powers: &[i64],
    max_negative_degree: u64,
) -> Result<bool, ArtifactError> {
    if root.is_empty() {
        return Err(invalid("rank scope needs a nonempty root"));
    }
    if root.len() != powers.len() {
        return Err(ArtifactError::WrongArity {
            expected: root.len(),
            actual: powers.len(),
        });
    }
    let mut degree = 0_u128;
    for (&active, &power) in root.iter().zip(powers) {
        if power > 0 && !active {
            return Ok(false);
        }
        if power < 0 {
            degree = degree
                .checked_add(u128::from(power.unsigned_abs()))
                .ok_or_else(|| overflow("rank-scope entry degree"))?;
        }
    }
    Ok(degree <= u128::from(max_negative_degree))
}

fn add(resource: &'static str, left: usize, right: usize) -> Result<usize, ArtifactError> {
    left.checked_add(right).ok_or_else(|| overflow(resource))
}

fn mul(resource: &'static str, left: usize, right: usize) -> Result<usize, ArtifactError> {
    left.checked_mul(right).ok_or_else(|| overflow(resource))
}

fn check(resource: &'static str, requested: usize, limit: usize) -> Result<(), ArtifactError> {
    if requested > limit {
        return Err(ArtifactError::ResourceLimit {
            resource,
            requested,
            limit,
        });
    }
    Ok(())
}

fn reserve<T>(
    target: &mut Vec<T>,
    requested: usize,
    resource: &'static str,
) -> Result<(), ArtifactError> {
    target
        .try_reserve_exact(requested)
        .map_err(|_| ArtifactError::AllocationFailure {
            resource,
            requested,
        })
}

fn overflow(resource: &'static str) -> ArtifactError {
    ArtifactError::ResourceCountOverflow { resource }
}

fn invalid(detail: &'static str) -> ArtifactError {
    ArtifactError::InvalidRuleShape { detail }
}

fn simplex_error(error: SimplexSupportError) -> ArtifactError {
    match error {
        SimplexSupportError::ResourceCountOverflow { resource } => overflow(resource),
        SimplexSupportError::AllocationFailure {
            resource,
            requested,
        } => ArtifactError::AllocationFailure {
            resource,
            requested,
        },
        SimplexSupportError::Invariant { detail } => invalid(detail),
    }
}

fn geometry_error(error: CompletionGeometryError) -> ArtifactError {
    match error {
        CompletionGeometryError::ResourceCountOverflow { resource } => overflow(resource),
        CompletionGeometryError::AllocationFailure {
            resource,
            requested,
        } => ArtifactError::AllocationFailure {
            resource,
            requested,
        },
        CompletionGeometryError::ResourceLimit {
            resource,
            requested,
            limit,
        } => ArtifactError::ResourceLimit {
            resource,
            requested,
            limit,
        },
        _ => invalid("preflighted rank-scope box construction failed"),
    }
}

#[cfg(test)]
mod tests;
