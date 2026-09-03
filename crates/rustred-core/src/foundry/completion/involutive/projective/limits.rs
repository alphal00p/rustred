use super::super::InvolutiveLimits;
use super::error::ProjectiveError;

/// Retained-shape and cumulative-work envelope for one proposal-only
/// projective calculation.
///
/// The GCD and exact-quotient counters are conservative ingress envelopes.
/// Symbolica does not expose native scratch allocation, so these limits do
/// not claim to be hard RSS bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ProjectiveLimits {
    pub(super) involutive: InvolutiveLimits,
    pub(super) max_row_terms: usize,
    pub(super) max_provenance_terms: usize,
    pub(super) max_augmented_entries: usize,
    /// Raw denominator/translated-guard candidates admitted before
    /// canonicalization and deduplication.
    pub(super) max_localization_guard_candidates: usize,
    pub(super) max_polynomial_operations: usize,
    pub(super) max_sum_input_terms: usize,
    pub(super) max_multiplication_term_pairs: usize,
    pub(super) max_gcd_calls: usize,
    pub(super) max_gcd_term_pairs: usize,
    pub(super) max_gcd_multiple_inputs: usize,
    pub(super) max_lcm_steps: usize,
    pub(super) max_exact_divisions: usize,
    pub(super) max_translations: usize,
    pub(super) max_generated_polynomial_terms: usize,
    pub(super) max_retained_polynomial_terms: usize,
    pub(super) max_retained_polynomial_exponent_cells: usize,
    pub(super) max_retained_polynomial_bytes: usize,
}

impl Default for ProjectiveLimits {
    fn default() -> Self {
        Self {
            involutive: InvolutiveLimits::default(),
            max_row_terms: 1_000_000,
            max_provenance_terms: 1_000_000,
            max_augmented_entries: 2_000_000,
            max_localization_guard_candidates: 2_000_000,
            max_polynomial_operations: 100_000_000,
            max_sum_input_terms: 1_000_000_000,
            max_multiplication_term_pairs: 1_000_000_000,
            max_gcd_calls: 4_000_000,
            max_gcd_term_pairs: 1_000_000_000,
            max_gcd_multiple_inputs: 2_000_000,
            max_lcm_steps: 2_000_000,
            max_exact_divisions: 8_000_000,
            max_translations: 4_000_000,
            max_generated_polynomial_terms: 1_000_000_000,
            max_retained_polynomial_terms: 1_000_000_000,
            max_retained_polynomial_exponent_cells: 16_000_000_000,
            max_retained_polynomial_bytes: 17_179_869_184,
        }
    }
}

/// Exact logical work performed before a successful result or typed stop.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ProjectiveWorkCensus {
    pub(super) polynomial_operations: usize,
    pub(super) sum_input_terms: usize,
    pub(super) multiplication_term_pairs: usize,
    pub(super) gcd_calls: usize,
    pub(super) gcd_term_pairs: usize,
    pub(super) gcd_multiple_inputs: usize,
    pub(super) lcm_steps: usize,
    pub(super) exact_divisions: usize,
    pub(super) translations: usize,
    pub(super) generated_polynomial_terms: usize,
    pub(super) content_normalizations: usize,
}

/// Monotone work ledger shared by every operation in one projective replay.
///
/// Charges describe attempted native work and are deliberately not rolled
/// back when an operation returns an error.  Consequently a caller cannot
/// evade a campaign cap by retrying a failed step or by constructing a fresh
/// polynomial helper for every cancellation.
#[derive(Debug)]
pub(super) struct ProjectiveWorkBudget {
    limits: ProjectiveLimits,
    pub(super) census: ProjectiveWorkCensus,
}

impl ProjectiveWorkBudget {
    pub(super) const fn new(limits: ProjectiveLimits) -> Self {
        Self {
            limits,
            census: ProjectiveWorkCensus {
                polynomial_operations: 0,
                sum_input_terms: 0,
                multiplication_term_pairs: 0,
                gcd_calls: 0,
                gcd_term_pairs: 0,
                gcd_multiple_inputs: 0,
                lcm_steps: 0,
                exact_divisions: 0,
                translations: 0,
                generated_polynomial_terms: 0,
                content_normalizations: 0,
            },
        }
    }

    pub(super) const fn census(&self) -> ProjectiveWorkCensus {
        self.census
    }

    pub(super) fn require_limits(&self, limits: ProjectiveLimits) -> Result<(), ProjectiveError> {
        if self.limits == limits {
            Ok(())
        } else {
            Err(ProjectiveError::WorkBudgetLimitsMismatch)
        }
    }
}

impl Default for ProjectiveWorkBudget {
    fn default() -> Self {
        Self::new(ProjectiveLimits::default())
    }
}

/// Deterministic augmented-content policy for intermediate projective rows.
///
/// Every variant preserves the exact physical-row plus provenance identity.
/// Deferred rows merely retain a common polynomial scale until an explicit
/// full-normalization checkpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ProjectiveNormalizationPolicy {
    EveryCancellation,
    AdmissionOnly,
    WhenAugmentedEntriesDoNotExceed { max_entries: usize },
}

impl ProjectiveNormalizationPolicy {
    pub(super) const fn normalize_after_cancellation(self, augmented_entries: usize) -> bool {
        match self {
            Self::EveryCancellation => true,
            Self::AdmissionOnly => false,
            Self::WhenAugmentedEntriesDoNotExceed { max_entries } => {
                augmented_entries <= max_entries
            }
        }
    }
}

impl ProjectiveWorkCensus {
    pub(super) const fn polynomial_operations(self) -> usize {
        self.polynomial_operations
    }

    pub(super) const fn multiplication_term_pairs(self) -> usize {
        self.multiplication_term_pairs
    }

    pub(super) const fn gcd_calls(self) -> usize {
        self.gcd_calls
    }

    pub(super) const fn gcd_term_pairs(self) -> usize {
        self.gcd_term_pairs
    }

    pub(super) const fn exact_divisions(self) -> usize {
        self.exact_divisions
    }

    pub(super) const fn lcm_steps(self) -> usize {
        self.lcm_steps
    }

    pub(super) const fn translations(self) -> usize {
        self.translations
    }

    pub(super) const fn content_normalizations(self) -> usize {
        self.content_normalizations
    }
}
