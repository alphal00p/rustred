use super::super::scan::{minimum_rank, partition_size};
use super::{engine::Budget, model::*};
use crate::foundry::artifact::sign_partition_with_limits;
use crate::foundry::completion::{CompletionGeometryLimits, LatticeBox};
use crate::solver::candidate_reduction::power_domain::{self, DomainPowerBounds};

pub(in crate::solver::candidate_reduction::owners::domains) fn copy_box(
    lower: &[u64],
    upper: &[Option<u64>],
) -> Result<LatticeBox, OwnerAppliedFailure> {
    LatticeBox::try_new(lower.iter().copied(), upper.iter().copied())
        .map_err(|e| OwnerAppliedFailure::Geometry(e.to_string()))
}

pub(super) fn sign_cells<const N: usize>(
    source: &LatticeBox,
    owner: &[bool; N],
    shift: &[i64; N],
    budget: &mut Budget<'_>,
) -> Result<Vec<LatticeBox>, OwnerAppliedFailure> {
    let count = partition_size(source, owner, shift)
        .map_err(|e| OwnerAppliedFailure::Geometry(format!("{e:?}")))?;
    let scratch = count
        .checked_add(12)
        .ok_or(OwnerAppliedFailure::CountOverflow {
            resource: "scratch boxes",
        })?;
    budget.check(scratch, budget.limits.max_scratch_boxes, "scratch boxes")?;
    let cells = scratch
        .checked_mul(N)
        .and_then(|n| n.checked_mul(2))
        .ok_or(OwnerAppliedFailure::CountOverflow {
            resource: "scratch coordinate cells",
        })?;
    budget.check(
        cells,
        budget.limits.max_scratch_coordinate_cells,
        "scratch coordinate cells",
    )?;
    budget.sign_splits(count - 1)?;
    sign_partition_with_limits(
        source,
        owner,
        shift,
        CompletionGeometryLimits {
            max_uncovered_boxes: count,
            max_uncovered_box_coordinate_cells: budget.limits.max_scratch_coordinate_cells,
            max_split_operations: count - 1,
            ..Default::default()
        },
    )
    .map_err(|e| OwnerAppliedFailure::Geometry(e.to_string()))
}

/// Streaming mixed-radix product over only finite sign-changing coordinates.
pub(super) struct Boundaries<'a> {
    source: &'a LatticeBox,
    axes: Vec<usize>,
    values: Vec<u64>,
    exhausted: bool,
}
impl<'a> Boundaries<'a> {
    pub fn new<const N: usize>(
        source: &'a LatticeBox,
        owner: &[bool; N],
        shift: &[i64; N],
        budget: &Budget<'_>,
    ) -> Result<Self, OwnerAppliedFailure> {
        let mut axes = Vec::new();
        let mut values = Vec::new();
        axes.try_reserve_exact(N)
            .map_err(|_| OwnerAppliedFailure::AllocationFailure {
                resource: "crossing axes",
            })?;
        values
            .try_reserve_exact(N)
            .map_err(|_| OwnerAppliedFailure::AllocationFailure {
                resource: "crossing values",
            })?;
        let mut product = 1usize;
        for axis in 0..N {
            let x = i128::from(source.lower()[axis]);
            let target = (if owner[axis] { x + 1 } else { -x }) + i128::from(shift[axis]) > 0;
            if target == owner[axis] {
                continue;
            }
            let upper = source.upper()[axis].ok_or(OwnerAppliedFailure::InternalInvariant(
                "sign-changing coordinate must be finite",
            ))?;
            let width = u128::from(upper) - u128::from(source.lower()[axis]) + 1;
            product = usize::try_from(width)
                .ok()
                .and_then(|n| product.checked_mul(n))
                .ok_or(OwnerAppliedFailure::CountOverflow {
                    resource: "crossing boundary product",
                })?;
            axes.push(axis);
            values.push(source.lower()[axis]);
        }
        let required = budget.stats.boundary_cells.checked_add(product).ok_or(
            OwnerAppliedFailure::CountOverflow {
                resource: "boundary cells",
            },
        )?;
        budget.check(required, budget.limits.max_boundary_cells, "boundary cells")?;
        Ok(Self {
            source,
            axes,
            values,
            exhausted: false,
        })
    }
    pub fn next(
        &mut self,
        budget: &mut Budget<'_>,
    ) -> Result<Option<LatticeBox>, OwnerAppliedFailure> {
        if self.exhausted {
            return Ok(None);
        }
        budget.cancelled()?;
        budget.boundary()?;
        let mut lower = self.source.lower().to_vec();
        let mut upper = self.source.upper().to_vec();
        for (&axis, &value) in self.axes.iter().zip(&self.values) {
            lower[axis] = value;
            upper[axis] = Some(value);
        }
        let mut carry = true;
        for pos in (0..self.axes.len()).rev() {
            let axis = self.axes[pos];
            if self.values[pos] < self.source.upper()[axis].unwrap() {
                self.values[pos] += 1;
                carry = false;
                break;
            }
            self.values[pos] = self.source.lower()[axis];
        }
        self.exhausted = carry;
        LatticeBox::try_new(lower, upper)
            .map(Some)
            .map_err(|e| OwnerAppliedFailure::Geometry(e.to_string()))
    }
}

pub(in crate::solver::candidate_reduction::owners::domains) fn rank_empty<const N: usize>(
    cell: &LatticeBox,
    owner: &[bool; N],
    rank: Option<u32>,
) -> bool {
    rank.is_some_and(|r| minimum_rank(cell.lower(), owner) > u128::from(r))
}

/// Preserve the unconstrained representation; normalize correlated cells before
/// crossing enumeration or exact coefficient work. Each cell owns its effective
/// rank, never a mutable query-global specialization.
pub(super) fn normalize<const N: usize>(
    cell: LatticeBox,
    owner: &[bool; N],
    rank: Option<u32>,
    powers: DomainPowerBounds,
    budget: &mut Budget<'_>,
) -> Result<Option<(LatticeBox, Option<u32>)>, OwnerAppliedFailure> {
    if rank_empty(&cell, owner, rank) {
        return Ok(None);
    }
    if powers.is_unconstrained() {
        return Ok(Some((cell, rank)));
    }
    let Some(projected) = power_domain::project(owner, cell.lower(), cell.upper(), rank, powers)
        .map_err(OwnerAppliedFailure::PowerDomain)?
    else {
        budget.stats.correlation_empty_cells =
            budget.stats.correlation_empty_cells.checked_add(1).ok_or(
                OwnerAppliedFailure::CountOverflow {
                    resource: "correlation empty cells",
                },
            )?;
        return Ok(None);
    };
    let cell = if cell.lower() == projected.lower && cell.upper() == projected.upper {
        cell
    } else {
        LatticeBox::try_new(projected.lower, projected.upper)
            .map_err(|e| OwnerAppliedFailure::Geometry(e.to_string()))?
    };
    Ok(Some((cell, projected.effective_rank)))
}

pub(in crate::solver::candidate_reduction::owners::domains) fn fixed<const N: usize>(
    cell: &LatticeBox,
    owner: &[bool; N],
    rank: Option<u32>,
) -> Result<Vec<(usize, i64)>, usize> {
    let minimum = minimum_rank(cell.lower(), owner);
    let mut out = Vec::with_capacity(N);
    for axis in 0..N {
        let forced = !owner[axis] && rank.is_some_and(|r| u128::from(r) == minimum);
        if cell.upper()[axis] == Some(cell.lower()[axis]) || forced {
            let x = i128::from(cell.lower()[axis]);
            let value = if owner[axis] { x + 1 } else { -x };
            out.push((axis, i64::try_from(value).map_err(|_| axis)?));
        }
    }
    Ok(out)
}

pub(super) struct Image<const N: usize> {
    pub sector: [bool; N],
    pub lower: Vec<u64>,
    pub upper: Vec<Option<u64>>,
    pub rank: Option<u32>,
    pub delta_rank: i128,
}
pub(super) fn image<const N: usize>(
    cell: &LatticeBox,
    owner: &[bool; N],
    shift: &[i64; N],
    rank: Option<u32>,
) -> Result<Image<N>, &'static str> {
    let mut lower = Vec::with_capacity(N);
    let mut upper = Vec::with_capacity(N);
    let mut sector = [false; N];
    let mut delta = 0i128;
    for axis in 0..N {
        let a = i128::from(cell.lower()[axis]);
        let s = i128::from(shift[axis]);
        let child = (if owner[axis] { a + 1 } else { -a }) + s;
        sector[axis] = child > 0;
        let (lo, hi) = match (owner[axis], sector[axis]) {
            (true, true) => (a + s, cell.upper()[axis].map(|b| i128::from(b) + s)),
            (false, false) => {
                delta -= s;
                (a - s, cell.upper()[axis].map(|b| i128::from(b) - s))
            }
            (true, false) => {
                if cell.upper()[axis] != Some(cell.lower()[axis]) {
                    return Err("pinching coordinate is not fixed");
                }
                let y = -1 - a - s;
                delta += y;
                (y, Some(y))
            }
            (false, true) => {
                if cell.upper()[axis] != Some(cell.lower()[axis]) {
                    return Err("activating coordinate is not fixed");
                }
                delta -= a;
                let y = -1 - a + s;
                (y, Some(y))
            }
        };
        lower.push(u64::try_from(lo).map_err(|_| "target lower endpoint outside u64")?);
        upper.push(
            hi.map(u64::try_from)
                .transpose()
                .map_err(|_| "target upper endpoint outside u64")?,
        );
    }
    let rank = rank
        .map(|r| u32::try_from(i128::from(r) + delta))
        .transpose()
        .map_err(|_| "exact translated rank cap outside u32")?;
    Ok(Image {
        sector,
        lower,
        upper,
        rank,
        delta_rank: delta,
    })
}
