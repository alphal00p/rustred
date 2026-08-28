use std::sync::Arc;

use super::error::ParametricRelationError;

/// A checked displacement in one family's integral-index lattice.
///
/// Construction fallibly allocates the component buffer before moving that
/// buffer into shared storage. Cloning a cached shift therefore only bumps an
/// `Arc` count; it neither copies nor allocates another arity-sized buffer.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IndexShift(Arc<Vec<i64>>);

impl IndexShift {
    pub(in crate::identity) fn try_new(
        values: impl IntoIterator<Item = i64>,
        expected_arity: usize,
    ) -> Result<Self, ParametricRelationError> {
        let mut retained = Vec::new();
        try_reserve_relation_entries("index-shift components", &mut retained, expected_arity)?;
        let mut values = values.into_iter();
        while retained.len() < expected_arity {
            let Some(value) = values.next() else {
                return Err(ParametricRelationError::WrongArity {
                    expected: expected_arity,
                    actual: retained.len(),
                });
            };
            retained.push(value);
        }
        if values.next().is_some() {
            let actual = expected_arity.checked_add(1).ok_or(
                ParametricRelationError::ResourceCountOverflow {
                    resource: "index-shift components",
                },
            )?;
            return Err(ParametricRelationError::WrongArity {
                expected: expected_arity,
                actual,
            });
        }
        Self::try_from_preallocated(retained, expected_arity)
    }

    /// Retain an allocation which its caller already acquired fallibly.
    fn try_from_preallocated(
        values: Vec<i64>,
        expected_arity: usize,
    ) -> Result<Self, ParametricRelationError> {
        if values.len() != expected_arity {
            return Err(ParametricRelationError::WrongArity {
                expected: expected_arity,
                actual: values.len(),
            });
        }
        Ok(Self(Arc::new(values)))
    }

    pub fn values(&self) -> &[i64] {
        self.0.as_slice()
    }

    pub(super) fn arity(&self) -> usize {
        self.0.len()
    }

    pub(crate) fn checked_add(&self, other: &Self) -> Result<Self, ParametricRelationError> {
        if self.arity() != other.arity() {
            return Err(ParametricRelationError::WrongArity {
                expected: self.arity(),
                actual: other.arity(),
            });
        }
        let mut values = Vec::new();
        try_reserve_relation_entries("summed index-shift components", &mut values, self.arity())?;
        for (position, (&left, &right)) in self.0.iter().zip(other.0.iter()).enumerate() {
            values.push(
                left.checked_add(right)
                    .ok_or(ParametricRelationError::IndexOverflow { position })?,
            );
        }
        Self::try_from_preallocated(values, self.arity())
    }
}

/// Constructs arity-authenticated shifts without repeating length checks at
/// every generator call site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct IndexSpace {
    arity: usize,
}

impl IndexSpace {
    pub(crate) fn try_new(arity: usize) -> Result<Self, ParametricRelationError> {
        if arity == 0 {
            Err(ParametricRelationError::EmptyIndexSpace)
        } else {
            Ok(Self { arity })
        }
    }

    /// Fallible zero-shift construction for resource-hardened callers.
    pub(crate) fn try_zero(self) -> Result<IndexShift, ParametricRelationError> {
        let mut values = Vec::new();
        try_reserve_relation_entries("zero index-shift components", &mut values, self.arity)?;
        values.resize(self.arity, 0);
        IndexShift::try_from_preallocated(values, self.arity)
    }

    pub(crate) fn unit(
        self,
        position: usize,
        direction: i64,
    ) -> Result<IndexShift, ParametricRelationError> {
        if position >= self.arity {
            return Err(ParametricRelationError::IndexOutOfRange {
                position,
                arity: self.arity,
            });
        }
        IndexShift::try_new(
            (0..self.arity).map(
                |component| {
                    if component == position { direction } else { 0 }
                },
            ),
            self.arity,
        )
    }
}

fn relation_checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ParametricRelationError> {
    left.checked_add(right)
        .ok_or(ParametricRelationError::ResourceCountOverflow { resource })
}

fn try_reserve_relation_entries<T>(
    resource: &'static str,
    values: &mut Vec<T>,
    additional: usize,
) -> Result<(), ParametricRelationError> {
    let requested = relation_checked_add(resource, values.len(), additional)?;
    values
        .try_reserve_exact(additional)
        .map_err(|_| ParametricRelationError::AllocationFailure {
            resource,
            requested,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_shift_clones_share_fallibly_built_storage() {
        let space = IndexSpace::try_new(3).unwrap();
        for shift in [
            space.try_zero().unwrap(),
            space.unit(1, 1).unwrap(),
            space.unit(2, -1).unwrap(),
        ] {
            let cloned = shift.clone();
            assert!(Arc::ptr_eq(&shift.0, &cloned.0));
            assert_eq!(shift.values(), cloned.values());
        }
    }
}
