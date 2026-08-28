use std::fmt;

use rustred::legacy_oracle_support::symbolica_atom::{Atom, FunctionBuilder, Symbol};

/// An integral identified by integer powers of a complete denominator basis.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Integral {
    powers: Vec<i32>,
}

impl Integral {
    pub fn new(powers: impl Into<Vec<i32>>) -> Self {
        Self {
            powers: powers.into(),
        }
    }

    pub fn powers(&self) -> &[i32] {
        &self.powers
    }

    pub fn denominator_count(&self) -> usize {
        self.powers.iter().filter(|&&power| power > 0).count()
    }

    /// Total number of powers above one, or `None` when the sum does not fit
    /// in the public `u32` degree representation.
    pub fn checked_dot_degree(&self) -> Option<u32> {
        self.powers.iter().try_fold(0_u32, |total, &power| {
            total.checked_add(power.saturating_sub(1).max(0) as u32)
        })
    }

    pub fn dot_degree(&self) -> u32 {
        self.checked_dot_degree().unwrap_or(u32::MAX)
    }

    /// Total absolute power of non-positive entries, or `None` when the sum
    /// does not fit in the public `u32` degree representation.
    pub fn checked_numerator_degree(&self) -> Option<u32> {
        self.powers.iter().try_fold(0_u32, |total, &power| {
            total.checked_add(if power <= 0 { power.unsigned_abs() } else { 0 })
        })
    }

    pub fn numerator_degree(&self) -> u32 {
        self.checked_numerator_degree().unwrap_or(u32::MAX)
    }

    /// Apply indexed power shifts, returning `None` for an invalid position or
    /// any intermediate/final integer overflow.
    pub fn checked_shifted(&self, shifts: &[(usize, i32)]) -> Option<Self> {
        let mut combined_shifts = vec![0_i64; self.powers.len()];
        for &(position, shift) in shifts {
            let accumulated = combined_shifts.get_mut(position)?;
            *accumulated = accumulated.checked_add(i64::from(shift))?;
        }
        let powers = self
            .powers
            .iter()
            .zip(combined_shifts)
            .map(|(&power, shift)| i32::try_from(i64::from(power).checked_add(shift)?).ok())
            .collect::<Option<Vec<_>>>()?;
        Some(Self { powers })
    }

    pub fn to_atom(&self, integral_symbol: Symbol) -> Atom {
        FunctionBuilder::new(integral_symbol)
            .add_args(self.powers.iter().copied().map(Atom::num))
            .finish()
    }
}

impl<const N: usize> From<[i32; N]> for Integral {
    fn from(value: [i32; N]) -> Self {
        Self::new(value.to_vec())
    }
}

impl fmt::Display for Integral {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "I({})",
            self.powers
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        )
    }
}
