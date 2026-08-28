use std::fmt;
use std::sync::Arc;

use super::error::{Error, try_collect_vec, try_reserve_exact};

/// A sector mask in denominator-index order.
///
/// Position zero is serialized first, matching the leftmost (most
/// significant) bit in LiteRed's `js[basis,...]`/`FromDigits[...,2]` usage.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Mask {
    pub(super) active: Arc<Vec<bool>>,
}

impl Mask {
    pub fn try_new(active: impl IntoIterator<Item = bool>) -> Result<Self, Error> {
        let active = try_collect_vec(active, "sector mask bits")?;
        Self::try_from_preallocated(active)
    }

    /// Retain the single caller-sized allocation which was obtained through a
    /// fallible reservation boundary. The `Arc` adds only its fixed-size
    /// control allocation; cloning never copies the bit buffer.
    pub(super) fn try_from_preallocated(active: Vec<bool>) -> Result<Self, Error> {
        if active.is_empty() {
            return Err(Error::EmptyIndexSpace);
        }
        Ok(Self {
            active: Arc::new(active),
        })
    }

    /// Derive the sector from unshifted integral powers.
    pub fn try_from_indices(indices: &[i64]) -> Result<Self, Error> {
        if indices.is_empty() {
            return Err(Error::EmptyIndexSpace);
        }
        Self::try_new(indices.iter().map(|&index| index >= 1))
    }

    pub fn arity(&self) -> usize {
        self.active.len()
    }

    pub fn active_bits(&self) -> &[bool] {
        self.active.as_slice()
    }

    pub fn is_active(&self, position: usize) -> Result<bool, Error> {
        self.active
            .get(position)
            .copied()
            .ok_or(Error::IndexOutOfRange {
                position,
                arity: self.arity(),
            })
    }

    pub fn active_count(&self) -> usize {
        self.active.iter().filter(|&&active| active).count()
    }

    /// The LiteRed corner: `1` in active slots and `0` in inactive slots.
    pub fn corner_indices(&self) -> impl ExactSizeIterator<Item = i64> + DoubleEndedIterator + '_ {
        self.active.iter().copied().map(i64::from)
    }

    pub fn with_activity(&self, position: usize, active: bool) -> Result<Self, Error> {
        if position >= self.arity() {
            return Err(Error::IndexOutOfRange {
                position,
                arity: self.arity(),
            });
        }
        let mut result = Vec::new();
        try_reserve_exact(&mut result, self.arity(), "sector mask bits")?;
        result.extend_from_slice(&self.active);
        result[position] = active;
        Self::try_from_preallocated(result)
    }

    /// `self` is a subsector of `other` iff every active bit of `self` is
    /// also active in `other`.
    pub fn is_subsector_of(&self, other: &Self) -> Result<bool, Error> {
        self.check_other_arity(other)?;
        Ok(self
            .active
            .iter()
            .zip(other.active.iter())
            .all(|(&candidate, &container)| !candidate || container))
    }

    pub fn is_strict_subsector_of(&self, other: &Self) -> Result<bool, Error> {
        Ok(self != other && self.is_subsector_of(other)?)
    }

    fn check_arity(&self, actual: usize) -> Result<(), Error> {
        if actual == self.arity() {
            Ok(())
        } else {
            Err(Error::WrongArity {
                expected: self.arity(),
                actual,
            })
        }
    }

    pub(super) fn check_other_arity(&self, other: &Self) -> Result<(), Error> {
        self.check_arity(other.arity())
    }
}

impl fmt::Display for Mask {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for &active in self.active.iter() {
            formatter.write_str(if active { "1" } else { "0" })?;
        }
        Ok(())
    }
}
