//! Optional capture of completed native descendants. No alternate evaluator,
//! algebra or persisted cache authority lives here.
mod provenance;
pub(crate) use provenance::{GuardNodeId, GuardProvenance};

use super::GuardedCacheBackingError;
use crate::algebra::CoefficientContext;
use crate::persistence::application_cache::{ApplicationCacheKey, ApplicationCacheRecord};

/// Caller-owned bounded stream of useful completed intermediate results.
/// `wants` is checked after native entry admission but before coefficient or
/// guard-vector materialization. The requested root is returned separately.
/// Records are in the existing guarded-root namespace and use its unchanged
/// artifact binding and terminal convention. A valid prefix can outlive an
/// interrupted or failed parent request; unfinished nodes are never emitted.
pub trait GuardedIntermediateCapture {
    fn wants(
        &mut self,
        key: &ApplicationCacheKey,
        context: &CoefficientContext,
    ) -> Result<bool, GuardedCacheBackingError>;
    fn store_completed(
        &mut self,
        record: &ApplicationCacheRecord,
        context: &CoefficientContext,
    ) -> Result<(), GuardedCacheBackingError>;
    /// Require Route/Apply capture to return the original typed error when an
    /// eligible completed coefficient cannot be materialized within its native
    /// allowance. The default keeps optional capture's historical skip behavior;
    /// durable running-cache sinks opt in when every eligible record is required.
    fn fail_on_materialization_limit(&self) -> bool {
        false
    }
    /// Recapture Route/Apply records loaded from persistent backing. The
    /// default preserves copy/preparation sinks; a cumulative running-cache
    /// sink opts out because these records are already durable in its backing.
    fn capture_backing_hits(&self) -> bool {
        true
    }
    /// Extra provenance metadata allowance, not a mathematical rank bound.
    /// Exhaustion disables further capture for this request and leaves native
    /// reduction and its complete root guards intact. Shared native memory
    /// limits continue to apply independently to the mandatory exact guard
    /// payload and coefficients. This is a separate metadata allowance: `None`
    /// defaults to the request's configured native coefficient-byte allowance.
    fn metadata_limit_bytes(&self) -> Option<usize> {
        None
    }
    fn report_statistics(&mut self, _statistics: GuardedCaptureStatistics) {}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GuardedCaptureStatistics {
    pub nodes: usize,
    pub edges: usize,
    pub unique_guards: usize,
    pub metadata_bytes: usize,
    pub peak_metadata_bytes: usize,
    pub captured_records: usize,
    pub skipped_records: usize,
    pub metadata_exhausted: bool,
}
