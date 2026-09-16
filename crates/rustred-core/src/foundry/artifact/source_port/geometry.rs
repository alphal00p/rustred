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
use std::sync::Arc;

use symbolica::prelude::Integer;

use crate::algebra::Coefficient;
use crate::foundry::completion::{BoxCover, CompletionGeometryLimits, LatticeBox};
use crate::sector::{Mask, OrderingPolicy};
use crate::solver::{Case, CoordinateCase, Integral, SectorRule};

use super::{AffineApplicationDomain, AffineOwnershipRole, SourcePortAuditError, error};

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
    let partition = application_partition(rule, indices, sector, additional_exceptions)?;
    if let Some(domain) = partition.affine_exclusions.first() {
        return Err(SourcePortAuditError::UnsupportedAffineOwnership {
            domain: (**domain).clone(),
            role: AffineOwnershipRole::Exceptional,
        });
    }
    Ok(partition.boxes)
}

/// Exact ownership partition behind the rectangular application prefilter.
///
/// Coordinate exceptional branches become box holes as before.  Coupled
/// affine branches are retained as exact predicates instead of being
/// approximated by a box.  The legacy `application_boxes` wrapper remains
/// fail-closed until callers provide predicate-aware replay/descent proof.
pub(super) struct ApplicationPartition {
    pub(super) boxes: Vec<LatticeBox>,
    pub(super) affine_exclusions: Vec<Arc<AffineApplicationDomain>>,
}

pub(super) fn application_partition<const N: usize>(
    rule: &SectorRule<N>,
    indices: &[usize; N],
    sector: &[bool; N],
    additional_exceptions: &[Case<N>],
) -> Result<ApplicationPartition, SourcePortAuditError> {
    if rule
        .candidate
        .case
        .affine()
        .is_some_and(|case| case.index_variables() != indices)
    {
        return Err(error(
            "affine target and application partition use different index maps",
        ));
    }
    let base = if let Some(case) = rule.candidate.case.coordinate() {
        if rule.candidate.target != case.integral() {
            return Err(error("rule target differs from its coordinate case"));
        }
        case_box(case, sector)?
    } else {
        let affine = rule
            .candidate
            .case
            .affine()
            .expect("non-coordinate case must be affine");
        // An exact contradiction with this sector is safe to discard.
        // This is intentionally the only affine target shortcut here:
        // unresolved affine loci still fail closed below.
        if affine.is_proved_empty_in_sector(sector) {
            return Ok(ApplicationPartition {
                boxes: Vec::new(),
                affine_exclusions: Vec::new(),
            });
        }
        // The coordinate face is an exact rectangular *prefilter* for the
        // coupled locus. Ownership remains affine and is carried separately
        // by the checked rule; it is still sound to remove coordinate
        // exceptional branches from this conservative prefilter before
        // proving descent. Never use this box as an affine coverage claim.
        case_box(affine.face(), sector)?
    };
    let original = rule.exceptional_cases(indices, sector).map_err(error)?;
    let mut excluded = Vec::new();
    let mut affine_exclusions = Vec::new();
    for case in original.iter().chain(additional_exceptions) {
        let coordinate = match case.coordinate() {
            Some(coordinate) => coordinate,
            None => {
                let affine = case.affine().expect("non-coordinate case must be affine");
                if affine.index_variables() != indices {
                    return Err(error(
                        "affine exception and application partition use different index maps",
                    ));
                }
                // A proved-empty exceptional branch contributes no excluded
                // points.  Do not approximate a nonempty affine branch by a
                // box: it remains an unsupported ownership case.
                if affine.is_proved_empty_in_sector(sector) {
                    continue;
                }
                // On an affine target, an exceptional child may add only
                // fixed coordinates while repeating equations already true
                // everywhere on the target. In that case its fixed face is
                // an exact relative exclusion, not an affine approximation.
                // This matters for source replay on activation boundaries:
                // the excluded face must not remain in the replay prefilter.
                if let Some(parent) = rule.candidate.case.affine() {
                    let mut implied = true;
                    for equation in affine.equations() {
                        if !parent.restrict_equation(equation).map_err(error)?.is_zero() {
                            implied = false;
                            break;
                        }
                    }
                    if implied {
                        excluded.push(case_box(affine.face(), sector)?);
                        continue;
                    }
                }
                affine_exclusions.push(Arc::new(
                    AffineApplicationDomain::from_case(affine, sector).map_err(error)?,
                ));
                continue;
            }
        };
        excluded.push(case_box(coordinate, sector)?);
    }
    let cover =
        BoxCover::try_new(N, excluded, CompletionGeometryLimits::default()).map_err(error)?;
    let applicable = cover.uncovered_within(base).map_err(error)?;
    Ok(ApplicationPartition {
        boxes: copy_boxes(applicable.boxes())?,
        affine_exclusions,
    })
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

pub(super) fn prove_descent<const N: usize>(
    rule: &SectorRule<N>,
    boxes: &[LatticeBox],
    sector: &[bool; N],
    ordering: OrderingPolicy,
    indices: &[usize; N],
) -> Result<(), SourcePortAuditError> {
    let affine = rule.candidate.case.affine();
    let affine_domain = affine
        .map(|case| AffineApplicationDomain::from_case(case, sector).map_err(error))
        .transpose()?;
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
            if affine_domain
                .as_ref()
                .is_some_and(|domain| domain.is_proved_empty_in_box(piece))
            {
                return Ok(true);
            }
            let restricted;
            let coefficient = if let Some(affine) = affine {
                restricted = affine.restrict_coefficient(coefficient).map_err(error)?;
                &restricted
            } else {
                coefficient
            };
            coefficient_vanishes_in_affine_domain(
                coefficient,
                piece,
                sector,
                indices,
                CompletionGeometryLimits::default(),
                affine_domain.as_ref(),
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
    let affine = rule.candidate.case.affine();
    if let (Some(affine), Some((value, indices))) = (affine, coefficient) {
        if affine.index_variables() != indices {
            return Err(error(
                "affine zero-product proof uses a different index-variable map",
            ));
        }
        let variables = affine.equations()[0].variables();
        if value.numerator.variables() != variables || value.denominator.variables() != variables {
            return Err(error(
                "affine zero-product proof uses a different polynomial variable map",
            ));
        }
    }
    let affine_domain = affine
        .map(|case| AffineApplicationDomain::from_case(case, sector).map_err(error))
        .transpose()?;
    uniformly_zero_wide_with_limits(
        &shifts,
        // Retain a callback payload even for an integral-only query: an
        // impossible affine sign cell has no contribution regardless of
        // whether a coefficient was supplied.
        Some(&coefficient),
        boxes,
        sector,
        zero_sectors,
        CompletionGeometryLimits::default(),
        |coefficient, piece| {
            // A coordinate exception on an affine target can fix several
            // axes. Its exact box subtraction may leave rectangular sign
            // cells which contain no point of the target. This is the same
            // equation-derived emptiness proof used by strict descent.
            if affine_domain
                .as_ref()
                .is_some_and(|domain| domain.is_proved_empty_in_box(piece))
            {
                return Ok(true);
            }
            match coefficient {
                Some((value, indices)) => coefficient_vanishes_in_affine_domain(
                    value,
                    piece,
                    sector,
                    indices.as_slice(),
                    CompletionGeometryLimits::default(),
                    affine_domain.as_ref(),
                ),
                None => Ok(false),
            }
        },
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
    coefficient_vanishes_in_affine_domain(coefficient, cell, sector, indices, limits, None)
}

fn coefficient_vanishes_in_affine_domain(
    coefficient: &Coefficient,
    cell: &LatticeBox,
    sector: &[bool],
    indices: &[usize],
    limits: CompletionGeometryLimits,
    affine: Option<&AffineApplicationDomain>,
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
    validate_affine_coefficient_domain(affine, coefficient, sector, indices.iter().copied())?;
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
    if affine.is_some() {
        let coordinate_work = usize::try_from(leaves)
            .ok()
            .and_then(|leaves| leaves.checked_mul(sector.len()));
        if sector.len() > limits.max_arity
            || leaves > limits.max_uncovered_boxes as u64
            || coordinate_work.is_none_or(|work| work > limits.max_uncovered_box_coordinate_cells)
        {
            return Err(error(
                "affine finite-leaf proof exceeded its geometry budget",
            ));
        }
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
        cell: &LatticeBox,
        affine: Option<&AffineApplicationDomain>,
        assignments: &mut Vec<(usize, u64)>,
    ) -> Result<bool, SourcePortAuditError> {
        let Some((&(axis, lower, upper), remaining)) = axes.split_first() else {
            if affine_leaf_is_empty(affine, cell, assignments.iter().copied())? {
                return Ok(true);
            }
            return Ok(!denominator.is_zero() && numerator.is_zero());
        };
        for local in lower..=upper {
            if affine.is_some() {
                assignments.push((axis, local));
            }
            let local = Integer::from(local);
            let value = if sector[axis] {
                &local + &Integer::from(1)
            } else {
                -local
            };
            let vanishes = restrict(
                &numerator.replace(indices[axis], &value),
                &denominator.replace(indices[axis], &value),
                remaining,
                sector,
                indices,
                cell,
                affine,
                assignments,
            )?;
            if affine.is_some() {
                assignments.pop();
            }
            if !vanishes {
                return Ok(false);
            }
        }
        Ok(true)
    }
    restrict(
        &numerator,
        &denominator,
        &finite_axes,
        sector,
        indices,
        cell,
        affine,
        &mut Vec::new(),
    )
}

/// Bind the optional exact domain before any empty-leaf shortcut. The
/// indexed cold path additionally authenticates the coefficient itself.
fn validate_affine_coefficient_domain(
    affine: Option<&AffineApplicationDomain>,
    coefficient: &Coefficient,
    sector: &[bool],
    indices: impl IntoIterator<Item = usize>,
) -> Result<(), SourcePortAuditError> {
    let Some(affine) = affine else {
        return Ok(());
    };
    if affine.sector() != sector
        || !affine.indices().iter().copied().eq(indices)
        || coefficient.numerator.variables() != coefficient.denominator.variables()
        || affine
            .equations()
            .iter()
            .any(|equation| equation.variables() != coefficient.numerator.variables())
    {
        return Err(error(
            "affine finite-leaf proof has incompatible sector or variable map",
        ));
    }
    Ok(())
}

/// Refine only already-enumerated finite coordinates in the original box.
/// Infinite coordinates stay infinite; the native equation/gcd/range proof
/// may discard a leaf, but an inconclusive result never discards one.
fn affine_leaf_is_empty(
    affine: Option<&AffineApplicationDomain>,
    cell: &LatticeBox,
    assignments: impl IntoIterator<Item = (usize, u64)>,
) -> Result<bool, SourcePortAuditError> {
    let Some(affine) = affine else {
        return Ok(false);
    };
    let mut lower = cell.lower().to_vec();
    let mut upper = cell.upper().to_vec();
    for (axis, value) in assignments {
        lower[axis] = value;
        upper[axis] = Some(value);
    }
    let leaf = LatticeBox::try_new(lower, upper).map_err(error)?;
    Ok(affine.is_proved_empty_in_box(&leaf))
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
