use rustred::solver::{
    CaseIntersectionLimits, CoefficientVariableOrder, FiniteCaseLimits, NumericalExactBackend,
    OwnerDomainAttemptLimits, OwnerFeedbackPolicy, SearchOptions, SymbolicExactBackend,
};

use super::super::model::{CaseRecord, IntegralRecord, RuleRecord};

pub(super) const SCHEMA: &str = "rustred.partial-domain-rules.binary.v1";

/// Partial rules plus explicit provenance. No field can assert domain closure.
#[derive(Clone, Debug, bincode::Encode, bincode::Decode)]
pub(super) struct DomainRecord {
    pub schema: String,
    pub base_owner_blake3: [u8; 32],
    pub family_fingerprint: String,
    pub owner_sector: Vec<bool>,
    pub root_sector: Vec<bool>,
    pub integral_order: String,
    pub permutation: Option<Vec<usize>>,
    pub max_numerator_rank: Option<u32>,
    pub finite_case_policy: String,
    pub policy: PolicyRecord,
    pub attempt_limits: AttemptLimitsRecord,
    pub requested_cases: Vec<CaseRecord>,
    pub rules: Vec<RuleRecord>,
    /// Version one explicitly rejects every nonempty list; no new terminals.
    pub finite_residuals: Vec<IntegralRecord>,
}

#[derive(Clone, Debug, bincode::Encode, bincode::Decode)]
pub(super) struct PolicyRecord {
    numerical_depth: u32,
    symbolic_max_depth: Option<u32>,
    prime: u64,
    sample_seed: u64,
    symbolic_backend: BackendRecord,
    numerical_factorized: bool,
    coefficient_order: CoefficientOrderRecord,
}

#[derive(Clone, Copy, Debug, bincode::Encode, bincode::Decode)]
enum CoefficientOrderRecord {
    Original,
    Reverse,
    IndicesFirst,
}

#[derive(Clone, Copy, Debug, bincode::Encode, bincode::Decode)]
enum BackendRecord {
    Sparse,
    SparseFactorized,
    SparseTargetOnly,
    SparseTargetOnlyFactorized,
    DenseFractionFree {
        max_matrix_entries: usize,
    },
    SemiNumerical {
        max_degree: u16,
        max_probes: usize,
        max_attempts: usize,
        max_primes: usize,
    },
    SemiNumericalSourceWeights {
        max_degree: u16,
        max_probes: usize,
        max_attempts: usize,
        max_primes: usize,
        max_cached_images: usize,
        max_cached_values: usize,
        max_weight_slots: usize,
    },
}

impl From<OwnerFeedbackPolicy> for PolicyRecord {
    fn from(value: OwnerFeedbackPolicy) -> Self {
        use SymbolicExactBackend as S;
        let symbolic_backend = match value.symbolic_exact_backend {
            S::Sparse => BackendRecord::Sparse,
            S::SparseFactorized => BackendRecord::SparseFactorized,
            S::SparseTargetOnly => BackendRecord::SparseTargetOnly,
            S::SparseTargetOnlyFactorized => BackendRecord::SparseTargetOnlyFactorized,
            S::DenseFractionFree { max_matrix_entries } => {
                BackendRecord::DenseFractionFree { max_matrix_entries }
            }
            #[cfg(feature = "reconstruction")]
            S::SemiNumerical {
                max_degree,
                max_probes,
                max_attempts,
                max_primes,
            } => BackendRecord::SemiNumerical {
                max_degree,
                max_probes,
                max_attempts,
                max_primes,
            },
            #[cfg(feature = "reconstruction")]
            S::SemiNumericalSourceWeights {
                max_degree,
                max_probes,
                max_attempts,
                max_primes,
                max_cached_images,
                max_cached_values,
                max_weight_slots,
            } => BackendRecord::SemiNumericalSourceWeights {
                max_degree,
                max_probes,
                max_attempts,
                max_primes,
                max_cached_images,
                max_cached_values,
                max_weight_slots,
            },
        };
        Self {
            numerical_depth: value.numerical_depth,
            symbolic_max_depth: value.symbolic.max_depth,
            prime: value.symbolic.prime,
            sample_seed: value.symbolic.sample_seed,
            symbolic_backend,
            numerical_factorized: value.numerical_exact_backend
                == NumericalExactBackend::SparseFactorized,
            coefficient_order: match value.coefficient_variable_order {
                CoefficientVariableOrder::Original => CoefficientOrderRecord::Original,
                CoefficientVariableOrder::Reverse => CoefficientOrderRecord::Reverse,
                CoefficientVariableOrder::IndicesFirst => CoefficientOrderRecord::IndicesFirst,
            },
        }
    }
}

impl PolicyRecord {
    pub(super) fn native(&self) -> Result<OwnerFeedbackPolicy, crate::AppError> {
        use SymbolicExactBackend as S;
        let symbolic_exact_backend = match self.symbolic_backend {
            BackendRecord::Sparse => S::Sparse,
            BackendRecord::SparseFactorized => S::SparseFactorized,
            BackendRecord::SparseTargetOnly => S::SparseTargetOnly,
            BackendRecord::SparseTargetOnlyFactorized => S::SparseTargetOnlyFactorized,
            BackendRecord::DenseFractionFree { max_matrix_entries } => {
                S::DenseFractionFree { max_matrix_entries }
            }
            #[cfg(feature = "reconstruction")]
            BackendRecord::SemiNumerical {
                max_degree,
                max_probes,
                max_attempts,
                max_primes,
            } => S::SemiNumerical {
                max_degree,
                max_probes,
                max_attempts,
                max_primes,
            },
            #[cfg(feature = "reconstruction")]
            BackendRecord::SemiNumericalSourceWeights {
                max_degree,
                max_probes,
                max_attempts,
                max_primes,
                max_cached_images,
                max_cached_values,
                max_weight_slots,
            } => S::SemiNumericalSourceWeights {
                max_degree,
                max_probes,
                max_attempts,
                max_primes,
                max_cached_images,
                max_cached_values,
                max_weight_slots,
            },
            #[cfg(not(feature = "reconstruction"))]
            BackendRecord::SemiNumerical { .. }
            | BackendRecord::SemiNumericalSourceWeights { .. } => {
                return Err(crate::AppError::input(
                    "saved overlay backend requires the reconstruction feature",
                ));
            }
        };
        Ok(OwnerFeedbackPolicy {
            numerical_depth: self.numerical_depth,
            symbolic: SearchOptions {
                max_depth: self.symbolic_max_depth,
                prime: self.prime,
                sample_seed: self.sample_seed,
            },
            symbolic_exact_backend,
            numerical_exact_backend: if self.numerical_factorized {
                NumericalExactBackend::SparseFactorized
            } else {
                NumericalExactBackend::Sparse
            },
            coefficient_variable_order: match self.coefficient_order {
                CoefficientOrderRecord::Original => CoefficientVariableOrder::Original,
                CoefficientOrderRecord::Reverse => CoefficientVariableOrder::Reverse,
                CoefficientOrderRecord::IndicesFirst => CoefficientVariableOrder::IndicesFirst,
            },
        })
    }
}

#[derive(Clone, Debug, bincode::Encode, bincode::Decode)]
pub(super) struct AttemptLimitsRecord {
    max_requested_cases: usize,
    finite_points: usize,
    finite_terminals: usize,
    intersection_work: usize,
    intersection_terms: usize,
    intersection_normalizations: usize,
    intersection_factorizations: usize,
    max_symbolic_cases: Option<usize>,
}

impl From<OwnerDomainAttemptLimits> for AttemptLimitsRecord {
    fn from(value: OwnerDomainAttemptLimits) -> Self {
        Self {
            max_requested_cases: value.max_requested_cases,
            finite_points: value.finite_case_limits.max_visited_points,
            finite_terminals: value.finite_case_limits.max_retained_terminals,
            intersection_work: value.case_intersection_limits.max_work_items,
            intersection_terms: value.case_intersection_limits.max_terms_per_conjunction,
            intersection_normalizations: value.case_intersection_limits.max_normalizations,
            intersection_factorizations: value.case_intersection_limits.max_factorizations,
            max_symbolic_cases: value.max_symbolic_cases,
        }
    }
}

impl AttemptLimitsRecord {
    pub(super) fn native(&self) -> OwnerDomainAttemptLimits {
        OwnerDomainAttemptLimits {
            max_requested_cases: self.max_requested_cases,
            finite_case_limits: FiniteCaseLimits {
                max_visited_points: self.finite_points,
                max_retained_terminals: self.finite_terminals,
            },
            case_intersection_limits: CaseIntersectionLimits {
                max_work_items: self.intersection_work,
                max_terms_per_conjunction: self.intersection_terms,
                max_normalizations: self.intersection_normalizations,
                max_factorizations: self.intersection_factorizations,
            },
            max_symbolic_cases: self.max_symbolic_cases,
        }
    }
}
