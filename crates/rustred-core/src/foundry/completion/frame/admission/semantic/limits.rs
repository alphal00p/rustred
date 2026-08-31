//! Aggregate resource policy for exact-circuit semantic compilation.

use crate::algebra::ExactAlgebraLimits;
use crate::foundry::completion::guard::CoefficientIdealGuardLimits;
use crate::foundry::completion::guard::decision::GuardDecisionDagLimits;

/// Aggregate resource policy for one exact-circuit semantic compilation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExactCircuitSemanticLimits {
    pub(crate) max_candidates: usize,
    pub(crate) max_residual_terms: usize,
    pub(crate) max_source_contributions: usize,
    pub(crate) max_pivot_guards: usize,
    pub(crate) max_nonzero_guards: usize,
    pub(crate) max_guard_origins: usize,
    pub(crate) max_condition_sources: usize,
    pub(crate) max_condition_source_coordinate_cells: usize,
    pub(crate) max_dependency_owners: usize,
    pub(crate) max_guard_coefficient_equations: usize,
    pub(crate) max_guard_base_monomial_exponents: usize,
    pub(crate) max_guard_generators: usize,
    pub(crate) max_guard_identity_bytes: usize,
    pub(crate) max_modular_sample_point_entries: usize,
    pub(crate) max_modular_diagnostic_entries: usize,
    /// Numerators and denominators are charged separately.
    pub(crate) max_exact_polynomials: usize,
    pub(crate) max_polynomial_terms: usize,
    pub(crate) max_exponent_entries: usize,
    pub(crate) max_integer_coefficient_bits: usize,
    pub(crate) exact_algebra: ExactAlgebraLimits,
    pub(crate) guard_atom: CoefficientIdealGuardLimits,
    pub(crate) guard_dag: GuardDecisionDagLimits,
}

impl Default for ExactCircuitSemanticLimits {
    fn default() -> Self {
        Self {
            max_candidates: 4_096,
            max_residual_terms: 262_144,
            max_source_contributions: 262_144,
            max_pivot_guards: 262_144,
            max_nonzero_guards: 262_144,
            max_guard_origins: 1_048_576,
            max_condition_sources: 1_048_576,
            max_condition_source_coordinate_cells: 67_108_864,
            max_dependency_owners: 1_048_576,
            max_guard_coefficient_equations: 1_048_576,
            max_guard_base_monomial_exponents: 67_108_864,
            max_guard_generators: 1_048_576,
            max_guard_identity_bytes: 134_217_728,
            max_modular_sample_point_entries: 16_777_216,
            max_modular_diagnostic_entries: 67_108_864,
            max_exact_polynomials: 2_097_152,
            max_polynomial_terms: 16_777_216,
            max_exponent_entries: 67_108_864,
            max_integer_coefficient_bits: 268_435_456,
            exact_algebra: ExactAlgebraLimits::default(),
            guard_atom: CoefficientIdealGuardLimits::default(),
            guard_dag: GuardDecisionDagLimits::default(),
        }
    }
}
