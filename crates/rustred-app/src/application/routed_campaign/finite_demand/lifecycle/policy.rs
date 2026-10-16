//! Strict transport policy shared by the public loop and the manual driver.
use super::super::{backend, storage, trace};
use crate::AppError;
use rustred::solver::CandidateRoutedDeltaExportLimits;
use serde::{Deserialize, Serialize};

macro_rules! limits_dto {
    ($name:ident => $native:ty { $($field:ident : $ty:ty),* $(,)? }) => {
        #[derive(Clone, Copy, Debug, Deserialize, Serialize)]
        #[serde(deny_unknown_fields)]
        pub(crate) struct $name { $( pub(crate) $field: $ty, )* }
        impl From<$name> for $native {
            fn from(v: $name) -> Self { Self { $( $field: v.$field.into(), )* } }
        }
    };
}
limits_dto!(RawLimits => storage::RawCensusLimits {
    max_page_records: usize, max_page_bytes: usize, max_runs: usize,
    max_pages: usize, max_directory_bytes: usize, max_stored_references: u64,
    max_merge_runs: usize, max_merge_cached_pages: usize,
    max_merge_payload_bytes: usize, max_merge_read_bytes: u64,
    max_decode_transient_bytes: usize,
});
limits_dto!(StoreLimits => storage::StorageLimits {
    raw: RawLimits, max_arity: usize, max_delta_records: usize,
    max_delta_payload_bytes: usize, max_page_records: usize, max_page_bytes: usize,
    max_cached_pages: usize, max_cached_page_bytes: usize,
    max_directory_bytes: usize, max_manifest_bytes: usize, max_state_bytes: usize,
    max_scope_rows: usize, max_census_records: usize, max_runs: usize,
    max_pages: usize, max_physical_files: usize, max_disk_bytes: u64,
    max_lookup_pages: usize, max_lookup_bytes: u64, max_transient_bytes: usize,
});
limits_dto!(ExportLimits => CandidateRoutedDeltaExportLimits {
    max_records: usize, max_live_payload_bytes: usize,
});

/// Bounds for adjacent immutable source/control documents, separate from the
/// native Work/raw store's disk grant. Counts include atomic staging pathnames.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ControlLimits {
    pub(crate) max_bytes: u64,
    pub(crate) max_files: usize,
}

/// Explicit bounded policy for a fresh finite-demand namespace. The window
/// partition is immutable on resume. Other grants are operational and can be
/// changed explicitly; none clips source domains or local algebra semantics.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FiniteDemandPolicy {
    pub(crate) workers: usize,
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
    pub(crate) max_total_rows: usize,
    pub(crate) max_source_bytes: usize,
    pub(crate) control: ControlLimits,
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
        if bytes.len() > 1024 * 1024 {
            return Err(AppError::limit("finite policy document exceeds 1 MiB"));
        }
        let value: Self = serde_json::from_slice(bytes)
            .map_err(|e| AppError::input(format!("invalid finite policy: {e}")))?;
        if value.workers == 0 || value.control.max_bytes == 0 || value.control.max_files == 0 {
            return Err(AppError::input(
                "finite workers and control grants must be positive",
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
            windows: self.windows.into(),
            per_batch_work: self.per_batch_work.into(),
            prepared_reduction_aggregates: self.prepared_reduction_aggregates,
            census: self.census.into(),
            export: self.export.into(),
            storage: self.storage.into(),
            max_total_rows: self.max_total_rows,
            max_source_bytes: self.max_source_bytes,
        }
    }
}
