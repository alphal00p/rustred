use std::sync::Arc;

use symbolica::atom::Atom;

use crate::algebra::{Coefficient, ExactAlgebraLimits};
use crate::family::IntegralKey;

/// Resource policy for polynomial scalar-product lowering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScalarNumeratorLimits {
    pub exact_algebra: ExactAlgebraLimits,
    pub max_input_nodes: usize,
    pub max_momentum_label_nodes: usize,
    /// Exact subtree comparisons used to identify loop-momentum dependence.
    pub max_loop_momentum_label_checks: usize,
    pub max_nesting_depth: usize,
    pub max_input_terms: usize,
    pub max_factors_per_term: usize,
    pub max_scalar_product_degree: usize,
    pub max_polynomial_terms: usize,
    pub max_lowered_terms: usize,
    pub max_output_atom_nodes: usize,
}

impl Default for ScalarNumeratorLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            max_input_nodes: 1_000_000,
            max_momentum_label_nodes: 100_000,
            max_loop_momentum_label_checks: 64_000_000,
            max_nesting_depth: 256,
            max_input_terms: 100_000,
            max_factors_per_term: 100_000,
            max_scalar_product_degree: 1_000_000,
            max_polynomial_terms: 100_000,
            max_lowered_terms: 1_000_000,
            max_output_atom_nodes: 100_000_000,
        }
    }
}

/// One polynomial numerator term lowered onto a typed artifact-family key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoweredScalarNumeratorTerm {
    pub(super) coefficient: Coefficient,
    pub(super) scalar_spectator: Atom,
    pub(super) integral: IntegralKey,
    pub(super) common_mass_squared_power: u32,
}

impl LoweredScalarNumeratorTerm {
    /// Exact coefficient in the sealed artifact's coefficient context.
    pub fn coefficient(&self) -> &Coefficient {
        &self.coefficient
    }

    /// Caller-owned coefficient independent of every admitted loop scalar product.
    pub fn scalar_spectator(&self) -> &Atom {
        &self.scalar_spectator
    }

    /// Integral key after cancelling affine denominator factors.
    pub fn integral(&self) -> &IntegralKey {
        &self.integral
    }

    /// Additional power of the physical common mass squared.
    ///
    /// A unit-mass affine constant has mass dimension two, whereas a
    /// denominator branch already carries that dimension. Each selected
    /// constant branch therefore contributes one explicit power.
    pub const fn common_mass_squared_power(&self) -> u32 {
        self.common_mass_squared_power
    }
}

/// Deterministically ordered result of one scalar-numerator lowering request.
#[derive(Clone, Debug)]
pub struct ScalarNumeratorLowering {
    pub(super) family_identity: Arc<String>,
    pub(super) terms: Vec<LoweredScalarNumeratorTerm>,
}

impl ScalarNumeratorLowering {
    /// Stable identity of the artifact family that minted every term.
    pub fn family_fingerprint(&self) -> &str {
        self.family_identity.as_str()
    }

    pub fn terms(&self) -> &[LoweredScalarNumeratorTerm] {
        &self.terms
    }

    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }
}
