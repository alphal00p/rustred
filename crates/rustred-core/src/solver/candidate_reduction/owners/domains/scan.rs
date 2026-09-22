use std::ops::ControlFlow;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::foundry::artifact::sign_partition_with_limits;
use crate::foundry::completion::{CompletionGeometryLimits, LatticeBox};

use super::*;
use crate::solver::candidate_reduction::owners::CandidateOwnerPrograms;

impl<const N: usize> CandidateOwnerPrograms<N> {
    /// Stream conservative one-hop successors without enumerating positive dots.
    ///
    /// The actual requested rank is explicit and may exceed the saved entry
    /// scope. All original guards remain borrowed predicates. No source search,
    /// first-applicable partition, recursive closure, or MissingRule conclusion
    /// is performed. The callback owns the budget for any retained descriptors.
    /// On failure/cancellation, prior callback output is only an incomplete prefix.
    pub fn visit_owner_rule_successors<'a>(
        &'a self,
        sector: [bool; N],
        max_numerator_rank: Option<u32>,
        limits: OwnerSuccessorLimits,
        cancellation: &AtomicBool,
        mut visit: impl FnMut(OwnerSuccessorRegion<'a, N>) -> ControlFlow<()>,
    ) -> Result<OwnerSuccessorStats, OwnerSuccessorError> {
        let mut stats = OwnerSuccessorStats::default();
        let result = scan(
            self,
            sector,
            max_numerator_rank,
            limits,
            cancellation,
            &mut visit,
            &mut stats,
        );
        result
            .map(|()| stats)
            .map_err(|failure| OwnerSuccessorError { failure, stats })
    }
}

fn charge(
    value: &mut usize,
    addition: usize,
    limit: usize,
    resource: &'static str,
) -> Result<(), OwnerSuccessorFailure> {
    let requested = value
        .checked_add(addition)
        .ok_or(OwnerSuccessorFailure::CountOverflow { resource })?;
    if requested > limit {
        return Err(OwnerSuccessorFailure::ResourceLimit {
            resource,
            requested,
            limit,
        });
    }
    *value = requested;
    Ok(())
}

fn cancelled(cancellation: &AtomicBool) -> Result<(), OwnerSuccessorFailure> {
    if cancellation.load(Ordering::Acquire) {
        Err(OwnerSuccessorFailure::Cancelled)
    } else {
        Ok(())
    }
}

fn scan<'a, const N: usize>(
    programs: &'a CandidateOwnerPrograms<N>,
    sector: [bool; N],
    rank: Option<u32>,
    limits: OwnerSuccessorLimits,
    cancellation: &AtomicBool,
    visit: &mut impl FnMut(OwnerSuccessorRegion<'a, N>) -> ControlFlow<()>,
    stats: &mut OwnerSuccessorStats,
) -> Result<(), OwnerSuccessorFailure> {
    cancelled(cancellation)?;
    let owner = programs
        .owners
        .get(&sector)
        .ok_or(OwnerSuccessorFailure::UnknownOwner)?;
    check(
        N,
        CompletionGeometryLimits::default().max_arity,
        "index arity",
    )?;
    let cells = N
        .checked_mul(2)
        .ok_or(OwnerSuccessorFailure::CountOverflow {
            resource: "source coordinate cells",
        })?;
    check(1, limits.max_scratch_boxes, "scratch boxes")?;
    check(
        cells,
        limits.max_scratch_coordinate_cells,
        "source coordinate cells",
    )?;
    for (batch, prepared) in owner.batches.iter().enumerate() {
        for rule in &prepared.rules {
            cancelled(cancellation)?;
            charge(&mut stats.rules, 1, limits.max_rules, "rules")?;
            let lower: [u64; N] = std::array::from_fn(|axis| {
                rule.fixed[axis].map_or(0, |n| {
                    if sector[axis] {
                        (i64::from(n) - 1) as u64
                    } else {
                        (-i64::from(n)) as u64
                    }
                })
            });
            let upper: [Option<u64>; N] = std::array::from_fn(|axis| {
                rule.fixed[axis].map(|_| lower[axis]).or_else(|| {
                    if sector[axis] {
                        None
                    } else {
                        rank.map(u64::from)
                    }
                })
            });
            if rank.is_some_and(|rank| minimum_rank(&lower, &sector) > u128::from(rank)) {
                charge(
                    &mut stats.rank_empty_prefilters,
                    1,
                    usize::MAX,
                    "rank-empty prefilters",
                )?;
                continue;
            }
            let source = LatticeBox::try_new(lower, upper)
                .map_err(|e| OwnerSuccessorFailure::Geometry(e.to_string()))?;
            for (term_ordinal, term) in rule.rhs.iter().enumerate() {
                cancelled(cancellation)?;
                charge(&mut stats.terms, 1, limits.max_terms, "terms")?;
                if term.coefficient.raw().is_zero() {
                    charge(&mut stats.exact_zero_terms, 1, usize::MAX, "zero terms")?;
                    continue;
                }
                let remaining_splits = limits
                    .max_split_operations
                    .saturating_sub(stats.split_operations);
                // Each crossing doubles the Cartesian sign partition. Preflight
                // the whole scratch envelope BEFORE the existing splitter, which
                // checks its staged output only at the end of each axis.
                let piece_count = partition_size(&source, &sector, &term.shift)?;
                check(piece_count - 1, remaining_splits, "sign splits")?;
                let scratch_boxes =
                    piece_count
                        .checked_add(4)
                        .ok_or(OwnerSuccessorFailure::CountOverflow {
                            resource: "scratch boxes",
                        })?;
                check(scratch_boxes, limits.max_scratch_boxes, "scratch boxes")?;
                let scratch_cells = scratch_boxes.checked_mul(cells).ok_or(
                    OwnerSuccessorFailure::CountOverflow {
                        resource: "scratch coordinate cells",
                    },
                )?;
                check(
                    scratch_cells,
                    limits.max_scratch_coordinate_cells,
                    "scratch coordinate cells",
                )?;
                let pieces = sign_partition_with_limits(
                    &source,
                    &sector,
                    &term.shift,
                    CompletionGeometryLimits {
                        max_uncovered_boxes: piece_count,
                        max_uncovered_box_coordinate_cells: limits.max_scratch_coordinate_cells,
                        max_split_operations: remaining_splits,
                        ..Default::default()
                    },
                )
                .map_err(|e| OwnerSuccessorFailure::Geometry(e.to_string()))?;
                charge(
                    &mut stats.split_operations,
                    pieces.len().saturating_sub(1),
                    limits.max_split_operations,
                    "sign splits",
                )?;
                for source_box in pieces {
                    cancelled(cancellation)?;
                    if rank
                        .is_some_and(|r| minimum_rank(source_box.lower(), &sector) > u128::from(r))
                    {
                        charge(
                            &mut stats.rank_empty_prefilters,
                            1,
                            usize::MAX,
                            "rank-empty prefilters",
                        )?;
                        continue;
                    }
                    let target_sector = std::array::from_fn(|axis| {
                        let x = i128::from(source_box.lower()[axis]);
                        let n = if sector[axis] { 1 + x } else { -x };
                        n + i128::from(term.shift[axis]) > 0
                    });
                    let transition = if target_sector == sector {
                        OwnerSuccessorTransition::SameSupport
                    } else if target_sector
                        .iter()
                        .zip(sector)
                        .all(|(&target, source)| !target || source)
                    {
                        OwnerSuccessorTransition::StrictPinch
                    } else {
                        OwnerSuccessorTransition::UnsupportedSupportChange
                    };
                    let delta = (target_sector == sector).then(|| {
                        sector
                            .iter()
                            .zip(term.shift)
                            .filter(|(active, _)| !**active)
                            .map(|(_, delta)| -i128::from(delta))
                            .sum()
                    });
                    let target_rank_upper =
                        rank_upper(&source_box, &sector, &target_sector, &term.shift, rank);
                    charge(
                        &mut stats.regions,
                        1,
                        limits.max_regions,
                        "successor regions",
                    )?;
                    let descriptor = OwnerSuccessorRegion {
                        source_sector: sector,
                        target_sector,
                        source_rank_limit: rank,
                        target_rank_upper,
                        same_support_rank_delta: delta,
                        source_box,
                        batch,
                        term_ordinal,
                        rule,
                        term,
                        source_conditions: &programs.context.shared.source_conditions,
                        transition,
                        installed_target_owner: programs.owners.contains_key(&target_sector),
                        exact_zero_sector: programs
                            .context
                            .shared
                            .zero_sectors
                            .contains(&target_sector),
                    };
                    if visit(descriptor).is_break() {
                        return Err(OwnerSuccessorFailure::StoppedByConsumer);
                    }
                }
            }
        }
    }
    cancelled(cancellation)
}

fn check(
    requested: usize,
    limit: usize,
    resource: &'static str,
) -> Result<(), OwnerSuccessorFailure> {
    if requested > limit {
        Err(OwnerSuccessorFailure::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

pub(super) fn partition_size<const N: usize>(
    cell: &LatticeBox,
    sector: &[bool; N],
    shift: &[i64; N],
) -> Result<usize, OwnerSuccessorFailure> {
    let mut pieces = 1_usize;
    for axis in 0..N {
        let threshold = if sector[axis] && shift[axis] < 0 {
            shift[axis].unsigned_abs()
        } else if !sector[axis] && shift[axis] > 0 {
            shift[axis] as u64
        } else {
            continue;
        };
        if cell.lower()[axis] < threshold
            && cell.upper()[axis].is_none_or(|upper| upper >= threshold)
        {
            pieces = pieces
                .checked_mul(2)
                .ok_or(OwnerSuccessorFailure::CountOverflow {
                    resource: "sign partition boxes",
                })?;
        }
    }
    Ok(pieces)
}

pub(super) fn minimum_rank(lower: &[u64], sector: &[bool]) -> u128 {
    lower
        .iter()
        .zip(sector)
        .filter(|(_, active)| !**active)
        .map(|(&x, _)| u128::from(x))
        .sum()
}

/// Exact sign cells make target rank affine. Maximize its positive-coefficient
/// inactive coordinates within the input simplex/box, and its negative-
/// coefficient pinched positive coordinates at their lower endpoints. Native
/// equalities/exclusions can only shrink this conservative prefilter bound.
fn rank_upper<const N: usize>(
    cell: &LatticeBox,
    source: &[bool; N],
    target: &[bool; N],
    shift: &[i64; N],
    rank: Option<u32>,
) -> Option<u128> {
    let mut inactive_upper = Some(0_u128);
    let mut spent_lower = 0_u128;
    let mut constant = 0_i128;
    for axis in 0..N {
        match (source[axis], target[axis]) {
            (false, false) => {
                inactive_upper = inactive_upper
                    .zip(cell.upper()[axis])
                    .map(|(sum, upper)| sum + u128::from(upper));
                constant -= i128::from(shift[axis]);
            }
            (false, true) => spent_lower += u128::from(cell.lower()[axis]),
            (true, false) => {
                constant += -i128::from(cell.lower()[axis]) - 1 - i128::from(shift[axis])
            }
            (true, true) => (),
        }
    }
    if let Some(rank) = rank {
        let available = u128::from(rank) - spent_lower; // nonempty rank prefilter already checked
        inactive_upper = Some(inactive_upper.map_or(available, |upper| upper.min(available)));
    }
    inactive_upper.map(|upper| {
        // N is bounded by the reused geometry utility (4096); u64 coordinates
        // and i64 shifts therefore fit this exact i128 calculation.
        let bound = i128::try_from(upper).expect("bounded coordinate sum") + constant;
        u128::try_from(bound).expect("admitted sign cell has nonnegative rank")
    })
}
