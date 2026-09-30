use std::cmp::Ordering;

use super::descriptor::arity;
use super::{CompiledOrder, CoordinateGroups, Direction, Error};

/// Honest first decisive component, suitable for higher-level descent evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Component {
    SupportCount,
    SupportWeight,
    SupportAxis(usize),
    DegreeRow(usize),
    Coordinate(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Comparison {
    pub ordering: Ordering,
    pub component: Option<Component>,
}

impl Comparison {
    fn at(ordering: Ordering, component: Component) -> Self {
        Self {
            ordering,
            component: (ordering != Ordering::Equal).then_some(component),
        }
    }
    const EQUAL: Self = Self {
        ordering: Ordering::Equal,
        component: None,
    };
}

enum Coordinates<'a> {
    Concrete,
    Mixed {
        symbolic: &'a [bool],
        sector: &'a [bool],
    },
    Shifts {
        support: &'a [bool],
    },
}

impl Coordinates<'_> {
    fn active(&self, axis: usize, value: i64) -> bool {
        match self {
            Self::Concrete => value > 0,
            Self::Mixed { symbolic, sector } => {
                if symbolic[axis] {
                    sector[axis]
                } else {
                    value > 0
                }
            }
            Self::Shifts { support } => support[axis],
        }
    }
    fn excess(&self, axis: usize, value: i64) -> i128 {
        let active = self.active(axis, value);
        let constant = match self {
            Self::Concrete => active,
            Self::Mixed { symbolic, .. } => active && !symbolic[axis],
            Self::Shifts { .. } => false,
        };
        if active {
            i128::from(value) - i128::from(constant)
        } else {
            -i128::from(value)
        }
    }
}

impl CompiledOrder {
    pub fn compare(&self, left: &[i64], right: &[i64]) -> Result<Comparison, Error> {
        self.compare_with(left, right, Coordinates::Concrete)
    }

    /// Compare equal symbolic patterns with one common underlying symbolic
    /// coordinate per slot. Numeric slots contain actual powers; symbolic slots
    /// contain offsets. Only after support equality do unknown constants cancel.
    /// This does not admit cut/delta conventions or prove domain applicability.
    pub fn compare_mixed(
        &self,
        sector: &[bool],
        symbolic: &[bool],
        left: &[i64],
        right: &[i64],
    ) -> Result<Comparison, Error> {
        arity(self.arity(), sector.len())?;
        arity(self.arity(), symbolic.len())?;
        self.compare_with(left, right, Coordinates::Mixed { symbolic, sector })
    }

    /// Compare physical shifts on a fixed support. Callers must separately
    /// establish that both translations remain inside this support throughout
    /// their application domain; crossing zero is not handled by extrapolation.
    pub fn compare_shifts(
        &self,
        support: &[bool],
        left: &[i64],
        right: &[i64],
    ) -> Result<Comparison, Error> {
        arity(self.arity(), support.len())?;
        self.compare_with(left, right, Coordinates::Shifts { support })
    }

    fn compare_with(
        &self,
        left: &[i64],
        right: &[i64],
        coordinates: Coordinates<'_>,
    ) -> Result<Comparison, Error> {
        let size = self.arity();
        arity(size, left.len())?;
        arity(size, right.len())?;
        let descriptor = self.descriptor();
        if !matches!(coordinates, Coordinates::Shifts { .. }) {
            let mut left_count = 0usize;
            let mut right_count = 0usize;
            let mut left_weight = 0u64;
            let mut right_weight = 0u64;
            for axis in 0..size {
                if coordinates.active(axis, left[axis]) {
                    left_count += 1;
                    left_weight += descriptor.support_weights[axis];
                }
                if coordinates.active(axis, right[axis]) {
                    right_count += 1;
                    right_weight += descriptor.support_weights[axis];
                }
            }
            let cmp = left_count.cmp(&right_count);
            if cmp != Ordering::Equal {
                return Ok(Comparison::at(cmp, Component::SupportCount));
            }
            let cmp = left_weight.cmp(&right_weight);
            if cmp != Ordering::Equal {
                return Ok(Comparison::at(cmp, Component::SupportWeight));
            }
            for &axis in &descriptor.support_priority {
                let cmp = coordinates
                    .active(axis, left[axis])
                    .cmp(&coordinates.active(axis, right[axis]));
                if cmp != Ordering::Equal {
                    return Ok(Comparison::at(cmp, Component::SupportAxis(axis)));
                }
            }
        }
        // The preceding full support tie makes these sign choices identical.
        // Validation bounds the sum of absolute products below i128::MAX even
        // for i64::MIN offsets; no per-comparison allocation or fallible math.
        for (ordinal, row) in descriptor.degree_rows.iter().enumerate() {
            let mut l = 0i128;
            let mut r = 0i128;
            for axis in 0..size {
                let weight = if coordinates.active(axis, left[axis]) {
                    row.active[axis]
                } else {
                    row.inactive[axis]
                };
                l += i128::from(weight) * coordinates.excess(axis, left[axis]);
                r += i128::from(weight) * coordinates.excess(axis, right[axis]);
            }
            let cmp = l.cmp(&r);
            if cmp != Ordering::Equal {
                return Ok(Comparison::at(cmp, Component::DegreeRow(ordinal)));
            }
        }
        let passes = if descriptor.coordinate_groups == CoordinateGroups::Interleaved {
            1
        } else {
            2
        };
        for pass in 0..passes {
            for &axis in &descriptor.coordinate_priority {
                let active = coordinates.active(axis, left[axis]);
                if descriptor.coordinate_groups != CoordinateGroups::Interleaved {
                    let active_pass = (descriptor.coordinate_groups
                        == CoordinateGroups::ActiveFirst)
                        == (pass == 0);
                    if active != active_pass {
                        continue;
                    }
                }
                let mut cmp = coordinates
                    .excess(axis, left[axis])
                    .cmp(&coordinates.excess(axis, right[axis]));
                let direction = if active {
                    descriptor.active_direction
                } else {
                    descriptor.inactive_direction
                };
                if direction == Direction::Descending {
                    cmp = cmp.reverse();
                }
                if cmp != Ordering::Equal {
                    return Ok(Comparison::at(cmp, Component::Coordinate(axis)));
                }
            }
        }
        Ok(Comparison::EQUAL)
    }
}
