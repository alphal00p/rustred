use std::cmp::Ordering;

use super::descriptor::arity;
use super::{CompiledOrder, CoordinateGroups, Direction, Error};

/// Honest first decisive component, suitable for higher-level descent evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Component {
    PreSupportDegreeRow(usize),
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

enum View<'a> {
    Powers {
        coordinates: &'a Coordinates<'a>,
        values: &'a [i64],
    },
    Unsigned {
        support: &'a [bool],
        values: &'a [u64],
    },
    Signed {
        support: &'a [bool],
        values: &'a [i128],
    },
}

impl View<'_> {
    fn active(&self, axis: usize) -> bool {
        match self {
            Self::Powers {
                coordinates,
                values,
            } => coordinates.active(axis, values[axis]),
            Self::Unsigned { support, .. } | Self::Signed { support, .. } => support[axis],
        }
    }
    fn excess(&self, axis: usize) -> i128 {
        match self {
            Self::Powers {
                coordinates,
                values,
            } => coordinates.excess(axis, values[axis]),
            Self::Unsigned { values, .. } => i128::from(values[axis]),
            Self::Signed { values, .. } => values[axis],
        }
    }

    /// Physical absolute power, or its signed offset when the common
    /// fixed-sign symbolic/base contribution has already been cancelled.
    fn absolute(&self, axis: usize) -> i128 {
        match self {
            Self::Powers {
                coordinates,
                values,
            } => {
                let value = i128::from(values[axis]);
                if coordinates.active(axis, values[axis]) {
                    value
                } else {
                    -value
                }
            }
            Self::Unsigned { support, values } => {
                i128::from(values[axis]) + i128::from(support[axis])
            }
            Self::Signed { values, .. } => values[axis],
        }
    }
}

impl CompiledOrder {
    /// Compare already retained concrete-key excess without allocating a
    /// second raw-power vector. Arity and representability are still checked.
    pub fn compare_excess(
        &self,
        left_support: &[bool],
        left: &[u64],
        right_support: &[bool],
        right: &[u64],
    ) -> Result<Comparison, Error> {
        for (support, values) in [(left_support, left), (right_support, right)] {
            arity(self.arity(), support.len())?;
            arity(self.arity(), values.len())?;
            for (axis, (&active, &value)) in support.iter().zip(values).enumerate() {
                let maximum = if active {
                    i64::MAX as u64 - 1
                } else {
                    1u64 << 63
                };
                if value > maximum {
                    return Err(Error::ExcessOutOfRange { axis });
                }
            }
        }
        self.compare_views(
            View::Unsigned {
                support: left_support,
                values: left,
            },
            View::Unsigned {
                support: right_support,
                values: right,
            },
            true,
        )
    }

    /// Compare retained signed excess offsets on one fixed support. These
    /// offsets must come from physical i64 shifts; applicability is separate.
    pub fn compare_shift_excess(
        &self,
        support: &[bool],
        left: &[i128],
        right: &[i128],
    ) -> Result<Comparison, Error> {
        arity(self.arity(), support.len())?;
        for values in [left, right] {
            arity(self.arity(), values.len())?;
            for (axis, (&active, &value)) in support.iter().zip(values).enumerate() {
                let physical = if active {
                    Some(value)
                } else {
                    value.checked_neg()
                };
                if physical.and_then(|n| i64::try_from(n).ok()).is_none() {
                    return Err(Error::ExcessOutOfRange { axis });
                }
            }
        }
        self.compare_views(
            View::Signed {
                support,
                values: left,
            },
            View::Signed {
                support,
                values: right,
            },
            false,
        )
    }

    /// Support-only comparison, including weights and the declared bit priority.
    /// This is NOT a complete order prefix when pre-support rows are present:
    /// it cannot establish support-changing descent without physical powers.
    /// No degree or coordinate comparison is performed.
    pub fn compare_support(&self, left: &[bool], right: &[bool]) -> Result<Comparison, Error> {
        arity(self.arity(), left.len())?;
        arity(self.arity(), right.len())?;
        Ok(self.support_comparison(&|axis| left[axis], &|axis| right[axis]))
    }

    pub fn compare(&self, left: &[i64], right: &[i64]) -> Result<Comparison, Error> {
        self.compare_with(left, right, Coordinates::Concrete)
    }

    /// Compare equal symbolic patterns with one common underlying symbolic
    /// coordinate per slot. Numeric slots contain actual powers; symbolic slots
    /// contain offsets. Common fixed-sign symbolic contributions cancel even
    /// in pre-support rows; numeric slots retain their actual absolute powers.
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
        self.compare_views(
            View::Powers {
                coordinates: &coordinates,
                values: left,
            },
            View::Powers {
                coordinates: &coordinates,
                values: right,
            },
            !matches!(coordinates, Coordinates::Shifts { .. }),
        )
    }

    fn support_comparison(
        &self,
        left: &impl Fn(usize) -> bool,
        right: &impl Fn(usize) -> bool,
    ) -> Comparison {
        let descriptor = self.descriptor();
        let (mut left_count, mut right_count) = (0usize, 0usize);
        let (mut left_weight, mut right_weight) = (0u64, 0u64);
        for axis in 0..self.arity() {
            if left(axis) {
                left_count += 1;
                left_weight += descriptor.support_weights[axis];
            }
            if right(axis) {
                right_count += 1;
                right_weight += descriptor.support_weights[axis];
            }
        }
        let cmp = left_count.cmp(&right_count);
        if cmp != Ordering::Equal {
            return Comparison::at(cmp, Component::SupportCount);
        }
        let cmp = left_weight.cmp(&right_weight);
        if cmp != Ordering::Equal {
            return Comparison::at(cmp, Component::SupportWeight);
        }
        for &axis in &descriptor.support_priority {
            let cmp = left(axis).cmp(&right(axis));
            if cmp != Ordering::Equal {
                return Comparison::at(cmp, Component::SupportAxis(axis));
            }
        }
        Comparison::EQUAL
    }

    fn compare_views(
        &self,
        left: View<'_>,
        right: View<'_>,
        compare_support: bool,
    ) -> Result<Comparison, Error> {
        let size = self.arity();
        let descriptor = self.descriptor();
        for (ordinal, row) in descriptor.pre_support_degree_rows.iter().enumerate() {
            let mut l = 0i128;
            let mut r = 0i128;
            for axis in 0..size {
                // Unlike the excess suffix, support has not yet tied. Each
                // operand must use its own physical sign's weight.
                let lw = if left.active(axis) {
                    row.active[axis]
                } else {
                    row.inactive[axis]
                };
                let rw = if right.active(axis) {
                    row.active[axis]
                } else {
                    row.inactive[axis]
                };
                l += i128::from(lw) * left.absolute(axis);
                r += i128::from(rw) * right.absolute(axis);
            }
            let cmp = l.cmp(&r);
            if cmp != Ordering::Equal {
                return Ok(Comparison::at(cmp, Component::PreSupportDegreeRow(ordinal)));
            }
        }
        if compare_support {
            let result =
                self.support_comparison(&|axis| left.active(axis), &|axis| right.active(axis));
            if result.ordering != Ordering::Equal {
                return Ok(result);
            }
        }
        // The preceding full support tie makes these sign choices identical.
        // Validation bounds the sum of absolute products below i128::MAX even
        // for i64::MIN offsets; no per-comparison allocation or fallible math.
        for (ordinal, row) in descriptor.degree_rows.iter().enumerate() {
            let mut l = 0i128;
            let mut r = 0i128;
            for axis in 0..size {
                let weight = if left.active(axis) {
                    row.active[axis]
                } else {
                    row.inactive[axis]
                };
                l += i128::from(weight) * left.excess(axis);
                r += i128::from(weight) * right.excess(axis);
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
                let active = left.active(axis);
                if descriptor.coordinate_groups != CoordinateGroups::Interleaved {
                    let active_pass = (descriptor.coordinate_groups
                        == CoordinateGroups::ActiveFirst)
                        == (pass == 0);
                    if active != active_pass {
                        continue;
                    }
                }
                let mut cmp = left.excess(axis).cmp(&right.excess(axis));
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
