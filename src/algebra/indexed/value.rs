//! Authenticated values over the exact indexed coefficient field.

use std::sync::Arc;

use symbolica::prelude::*;

use crate::algebra::{Coefficient, CoefficientPolynomial};

/// A canonical coefficient known to belong to one exact `K(n)` variable map.
///
/// All public constructors normalize numerator and denominator to coprime
/// factors. This invariant lets integral index translations avoid a second
/// polynomial GCD: `n -> n + a` is a polynomial-ring automorphism and thus
/// preserves coprimality.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexedCoefficient {
    pub(super) raw: Coefficient,
    pub(super) context: Arc<str>,
}

impl IndexedCoefficient {
    pub fn raw(&self) -> &Coefficient {
        &self.raw
    }

    pub fn is_zero(&self) -> bool {
        self.raw.is_zero()
    }

    pub fn to_expression(&self) -> Atom {
        self.raw.to_expression()
    }
}

/// A polynomial over the exact index-extended map `K(n)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexedPolynomial {
    pub(super) raw: CoefficientPolynomial,
    pub(super) context: Arc<str>,
}

impl IndexedPolynomial {
    pub fn raw(&self) -> &CoefficientPolynomial {
        &self.raw
    }

    pub fn to_expression(&self) -> Atom {
        self.raw.to_expression()
    }

    pub fn is_zero(&self) -> bool {
        self.raw.is_zero()
    }

    pub fn is_one(&self) -> bool {
        self.raw.is_one()
    }

    pub fn is_nonzero_constant(&self) -> bool {
        self.raw.is_constant() && !self.raw.is_zero()
    }

    /// Number of sparse monomials retained by the authenticated Symbolica
    /// polynomial.  Proof-bearing layers use this to preflight the memory
    /// cost of duplicating a predicate across complementary case branches.
    pub fn term_count(&self) -> usize {
        self.raw.nterms()
    }
}
