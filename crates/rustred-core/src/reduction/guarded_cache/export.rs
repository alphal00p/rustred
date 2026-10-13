use super::{ClosedUnitMassProgram, GuardedCacheBacking, GuardedCacheError, GuardedRootCache};
use crate::algebra::CoefficientContext;
use crate::persistence::application_cache::ApplicationCacheBinding;
use crate::reduction::{CacheWeight, ReductionError};

/// Records accepted by the backing callback, not a claim that its writer has
/// checkpointed them. Durability and interrupted-prefix recovery belong to
/// the existing backing writer, never to this native cache.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GuardedRootExportStatistics {
    pub exported_records: usize,
    pub exported_coefficient_terms: usize,
    pub exported_coefficient_bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuardedRootExportError {
    pub accepted_prefix: GuardedRootExportStatistics,
    pub source: GuardedCacheError,
}
impl std::fmt::Display for GuardedRootExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "guarded root export stopped after {} accepted records: {}",
            self.accepted_prefix.exported_records, self.source
        )
    }
}
impl std::error::Error for GuardedRootExportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

impl GuardedRootExportStatistics {
    fn checked_next(self, weight: CacheWeight) -> Result<Self, ReductionError> {
        let overflow = || ReductionError::CacheResourceCountOverflow {
            resource: "guarded export totals",
        };
        Ok(Self {
            exported_records: self.exported_records.checked_add(1).ok_or_else(overflow)?,
            exported_coefficient_terms: self
                .exported_coefficient_terms
                .checked_add(weight.coefficient_terms)
                .ok_or_else(overflow)?,
            exported_coefficient_bytes: self
                .exported_coefficient_bytes
                .checked_add(weight.coefficient_bytes)
                .ok_or_else(overflow)?,
        })
    }
}

impl ClosedUnitMassProgram<'_> {
    /// Export only completed guarded roots, one shared immutable record at a
    /// time. The callback runs without a root-cache or budget mutex held.
    /// No full-cache clone, serialization, arithmetic, or filesystem I/O is
    /// performed here; the backing applies its own bounded codec/write policy.
    ///
    /// Exact artifact/context binding is checked once. Immutable payloads were
    /// already validated on admission. LRU/LFU usage, aging, work counters and
    /// request state are neither changed nor exported; hydration starts fresh
    /// policy metadata. An error reports only the prefix accepted by callbacks;
    /// the backing alone determines which prefix is durable.
    pub fn export_completed(
        &self,
        cache: &GuardedRootCache,
        backing: &mut dyn GuardedCacheBacking,
    ) -> Result<GuardedRootExportStatistics, GuardedRootExportError> {
        export_bound(
            self.binding(),
            self.artifact_identity(),
            self.context(),
            cache,
            backing,
        )
    }
}

pub(crate) fn export_bound(
    binding: &ApplicationCacheBinding,
    identity: usize,
    context: &CoefficientContext,
    cache: &GuardedRootCache,
    backing: &mut dyn GuardedCacheBacking,
) -> Result<GuardedRootExportStatistics, GuardedRootExportError> {
    let mut statistics = GuardedRootExportStatistics::default();
    if &cache.binding != binding
        || cache.artifact_identity != identity
        || backing.binding() != binding
    {
        return Err(GuardedRootExportError {
            accepted_prefix: statistics,
            source: GuardedCacheError::BindingMismatch,
        });
    }
    let mut cursor = None;
    while let Some((key, record, weight)) = cache.export_next(cursor.as_ref()) {
        let prospective =
            statistics
                .checked_next(weight)
                .map_err(|source| GuardedRootExportError {
                    accepted_prefix: statistics,
                    source: source.into(),
                })?;
        backing
            .store_completed(&record, context)
            .map_err(|source| GuardedRootExportError {
                accepted_prefix: statistics,
                source: source.into(),
            })?;
        statistics = prospective;
        cursor = Some(key);
    }
    Ok(statistics)
}
