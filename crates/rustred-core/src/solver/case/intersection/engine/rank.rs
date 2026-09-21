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
