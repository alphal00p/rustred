//! Authenticated coefficient/parameter context and checked polynomial operations.

mod arithmetic;
mod authentication;
mod gradient;
mod state;

use std::sync::Arc;

use symbolica::domains::rational_polynomial::RationalPolynomialField;
use symbolica::prelude::{IntegerRing, PolyVariable};

use crate::algebra::CoefficientContext;

use super::model::{FeynmanPolynomialLimits, RawFeynmanPolynomial};

/// Authenticated coefficient and variable map for `K[x_0,...,x_{N-1}]`.
#[derive(Debug)]
pub struct FeynmanPolynomialContext {
    pub(super) family_fingerprint: Arc<String>,
    pub(super) coefficients: CoefficientContext,
    pub(super) variables: Arc<Vec<PolyVariable>>,
    pub(super) field: RationalPolynomialField<IntegerRing, u16>,
    pub(super) template: RawFeynmanPolynomial,
    pub(super) limits: FeynmanPolynomialLimits,
}
