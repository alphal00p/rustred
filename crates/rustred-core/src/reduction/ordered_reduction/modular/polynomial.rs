//! Slot of `OrderedWeightArithmetic::ModularPolynomial`: scheduler passes
//! whose weights are rational functions over `Z_p[d]` with factorized
//! denominators, lifted by the shared driver ([`super::lift::lift`]).
//!
//! Not implemented yet: [`run`] validates the request and fails with
//! [`unsupported`]. The implementation provides an
//! [`ImageSource`](super::lift::ImageSource) whose `Structure` holds what
//! must agree between primes before their images may be combined (the
//! terminal key list and the guard list of a pass), returns per prime the
//! reduced image of every terminal in that key order (`None` for a prime it
//! cannot use), and maps the [`Lifted`](super::lift::Lifted) structure and
//! coefficients to the scheduler output.
use std::collections::BTreeMap;

use super::error;
use crate::algebra::Coefficient;
use crate::reduction::ordered_reduction::arithmetic::{ModularCommon, PolynomialOptions};
use crate::reduction::ordered_reduction::{OrderedKernel, OrderedOutput};
use crate::reduction::{ReductionError, ReductionRequest, ReductionStatistics};

/// Error of every `ModularPolynomial` request until the mode is implemented.
pub(crate) fn unsupported() -> ReductionError {
    error(
        "modular_polynomial weight arithmetic is not implemented yet; \
         select exact or modular_sampled",
    )
}

/// Run one request with `ModularPolynomial` weights.
pub(in crate::reduction::ordered_reduction) fn run<K: OrderedKernel>(
    kernel: &mut K,
    _request: &mut ReductionRequest,
    _initial: BTreeMap<K::Key, Coefficient>,
    _nonzero_conditions: Vec<Coefficient>,
    common: ModularCommon,
    options: PolynomialOptions,
    _work: &mut ReductionStatistics,
) -> Result<OrderedOutput, K::Error> {
    common.validate().map_err(error)?;
    options.validate().map_err(error)?;
    super::check_context(kernel.context())?;
    Err(unsupported().into())
}
