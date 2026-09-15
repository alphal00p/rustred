//! Exact whole-orthant coordinate cover and uniform source-port descent.
//!
//! Local coordinates are x=n-1 for positive powers and x=-n otherwise.
//! `LatticeBox::upper == None` remains genuine mathematical infinity. No i64
//! runtime endpoint is used as a proxy for an infinite proof domain.

#[cfg(test)]
#[path = "geometry/grounding.rs"]
mod grounding;

pub(super) mod bounded;

use std::cmp::Ordering;
use std::collections::BTreeSet;

use symbolica::prelude::Integer;

use crate::algebra::Coefficient;
use crate::foundry::completion::{BoxCover, CompletionGeometryLimits, LatticeBox};
use crate::sector::{Mask, OrderingPolicy};
use crate::solver::{Case, CoordinateCase, Integral, SectorRule};

use super::{error, AffineApplicationDomain, AffineOwnershipRole, SourcePortAuditError};

pub(super) fn copy_boxes(boxes: &[LatticeBox]) -> Result<Vec<LatticeBox>, SourcePortAuditError> {
    boxes.iter().map(copy_box).collect()
}

fn copy_box(cell: &LatticeBox) -> Result<LatticeBox, SourcePortAuditError> {
    LatticeBox::try_new(cell.lower().iter().copied(), cell.upper().iter().copied()).map_err(error)
}

fn case_box<const N: usize>(
    case: &CoordinateCase<N>,
    sector: &[bool; N],
) -> Result<LatticeBox, SourcePortAuditError> {
    if !case.is_in_sector(sector) {
        return Err(error("coordinate case lies outside its declared sector"));
    }
    let mut lower = [0; N];
    let mut upper = [None; N];
    for (axis, fixed) in case.fixed().iter().enumerate() {
        if let Some(fixed) = fixed {
            let local = if sector[axis] {
                i64::from(*fixed) - 1
            } else {
                -i64::from(*fixed)
            };
            lower[axis] = u64::try_from(local).map_err(error)?;
            upper[axis] = Some(lower[axis]);
        }
    }
    LatticeBox::try_new(lower, upper).map_err(error)
}

pub(super) fn application_boxes<const N: usize>(
    rule: &SectorRule<N>,
    indices: &[usize; N],
    sector: &[bool; N],
    additional_exceptions: &[Case<N>],
) -> Result<Vec<LatticeBox>, SourcePortAuditError> {
    let case = match rule.candidate.case.coordinate() {
        Some(case) => case,
        None => {
            let affine = rule
                .candidate
                .case
                .affine()
                .expect("non-coordinate case must be affine");
            // An exact contradiction with this sector is safe to discard.
            // This is intentionally the only affine target shortcut here:
            // unresolved affine loci still fail closed below.
            if affine.is_proved_empty_in_sector(sector) {
                return Ok(Vec::new());
            }
            // The coordinate face is an exact rectangular *prefilter* for
            // the coupled locus.  Ownership remains affine and is carried
            // separately by the checked rule; callers must apply its exact
            // predicate after this cheap box test.  Never use this box as a
            // coverage claim for the affine equations themselves.
            return Ok(vec![case_box(affine.face(), sector)?]);
        }
    };
    if rule.candidate.target != case.integral() {
        return Err(error("rule target differs from its coordinate case"));
    }
    let base = case_box(case, sector)?;
    let original = rule.exceptional_cases(indices, sector).map_err(error)?;
    let mut excluded = Vec::new();
    for case in original.iter().chain(additional_exceptions) {
        let coordinate = match case.coordinate() {
            Some(coordinate) => coordinate,
            None => {
                let affine = case.affine().expect("non-coordinate case must be affine");
                // A proved-empty exceptional branch contributes no excluded
                // points.  Do not approximate a nonempty affine branch by a
                // box: it remains an unsupported ownership case.
                if affine.is_proved_empty_in_sector(sector) {
                    continue;
                }
                let domain = AffineApplicationDomain::from_case(affine, sector).map_err(error)?;
                return Err(SourcePortAuditError::UnsupportedAffineOwnership {
                    domain,
                    role: AffineOwnershipRole::Exceptional,
                });
            }
        };
        excluded.push(case_box(coordinate, sector)?);
    }
    let cover =
        BoxCover::try_new(N, excluded, CompletionGeometryLimits::default()).map_err(error)?;
    let applicable = cover.uncovered_within(base).map_err(error)?;
    copy_boxes(applicable.boxes())
}

pub(super) fn terminal_boxes<const N: usize>(
    terminals: &[Integral<N>],
    sector: &[bool; N],
) -> Result<Vec<LatticeBox>, SourcePortAuditError> {
    let mut seen = BTreeSet::new();
    terminals
        .iter()
        .map(|terminal| {
            if terminal.powers().iter().any(|power| power.is_symbolic()) {
                return Err(error(
                    "a positive-dimensional residual cannot be a finite terminal",
                ));
            }
            let fixed: [Option<i16>; N] = std::array::from_fn(|axis| Some(terminal[axis].value()));
            if !seen.insert(fixed) {
                return Err(error("duplicate finite terminal"));
            }
            case_box(&CoordinateCase::new(fixed).map_err(error)?, sector)
        })
        .collect()
}

pub(super) fn uncovered(
    arity: usize,
    boxes: Vec<LatticeBox>,
) -> Result<(usize, usize), SourcePortAuditError> {
    let cover =
        BoxCover::try_new(arity, boxes, CompletionGeometryLimits::default()).map_err(error)?;
    let complement = cover.uncovered_partition().map_err(error)?;
    Ok((
        complement.boxes().len(),
        complement
            .boxes()
            .iter()
            .filter(|cell| cell.free_dimension() > 0)
            .count(),
    ))
}

pub(super) fn prove_descent<const N: usize>(
    rule: &SectorRule<N>,
    boxes: &[LatticeBox],
    sector: &[bool; N],
    ordering: OrderingPolicy,
    indices: &[usize; N],
) -> Result<(), SourcePortAuditError> {
    let mut terms = Vec::with_capacity(rule.candidate.rhs.len());
    for term in &rule.candidate.rhs {
        let mut shifts = [0_i64; N];
        for axis in 0..N {
            let parent = rule.candidate.target[axis];
            let child = term.integral[axis];
            if parent.is_symbolic() != child.is_symbolic() {
                return Err(error("RHS changes the case's symbolic coordinate pattern"));
            }
            shifts[axis] = i64::from(child.value()) - i64::from(parent.value());
        }
        terms.push((shifts, &term.coefficient));
    }
    prove_wide_descent_with_limits(
        terms
            .iter()
            .map(|(shift, coefficient)| (shift.as_slice(), *coefficient)),
        boxes,
        sector,
        ordering,
        CompletionGeometryLimits::default(),
        |coefficient, piece| {
            coefficient_vanishes(
                coefficient,
                piece,
                sector,
                indices,
                CompletionGeometryLimits::default(),
            )
        },
    )
}

/// The same whole-unbounded order proof for durable wide-coordinate rows.
/// This entry accepts arithmetic data, not a source replay certificate.
pub(super) fn prove_wide_descent_with_limits<'a, T: 'a>(
    terms: impl IntoIterator<Item = (&'a [i64], &'a T)>,
    boxes: &[LatticeBox],
    sector: &[bool],
    ordering: OrderingPolicy,
    limits: CompletionGeometryLimits,
    mut vanishes: impl FnMut(&T, &LatticeBox) -> Result<bool, SourcePortAuditError>,
) -> Result<(), SourcePortAuditError> {
    let parent_mask = Mask::try_new(sector.iter().copied()).map_err(error)?;
    let zero = vec![0; sector.len()];
    let parent_key = ordering
        .shift_complexity_key(&parent_mask, &zero)
        .map_err(error)?;
    for (term_ordinal, (shifts, coefficient)) in terms.into_iter().enumerate() {
        if shifts.len() != sector.len() {
            return Err(error("wide descent has incompatible shift arity"));
        }
        let child_key = ordering
            .shift_complexity_key(&parent_mask, shifts)
            .map_err(error)?;
        for cell in boxes {
            for piece in sign_partition_with_limits(cell, sector, shifts, limits)? {
                let actual: Vec<bool> = (0..sector.len())
                    .map(|axis| {
                        let local = i128::from(piece.lower()[axis]);
                        let parent = if sector[axis] { 1 + local } else { -local };
                        parent + i128::from(shifts[axis]) > 0
                    })
                    .collect();
                let comparison = if actual == sector {
                    child_key.cmp(&parent_key)
                } else {
                    actual
                        .iter()
                        .filter(|active| **active)
                        .count()
                        .cmp(&parent_mask.active_count())
                        .then_with(|| actual.as_slice().cmp(sector))
                };
                if comparison == Ordering::Less || vanishes(coefficient, &piece)? {
                    continue;
                }
                return Err(error(format!(
                    "term {term_ordinal} is not uniformly lower on local box {:?}..{:?}; shift={shifts:?}, child_sector={actual:?}",
                    piece.lower(),
                    piece.upper(),
                )));
            }
        }
    }
    Ok(())
}

/// A zero projection is admitted only if every physical sign cell belongs
/// to the independently authenticated zero census. This deliberately does
/// not reuse the source solver's symbolic assumed-sector pruning.
#[cfg(test)]
pub(super) fn uniformly_zero_column<const N: usize>(
    rule: &SectorRule<N>,
    integral: Integral<N>,
    boxes: &[LatticeBox],
    sector: &[bool; N],
    zero_sectors: &[[bool; N]],
) -> Result<bool, SourcePortAuditError> {
    uniformly_zero_contribution(rule, integral, None, boxes, sector, zero_sectors)
}

/// Prove the complete coefficient-times-integral contribution zero. An
/// activating column need not itself vanish if its coefficient vanishes
/// exactly on every nonzero-sector sign cell. This uses that original term's
/// own coefficient or the complete weighted residual coefficient, never an
/// unrelated coefficient copied from the final rule.
pub(super) fn uniformly_zero_term<const N: usize>(
    rule: &SectorRule<N>,
    term: &crate::solver::Term<N, Coefficient>,
    boxes: &[LatticeBox],
    sector: &[bool; N],
    zero_sectors: &[[bool; N]],
    indices: &[usize; N],
) -> Result<bool, SourcePortAuditError> {
    uniformly_zero_contribution(
        rule,
        term.integral,
        Some((&term.coefficient, indices)),
        boxes,
        sector,
        zero_sectors,
    )
}

fn uniformly_zero_contribution<const N: usize>(
    rule: &SectorRule<N>,
    integral: Integral<N>,
    coefficient: Option<(&Coefficient, &[usize; N])>,
    boxes: &[LatticeBox],
    sector: &[bool; N],
    zero_sectors: &[[bool; N]],
) -> Result<bool, SourcePortAuditError> {
    if boxes.is_empty() {
        return Ok(false);
    }
    let mut shifts = [0_i64; N];
    for axis in 0..N {
        if integral[axis].is_symbolic() != rule.candidate.target[axis].is_symbolic() {
            return Err(error(
                "source column changes the target case's symbolic pattern",
            ));
        }
        shifts[axis] =
            i64::from(integral[axis].value()) - i64::from(rule.candidate.target[axis].value());
    }
    uniformly_zero_wide(
        &shifts,
        coefficient.map(|(value, indices)| (value, indices.as_slice())),
        boxes,
        sector,
        zero_sectors,
    )
}

/// Wide original-source/product proof used by durable-payload lowering.
/// No compact search-power conversion occurs at this boundary.
pub(super) fn uniformly_zero_wide<Z: AsRef<[bool]>>(
    shifts: &[i64],
    coefficient: Option<(&Coefficient, &[usize])>,
    boxes: &[LatticeBox],
    sector: &[bool],
    zero_sectors: &[Z],
) -> Result<bool, SourcePortAuditError> {
    uniformly_zero_wide_with_limits(
        shifts,
        coefficient.as_ref(),
        boxes,
        sector,
        zero_sectors,
        CompletionGeometryLimits::default(),
        |(coefficient, indices), piece| {
            coefficient_vanishes(
                coefficient,
                piece,
                sector,
                indices,
                CompletionGeometryLimits::default(),
            )
        },
    )
}

pub(super) fn uniformly_zero_wide_with_limits<Z: AsRef<[bool]>, T>(
    shifts: &[i64],
    coefficient: Option<&T>,
    boxes: &[LatticeBox],
    sector: &[bool],
    zero_sectors: &[Z],
    limits: CompletionGeometryLimits,
    mut coefficient_vanishes: impl FnMut(&T, &LatticeBox) -> Result<bool, SourcePortAuditError>,
) -> Result<bool, SourcePortAuditError> {
    if boxes.is_empty() {
        return Ok(false);
    }
    if shifts.len() != sector.len() {
        return Err(error("wide product proof has incompatible index arity"));
    }
    for cell in boxes {
        for piece in sign_partition_with_limits(cell, sector, shifts, limits)? {
            let actual: Vec<bool> = (0..sector.len())
                .map(|axis| {
                    let local = i128::from(piece.lower()[axis]);
                    let parent = if sector[axis] { 1 + local } else { -local };
                    parent + i128::from(shifts[axis]) > 0
                })
                .collect();
            if zero_sectors
                .iter()
                .any(|zero| zero.as_ref() == actual.as_slice())
            {
                continue;
            }
            let vanishes = match coefficient {
                Some(coefficient) => coefficient_vanishes(coefficient, &piece)?,
                None => false,
            };
            if !vanishes {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn coefficient_vanishes(
    coefficient: &Coefficient,
    cell: &LatticeBox,
    sector: &[bool],
    indices: &[usize],
    limits: CompletionGeometryLimits,
) -> Result<bool, SourcePortAuditError> {
    if cell.arity() != sector.len()
        || indices.len() != sector.len()
        || indices
            .iter()
            .any(|index| *index >= coefficient.numerator.nvars())
    {
        return Err(error(
            "coefficient restriction has incompatible variable geometry",
        ));
    }
    let mut numerator = coefficient.numerator.clone();
    let mut denominator = coefficient.denominator.clone();
    for axis in 0..sector.len() {
        if cell.upper()[axis] == Some(cell.lower()[axis]) {
            let local = Integer::from(cell.lower()[axis]);
            let value = if sector[axis] {
                &local + &Integer::from(1)
            } else {
                -local
            };
            numerator = numerator.replace(indices[axis], &value);
            denominator = denominator.replace(indices[axis], &value);
        }
    }
    let mut finite_axes = Vec::new();
    let mut leaves = 1_u64;
    for axis in 0..sector.len() {
        let Some(upper) = cell.upper()[axis] else {
            continue;
        };
        if upper == cell.lower()[axis]
            || (numerator.degree(indices[axis]) == 0 && denominator.degree(indices[axis]) == 0)
        {
            continue;
        }
        let width = upper
            .checked_sub(cell.lower()[axis])
            .and_then(|width| width.checked_add(1))
            .ok_or_else(|| {
                error("finite zero-product proof interval exceeds its geometry budget")
            })?;
        leaves = leaves
            .checked_mul(width)
            .filter(|leaves| *leaves <= limits.max_uncovered_boxes as u64)
            .ok_or_else(|| error("finite zero-product proof exceeded its geometry budget"))?;
        finite_axes.push((axis, cell.lower()[axis], upper));
    }
    // This is exhaustive restriction of a finite lattice, not interpolation:
    // no value from an infinite interval is ever enumerated, and all remaining
    // symbolic variables must disappear identically from the numerator.
    fn restrict(
        numerator: &crate::algebra::CoefficientPolynomial,
        denominator: &crate::algebra::CoefficientPolynomial,
        axes: &[(usize, u64, u64)],
        sector: &[bool],
        indices: &[usize],
    ) -> bool {
        let Some((&(axis, lower, upper), remaining)) = axes.split_first() else {
            return !denominator.is_zero() && numerator.is_zero();
        };
        (lower..=upper).all(|local| {
            let local = Integer::from(local);
            let value = if sector[axis] {
                &local + &Integer::from(1)
            } else {
                -local
            };
            restrict(
                &numerator.replace(indices[axis], &value),
                &denominator.replace(indices[axis], &value),
                remaining,
                sector,
                indices,
            )
        })
    }
    Ok(restrict(
        &numerator,
        &denominator,
        &finite_axes,
        sector,
        indices,
    ))
}

/// Split exactly where a translated power changes sign. This is a bounded
/// combinatorial operation on intervals, not an algebraic root solver.
pub(super) fn sign_partition(
    cell: &LatticeBox,
    sector: &[bool],
    shifts: &[i64],
) -> Result<Vec<LatticeBox>, SourcePortAuditError> {
    sign_partition_with_limits(cell, sector, shifts, CompletionGeometryLimits::default())
}

fn sign_partition_with_limits(
    cell: &LatticeBox,
    sector: &[bool],
    shifts: &[i64],
    limits: CompletionGeometryLimits,
) -> Result<Vec<LatticeBox>, SourcePortAuditError> {
    if cell.arity() != sector.len() || shifts.len() != sector.len() {
        return Err(error("sign partition has incompatible index arity"));
    }
    if sector.len() > limits.max_arity || limits.max_uncovered_boxes == 0 {
        return Err(error("sign partition exceeds its geometry resource policy"));
    }
    if sector
        .len()
        .checked_mul(2)
        .is_none_or(|entries| entries > limits.max_uncovered_box_coordinate_cells)
    {
        return Err(error("sign partition has incompatible index arity"));
    }
    let mut pieces = vec![copy_box(cell)?];
    let mut splits = 0usize;
    for axis in 0..sector.len() {
        let threshold = if sector[axis] && shifts[axis] < 0 {
            shifts[axis].unsigned_abs()
        } else if !sector[axis] && shifts[axis] > 0 {
            shifts[axis] as u64
        } else {
            continue;
        };
        let mut next = Vec::new();
        for piece in pieces {
            if piece.lower()[axis] >= threshold
                || piece.upper()[axis].is_some_and(|upper| upper < threshold)
            {
                next.push(piece);
                continue;
            }
            splits = splits
                .checked_add(1)
                .filter(|count| *count <= limits.max_split_operations)
                .ok_or_else(|| error("sign partition exceeds its split-operation budget"))?;
            let mut left_upper = piece.upper().to_vec();
            left_upper[axis] = Some(threshold - 1);
            next.push(
                LatticeBox::try_new(piece.lower().iter().copied(), left_upper).map_err(error)?,
            );
            let mut right_lower = piece.lower().to_vec();
            right_lower[axis] = threshold;
            next.push(
                LatticeBox::try_new(right_lower, piece.upper().iter().copied()).map_err(error)?,
            );
        }
        if next.len() > limits.max_uncovered_boxes
            || next
                .len()
                .checked_mul(sector.len())
                .and_then(|n| n.checked_mul(2))
                .is_none_or(|cells| cells > limits.max_uncovered_box_coordinate_cells)
        {
            return Err(error(
                "source-port descent sign partition exceeded its geometry budget",
            ));
        }
        pieces = next;
    }
    Ok(pieces)
}
