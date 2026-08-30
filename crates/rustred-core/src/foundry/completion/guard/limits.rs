use crate::algebra::{IndexedAlgebraLimits, IndexedGuardLimits};
use crate::foundry::completion::stratum::StratumRegistryLimits;

/// Resource policy for one semantic guard atom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CoefficientIdealGuardLimits {
    /// Cumulative canonical generator-identity bytes processed before
    /// associate deduplication.
    pub(crate) max_generator_identity_bytes: usize,
    pub(crate) indexed_algebra: IndexedAlgebraLimits,
    pub(crate) guard_algebra: IndexedGuardLimits,
    pub(crate) predicate_identity: StratumRegistryLimits,
}

impl Default for CoefficientIdealGuardLimits {
    fn default() -> Self {
        Self {
            max_generator_identity_bytes: 67_108_864,
            indexed_algebra: IndexedAlgebraLimits::default(),
            guard_algebra: IndexedGuardLimits::default(),
            predicate_identity: StratumRegistryLimits::default(),
        }
    }
}
