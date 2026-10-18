//! Strict transport policy shared by the public loop and the manual driver.
use super::super::{backend, storage, trace};
use crate::AppError;
use rustred::solver::CandidateRoutedDeltaExportLimits;
use serde::{Deserialize, Serialize};

macro_rules! limits_dto {
    ($name:ident => $native:ty { $($(#[$attr:meta])* $field:ident : $ty:ty),* $(,)? }) => {
        #[derive(Clone, Copy, Debug, Deserialize, Serialize)]
        #[serde(deny_unknown_fields)]
        pub(crate) struct $name { $( $(#[$attr])* pub(crate) $field: $ty, )* }
        impl From<$name> for $native {
            fn from(v: $name) -> Self { Self { $( $field: v.$field.into(), )* } }
        }
    };
}
// Aggregate stops accept an explicit null without changing the native integer
// types. Native checked arithmetic continues to reject counter overflows.
macro_rules! nullable_limit {
    ($name:ident, $ty:ty) => {
        pub(crate) mod $name {
            use serde::{Deserialize, Deserializer, Serialize, Serializer};
            pub fn deserialize<'de, D: Deserializer<'de>>(reader: D) -> Result<$ty, D::Error> {
                Ok(Option::<$ty>::deserialize(reader)?.unwrap_or(<$ty>::MAX))
            }
            pub fn serialize<S: Serializer>(value: &$ty, writer: S) -> Result<S::Ok, S::Error> {
                if *value == <$ty>::MAX {
                    Option::<$ty>::None.serialize(writer)
                } else {
                    value.serialize(writer)
                }
            }
        }
    };
}
nullable_limit!(nullable_usize, usize);
nullable_limit!(nullable_u64, u64);
limits_dto!(RawLimits => storage::RawCensusLimits {
    max_page_records: usize, max_page_bytes: usize,
    #[serde(with = "nullable_usize")] max_runs: usize,
    #[serde(with = "nullable_usize")] max_pages: usize,
    #[serde(with = "nullable_usize")] max_directory_bytes: usize,
    #[serde(with = "nullable_u64")] max_stored_references: u64,
    #[serde(with = "nullable_usize")] max_merge_runs: usize,
    max_merge_cached_pages: usize,
    #[serde(with = "nullable_usize")] max_merge_payload_bytes: usize,
    #[serde(with = "nullable_u64")] max_merge_read_bytes: u64,
    max_decode_transient_bytes: usize,
});
limits_dto!(StoreLimits => storage::StorageLimits {
    raw: RawLimits,
    #[serde(with = "nullable_usize")] max_arity: usize,
    #[serde(with = "nullable_usize")] max_delta_records: usize,
    #[serde(with = "nullable_usize")] max_delta_payload_bytes: usize,
    max_page_records: usize, max_page_bytes: usize,
    max_cached_pages: usize, max_cached_page_bytes: usize,
    #[serde(with = "nullable_usize")] max_directory_bytes: usize,
    max_manifest_bytes: usize, max_state_bytes: usize,
    #[serde(with = "nullable_usize")] max_scope_rows: usize,
    #[serde(with = "nullable_usize")] max_census_records: usize,
    #[serde(with = "nullable_usize")] max_runs: usize,
    #[serde(with = "nullable_usize")] max_pages: usize,
    #[serde(with = "nullable_usize")] max_physical_files: usize,
    #[serde(with = "nullable_u64")] max_disk_bytes: u64,
    #[serde(with = "nullable_usize")] max_lookup_pages: usize,
    #[serde(with = "nullable_u64")] max_lookup_bytes: u64,
    max_transient_bytes: usize,
});
limits_dto!(ExportLimits => CandidateRoutedDeltaExportLimits {
    #[serde(with = "nullable_usize")] max_records: usize,
    #[serde(with = "nullable_usize")] max_live_payload_bytes: usize,
});

/// Bounds for adjacent immutable source/control documents, separate from the
/// native Work/raw store's disk grant. Counts include atomic staging pathnames.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ControlLimits {
    #[serde(with = "nullable_u64")]
    pub(crate) max_bytes: u64,
    #[serde(with = "nullable_usize")]
    pub(crate) max_files: usize,
}

/// Explicit bounded policy for a fresh finite-demand namespace. The window
/// partition is immutable on resume. Other grants are operational and can be
/// changed explicitly; none clips source domains or local algebra semantics.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FiniteDemandPolicy {
    pub(crate) workers: usize,
    /// Group this many existing prefix windows into a single parallel traversal
    /// and atomic commit. Operational only: may change on cold resume.
    #[serde(default = "one_window")]
    pub(crate) max_windows_per_batch: usize,
    /// Reuse one bounded native pool for this process; operational on resume.
    #[serde(default = "persistent_workers")]
    pub(crate) reuse_worker_pool: bool,
    #[serde(default)]
    pub(crate) parallel_window_preparation: bool,
    pub(crate) windows: trace::SeedLimits,
    #[serde(default = "uncapped_work")]
    pub(crate) per_batch_work: trace::WorkLimits,
    /// Omitted means no aggregate stop. Explicit null preserves source grants;
    /// neither setting changes native per-formula algebra/expansion limits.
    #[serde(default = "uncapped_reduction")]
    pub(crate) prepared_reduction_aggregates: Option<trace::ReductionAggregateOverrides>,
    pub(crate) census: trace::CensusLimits,
    pub(crate) export: ExportLimits,
    pub(crate) storage: StoreLimits,
    #[serde(with = "nullable_usize")]
    pub(crate) max_total_rows: usize,
    #[serde(with = "nullable_usize")]
    pub(crate) max_source_bytes: usize,
    /// Optional exact full-Work acceleration; zero preserves disk-backed lookup.
    #[serde(default)]
    pub(crate) max_resident_work_bytes: usize,
    pub(crate) control: ControlLimits,
}
pub(crate) fn one_window() -> usize {
    1
}
pub(crate) fn persistent_workers() -> bool {
    true
}
fn uncapped_work() -> trace::WorkLimits {
    trace::WorkLimits {
        max_nodes: usize::MAX,
        max_rule_applications: usize::MAX,
        max_transport_calls: usize::MAX,
        max_transport_operations: usize::MAX,
        max_transport_endpoints: usize::MAX,
        max_coalescing_additions: usize::MAX,
    }
}
fn uncapped_reduction() -> Option<trace::ReductionAggregateOverrides> {
    Some(trace::ReductionAggregateOverrides {
        max_rule_applications: usize::MAX,
        max_pending_frames: usize::MAX,
        max_coalescing_additions: usize::MAX,
    })
}
impl FiniteDemandPolicy {
    pub fn from_json(bytes: &[u8]) -> Result<Self, AppError> {
        let value: Self = serde_json::from_slice(bytes)
            .map_err(|e| AppError::input(format!("invalid finite policy: {e}")))?;
        if value.workers == 0
            || value.max_windows_per_batch == 0
            || value.control.max_bytes == 0
            || value.control.max_files == 0
        {
            return Err(AppError::input(
                "finite workers, windows per batch and control grants must be positive",
            ));
        }
        if value.parallel_window_preparation && !value.reuse_worker_pool {
            return Err(AppError::input(
                "parallel window preparation requires the persistent worker pool",
            ));
        }
        Ok(value)
    }
    pub fn to_json(&self) -> Result<Vec<u8>, AppError> {
        serde_json::to_vec_pretty(self).map_err(|e| AppError::serialization(e.to_string()))
    }
    pub(crate) fn native(&self) -> backend::Policy {
        backend::Policy {
            workers: self.workers,
            max_windows_per_batch: self.max_windows_per_batch,
            reuse_worker_pool: self.reuse_worker_pool,
            parallel_window_preparation: self.parallel_window_preparation,
            windows: self.windows.into(),
            per_batch_work: self.per_batch_work.into(),
            prepared_reduction_aggregates: self.prepared_reduction_aggregates,
            census: self.census.into(),
            export: self.export.into(),
            storage: self.storage.into(),
            max_total_rows: self.max_total_rows,
            max_source_bytes: self.max_source_bytes,
            max_resident_work_bytes: self.max_resident_work_bytes,
        }
    }
}
