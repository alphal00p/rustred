//! Two-pass trace support, with the coefficient-returning lane's virtual caps.

use std::cmp::Ordering;

use symbolica::prelude::Integer;

use super::*;

/// Fully admitted native support, visited in ascending IntegralKey order.
///
/// Own the exact Q polynomial until visitation ends; its coalescence and
/// cancellations remain Symbolica's. No contextual coefficient wrappers or
/// whole endpoint vector are constructed. All algebra, scalar, shape, shift
/// and configured-budget admission precedes returning this iterator. Later
/// allocation/publication failures remain typed incomplete traversal, never a
/// successful partial expansion.
pub(crate) struct AdmittedSupport {
    native: Option<(EndpointPolynomial, IntegralKey)>,
    remaining: usize,
}

pub(crate) fn try_expand_multi_affine_support_with_usage(
    family: &IntegralFamily,
    base: &IntegralKey,
    factors: &[MultiAffineNumeratorFactor],
    limits: MultiAffineNumeratorExpansionLimits,
    reserve: impl FnOnce(ExpansionUsage) -> Result<(), MultiAffineNumeratorExpansionError>,
) -> Result<AdmittedSupport, MultiAffineNumeratorExpansionError> {
    let Some(expanded) = expand_native(family, base, factors, limits, reserve)? else {
        return Ok(AdmittedSupport {
            native: None,
            remaining: 0,
        });
    };
    admit_support(&expanded, base, limits)?;
    let base = IntegralKey::try_from_preallocated(try_clone_powers(base.powers())?)?;
    Ok(AdmittedSupport {
        remaining: expanded.polynomial.nterms(),
        native: Some((expanded.polynomial, base)),
    })
}

fn admit_support(
    expanded: &NativeExpansion,
    base: &IntegralKey,
    limits: MultiAffineNumeratorExpansionLimits,
) -> Result<(), MultiAffineNumeratorExpansionError> {
    let polynomial = &expanded.polynomial;
    let arity = base.powers().len();
    admit_limit(
        "multi-affine endpoints",
        polynomial.nterms(),
        limits.max_endpoints,
    )?;
    preflight_endpoint_key_storage(polynomial.nterms(), arity, limits)?;
    // Check layout before indexing/chunking public native buffers. IntegralKey
    // forbids arity zero. Exact exponent order is checked, not assumed.
    if polynomial.nvars() != arity {
        return Err(MultiAffineNumeratorExpansionError::NativeExponentWidth {
            expected: arity,
            actual: polynomial.nvars(),
        });
    }
    let expected = polynomial.nterms().checked_mul(arity).ok_or(
        MultiAffineNumeratorExpansionError::ResourceCountOverflow {
            resource: "multi-affine native exponent layout",
        },
    )?;
    if polynomial.exponents.len() != expected {
        return Err(MultiAffineNumeratorExpansionError::Invariant {
            detail: "Symbolica returned malformed support exponent layout",
        });
    }
    let live_base = expanded.input_weight.checked_add(polynomial_weight(
        polynomial,
        expanded.constant_wrapper_bytes,
    )?)?;
    admit_live_coefficients(live_base, limits)?;
    let mut output_weight = CoefficientWeight::default();
    let mut previous: Option<&[u32]> = None;
    for (coefficient, exponents) in polynomial
        .coefficients
        .iter()
        .zip(polynomial.exponents_iter())
    {
        if previous.is_some_and(|previous| previous >= exponents) {
            return Err(MultiAffineNumeratorExpansionError::Invariant {
                detail: "Symbolica returned duplicate or unordered support monomials",
            });
        }
        previous = Some(exponents);
        // Scalar-only equivalent of authenticating a contextual constant:
        // expand_native authenticated context.one() on the original map, so
        // both parts have one all-zero exponent row within exact term/exponent
        // limits. Native Q owns reduction; check numeric signs, including
        // noncanonical backend zero variants, before omitting the wrappers.
        if coefficient.numerator_ref().cmp(&Integer::Single(0)) == Ordering::Equal
            || coefficient.denominator_ref().cmp(&Integer::Single(0)) != Ordering::Greater
        {
            return Err(MultiAffineNumeratorExpansionError::Invariant {
                detail: "native Q returned a zero or noncanonical sparse coefficient",
            });
        }
        for (position, &exponent) in exponents.iter().enumerate() {
            checked_lower_power(position, base.powers()[position], u64::from(exponent))?;
        }
        // EXACTLY the conservative charge used before each output wrapper in
        // materialize_endpoints: same input + native + cumulative rational
        // weights, including source GMP capacity. Native constant() allocates
        // one coefficient and a zero exponent row on the authenticated map;
        // bounded_integer_copy can only shrink retained GMP capacity. Thus the
        // old actual<=retained invariant needs no per-term wrapper allocation.
        let retained = rational_weight(coefficient, expanded.constant_wrapper_bytes)?;
        admit_live_coefficients(
            live_base
                .checked_add(output_weight)?
                .checked_add(retained)?,
            limits,
        )?;
        output_weight = output_weight.checked_add(retained)?;
    }
    Ok(())
}

impl Iterator for AdmittedSupport {
    type Item = Result<IntegralKey, MultiAffineNumeratorExpansionError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        let (polynomial, base) = self.native.as_ref().expect("nonempty admitted support");
        // Native Lex rows are ascending. e -> base-e reverses their order,
        // exactly matching the sorted keys of materialize_endpoints.
        let exponents = polynomial.exponents(self.remaining);
        let result = (|| {
            let mut powers = try_clone_powers(base.powers())?;
            for (position, &exponent) in exponents.iter().enumerate() {
                powers[position] =
                    checked_lower_power(position, powers[position], u64::from(exponent))?;
            }
            Ok(IntegralKey::try_from_preallocated(powers)?)
        })();
        if result.is_err() {
            self.remaining = 0;
        }
        Some(result)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // An allocation failure may end visitation early.
        (usize::from(self.remaining != 0), Some(self.remaining))
    }
}

impl std::iter::FusedIterator for AdmittedSupport {}

#[cfg(test)]
#[path = "visit_tests.rs"]
mod tests;
