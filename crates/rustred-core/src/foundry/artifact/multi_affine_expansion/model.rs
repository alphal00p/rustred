//! Immutable inputs and outputs of one cold multi-affine expansion.

use crate::algebra::Coefficient;
use crate::family::IntegralKey;

use super::error::MultiAffineNumeratorExpansionError;

/// One exact affine form raised to a fixed nonnegative numerator power.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MultiAffineNumeratorFactor {
    constant: Coefficient,
    denominator_coefficients: Box<[Coefficient]>,
    power: u64,
}

impl MultiAffineNumeratorFactor {
    pub(crate) fn try_new(
        constant: Coefficient,
        denominator_coefficients: impl IntoIterator<Item = Coefficient>,
        power: u64,
    ) -> Result<Self, MultiAffineNumeratorExpansionError> {
        let mut retained = Vec::new();
        for coefficient in denominator_coefficients {
            let requested = retained.len().checked_add(1).ok_or(
                MultiAffineNumeratorExpansionError::ResourceCountOverflow {
                    resource: "multi-affine relation coefficients",
                },
            )?;
            retained.try_reserve_exact(1).map_err(|_| {
                MultiAffineNumeratorExpansionError::AllocationFailure {
                    resource: "multi-affine relation coefficients",
                    requested,
                }
            })?;
            retained.push(coefficient);
        }
        Ok(Self {
            constant,
            denominator_coefficients: retained.into_boxed_slice(),
            power,
        })
    }

    pub(crate) fn constant(&self) -> &Coefficient {
        &self.constant
    }

    pub(crate) fn denominator_coefficients(&self) -> &[Coefficient] {
        &self.denominator_coefficients
    }

    pub(crate) const fn power(&self) -> u64 {
        self.power
    }
}

/// One exactly coalesced endpoint of a cold structural identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MultiAffineNumeratorEndpoint {
    pub(super) key: IntegralKey,
    pub(super) coefficient: Coefficient,
}

impl MultiAffineNumeratorEndpoint {
    pub(crate) fn key(&self) -> &IntegralKey {
        &self.key
    }

    pub(crate) fn coefficient(&self) -> &Coefficient {
        &self.coefficient
    }
}
