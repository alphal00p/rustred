//! Tensor requests, resource policy, and typed operation results.

use std::sync::Arc;

use symbolica::atom::Atom;

use crate::algebra::{Coefficient, CoefficientPolynomial, ExactAlgebraLimits};
use crate::family::{IntegralKey, ScalarProductCoordinate};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TensorLane {
    Auto,
    SingleScaleVacuum,
    Generic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResolvedTensorLane {
    SingleScaleVacuum,
}

/// Explicit admission policy for one tensor request and its lowered result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TensorLimits {
    pub exact_algebra: ExactAlgebraLimits,
    pub max_input_nodes: usize,
    pub max_momentum_label_nodes: usize,
    /// Worst-case exact subtree comparisons used to exclude hidden loop
    /// momentum dependence from opaque scalar factors.
    pub max_loop_momentum_label_checks: usize,
    pub max_nesting_depth: usize,
    pub max_input_terms: usize,
    pub max_factors_per_term: usize,
    pub max_internal_rank: usize,
    pub max_scalar_products_per_term: usize,
    pub max_projected_terms: usize,
    pub max_lowered_terms: usize,
    /// Aggregate Atom nodes cloned into lowered spectator/tensor outputs.
    pub max_output_atom_nodes: usize,
}

impl Default for TensorLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            max_input_nodes: 1_000_000,
            max_momentum_label_nodes: 100_000,
            max_loop_momentum_label_checks: 64_000_000,
            max_nesting_depth: 256,
            max_input_terms: 100_000,
            max_factors_per_term: 100_000,
            max_internal_rank: 64,
            max_scalar_products_per_term: 64,
            max_projected_terms: 100_000,
            max_lowered_terms: 1_000_000,
            max_output_atom_nodes: 100_000_000,
        }
    }
}

/// Exact Atom labels used in the first argument of caller vector heads and in
/// caller scalar products.  External labels are numerator spectators and are
/// therefore deliberately independent of `IntegralFamily::external_count`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TensorMomenta {
    loop_momenta: Vec<Atom>,
    external_momenta: Vec<Atom>,
}

impl TensorMomenta {
    pub fn new(loop_momenta: Vec<Atom>, external_momenta: Vec<Atom>) -> Self {
        Self {
            loop_momenta,
            external_momenta,
        }
    }

    pub fn loop_momenta(&self) -> &[Atom] {
        &self.loop_momenta
    }

    pub fn external_momenta(&self) -> &[Atom] {
        &self.external_momenta
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TensorGuardOrigin {
    RankTwoProjectorDimension,
}

/// One exact polynomial required to remain nonzero. Denominator conditions of
/// the rational value from which it arose remain inherited from the
/// authenticated family/presentation domain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TensorGuard {
    pub(super) polynomial: CoefficientPolynomial,
    pub(super) origin: TensorGuardOrigin,
}

impl TensorGuard {
    pub fn polynomial(&self) -> &CoefficientPolynomial {
        &self.polynomial
    }

    pub const fn origin(&self) -> TensorGuardOrigin {
        self.origin
    }
}

/// One projected term before family scalar products are expanded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectedTensorTerm {
    pub(super) coefficient: Coefficient,
    pub(super) scalar_spectator: Atom,
    pub(super) outside_tensor: Atom,
    pub(super) scalar_products: Vec<ScalarProductCoordinate>,
}

impl ProjectedTensorTerm {
    pub fn coefficient(&self) -> &Coefficient {
        &self.coefficient
    }

    pub fn scalar_spectator(&self) -> &Atom {
        &self.scalar_spectator
    }

    pub fn outside_tensor(&self) -> &Atom {
        &self.outside_tensor
    }

    pub fn scalar_products(&self) -> &[ScalarProductCoordinate] {
        &self.scalar_products
    }
}

/// Result of a Lorentz projection admitted against an eligible scalar integral
/// key, still carrying typed family scalar products.
#[derive(Clone, Debug)]
pub struct TensorProjection {
    pub(super) family_identity: Arc<String>,
    pub(super) lane: ResolvedTensorLane,
    pub(super) terms: Vec<ProjectedTensorTerm>,
    pub(super) guards: Vec<TensorGuard>,
}

impl TensorProjection {
    pub const fn lane(&self) -> ResolvedTensorLane {
        self.lane
    }

    pub fn terms(&self) -> &[ProjectedTensorTerm] {
        &self.terms
    }

    pub fn guards(&self) -> &[TensorGuard] {
        &self.guards
    }

    pub fn family_fingerprint(&self) -> &str {
        self.family_identity.as_str()
    }

    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }
}

/// One family-keyed term after exact affine scalar-product expansion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TensorReductionTerm {
    pub(super) coefficient: Coefficient,
    pub(super) scalar_spectator: Atom,
    pub(super) outside_tensor: Atom,
    pub(super) integral: IntegralKey,
}

impl TensorReductionTerm {
    pub fn coefficient(&self) -> &Coefficient {
        &self.coefficient
    }

    pub fn scalar_spectator(&self) -> &Atom {
        &self.scalar_spectator
    }

    pub fn outside_tensor(&self) -> &Atom {
        &self.outside_tensor
    }

    pub fn integral(&self) -> &IntegralKey {
        &self.integral
    }
}

/// Result of the standalone scalar-product lowering operation.
#[derive(Clone, Debug)]
pub struct ScalarProductLowering {
    pub(super) family_identity: Arc<String>,
    pub(super) lane: ResolvedTensorLane,
    pub(super) terms: Vec<TensorReductionTerm>,
    pub(super) guards: Vec<TensorGuard>,
}

impl ScalarProductLowering {
    pub const fn lane(&self) -> ResolvedTensorLane {
        self.lane
    }

    pub fn terms(&self) -> &[TensorReductionTerm] {
        &self.terms
    }

    pub fn guards(&self) -> &[TensorGuard] {
        &self.guards
    }

    pub fn family_fingerprint(&self) -> &str {
        self.family_identity.as_str()
    }

    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }
}

/// Composed projection plus scalar-product lowering result.
#[derive(Clone, Debug)]
pub struct TensorReduction(ScalarProductLowering);

impl TensorReduction {
    pub const fn lane(&self) -> ResolvedTensorLane {
        self.0.lane()
    }

    pub fn terms(&self) -> &[TensorReductionTerm] {
        self.0.terms()
    }

    pub fn guards(&self) -> &[TensorGuard] {
        self.0.guards()
    }

    pub fn family_fingerprint(&self) -> &str {
        self.0.family_fingerprint()
    }

    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    pub fn into_lowering(self) -> ScalarProductLowering {
        self.0
    }

    pub(super) fn from_lowering(lowering: ScalarProductLowering) -> Self {
        Self(lowering)
    }
}
