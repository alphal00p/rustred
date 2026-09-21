//! Bounded negative-coordinate refinement; all algebra stays in the existing
//! native intersection engine. No bound is applied to positive powers.

use symbolica::prelude::Integer;

use crate::solver::{AffineGeometryError, CoordinateCase, GeometryError, Power};

use super::{Case, CaseIntersectionBudget, CaseIntersectionFailure, Engine, WorkItem};

fn fixed_degree<const N: usize>(case: &Case<N>) -> u128 {
    case.fixed()
        .iter()
        .flatten()
        .filter(|&&n| n < 0)
        .map(|n| u128::from(n.unsigned_abs()))
        .sum()
}

pub(super) fn in_scope<const N: usize>(case: &Case<N>, maximum: Option<u32>) -> bool {
    maximum.is_none_or(|maximum| fixed_degree(case) <= u128::from(maximum))
}

impl<const N: usize> Engine<'_, N> {
    pub(super) fn prefer_finite_rank_split(&self, pending: usize) -> bool {
        let Some(reserved) = self.stats.work_items.checked_add(pending) else {
            return false;
        };
        // The current work item was already charged. Reserve one visit for
        // each existing pending item and admit the extra simplex tree only.
        let Some(maximum_nodes) = self
            .limits
            .max_work_items
            .checked_sub(reserved)
            .and_then(|remaining| remaining.checked_add(1))
        else {
            return false;
        };
        finite_rank_tree_nodes(
            &self.current.parent,
            self.sector,
            self.max_numerator_rank,
            maximum_nodes,
        )
        .is_some()
    }

    pub(super) fn split_rank_coordinate(
        &mut self,
        pending: &mut Vec<WorkItem<N>>,
    ) -> Result<bool, CaseIntersectionFailure> {
        let Some(maximum) = self.max_numerator_rank else {
            return Ok(false);
        };
        let Some(remaining) = u128::from(maximum).checked_sub(fixed_degree(&self.current.parent))
        else {
            return Ok(true);
        };
        // Prefer an axis occurring in the restricted nonlinear conjunction;
        // otherwise an inactive axis may still constrain the affine parent.
        let eligible =
            |axis: usize| !self.sector[axis] && self.current.parent.fixed()[axis].is_none();
        let axis = (0..N)
            .find(|&axis| {
                eligible(axis)
                    && self
                        .current
                        .equations
                        .iter()
                        .any(|equation| equation.degree(self.indices[axis]) != 0)
            })
            .or_else(|| (0..N).find(|&axis| eligible(axis)));
        let Some(axis) = axis else { return Ok(false) };
        if remaining > u128::from(Power::MIN.unsigned_abs()) {
            return Err(CaseIntersectionFailure::Admission(
                AffineGeometryError::Coordinate(GeometryError::CompactOverflow {
                    axis,
                    value: -Integer::from(remaining as u64),
                }),
            ));
        }
        let count = remaining as usize + 1;
        if self
            .stats
            .work_items
            .checked_add(pending.len())
            .and_then(|n| n.checked_add(count))
            .is_none_or(|n| n > self.limits.max_work_items)
        {
            return Err(CaseIntersectionFailure::Budget {
                kind: CaseIntersectionBudget::WorkItems,
                limit: self.limits.max_work_items,
            });
        }
        // Retain the complete current affine parent when moving its fixed
        // face into the ordinary queue. No chart coordinate is mistaken for
        // an original numerator index, and every unresolved AND is retained.
        let mut equations = self.current.equations.to_vec();
        if let Some(parent) = self.current.parent.affine() {
            equations.extend_from_slice(parent.equations());
        }
        self.check_terms(&equations)?;
        let equations = std::sync::Arc::from(equations);
        for degree in (0..count).rev() {
            let mut fixed = *self.current.parent.fixed();
            fixed[axis] = Some((-(degree as i32)) as i16);
            let face =
                CoordinateCase::new(fixed).expect("rank refinement preflighted compact powers");
            pending.push(WorkItem {
                parent: face.into(),
                equations: std::sync::Arc::clone(&equations),
                ancestry: self.current.ancestry.clone(),
            });
        }
        self.stats.rank_splits += 1;
        self.stats.rank_children += count;
        Ok(true)
    }
}

/// A conservative count of the entire finite original-coordinate refinement
/// tree, including its already-charged root. This is work scheduling only,
/// not polynomial evaluation, terminal retention, or a proof of feasibility.
fn finite_rank_tree_nodes<const N: usize>(
    parent: &Case<N>,
    sector: &[bool; N],
    maximum: Option<u32>,
    limit: usize,
) -> Option<usize> {
    if sector
        .iter()
        .zip(parent.fixed())
        .any(|(&active, value)| active && value.is_none())
    {
        return None;
    }
    let remaining = u128::from(maximum?).checked_sub(fixed_degree(parent))?;
    // Do not introduce an earlier compact-overflow failure on a branch that
    // the existing symbolic refinements might solve without enumeration.
    if remaining > u128::from(Power::MIN.unsigned_abs()) {
        return None;
    }
    let free = sector
        .iter()
        .zip(parent.fixed())
        .filter(|(active, value)| !**active && value.is_none())
        .count();
    if free == 0 || limit == 0 {
        return None;
    }
    // At depth j there are C(r+j,j) assignments with consumed degree <=r.
    // Summing every depth gives C(r+free+1,free), not just the leaf count.
    // Checked intermediate overflow conservatively declines the priority.
    let remaining = remaining as usize;
    let mut nodes = 1usize;
    for depth in 1..=free {
        let factor = remaining.checked_add(depth)?.checked_add(1)?;
        nodes = nodes.checked_mul(factor)?.checked_div(depth)?;
        if nodes > limit {
            return None;
        }
    }
    Some(nodes)
}

#[cfg(test)]
mod priority_count_tests {
    use super::*;

    #[test]
    fn full_tree_not_just_leaves_controls_priority() {
        let case: Case<3> = CoordinateCase::new([None; 3]).unwrap().into();
        assert_eq!(
            finite_rank_tree_nodes(&case, &[false; 3], Some(8), 220),
            Some(220)
        );
        assert_eq!(
            finite_rank_tree_nodes(&case, &[false; 3], Some(8), 219),
            None
        );
        let case: Case<4> = CoordinateCase::new([None; 4]).unwrap().into();
        assert_eq!(
            finite_rank_tree_nodes(&case, &[false; 4], Some(10), 1365),
            Some(1365)
        );
        assert_eq!(
            finite_rank_tree_nodes(&case, &[false; 4], Some(10), 1364),
            None
        );
    }

    #[test]
    fn no_positive_bound_unscoped_or_overflow_shortcut() {
        let case: Case<3> = CoordinateCase::new([None; 3]).unwrap().into();
        assert_eq!(
            finite_rank_tree_nodes(&case, &[true, false, false], Some(8), usize::MAX),
            None
        );
        assert_eq!(
            finite_rank_tree_nodes(&case, &[false; 3], None, usize::MAX),
            None
        );
        assert_eq!(finite_rank_tree_nodes(&case, &[false; 3], Some(8), 0), None);
        assert_eq!(
            finite_rank_tree_nodes(&case, &[false; 3], Some(u32::MAX), usize::MAX),
            None
        );
        let large: Case<64> = CoordinateCase::new([None; 64]).unwrap().into();
        assert_eq!(
            finite_rank_tree_nodes(
                &large,
                &[false; 64],
                Some(u32::from(Power::MIN.unsigned_abs())),
                usize::MAX
            ),
            None
        );
    }

    #[test]
    fn fixed_negative_degree_is_charged_but_positive_dots_are_not() {
        let case: Case<4> = CoordinateCase::new([Some(-2), None, None, None])
            .unwrap()
            .into();
        assert_eq!(
            finite_rank_tree_nodes(&case, &[false; 4], Some(10), 220),
            Some(220)
        );
        assert_eq!(
            finite_rank_tree_nodes(&case, &[false; 4], Some(1), usize::MAX),
            None
        );
        let case: Case<2> = CoordinateCase::new([Some(Power::MAX), None])
            .unwrap()
            .into();
        assert_eq!(
            finite_rank_tree_nodes(&case, &[true, false], Some(2), 4),
            Some(4)
        );
        let fixed: Case<2> = CoordinateCase::new([Some(1), Some(0)]).unwrap().into();
        assert_eq!(
            finite_rank_tree_nodes(&fixed, &[true, false], Some(10), usize::MAX),
            None
        );
    }
}
