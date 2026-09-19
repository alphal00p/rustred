use std::sync::Arc;

use symbolica::domains::rational_polynomial::RationalPolynomialField;
use symbolica::prelude::{IntegerRing, MultivariatePolynomial};

use crate::family::symanzik::FeynmanPolynomialLimits;

/// Native Symbolica arithmetic throughout: no private polynomial encoding.
pub(super) type VacuumPolynomial =
    MultivariatePolynomial<RationalPolynomialField<IntegerRing, u16>, u16>;

/// Explicit preparation bounds, not a hard timeout or a bound on Symbolica's
/// internal scratch allocations. In particular graph canonization is native
/// and has no abort callback; input graph size is bounded before entering it.
#[derive(Clone, Copy, Debug)]
pub struct VacuumParametricLimits {
    pub symanzik: FeynmanPolynomialLimits,
    pub max_supports: usize,
    pub max_canonicalizations: usize,
    pub max_graph_vertices: usize,
    pub max_graph_edges: usize,
}

impl Default for VacuumParametricLimits {
    fn default() -> Self {
        Self {
            symanzik: FeynmanPolynomialLimits {
                max_polynomial_terms: 20_000,
                max_exponent_entries: 1_000_000,
                max_term_operations: 2_000_000,
                max_determinant_ring_operations: 1_000_000,
                ..FeynmanPolynomialLimits::default()
            },
            max_supports: 4096,
            max_canonicalizations: 16_384,
            max_graph_vertices: 4096,
            max_graph_edges: 65_536,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct Support {
    pub slots: Vec<usize>,
    pub u: VacuumPolynomial,
}

/// A sealed equality of two positive-power, unit-mass vacuum parameter
/// integrals. Full native U polynomials (including their overall scale) were
/// compared exactly after the indicated power-preserving permutation.
/// Supports are shared across aliases; this does not assert a momentum map.
#[derive(Clone, Debug)]
pub struct VerifiedVacuumParameterMap {
    pub(super) loops: usize,
    pub(super) source: Arc<Support>,
    pub(super) representative: Arc<Support>,
    pub(super) permutation: Vec<usize>,
}

impl VerifiedVacuumParameterMap {
    pub fn loop_count(&self) -> usize {
        self.loops
    }
    pub fn source_slots(&self) -> &[usize] {
        &self.source.slots
    }
    pub fn representative_slots(&self) -> &[usize] {
        &self.representative.slots
    }
    /// Source-local parameter position → representative-local position.
    pub fn parameter_permutation(&self) -> &[usize] {
        &self.permutation
    }
    pub fn source_u_term_count(&self) -> usize {
        self.source.u.nterms()
    }
    pub fn representative_u_term_count(&self) -> usize {
        self.representative.u.nterms()
    }
}
