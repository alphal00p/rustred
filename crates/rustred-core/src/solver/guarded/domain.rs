//! Exact integer boxes used by guarded source identities.
//!
//! Unbounded endpoints denote mathematical integer rays. Operations never
//! saturate endpoints: an unrepresentable finite endpoint is an error.

use crate::solver::{CoordinateCase, SolverError};

/// The semantic meaning of one integral coordinate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IndexRole {
    Ordinary,
    RequiredCut,
    Occupation,
}

/// Inclusive endpoints for one integer coordinate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IndexBounds {
    pub(in crate::solver) lower: Option<i64>,
    pub(in crate::solver) upper: Option<i64>,
}

impl IndexBounds {
    pub fn new(lower: Option<i64>, upper: Option<i64>) -> Result<Self, SolverError> {
        if matches!((lower, upper), (Some(lower), Some(upper)) if lower > upper) {
            return Err(invalid(
                "integer-domain lower endpoint exceeds its upper endpoint",
            ));
        }
        Ok(Self { lower, upper })
    }

    pub const fn unbounded() -> Self {
        Self {
            lower: None,
            upper: None,
        }
    }

    pub const fn fixed(value: i64) -> Self {
        Self {
            lower: Some(value),
            upper: Some(value),
        }
    }

    pub const fn lower(&self) -> Option<i64> {
        self.lower
    }

    pub const fn upper(&self) -> Option<i64> {
        self.upper
    }

    pub fn contains(&self, value: i64) -> bool {
        self.lower.is_none_or(|lower| lower <= value)
            && self.upper.is_none_or(|upper| value <= upper)
    }

    fn intersection(self, other: Self) -> Option<Self> {
        let lower = match (self.lower, other.lower) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
        let upper = match (self.upper, other.upper) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        if matches!((lower, upper), (Some(a), Some(b)) if a > b) {
            None
        } else {
            Some(Self { lower, upper })
        }
    }

    fn is_subset_of(self, other: Self) -> bool {
        other
            .lower
            .is_none_or(|lower| self.lower.is_some_and(|value| value >= lower))
            && other
                .upper
                .is_none_or(|upper| self.upper.is_some_and(|value| value <= upper))
    }
}

/// A nonempty Cartesian product of inclusive integer intervals.
///
/// Empty intersections are represented by `None`, never by reversed bounds.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IndexDomain<const N: usize> {
    pub(in crate::solver) bounds: [IndexBounds; N],
}

impl<const N: usize> IndexDomain<N> {
    pub fn new(bounds: [IndexBounds; N]) -> Result<Self, SolverError> {
        for bound in &bounds {
            IndexBounds::new(bound.lower, bound.upper)?;
        }
        Ok(Self { bounds })
    }

    pub const fn unrestricted() -> Self {
        Self {
            bounds: [IndexBounds::unbounded(); N],
        }
    }

    /// Coordinate admissibility, independently of physical cut-zero rules.
    ///
    /// A negative occupation index is undefined. A nonpositive required-cut
    /// power is a defined zero integral and is handled separately by callers.
    pub fn for_roles(roles: &[IndexRole; N]) -> Self {
        Self {
            bounds: std::array::from_fn(|axis| IndexBounds {
                lower: (roles[axis] == IndexRole::Occupation).then_some(0),
                upper: None,
            }),
        }
    }

    /// Exact ordinary-sector sign bounds intersected with a coordinate case.
    /// Intersect with [`Self::for_roles`] to restrict inactive occupations to 0.
    pub fn from_sector_case(sector: &[bool; N], case: &CoordinateCase<N>) -> Option<Self> {
        if !case.is_in_sector(sector) {
            return None;
        }
        Some(Self {
            bounds: std::array::from_fn(|axis| match case.fixed()[axis] {
                Some(value) => IndexBounds::fixed(i64::from(value)),
                None if sector[axis] => IndexBounds {
                    lower: Some(1),
                    upper: None,
                },
                None => IndexBounds {
                    lower: None,
                    upper: Some(0),
                },
            }),
        })
    }

    pub const fn bounds(&self) -> &[IndexBounds; N] {
        &self.bounds
    }

    pub fn contains(&self, point: &[i64; N]) -> bool {
        self.bounds
            .iter()
            .zip(point)
            .all(|(bound, value)| bound.contains(*value))
    }

    pub fn intersection(&self, other: &Self) -> Option<Self> {
        let mut bounds = self.bounds;
        for (axis, bound) in bounds.iter_mut().enumerate() {
            *bound = bound.intersection(other.bounds[axis])?;
        }
        Some(Self { bounds })
    }

    pub fn is_subset_of(&self, other: &Self) -> bool {
        self.bounds
            .iter()
            .zip(&other.bounds)
            .all(|(a, b)| a.is_subset_of(*b))
    }

    /// Image under `x -> x + shift`.
    pub fn translate(&self, shift: &[i64; N]) -> Result<Self, SolverError> {
        self.map_endpoints(shift, i64::checked_add)
    }

    /// Values of `x` for which `x + shift` belongs to this domain.
    pub fn pullback(&self, shift: &[i64; N]) -> Result<Self, SolverError> {
        self.map_endpoints(shift, i64::checked_sub)
    }

    fn map_endpoints(
        &self,
        shift: &[i64; N],
        operation: fn(i64, i64) -> Option<i64>,
    ) -> Result<Self, SolverError> {
        let mut bounds = self.bounds;
        for (axis, bound) in bounds.iter_mut().enumerate() {
            for endpoint in [&mut bound.lower, &mut bound.upper] {
                if let Some(value) = endpoint {
                    *value = operation(*value, shift[axis])
                        .ok_or_else(|| invalid("integer-domain translation endpoint overflow"))?;
                }
            }
        }
        Ok(Self { bounds })
    }

    /// Split into `x[axis] <= at` and `x[axis] > at`.
    pub fn split(&self, axis: usize, at: i64) -> Result<(Option<Self>, Option<Self>), SolverError> {
        let bound = self
            .bounds
            .get(axis)
            .ok_or_else(|| invalid("integer-domain split axis is out of range"))?;
        if bound.upper.is_some_and(|upper| upper <= at) {
            return Ok((Some(self.clone()), None));
        }
        if bound.lower.is_some_and(|lower| lower > at) {
            return Ok((None, Some(self.clone())));
        }
        let next = at
            .checked_add(1)
            .ok_or_else(|| invalid("integer-domain split endpoint overflow"))?;
        let mut left = self.clone();
        left.bounds[axis].upper = Some(at);
        let mut right = self.clone();
        right.bounds[axis].lower = Some(next);
        Ok((Some(left), Some(right)))
    }

    /// Pairwise disjoint boxes covering `self \ other`, within a caller budget.
    /// At most two slabs per axis are needed for one subtraction.
    pub fn difference(&self, other: &Self, max_boxes: usize) -> Result<Vec<Self>, SolverError> {
        let Some(overlap) = self.intersection(other) else {
            if max_boxes == 0 {
                return Err(invalid("integer-domain difference box budget exhausted"));
            }
            return Ok(vec![self.clone()]);
        };
        let mut result = Vec::new();
        let mut core = self.clone();
        for axis in 0..N {
            if core.bounds[axis].lower != overlap.bounds[axis].lower {
                let lower = overlap.bounds[axis]
                    .lower
                    .expect("a stricter lower bound is finite");
                let upper = lower
                    .checked_sub(1)
                    .ok_or_else(|| invalid("integer-domain difference endpoint overflow"))?;
                let mut slab = core.clone();
                slab.bounds[axis].upper = Some(upper);
                push_box(&mut result, slab, max_boxes)?;
                core.bounds[axis].lower = Some(lower);
            }
            if core.bounds[axis].upper != overlap.bounds[axis].upper {
                let upper = overlap.bounds[axis]
                    .upper
                    .expect("a stricter upper bound is finite");
                let lower = upper
                    .checked_add(1)
                    .ok_or_else(|| invalid("integer-domain difference endpoint overflow"))?;
                let mut slab = core.clone();
                slab.bounds[axis].lower = Some(lower);
                push_box(&mut result, slab, max_boxes)?;
                core.bounds[axis].upper = Some(upper);
            }
        }
        Ok(result)
    }
}

fn invalid(message: &str) -> SolverError {
    SolverError::InvalidInput(message.into())
}

fn push_box<const N: usize>(
    result: &mut Vec<IndexDomain<N>>,
    value: IndexDomain<N>,
    maximum: usize,
) -> Result<(), SolverError> {
    if result.len() >= maximum {
        return Err(invalid("integer-domain difference box budget exhausted"));
    }
    result.push(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn interval(lower: i64, upper: i64) -> IndexBounds {
        IndexBounds::new(Some(lower), Some(upper)).unwrap()
    }

    #[test]
    fn occupation_bulk_is_a_point_and_cut_zero_is_not_an_invalid_coordinate() {
        let roles = [IndexRole::RequiredCut, IndexRole::Occupation];
        let admissible = IndexDomain::for_roles(&roles);
        assert!(admissible.contains(&[-1, 0]));
        assert!(!admissible.contains(&[1, -1]));
        let sector =
            IndexDomain::from_sector_case(&[true, false], &CoordinateCase::generic()).unwrap();
        let bulk = sector.intersection(&admissible).unwrap();
        assert_eq!(bulk.bounds()[1], IndexBounds::fixed(0));
        assert!(bulk.contains(&[2, 0]));
        assert!(!bulk.contains(&[2, 1]));
        assert!(
            IndexDomain::from_sector_case(
                &[true, true],
                &CoordinateCase::new([None, Some(0)]).unwrap()
            )
            .is_none()
        );
    }

    #[test]
    fn translated_source_guards_are_pulled_back_without_negating_minimum() {
        let surface = IndexDomain::new([IndexBounds::new(Some(2), None).unwrap()]).unwrap();
        let shifted = surface.pullback(&[1]).unwrap();
        assert!(shifted.contains(&[1]));
        assert!(!shifted.contains(&[0]));
        assert_eq!(shifted.translate(&[1]).unwrap(), surface);
        let point = IndexDomain::new([IndexBounds::fixed(i64::MIN)]).unwrap();
        assert_eq!(
            point.pullback(&[i64::MIN]).unwrap().bounds()[0],
            IndexBounds::fixed(0)
        );
        assert!(point.translate(&[-1]).is_err());
    }

    #[test]
    fn difference_is_disjoint_exact_and_budgeted() {
        let outer = IndexDomain::new([interval(-2, 4), interval(-3, 5)]).unwrap();
        let inner = IndexDomain::new([interval(0, 2), interval(-1, 2)]).unwrap();
        let pieces = outer.difference(&inner, 4).unwrap();
        assert_eq!(pieces.len(), 4);
        for x in -4..7 {
            for y in -5..8 {
                let count = pieces
                    .iter()
                    .filter(|piece| piece.contains(&[x, y]))
                    .count();
                assert_eq!(
                    count,
                    usize::from(outer.contains(&[x, y]) && !inner.contains(&[x, y]))
                );
            }
        }
        assert!(pieces.iter().all(|piece| piece.is_subset_of(&outer)));
        assert!(outer.difference(&inner, 3).is_err());
        assert!(outer.difference(&outer, 0).unwrap().is_empty());
        assert!(IndexBounds::new(Some(1), Some(0)).is_err());
        assert!(outer.split(2, 0).is_err());
        let (lower, upper) = outer.split(0, 0).unwrap();
        assert!(lower.unwrap().contains(&[0, 0]));
        assert!(upper.unwrap().contains(&[1, 0]));
    }
}
