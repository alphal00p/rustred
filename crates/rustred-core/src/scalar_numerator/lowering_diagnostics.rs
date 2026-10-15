//! Private opt-in dependent-test gate. No process-global conversion policy.
use super::WeightedScalarNumeratorLowering;
use super::weighted::polynomial::testing;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Counts {
    pub rational: usize,
    pub generic: usize,
}

pub fn current() -> Option<Counts> {
    testing::current().map(|(rational, generic)| Counts { rational, generic })
}

/// Synchronous scoped policy; never propagates implicitly to another thread.
/// Dependent tests must check `current()` before/after every observed lowering.
pub fn scoped<T>(force_generic: bool, run: impl FnOnce() -> T) -> (T, Counts) {
    let (value, rational, generic) = testing::observed(force_generic, run);
    (value, Counts { rational, generic })
}

/// Reuse the existing native cache charge for exact polynomial coefficients.
/// Atom payload/container sizes are charged separately by the dependent test;
/// neither accounting measure claims to equal process RSS.
pub fn coefficient_payload_bytes(value: &WeightedScalarNumeratorLowering) -> Result<usize, String> {
    crate::reduction::coefficient_cache_weight(value.terms().iter().map(|term| term.coefficient()))
        .map(|weight| weight.coefficient_bytes)
        .map_err(|error| error.to_string())
}
