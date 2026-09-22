//! Compact total-power predicates on a fixed-support integer box.
//!
//! These are discrete domain bounds, not a general inequality or CAS service.
//! Coordinate projection preserves the predicates: its rectangle alone is
//! generally a strict overcover of the represented integer domain.

mod geometry;

pub(crate) use geometry::project;

/// Additional physical-index predicates A <= maximum and lower <= D <= upper,
/// where A=sum(max(n_i,0)) and D=sum(n_i)=A-R. Positive local coordinates are
/// n=x+1, not n=x. None denotes mathematical infinity, never a machine bound.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DomainPowerBounds {
    pub max_positive_power: Option<u64>,
    pub min_power_difference: Option<i64>,
    pub max_power_difference: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DomainPowerError {
    InvalidArity,
    InvertedCoordinate { axis: usize },
    InvertedDifferenceBounds,
    ArithmeticOverflow(&'static str),
    OutOfRange(&'static str),
}

impl std::fmt::Display for DomainPowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid or unrepresentable power domain: {self:?}")
    }
}
impl std::error::Error for DomainPowerError {}

impl DomainPowerBounds {
    pub fn validate(&self) -> Result<(), DomainPowerError> {
        if let (Some(lower), Some(upper)) = (self.min_power_difference, self.max_power_difference)
            && lower > upper
        {
            return Err(DomainPowerError::InvertedDifferenceBounds);
        }
        Ok(())
    }

    pub fn is_unconstrained(&self) -> bool {
        *self == Self::default()
    }

    /// Sufficient componentwise implication. Coordinate/rank containment is
    /// separate; this deliberately does not solve stronger derived implication.
    pub fn contains(&self, other: &Self) -> bool {
        self.max_positive_power
            .is_none_or(|a| other.max_positive_power.is_some_and(|b| b <= a))
            && self
                .min_power_difference
                .is_none_or(|a| other.min_power_difference.is_some_and(|b| b >= a))
            && self
                .max_power_difference
                .is_none_or(|a| other.max_power_difference.is_some_and(|b| b <= a))
    }

    /// Exact translation after sign-crossing coordinates have been fixed.
    /// The caller proves constant delta_A and delta_D on that source cell.
    /// None means the translated A upper is negative and therefore empty.
    /// Descendant bounds must not be intersected with the original entry caps.
    pub fn shifted(self, delta_a: i128, delta_d: i128) -> Result<Option<Self>, DomainPowerError> {
        self.validate()?;
        let max_positive_power = if let Some(value) = self.max_positive_power {
            let shifted = i128::from(value)
                .checked_add(delta_a)
                .ok_or(DomainPowerError::ArithmeticOverflow("translated A bound"))?;
            if shifted < 0 {
                return Ok(None);
            }
            Some(
                u64::try_from(shifted)
                    .map_err(|_| DomainPowerError::OutOfRange("translated A bound"))?,
            )
        } else {
            None
        };
        let translate_d = |value: Option<i64>| {
            value
                .map(|value| {
                    let shifted = i128::from(value)
                        .checked_add(delta_d)
                        .ok_or(DomainPowerError::ArithmeticOverflow("translated D bound"))?;
                    i64::try_from(shifted)
                        .map_err(|_| DomainPowerError::OutOfRange("translated D bound"))
                })
                .transpose()
        };
        Ok(Some(Self {
            max_positive_power,
            min_power_difference: translate_d(self.min_power_difference)?,
            max_power_difference: translate_d(self.max_power_difference)?,
        }))
    }
}

#[cfg(test)]
mod tests;
