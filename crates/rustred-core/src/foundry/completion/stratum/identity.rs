use crate::algebra::{ExactAlgebraLimits, IndexedCoefficientContext, IndexedPolynomial};

use super::{StratumRegistryError, check_limit, checked_add};

const INDEXED_POLYNOMIAL_GUARD_V1: &str = "rustred.indexed-polynomial-guard.v1:";

/// Fallible byte-bounded builder for diagnostic identities retained by the
/// completion prototype. Mathematical authority remains in the sealed values
/// whose payload these identities commit to.
pub(super) struct BoundedIdentityBuilder {
    value: String,
    limit: usize,
    resource: &'static str,
}

impl BoundedIdentityBuilder {
    pub(super) fn new(limit: usize, resource: &'static str) -> Self {
        Self {
            value: String::new(),
            limit,
            resource,
        }
    }

    pub(super) fn push(&mut self, value: &str) -> Result<(), StratumRegistryError> {
        let requested = checked_add(self.resource, self.value.len(), value.len())?;
        check_limit(self.resource, requested, self.limit)?;
        self.value.try_reserve_exact(value.len()).map_err(|_| {
            StratumRegistryError::AllocationFailure {
                resource: self.resource,
                requested,
            }
        })?;
        self.value.push_str(value);
        Ok(())
    }

    pub(super) fn push_usize(&mut self, value: usize) -> Result<(), StratumRegistryError> {
        self.push(&value.to_string())
    }

    pub(super) fn push_i64(&mut self, value: i64) -> Result<(), StratumRegistryError> {
        self.push(&value.to_string())
    }

    pub(super) fn finish(self) -> String {
        self.value
    }

    pub(super) fn remaining(&self) -> usize {
        self.limit - self.value.len()
    }
}

pub(super) fn try_copy_identity(
    value: &str,
    resource: &'static str,
    limit: usize,
) -> Result<String, StratumRegistryError> {
    check_limit(resource, value.len(), limit)?;
    let mut retained = String::new();
    retained.try_reserve_exact(value.len()).map_err(|_| {
        StratumRegistryError::AllocationFailure {
            resource,
            requested: value.len(),
        }
    })?;
    retained.push_str(value);
    Ok(retained)
}

/// Canonical sparse identity of one exact polynomial zero locus.
///
/// The context fingerprint commits to the complete variable order. Symbolica
/// supplies the primitive associate, so integer multiples and sign changes
/// cannot manufacture distinct Boolean branches.
pub(super) fn try_indexed_polynomial_guard_identity(
    context: &IndexedCoefficientContext,
    polynomial: &IndexedPolynomial,
    algebra_limits: ExactAlgebraLimits,
    byte_limit: usize,
) -> Result<String, StratumRegistryError> {
    if polynomial.is_zero() {
        return Err(StratumRegistryError::ZeroGuardPolynomial);
    }
    let mut stable = BoundedIdentityBuilder::new(byte_limit, "guard predicate identity bytes");
    stable.push(INDEXED_POLYNOMIAL_GUARD_V1)?;
    stable.push_usize(context.fingerprint().len())?;
    stable.push("#")?;
    stable.push(context.fingerprint())?;
    stable.push(":")?;
    let nvars = polynomial.raw().nvars();
    let nterms = polynomial.raw().nterms();
    stable.push_usize(nvars)?;
    stable.push(":")?;
    stable.push_usize(nterms)?;
    stable.push(":")?;
    let polynomial = context.primitive_guard_associate_with_limits(
        polynomial,
        algebra_limits,
        stable.remaining(),
    )?;
    let raw = polynomial.raw();
    debug_assert_eq!(raw.nvars(), nvars);
    debug_assert_eq!(raw.nterms(), nterms);
    for (coefficient, exponents) in raw.coefficients.iter().zip(raw.exponents_iter()) {
        let coefficient = coefficient.to_string();
        stable.push_usize(coefficient.len())?;
        stable.push("#")?;
        stable.push(&coefficient)?;
        stable.push("@[")?;
        for (position, &exponent) in exponents.iter().enumerate() {
            if position != 0 {
                stable.push(",")?;
            }
            stable.push_usize(usize::from(exponent))?;
        }
        stable.push("];")?;
    }
    Ok(stable.finish())
}
