use super::RequestedDomainSupportError;

pub(super) const RAW_DOMAINS: &str = "raw requested-domain support proposals";
pub(super) const UNIQUE_DOMAINS: &str = "unique requested-domain support proposals";
pub(super) const RAW_PROVENANCE: &str = "raw requested-domain provenance records";
pub(super) const UNIQUE_PROVENANCE: &str = "unique requested-domain provenance records";
pub(super) const RAW_SUPPORT: &str = "raw requested-domain parent-support entries";
pub(super) const UNIQUE_SUPPORT: &str = "unique requested-domain parent-support entries";
pub(super) const RAW_SUPPORT_CELLS: &str = "raw requested-domain parent-support coordinate cells";
pub(super) const UNIQUE_SUPPORT_CELLS: &str =
    "unique requested-domain parent-support coordinate cells";
pub(super) const CANONICALIZATION_WORK: &str = "requested-domain support canonicalization work";
pub(super) const RETAINED_BYTES: &str = "requested-domain support retained bytes";

/// Resource envelope for one atomic proposal and one deterministic union.
///
/// `max_retained_bytes` is a deterministic logical charge: it includes fixed
/// retained records, fixed `Vec`/`String` values owned behind `Arc`, sector,
/// coordinate, and string payloads, and charges shared coordinate buffers once
/// per retained semantic occurrence. Allocator metadata and platform-specific
/// `Arc` control blocks are intentionally not an externally observable census.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RequestedDomainSupportLimits {
    pub(crate) max_arity: usize,
    pub(crate) max_raw_domains: usize,
    pub(crate) max_unique_domains: usize,
    pub(crate) max_raw_provenance_records: usize,
    pub(crate) max_unique_provenance_records: usize,
    pub(crate) max_raw_support_entries: usize,
    pub(crate) max_unique_support_entries: usize,
    pub(crate) max_raw_support_coordinate_cells: usize,
    pub(crate) max_unique_support_coordinate_cells: usize,
    pub(crate) max_canonicalization_work: usize,
    pub(crate) max_retained_bytes: usize,
}

impl Default for RequestedDomainSupportLimits {
    fn default() -> Self {
        Self {
            max_arity: 4_096,
            max_raw_domains: 1_000_000,
            max_unique_domains: 1_000_000,
            max_raw_provenance_records: 4_000_000,
            max_unique_provenance_records: 4_000_000,
            max_raw_support_entries: 16_000_000,
            max_unique_support_entries: 16_000_000,
            max_raw_support_coordinate_cells: 64_000_000,
            max_unique_support_coordinate_cells: 64_000_000,
            max_canonicalization_work: 1_000_000_000,
            max_retained_bytes: 1_073_741_824,
        }
    }
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), RequestedDomainSupportError> {
    if requested > limit {
        Err(RequestedDomainSupportError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

pub(super) fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, RequestedDomainSupportError> {
    left.checked_add(right)
        .ok_or(RequestedDomainSupportError::ResourceCountOverflow { resource })
}

pub(super) fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, RequestedDomainSupportError> {
    left.checked_mul(right)
        .ok_or(RequestedDomainSupportError::ResourceCountOverflow { resource })
}

pub(super) fn checked_sum(
    resource: &'static str,
    values: impl IntoIterator<Item = usize>,
) -> Result<usize, RequestedDomainSupportError> {
    values
        .into_iter()
        .try_fold(0usize, |sum, value| checked_add(resource, sum, value))
}

pub(super) fn logical_sort_work(count: usize) -> Result<usize, RequestedDomainSupportError> {
    let normalized = count.max(2);
    // `normalized >= 2`, so this exact subtraction cannot underflow.
    let levels = usize::BITS as usize - (normalized - 1).leading_zeros() as usize;
    checked_mul(CANONICALIZATION_WORK, count, levels)
}

pub(super) fn try_vec<T>(
    resource: &'static str,
    requested: usize,
) -> Result<Vec<T>, RequestedDomainSupportError> {
    let mut output = Vec::new();
    output.try_reserve_exact(requested).map_err(|_| {
        RequestedDomainSupportError::AllocationFailure {
            resource,
            requested,
        }
    })?;
    Ok(output)
}

pub(super) fn try_copy_slice<T: Clone>(
    resource: &'static str,
    values: &[T],
) -> Result<Vec<T>, RequestedDomainSupportError> {
    let mut output = try_vec(resource, values.len())?;
    output.extend_from_slice(values);
    Ok(output)
}

pub(super) fn try_copy_string(
    resource: &'static str,
    value: &str,
) -> Result<String, RequestedDomainSupportError> {
    let mut output = String::new();
    output.try_reserve_exact(value.len()).map_err(|_| {
        RequestedDomainSupportError::AllocationFailure {
            resource,
            requested: value.len(),
        }
    })?;
    output.push_str(value);
    Ok(output)
}
