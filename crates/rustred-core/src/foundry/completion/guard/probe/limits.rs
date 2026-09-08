use crate::algebra::IndexedAlgebraLimits;

use super::super::CoefficientIdealGuardLimits;

/// Resource policy for one immutable catalog of exact guard predicates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExactGuardPredicateCatalogLimits {
    pub(crate) atom: CoefficientIdealGuardLimits,
    pub(crate) max_predicates: usize,
    pub(crate) max_input_terms: usize,
    pub(crate) max_predicate_identity_bytes: usize,
}

impl Default for ExactGuardPredicateCatalogLimits {
    fn default() -> Self {
        Self {
            atom: CoefficientIdealGuardLimits::default(),
            max_predicates: 16_384,
            max_input_terms: 4_194_304,
            max_predicate_identity_bytes: 67_108_864,
        }
    }
}

/// Aggregate policy for proving one exact integer/base probe belongs to every
/// branch of one decorated stratum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExactGuardProbeLimits {
    pub(crate) indexed_algebra: IndexedAlgebraLimits,
    pub(crate) max_guards: usize,
    pub(crate) max_input_terms: usize,
    pub(crate) max_index_specialization_power_operations: usize,
    pub(crate) max_base_evaluation_power_operations: usize,
    pub(crate) max_retained_coordinates: usize,
    pub(crate) max_retained_exact_value_bits: usize,
}

impl Default for ExactGuardProbeLimits {
    fn default() -> Self {
        Self {
            indexed_algebra: IndexedAlgebraLimits::default(),
            max_guards: 16_384,
            max_input_terms: 4_194_304,
            max_index_specialization_power_operations: 67_108_864,
            max_base_evaluation_power_operations: 67_108_864,
            max_retained_coordinates: 8_192,
            max_retained_exact_value_bits: 67_108_864,
        }
    }
}
