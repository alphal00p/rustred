use std::time::Duration;

use rustred::solver::CandidateRoutedCachePolicy;
use serde_json::{Value, json};

use crate::{AppError, SavedApplicationCachePackLimits};

/// Reported by an integrated backend, never inferred from lazy owner loading.
/// It grants no implicit storage quota or mathematical admission.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CachePreparationCapabilities {
    validated_lazy_storage: bool,
}

impl CachePreparationCapabilities {
    pub const fn has_validated_lazy_storage(self) -> bool {
        self.validated_lazy_storage
    }

    // Only an integrated native backend may enable this capability. Current
    // standalone chunk/pack codecs alone do not establish it.
    #[cfg(test)]
    pub(crate) const fn validated_lazy_storage() -> Self {
        Self {
            validated_lazy_storage: true,
        }
    }
}

/// Options for an explicitly requested preparation job, not evaluate defaults.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachePreparationOptions {
    /// Enable caller-selected count diagnostics. Count templates do not bound
    /// ordinary preparation; byte quotas and the job time budget are separate.
    pub diagnostics_enabled: bool,
    /// LRU remains the default; an alternative policy is always explicit.
    pub cache_policy: CandidateRoutedCachePolicy,
    /// Retained exact coefficient payload, separate from IO buffers and RSS.
    /// None leaves this cache capacity unbounded.
    pub ram_bytes: Option<u64>,
    /// Aggregate disk quota, including all namespaces and interrupted chunks.
    /// None leaves storage unbounded.
    pub disk_bytes: Option<u64>,
    /// Cooperative wall budget; one native operation may overrun the deadline.
    pub time_limit_seconds: u64,
    pub checkpoint_interval_seconds: u64,
    /// Diagnostic maximum selected warmup roots, not intermediate record count.
    /// Applied only when `diagnostics_enabled` is true.
    pub max_roots: usize,
    /// Maximum staged completed records across namespaces before a flush.
    /// This layout target does not reduce existing-pack decoding allowances.
    pub batch_records: usize,
    pub resume: bool,
    pub(crate) storage_limits: SavedApplicationCachePackLimits,
}

impl Default for CachePreparationOptions {
    fn default() -> Self {
        Self {
            diagnostics_enabled: false,
            cache_policy: CandidateRoutedCachePolicy::EvictLeastRecentlyUsed,
            ram_bytes: None,
            disk_bytes: None,
            time_limit_seconds: 36_000,
            checkpoint_interval_seconds: 60,
            max_roots: 1_000_000,
            batch_records: 64,
            resume: true,
            storage_limits: Default::default(),
        }
    }
}

/// Resolved byte limits are portable u64 values. Only allocation-facing
/// allowances are converted to usize, with an explicit platform-fit error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CachePreparationResolvedOptions {
    pub diagnostics_enabled: bool,
    pub loop_count: usize,
    pub ram_bytes: u64,
    pub retained_coefficient_bytes: usize,
    pub disk_bytes: u64,
    pub cache_policy: CandidateRoutedCachePolicy,
    pub time_limit: Duration,
    pub checkpoint_interval: Duration,
    pub max_roots: usize,
    pub batch_records: usize,
    pub resume: bool,
    pub storage_limits: SavedApplicationCachePackLimits,
    pub capabilities: CachePreparationCapabilities,
}

impl CachePreparationOptions {
    /// Enable this preparation policy's root-count diagnostics. Storage/codec
    /// diagnostics remain independently selected by their own policy owners.
    pub fn with_diagnostics(mut self) -> Self {
        self.diagnostics_enabled = true;
        self
    }

    /// One owner for preparation/snapshot disk defaults. This does not reserve
    /// RAM or grant backend availability or mathematical admission.
    pub(super) fn resolve_disk_bytes(
        loop_count: usize,
        requested: Option<u64>,
        _capabilities: CachePreparationCapabilities,
    ) -> Result<u64, AppError> {
        if loop_count == 0 {
            return Err(AppError::input("cache storage loop count must be positive"));
        }
        let bytes = requested.unwrap_or(u64::MAX);
        if bytes == 0 {
            return Err(AppError::input(
                "cache storage disk budget must be positive",
            ));
        }
        Ok(bytes)
    }
    /// Override native pack IO policy as a unit. Its explicit disk allowance
    /// becomes the requested quota, avoiding two competing disk-limit fields.
    pub fn with_storage_limits(mut self, limits: SavedApplicationCachePackLimits) -> Self {
        self.disk_bytes = Some(limits.max_disk_bytes);
        self.storage_limits = limits;
        self
    }

    /// Resolve operational policy without imposing a storage quota when none
    /// was requested. Explicit zero byte allowances and enabled zero root
    /// diagnostics are rejected before opening files.
    pub fn resolve(
        &self,
        loop_count: usize,
        capabilities: CachePreparationCapabilities,
    ) -> Result<CachePreparationResolvedOptions, AppError> {
        if loop_count == 0 {
            return Err(AppError::input(
                "cache preparation loop count must be positive",
            ));
        }
        if self.checkpoint_interval_seconds == 0 {
            return Err(AppError::input(
                "cache checkpoint interval must be positive",
            ));
        }
        let disk_bytes = Self::resolve_disk_bytes(loop_count, self.disk_bytes, capabilities)?;
        let ram_bytes = self.ram_bytes.unwrap_or(usize::MAX as u64);
        if disk_bytes == 0 || ram_bytes == 0 {
            return Err(AppError::input(
                "cache preparation RAM and disk budgets must be positive",
            ));
        }
        if self.diagnostics_enabled && self.max_roots == 0 {
            return Err(AppError::input(
                "cache preparation root allowance must be positive",
            ));
        }
        let retained_coefficient_bytes = usize::try_from(ram_bytes)
            .map_err(|_| AppError::limit("preparation RAM allowance does not fit this platform"))?;
        let mut storage_limits = self.storage_limits;
        storage_limits.max_disk_bytes = disk_bytes;
        let resolved = CachePreparationResolvedOptions {
            diagnostics_enabled: self.diagnostics_enabled,
            loop_count,
            ram_bytes,
            retained_coefficient_bytes,
            disk_bytes,
            cache_policy: self.cache_policy,
            time_limit: Duration::from_secs(self.time_limit_seconds),
            checkpoint_interval: Duration::from_secs(self.checkpoint_interval_seconds),
            max_roots: self.max_roots,
            batch_records: self.batch_records,
            resume: self.resume,
            storage_limits,
            capabilities,
        };
        resolved.validate()?;
        Ok(resolved)
    }
}

impl CachePreparationResolvedOptions {
    /// Recheck mutable resolved fields at the execution boundary, before any
    /// backend configuration or I/O. Reported and enforced byte limits must
    /// describe the same allowance even after an explicit caller edit.
    pub fn validate(&self) -> Result<(), AppError> {
        if self.loop_count == 0
            || self.ram_bytes == 0
            || self.disk_bytes == 0
            || (self.diagnostics_enabled && self.max_roots == 0)
            || self.batch_records == 0
            || self.checkpoint_interval.is_zero()
            || (self.storage_limits.chunk.diagnostics_enabled
                && self.storage_limits.chunk.max_records == 0)
        {
            return Err(AppError::input(
                "cache preparation requires positive loop, RAM, disk, root, checkpoint and batch allowances",
            ));
        }
        if self.storage_limits.chunk.diagnostics_enabled
            && self.batch_records > self.storage_limits.chunk.max_records
        {
            return Err(AppError::input(
                "cache preparation batch target exceeds the chunk record safety limit",
            ));
        }
        let ram = usize::try_from(self.ram_bytes)
            .map_err(|_| AppError::limit("preparation RAM allowance does not fit this platform"))?;
        if ram != self.retained_coefficient_bytes
            || self.disk_bytes != self.storage_limits.max_disk_bytes
        {
            return Err(AppError::input(
                "resolved cache preparation byte allowances are inconsistent",
            ));
        }
        Ok(())
    }

    /// Operational profile only: callers report program/reduction allowances
    /// separately rather than silently increasing unrelated algebra limits.
    pub fn to_document(&self) -> Value {
        let policy = match self.cache_policy {
            CandidateRoutedCachePolicy::FailOnLimit => "fail-on-limit",
            CandidateRoutedCachePolicy::EvictLeastRecentlyUsed => "evict-lru",
            CandidateRoutedCachePolicy::EvictAgedLeastFrequentlyUsed => "evict-aged-lfu",
        };
        json!({
            "diagnostics_enabled": self.diagnostics_enabled,
            "loop_count": self.loop_count,
            "ram_coefficient_bytes": self.ram_bytes,
            "disk_bytes": self.disk_bytes,
            "cache_policy": policy,
            "time_limit_seconds": self.time_limit.as_secs(),
            "checkpoint_interval_seconds": self.checkpoint_interval.as_secs(),
            "max_roots": self.max_roots,
            "batch_records": self.batch_records,
            "intermediate_capture": true,
            "capture_metadata_limit_bytes_per_request": self.storage_limits.max_directory_owned_bytes,
            "resume": self.resume,
            "validated_lazy_storage": self.capabilities.has_validated_lazy_storage(),
            "storage_limits": format!("{:?}", self.storage_limits),
            "ram_is_process_rss": false,
            "deadline_is_cooperative": true,
            "time_budget_scope": "entire-preparation-job",
        })
    }
}
