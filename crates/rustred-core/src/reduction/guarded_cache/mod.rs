//! Explicit completed guarded roots for borrowed sealed programs. This cache
//! never stores the ordinary reducer's guardless descendants or an active DAG.
mod cache;
mod export;
mod program;
#[cfg(test)]
mod tests;

use super::{ReductionError, ReductionStatistics};
use crate::algebra::CoefficientContext;
use crate::persistence::application_cache::{
    ApplicationCacheBinding, ApplicationCacheKey, ApplicationCacheRecord,
};
use std::fmt;

pub use cache::{GuardedRootCache, GuardedRootCacheStatistics};
pub use export::{GuardedRootExportError, GuardedRootExportStatistics};
pub use program::ClosedUnitMassProgram;

#[derive(Clone, Copy, PartialEq, Eq)]
enum RootUse {
    Entry,
    Descendant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardedCacheBackingErrorKind {
    Invalid,
    Limit,
    Io,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuardedCacheBackingError {
    pub kind: GuardedCacheBackingErrorKind,
    pub message: String,
}
impl fmt::Display for GuardedCacheBackingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for GuardedCacheBackingError {}

/// Optional caller-owned native backing. Errors are never interpreted as
/// misses. The adapter owns persistence, checkpoint cadence and I/O bounds;
/// the core revalidates every returned record before RAM admission or use.
pub trait GuardedCacheBacking {
    fn binding(&self) -> &ApplicationCacheBinding;
    fn lookup(
        &mut self,
        key: &ApplicationCacheKey,
        context: &CoefficientContext,
    ) -> Result<Option<ApplicationCacheRecord>, GuardedCacheBackingError>;
    fn store_completed(
        &mut self,
        record: &ApplicationCacheRecord,
        context: &CoefficientContext,
    ) -> Result<(), GuardedCacheBackingError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuardedCacheError {
    Reduction(ReductionError),
    BindingMismatch,
    InvalidRecord(&'static str),
    Backing(GuardedCacheBackingError),
}
impl From<ReductionError> for GuardedCacheError {
    fn from(error: ReductionError) -> Self {
        Self::Reduction(error)
    }
}
impl From<GuardedCacheBackingError> for GuardedCacheError {
    fn from(error: GuardedCacheBackingError) -> Self {
        Self::Backing(error)
    }
}
impl fmt::Display for GuardedCacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reduction(error) => error.fmt(f),
            Self::BindingMismatch => f.write_str("guarded cache artifact/program binding mismatch"),
            Self::InvalidRecord(detail) => write!(f, "invalid guarded cache record: {detail}"),
            Self::Backing(error) => write!(f, "guarded cache backing: {error}"),
        }
    }
}
impl std::error::Error for GuardedCacheError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardedRootSource {
    Ram,
    Backing,
    Reduced,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuardedRootResult {
    /// Raw exact unit-mass coefficients and the original ordered guards.
    pub record: ApplicationCacheRecord,
    /// Native work for this successful request, never historical cached work.
    /// For a cold solve, cached_* fields are the aggregate shared-budget
    /// snapshot before transient reducer cleanup, not final root-cache size.
    /// Use GuardedRootCache::statistics for that final retained census.
    pub work: ReductionStatistics,
    pub source: GuardedRootSource,
}
