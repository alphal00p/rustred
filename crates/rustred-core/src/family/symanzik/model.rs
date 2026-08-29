//! Authenticated Feynman-polynomial values and resource policy.

use std::sync::Arc;

use symbolica::prelude::*;

use crate::algebra::{Coefficient, ExactAlgebraLimits};

/// Sparse polynomials in Feynman parameters with coefficients in the
/// authenticated family field `K`.
pub(super) type RawFeynmanPolynomial =
    MultivariatePolynomial<RationalPolynomialField<IntegerRing, u16>, u16>;

/// Symbolica's native polynomial-ring adapter for the natural `K[x]` domain.
pub(super) type FeynmanPolynomialRing =
    PolynomialRing<RationalPolynomialField<IntegerRing, u16>, u16>;

/// Checked work and representation budgets for one `U/F/G` construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeynmanPolynomialLimits {
    pub exact_algebra: ExactAlgebraLimits,
    pub max_parameters: usize,
    pub max_parameter_exponent: u16,
    /// Maximum terms in an authenticated polynomial and in the prospective
    /// outer-ring result buffer admitted before native Symbolica arithmetic.
    pub max_polynomial_terms: usize,
    /// Maximum dense Feynman-exponent entries retained or constructed by one
    /// polynomial operation.  Symbolica stores one exponent for every
    /// `(term, parameter)` pair even when most exponents are zero.
    pub max_exponent_entries: usize,
    /// Aggregate RustRed-observable polynomial-term work for one public
    /// construction or differentiation call. Symbolica's native outer-ring
    /// arithmetic and determinant do not expose the census of coefficient-ring
    /// or dense/heap intermediates. RustRed therefore admits their prospective
    /// structural work before entry and authenticates every retained result;
    /// this is not an RSS or opaque-native-temporary bound.
    pub max_term_operations: usize,
    /// Maximum structural entries in one square matrix handed to Symbolica's
    /// native determinant implementation.  This is not an RSS bound: campaign
    /// admission must separately charge the resident caller input, RustRed's
    /// input clone, Symbolica's full Bareiss matrix clone for sizes at least
    /// four, intermediate polynomial/coefficient swell, exact-division and GCD
    /// temporaries, allocator/TLS scratch, and any adjugate-minor clones.
    pub max_determinant_matrix_entries: usize,
    /// Aggregate conservative count of structural arithmetic ring calls made
    /// by Symbolica determinants. Sizes two and three use the exact native
    /// formulas; larger sizes use the public fraction-free Bareiss structure.
    /// Pivot zero probes are excluded, and one counted polynomial operation can
    /// own substantial opaque native algebra and memory.
    pub max_determinant_ring_operations: usize,
    pub max_adjugate_minors: usize,
}

impl Default for FeynmanPolynomialLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            max_parameters: 4_096,
            max_parameter_exponent: u16::MAX,
            max_polynomial_terms: 4_000_000,
            max_exponent_entries: 64_000_000,
            max_term_operations: 16_000_000,
            max_determinant_matrix_entries: 1_048_576,
            max_determinant_ring_operations: 16_000_000,
            max_adjugate_minors: 1_048_576,
        }
    }
}
/// One polynomial authenticated as a member of a specific family's `K[x]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeynmanPolynomial {
    pub(super) raw: RawFeynmanPolynomial,
    pub(super) context: Arc<String>,
}

impl FeynmanPolynomial {
    pub(crate) fn raw(&self) -> &RawFeynmanPolynomial {
        &self.raw
    }

    pub fn is_zero(&self) -> bool {
        self.raw.is_zero()
    }

    pub fn term_count(&self) -> usize {
        self.raw.nterms()
    }

    pub fn terms(&self) -> impl Iterator<Item = (&Coefficient, &[u16])> {
        self.raw.coefficients.iter().zip(self.raw.exponents_iter())
    }

    pub fn coefficient(&self, exponents: &[u16]) -> Option<&Coefficient> {
        self.raw
            .exponents_iter()
            .position(|candidate| candidate == exponents)
            .map(|term| &self.raw.coefficients[term])
    }
}
