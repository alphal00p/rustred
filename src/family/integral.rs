//! Exact integer-power keys for integrals in an authenticated family.

use std::fmt;

/// One point in an integral family's integer-power lattice.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IntegralKey(Vec<i64>);

impl IntegralKey {
    /// Construct an exact integral key from unshifted integer powers.
    pub fn try_new(powers: impl IntoIterator<Item = i64>) -> Result<Self, IntegralKeyError> {
        let mut retained = Vec::new();
        for power in powers {
            let requested = retained
                .len()
                .checked_add(1)
                .ok_or(IntegralKeyError::PowerCountOverflow)?;
            retained
                .try_reserve_exact(1)
                .map_err(|_| IntegralKeyError::AllocationFailure { requested })?;
            retained.push(power);
        }
        Self::try_from_preallocated(retained)
    }

    pub(crate) fn try_from_preallocated(powers: Vec<i64>) -> Result<Self, IntegralKeyError> {
        if powers.is_empty() {
            Err(IntegralKeyError::EmptyPowers)
        } else {
            Ok(Self(powers))
        }
    }

    pub fn powers(&self) -> &[i64] {
        &self.0
    }
}

/// Typed failures while constructing one exact integral key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IntegralKeyError {
    EmptyPowers,
    PowerCountOverflow,
    AllocationFailure { requested: usize },
}

impl fmt::Display for IntegralKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPowers => formatter.write_str("an integral key cannot be empty"),
            Self::PowerCountOverflow => {
                formatter.write_str("the integral-key power count overflowed usize")
            }
            Self::AllocationFailure { requested } => write!(
                formatter,
                "could not reserve {requested} integral-key powers"
            ),
        }
    }
}

impl std::error::Error for IntegralKeyError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_require_at_least_one_power() {
        assert_eq!(IntegralKey::try_new([]), Err(IntegralKeyError::EmptyPowers));
    }
}
