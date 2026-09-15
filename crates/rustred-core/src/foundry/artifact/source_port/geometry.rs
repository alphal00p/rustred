//! Exact whole-orthant coordinate cover and uniform source-port descent.
//!
//! Local coordinates are x=n-1 for positive powers and x=-n otherwise.
//! `LatticeBox::upper == None` remains genuine mathematical infinity. No i64
//! runtime endpoint is used as a proxy for an infinite proof domain.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use symbolica::prelude::Integer;

use crate::algebra::Coefficient;
use crate::foundry::completion::{BoxCover, CompletionGeometryLimits, LatticeBox};
use crate::sector::{Mask, OrderingPolicy};
use crate::solver::{Case, CoordinateCase, Integral, SectorRule};

use super::{SourcePortAuditError, error};

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
    let case = rule.candidate.case.coordinate().ok_or_else(|| {
        error("coupled affine ownership is not yet supported by the artifact bridge")
    })?;
    if rule.candidate.target != case.integral() {
        return Err(error("rule target differs from its coordinate case"));
    }
    let base = case_box(case, sector)?;
    let original = rule.exceptional_cases(indices, sector).map_err(error)?;
    let mut excluded = Vec::new();
    for case in original.iter().chain(additional_exceptions) {
        let coordinate = case
            .coordinate()
            .ok_or_else(|| error("non-coordinate exceptional ownership remains unsupported"))?;
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
    let parent_mask = Mask::try_new(*sector).map_err(error)?;
    let parent_key = ordering
        .shift_complexity_key(&parent_mask, &[0; N])
        .map_err(error)?;
    for (term_ordinal, term) in rule.candidate.rhs.iter().enumerate() {
        let mut shifts = [0_i64; N];
        for axis in 0..N {
            let parent = rule.candidate.target[axis];
            let child = term.integral[axis];
            if parent.is_symbolic() != child.is_symbolic() {
                return Err(error("RHS changes the case's symbolic coordinate pattern"));
            }
            shifts[axis] = i64::from(child.value()) - i64::from(parent.value());
        }
        let child_key = ordering
            .shift_complexity_key(&parent_mask, &shifts)
            .map_err(error)?;
        for cell in boxes {
            for piece in sign_partition(cell, sector, &shifts)? {
                let actual: [bool; N] = std::array::from_fn(|axis| {
                    let local = i128::from(piece.lower()[axis]);
                    let parent = if sector[axis] { 1 + local } else { -local };
                    parent + i128::from(shifts[axis]) > 0
                });
                let comparison = if actual == *sector {
                    child_key.cmp(&parent_key)
                } else {
                    actual
                        .iter()
                        .filter(|active| **active)
                        .count()
                        .cmp(&parent_mask.active_count())
                        .then_with(|| actual.cmp(sector))
                };
                if comparison == Ordering::Less {
                    continue;
                }
                // Fixed faces and bounded finite sign cells can disappear
                // exactly. Every finite point is checked, while unbounded
                // coordinates always remain symbolic.
                if coefficient_vanishes(&term.coefficient, &piece, sector, indices)? {
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
    for cell in boxes {
        for piece in sign_partition(cell, sector, &shifts)? {
            let actual = std::array::from_fn(|axis| {
                let local = i128::from(piece.lower()[axis]);
                let parent = if sector[axis] { 1 + local } else { -local };
                parent + i128::from(shifts[axis]) > 0
            });
            if zero_sectors.contains(&actual) {
                continue;
            }
            let vanishes = match coefficient {
                Some((coefficient, indices)) => {
                    coefficient_vanishes(coefficient, &piece, sector, indices)?
                }
                None => false,
            };
            if !vanishes {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn coefficient_vanishes<const N: usize>(
    coefficient: &Coefficient,
    cell: &LatticeBox,
    sector: &[bool; N],
    indices: &[usize; N],
) -> Result<bool, SourcePortAuditError> {
    let mut numerator = coefficient.numerator.clone();
    let mut denominator = coefficient.denominator.clone();
    for axis in 0..N {
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
    for axis in 0..N {
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
            .filter(|leaves| {
                *leaves <= CompletionGeometryLimits::default().max_uncovered_boxes as u64
            })
            .ok_or_else(|| error("finite zero-product proof exceeded its geometry budget"))?;
        finite_axes.push((axis, cell.lower()[axis], upper));
    }
    // This is exhaustive restriction of a finite lattice, not interpolation:
    // no value from an infinite interval is ever enumerated, and all remaining
    // symbolic variables must disappear identically from the numerator.
    fn restrict<const N: usize>(
        numerator: &crate::algebra::CoefficientPolynomial,
        denominator: &crate::algebra::CoefficientPolynomial,
        axes: &[(usize, u64, u64)],
        sector: &[bool; N],
        indices: &[usize; N],
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
fn sign_partition<const N: usize>(
    cell: &LatticeBox,
    sector: &[bool; N],
    shifts: &[i64; N],
) -> Result<Vec<LatticeBox>, SourcePortAuditError> {
    let mut pieces = vec![copy_box(cell)?];
    for axis in 0..N {
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
        if next.len() > CompletionGeometryLimits::default().max_uncovered_boxes {
            return Err(error(
                "source-port descent sign partition exceeded its geometry budget",
            ));
        }
        pieces = next;
    }
    Ok(pieces)
}
