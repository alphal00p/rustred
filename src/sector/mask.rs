use std::fmt;

use super::error::{Error, try_collect_vec, try_reserve_exact};

/// A sector mask in denominator-index order.
///
/// Position zero is serialized first, matching the leftmost (most
/// significant) bit in LiteRed's `js[basis,...]`/`FromDigits[...,2]` usage.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Mask {
    pub(super) active: Vec<bool>,
}

impl Mask {
    pub fn try_new(active: impl IntoIterator<Item = bool>) -> Result<Self, Error> {
        let active = try_collect_vec(active, "sector mask bits")?;
        Self::try_from_preallocated(active)
    }

    /// Retain an allocation which its caller already obtained through a
    /// fallible reservation boundary. No proportional copy or shrink occurs.
    pub(super) fn try_from_preallocated(active: Vec<bool>) -> Result<Self, Error> {
        if active.is_empty() {
            return Err(Error::EmptyIndexSpace);
        }
        Ok(Self { active })
    }

    /// Derive the sector from unshifted integral powers.
    pub fn try_from_indices(indices: &[i64]) -> Result<Self, Error> {
        if indices.is_empty() {
            return Err(Error::EmptyIndexSpace);
        }
        Self::try_new(indices.iter().map(|&index| index >= 1))
    }

    /// Parse the stable index-major `0`/`1` representation.
    pub fn try_from_bit_string(bits: &str) -> Result<Self, Error> {
        if bits.is_empty() {
            return Err(Error::EmptyIndexSpace);
        }
        let mut active = Vec::new();
        try_reserve_exact(&mut active, bits.len(), "sector mask bits")?;
        for (position, byte) in bits.bytes().enumerate() {
            match byte {
                b'0' => active.push(false),
                b'1' => active.push(true),
                _ => return Err(Error::InvalidSectorBit { position, byte }),
            }
        }
        Self::try_from_preallocated(active)
    }

    pub fn arity(&self) -> usize {
        self.active.len()
    }

    pub fn active_bits(&self) -> &[bool] {
        &self.active
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
    pub fn corner_indices(&self) -> Vec<i64> {
        self.active
            .iter()
            .map(|&active| i64::from(active))
            .collect()
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
            .zip(&other.active)
            .all(|(&candidate, &container)| !candidate || container))
    }

    pub fn is_strict_subsector_of(&self, other: &Self) -> Result<bool, Error> {
        Ok(self != other && self.is_subsector_of(other)?)
    }

    pub fn to_bit_string(&self) -> String {
        self.active
            .iter()
            .map(|&active| if active { '1' } else { '0' })
            .collect()
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
        formatter.write_str(&self.to_bit_string())
    }
}
