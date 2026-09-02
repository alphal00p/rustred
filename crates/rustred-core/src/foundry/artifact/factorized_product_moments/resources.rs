//! Aggregate retained-payload admission for the cold prototype.

use std::mem::size_of;

use symbolica::prelude::Integer;

use crate::algebra::{Coefficient, coefficient_clone_owned_retained_byte_bound};
use crate::family::IntegralKey;

use super::compile::admit_limit;
use super::error::FactorizedProductMomentError;
use super::limits::FactorizedProductMomentLimits;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct CoefficientWeight {
    terms: usize,
    clone_owned_bytes: usize,
}

impl CoefficientWeight {
    fn checked_add(self, other: Self) -> Result<Self, FactorizedProductMomentError> {
        Ok(Self {
            terms: self.terms.checked_add(other.terms).ok_or(
                FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "product retained coefficient terms",
                },
            )?,
            clone_owned_bytes: self
                .clone_owned_bytes
                .checked_add(other.clone_owned_bytes)
                .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "product retained coefficient clone-owned bytes",
                })?,
        })
    }

    fn checked_sub(self, other: Self) -> Result<Self, FactorizedProductMomentError> {
        Ok(Self {
            terms: self.terms.checked_sub(other.terms).ok_or(
                FactorizedProductMomentError::Invariant {
                    detail: "the retained coefficient-term census underflowed",
                },
            )?,
            clone_owned_bytes: self
                .clone_owned_bytes
                .checked_sub(other.clone_owned_bytes)
                .ok_or(FactorizedProductMomentError::Invariant {
                    detail: "the retained coefficient-byte census underflowed",
                })?,
        })
    }
}

pub(super) fn coefficient_weight(
    coefficient: &Coefficient,
) -> Result<CoefficientWeight, FactorizedProductMomentError> {
    let terms = coefficient
        .numerator
        .nterms()
        .checked_add(coefficient.denominator.nterms())
        .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product retained coefficient terms",
        })?;
    let clone_owned_bytes = coefficient_clone_owned_retained_byte_bound(coefficient).ok_or(
        FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product retained coefficient clone-owned bytes",
        },
    )?;
    Ok(CoefficientWeight {
        terms,
        clone_owned_bytes,
    })
}

/// Exact aggregate of coefficients retained by native polynomial input,
/// memoization maps, guards, and output maps. Temporary checks include the
/// already-retained aggregate, so old and replacement values coexist at every
/// transactional update boundary.
pub(super) struct CoefficientBudget {
    current: CoefficientWeight,
    limits: FactorizedProductMomentLimits,
}

impl CoefficientBudget {
    pub(super) fn new(limits: FactorizedProductMomentLimits) -> Self {
        Self {
            current: CoefficientWeight::default(),
            limits,
        }
    }

    pub(super) fn retain(
        &mut self,
        coefficient: &Coefficient,
    ) -> Result<(), FactorizedProductMomentError> {
        self.retain_weight(coefficient_weight(coefficient)?)
    }

    pub(super) fn release(
        &mut self,
        coefficient: &Coefficient,
    ) -> Result<(), FactorizedProductMomentError> {
        self.current = self.current.checked_sub(coefficient_weight(coefficient)?)?;
        Ok(())
    }

    pub(super) fn replace(
        &mut self,
        old: &Coefficient,
        new: &Coefficient,
    ) -> Result<(), FactorizedProductMomentError> {
        let old = coefficient_weight(old)?;
        let new = coefficient_weight(new)?;
        // The arithmetic result exists before the old retained value is
        // replaced. Admit that live peak transactionally first.
        self.admit(self.current.checked_add(new)?)?;
        let prospective = self.current.checked_sub(old)?.checked_add(new)?;
        self.admit(prospective)?;
        self.current = prospective;
        Ok(())
    }

    pub(super) fn admit_temporaries<'coefficient>(
        &self,
        coefficients: impl IntoIterator<Item = &'coefficient Coefficient>,
    ) -> Result<(), FactorizedProductMomentError> {
        let mut prospective = self.current;
        for coefficient in coefficients {
            prospective = prospective.checked_add(coefficient_weight(coefficient)?)?;
        }
        self.admit(prospective)
    }

    fn retain_weight(
        &mut self,
        weight: CoefficientWeight,
    ) -> Result<(), FactorizedProductMomentError> {
        let prospective = self.current.checked_add(weight)?;
        self.admit(prospective)?;
        self.current = prospective;
        Ok(())
    }

    fn admit(&self, weight: CoefficientWeight) -> Result<(), FactorizedProductMomentError> {
        admit_limit(
            "product retained coefficient terms",
            weight.terms,
            self.limits.max_retained_coefficient_terms,
        )?;
        admit_limit(
            "product retained coefficient clone-owned bytes",
            weight.clone_owned_bytes,
            self.limits.max_retained_coefficient_clone_owned_bytes,
        )
    }
}

pub(super) fn admit_dense_payload(
    entry_resource: &'static str,
    byte_resource: &'static str,
    rows: usize,
    width: usize,
    entry_bytes: usize,
    owner_bytes: usize,
    max_entries: usize,
    max_bytes: usize,
) -> Result<(), FactorizedProductMomentError> {
    let entries =
        rows.checked_mul(width)
            .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                resource: entry_resource,
            })?;
    admit_limit(entry_resource, entries, max_entries)?;
    let payload_bytes = entries.checked_mul(entry_bytes).ok_or(
        FactorizedProductMomentError::ResourceCountOverflow {
            resource: byte_resource,
        },
    )?;
    let owner_bytes = rows.checked_mul(owner_bytes).ok_or(
        FactorizedProductMomentError::ResourceCountOverflow {
            resource: byte_resource,
        },
    )?;
    let bytes = payload_bytes.checked_add(owner_bytes).ok_or(
        FactorizedProductMomentError::ResourceCountOverflow {
            resource: byte_resource,
        },
    )?;
    admit_limit(byte_resource, bytes, max_bytes)
}

pub(super) fn admit_state_key_payload(
    rows: usize,
    width: usize,
    limits: FactorizedProductMomentLimits,
) -> Result<(), FactorizedProductMomentError> {
    admit_dense_payload(
        "product moment state-key entries",
        "product moment state-key bytes",
        rows,
        width,
        size_of::<u128>(),
        size_of::<Box<[u128]>>(),
        limits.max_state_key_entries,
        limits.max_state_key_bytes,
    )
}

pub(super) fn admit_angular_order_key_payload(
    retained_rows: usize,
    order_rows: usize,
    width: usize,
    limits: FactorizedProductMomentLimits,
) -> Result<(), FactorizedProductMomentError> {
    let rows = retained_rows.checked_add(order_rows).ok_or(
        FactorizedProductMomentError::ResourceCountOverflow {
            resource: "angular retained state keys",
        },
    )?;
    let entries =
        rows.checked_mul(width)
            .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                resource: "product moment state-key entries",
            })?;
    admit_limit(
        "product moment state-key entries",
        entries,
        limits.max_state_key_entries,
    )?;
    let payload_bytes = entries.checked_mul(size_of::<u128>()).ok_or(
        FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product moment state-key bytes",
        },
    )?;
    let retained_owner_bytes = retained_rows.checked_mul(size_of::<Box<[u128]>>()).ok_or(
        FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product moment state-key bytes",
        },
    )?;
    let order_owner_bytes = order_rows
        .checked_mul(size_of::<(usize, Box<[u128]>)>())
        .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product moment state-key bytes",
        })?;
    let bytes = payload_bytes
        .checked_add(retained_owner_bytes)
        .and_then(|value| value.checked_add(order_owner_bytes))
        .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product moment state-key bytes",
        })?;
    admit_limit(
        "product moment state-key bytes",
        bytes,
        limits.max_state_key_bytes,
    )
}

pub(super) fn constant_integer_magnitude_bits(coefficient: &Coefficient) -> Option<usize> {
    if !coefficient.is_constant() || !coefficient.denominator.is_one() {
        return None;
    }
    constant_polynomial_magnitude_bits(&coefficient.numerator)
}

pub(super) fn constant_rational_magnitude_bits(
    coefficient: &Coefficient,
) -> Option<(usize, usize)> {
    if !coefficient.is_constant() {
        return None;
    }
    Some((
        constant_polynomial_magnitude_bits(&coefficient.numerator)?,
        constant_polynomial_magnitude_bits(&coefficient.denominator)?,
    ))
}

fn constant_polynomial_magnitude_bits(
    polynomial: &crate::algebra::CoefficientPolynomial,
) -> Option<usize> {
    if polynomial.is_zero() {
        return Some(0);
    }
    let [integer] = polynomial.coefficients.as_slice() else {
        return None;
    };
    let bits = match integer {
        Integer::Single(value) => u64::from(u64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u64::from(u128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u64::from(value.significant_bits()),
    };
    usize::try_from(bits).ok()
}

pub(super) fn admit_compiled_embedding_key_payload(
    rows: usize,
    width: usize,
    limits: FactorizedProductMomentLimits,
) -> Result<(), FactorizedProductMomentError> {
    admit_dense_payload(
        "compiled product embedding key power entries",
        "compiled product embedding key bytes",
        rows,
        width,
        size_of::<i64>(),
        size_of::<IntegralKey>(),
        limits.max_compiled_embedding_key_power_entries,
        limits.max_compiled_embedding_key_bytes,
    )
}

pub(super) fn admit_guard_key_payload(
    rows: usize,
    limits: FactorizedProductMomentLimits,
) -> Result<(), FactorizedProductMomentError> {
    let bytes = rows.checked_mul(size_of::<(usize, u128)>()).ok_or(
        FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product moment guard-key bytes",
        },
    )?;
    admit_limit(
        "product moment guard-key bytes",
        bytes,
        limits.max_guard_key_bytes,
    )
}
