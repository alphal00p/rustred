use super::super::super::InvolutiveLimits;
use super::super::limits::ModularGuideLimits;

/// Immutable retained-shape and ingress-work contract of one ELC1 session.
///
/// The nested exact and coefficient-circuit limits are copied into the owner;
/// callers cannot silently widen either contract after proofs exist.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ExactLazyLimits {
    pub(super) exact: InvolutiveLimits,
    pub(super) coefficient: ModularGuideLimits,
    pub(super) max_transaction_attempts: usize,
    pub(super) max_committed_transactions: usize,
    pub(super) max_imported_physical_terms: usize,
    pub(super) max_imported_provenance_terms: usize,
    pub(super) max_imported_guard_descriptors: usize,
    pub(super) max_total_imported_physical_terms: usize,
    pub(super) max_total_imported_provenance_terms: usize,
    pub(super) max_total_imported_guard_descriptors: usize,
}

impl Default for ExactLazyLimits {
    fn default() -> Self {
        Self {
            exact: InvolutiveLimits::default(),
            coefficient: ModularGuideLimits::default(),
            max_transaction_attempts: 16_000_000,
            max_committed_transactions: 16_000_000,
            max_imported_physical_terms: 16_000_000,
            max_imported_provenance_terms: 16_000_000,
            max_imported_guard_descriptors: 16_000_000,
            max_total_imported_physical_terms: 1_000_000_000,
            max_total_imported_provenance_terms: 1_000_000_000,
            max_total_imported_guard_descriptors: 1_000_000_000,
        }
    }
}

/// Monotone session accounting. Failed transactions keep attempted ingress
/// and coefficient-DAG churn charged; only live DAG storage rolls back.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ExactLazyCensus {
    pub(super) transaction_attempts: usize,
    pub(super) committed_transactions: usize,
    pub(super) imported_physical_terms: usize,
    pub(super) imported_provenance_terms: usize,
    pub(super) imported_guard_descriptors: usize,
}

impl ExactLazyCensus {
    pub(super) const fn transaction_attempts(self) -> usize {
        self.transaction_attempts
    }

    pub(super) const fn committed_transactions(self) -> usize {
        self.committed_transactions
    }

    pub(super) const fn imported_physical_terms(self) -> usize {
        self.imported_physical_terms
    }

    pub(super) const fn imported_provenance_terms(self) -> usize {
        self.imported_provenance_terms
    }

    pub(super) const fn imported_guard_descriptors(self) -> usize {
        self.imported_guard_descriptors
    }
}
