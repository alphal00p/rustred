use std::sync::Arc;

use super::error::{check_limit, checked_add, try_push_bounded, try_vec};
use super::{InvolutiveError, InvolutiveLimits};

/// One bounded exponent vector in the sector-local forward-shift monoid.
///
/// A forward coordinate means increasing sector-chart complexity. On an
/// active line this is `n_i -> n_i + a_i`; on an inactive line it is
/// `n_i -> n_i - a_i`. [`super::OreOrderingAdapter`] owns that signed map.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct ForwardShift {
    values: Arc<Vec<u64>>,
    total_degree: usize,
}

impl ForwardShift {
    pub(crate) fn try_new(
        values: impl IntoIterator<Item = u64>,
        limits: InvolutiveLimits,
    ) -> Result<Self, InvolutiveError> {
        let mut retained = Vec::new();
        let mut total_degree = 0usize;
        for (position, value) in values.into_iter().enumerate() {
            if value > limits.max_shift_coordinate {
                return Err(InvolutiveError::ShiftCoordinateLimit {
                    position,
                    requested: value,
                    limit: limits.max_shift_coordinate,
                });
            }
            if value > i64::MAX as u64 {
                return Err(InvolutiveError::ShiftCoordinateNotRepresentable {
                    position,
                    coordinate: value,
                });
            }
            let degree =
                usize::try_from(value).map_err(|_| InvolutiveError::ResourceCountOverflow {
                    resource: "forward-shift total degree",
                })?;
            total_degree = checked_add("forward-shift total degree", total_degree, degree)?;
            check_limit(
                "forward-shift total degree",
                total_degree,
                limits.max_total_shift_degree,
            )?;
            try_push_bounded(
                &mut retained,
                value,
                "forward-shift arity",
                limits.max_arity,
            )?;
        }
        if retained.is_empty() {
            return Err(InvolutiveError::EmptyCoordinateSpace);
        }
        Ok(Self {
            values: Arc::new(retained),
            total_degree,
        })
    }

    pub(crate) fn try_zero(
        arity: usize,
        limits: InvolutiveLimits,
    ) -> Result<Self, InvolutiveError> {
        if arity == 0 {
            return Err(InvolutiveError::EmptyCoordinateSpace);
        }
        check_limit("forward-shift arity", arity, limits.max_arity)?;
        let mut values = try_vec("zero forward-shift coordinates", arity)?;
        values.resize(arity, 0);
        Ok(Self {
            values: Arc::new(values),
            total_degree: 0,
        })
    }

    pub(crate) fn try_unit(
        arity: usize,
        position: usize,
        limits: InvolutiveLimits,
    ) -> Result<Self, InvolutiveError> {
        if arity == 0 {
            return Err(InvolutiveError::EmptyCoordinateSpace);
        }
        if position >= arity {
            return Err(InvolutiveError::CoordinateOutOfRange { position, arity });
        }
        check_limit("forward-shift arity", arity, limits.max_arity)?;
        if limits.max_shift_coordinate < 1 {
            return Err(InvolutiveError::ShiftCoordinateLimit {
                position,
                requested: 1,
                limit: limits.max_shift_coordinate,
            });
        }
        check_limit(
            "forward-shift total degree",
            1,
            limits.max_total_shift_degree,
        )?;
        let mut values = try_vec("unit forward-shift coordinates", arity)?;
        values.resize(arity, 0);
        values[position] = 1;
        Ok(Self {
            values: Arc::new(values),
            total_degree: 1,
        })
    }

    pub(crate) fn values(&self) -> &[u64] {
        self.values.as_slice()
    }

    pub(crate) fn arity(&self) -> usize {
        self.values.len()
    }

    pub(crate) fn total_degree(&self) -> usize {
        self.total_degree
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.total_degree == 0
    }

    pub(crate) fn is_pure_power(&self, position: usize) -> bool {
        position < self.arity()
            && self.values[position] > 0
            && self
                .values
                .iter()
                .enumerate()
                .all(|(candidate, &value)| candidate == position || value == 0)
    }

    pub(crate) fn componentwise_divides(&self, target: &Self) -> bool {
        self.arity() == target.arity()
            && self
                .values
                .iter()
                .zip(target.values.iter())
                .all(|(&left, &right)| left <= right)
    }

    pub(crate) fn try_checked_add(
        &self,
        right: &Self,
        limits: InvolutiveLimits,
    ) -> Result<Self, InvolutiveError> {
        if self.arity() != right.arity() {
            return Err(InvolutiveError::WrongArity {
                object: "forward-shift addend",
                expected: self.arity(),
                actual: right.arity(),
            });
        }
        check_limit("forward-shift arity", self.arity(), limits.max_arity)?;
        let mut total_degree = 0usize;
        for (position, (&left, &right)) in self.values.iter().zip(right.values.iter()).enumerate() {
            let value = left
                .checked_add(right)
                .ok_or(InvolutiveError::ResourceCountOverflow {
                    resource: "forward-shift coordinate",
                })?;
            if value > limits.max_shift_coordinate {
                return Err(InvolutiveError::ShiftCoordinateLimit {
                    position,
                    requested: value,
                    limit: limits.max_shift_coordinate,
                });
            }
            if value > i64::MAX as u64 {
                return Err(InvolutiveError::ShiftCoordinateNotRepresentable {
                    position,
                    coordinate: value,
                });
            }
            let degree =
                usize::try_from(value).map_err(|_| InvolutiveError::ResourceCountOverflow {
                    resource: "forward-shift total degree",
                })?;
            total_degree = checked_add("forward-shift total degree", total_degree, degree)?;
            check_limit(
                "forward-shift total degree",
                total_degree,
                limits.max_total_shift_degree,
            )?;
        }
        let mut values = try_vec("summed forward-shift coordinates", self.arity())?;
        for (&left, &right) in self.values.iter().zip(right.values.iter()) {
            let value = left
                .checked_add(right)
                .ok_or(InvolutiveError::ResourceCountOverflow {
                    resource: "forward-shift coordinate",
                })?;
            values.push(value);
        }
        Ok(Self {
            values: Arc::new(values),
            total_degree,
        })
    }

    pub(crate) fn try_increment(
        &self,
        position: usize,
        limits: InvolutiveLimits,
    ) -> Result<Self, InvolutiveError> {
        let unit = Self::try_unit(self.arity(), position, limits)?;
        self.try_checked_add(&unit, limits)
    }

    /// Checked componentwise quotient in the additive forward-shift monoid.
    pub(crate) fn try_checked_sub(
        &self,
        divisor: &Self,
        limits: InvolutiveLimits,
    ) -> Result<Self, InvolutiveError> {
        if self.arity() != divisor.arity() {
            return Err(InvolutiveError::WrongArity {
                object: "forward-shift divisor",
                expected: self.arity(),
                actual: divisor.arity(),
            });
        }
        check_limit("forward-shift arity", self.arity(), limits.max_arity)?;
        let mut total_degree = 0usize;
        for (position, (&dividend, &right)) in
            self.values.iter().zip(divisor.values.iter()).enumerate()
        {
            let value =
                dividend
                    .checked_sub(right)
                    .ok_or(InvolutiveError::NonDivisibleForwardShift {
                        position,
                        dividend,
                        divisor: right,
                    })?;
            let degree =
                usize::try_from(value).map_err(|_| InvolutiveError::ResourceCountOverflow {
                    resource: "forward-shift total degree",
                })?;
            total_degree = checked_add("forward-shift total degree", total_degree, degree)?;
            check_limit(
                "forward-shift total degree",
                total_degree,
                limits.max_total_shift_degree,
            )?;
        }
        let mut values = try_vec("subtracted forward-shift coordinates", self.arity())?;
        for (&dividend, &right) in self.values.iter().zip(divisor.values.iter()) {
            values.push(dividend - right);
        }
        Ok(Self {
            values: Arc::new(values),
            total_degree,
        })
    }
}
