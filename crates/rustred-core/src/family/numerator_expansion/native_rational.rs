//! Native-Q boundary conversion and conservative context-wrapper ownership.

use symbolica::domains::integer::MultiPrecisionInteger;
use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::prelude::{Integer, Rational, Z};

use crate::algebra::{Coefficient, CoefficientContext, integer_clone_owned_heap_byte_bound};

use super::{CoefficientWeight, MultiAffineNumeratorExpansionError};

/// Called only after complete context/shape/constant admission of every input.
pub(super) fn authenticated_rational(coefficient: &Coefficient) -> Rational {
    Rational::from((
        coefficient.numerator.get_constant(),
        coefficient.denominator.get_constant(),
    ))
}

pub(super) fn contextual_coefficient(
    context: &CoefficientContext,
    value: &Rational,
) -> Coefficient {
    Coefficient::from_num_den(
        context
            .template()
            .numerator
            .constant(bounded_integer_copy(value.numerator_ref())),
        context
            .template()
            .numerator
            .constant(bounded_integer_copy(value.denominator_ref())),
        &Z,
        false,
    )
}

/// Copy output scalars through the native raw-backend ownership API. The
/// ordinary Symbolica integer clone may reuse an arbitrarily larger cached
/// GMP allocation, so its capacity is not bounded by the source's. Native
/// raw cloning starts fresh and copies only the occupied limbs; wrapping it
/// does not allocate or normalize. Small variants are already inline.
fn bounded_integer_copy(value: &Integer) -> Integer {
    match value {
        Integer::Single(value) => Integer::Single(*value),
        Integer::Double(value) => Integer::Double(*value),
        Integer::Large(value) => Integer::Large(MultiPrecisionInteger::from_raw(value.to_raw())),
    }
}

/// Charge native Q scalars as the larger context-bound constant wrapper, plus
/// actual native integer heap capacities. This keeps the original two-term
/// accounting and conservatively admits output construction before cloning.
pub(super) fn rational_weight(
    value: &Rational,
    constant_wrapper_bytes: usize,
) -> Result<CoefficientWeight, MultiAffineNumeratorExpansionError> {
    let mut bytes = Some(constant_wrapper_bytes);
    for integer in [value.numerator_ref(), value.denominator_ref()] {
        // Backend-aware ownership accounting is shared with coefficient
        // admission: retained GMP capacity or fresh Malachite clone payload.
        bytes = bytes
            .and_then(|bytes| bytes.checked_add(integer_clone_owned_heap_byte_bound(integer)?));
    }
    Ok(CoefficientWeight {
        terms: if value.is_zero() { 1 } else { 2 },
        clone_owned_bytes: bytes.ok_or(
            MultiAffineNumeratorExpansionError::ResourceCountOverflow {
                resource: "multi-affine retained coefficient clone-owned bytes",
            },
        )?,
    })
}
