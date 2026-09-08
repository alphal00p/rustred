use super::SpiredModularError;

pub(super) const ROWS: &str = "streamed rows";
pub(super) const REQUEST_SHIFT_COMPONENTS: &str = "request shift components";
pub(super) const FORBIDDEN_COLUMNS: &str = "forbidden columns";
pub(super) const ROW_STRUCTURAL_TERMS: &str = "row structural forbidden terms";
pub(super) const STRUCTURAL_TERMS: &str = "structural forbidden terms";
pub(super) const RETAINED_NONZEROS: &str = "retained modular nonzeros";
pub(super) const REDUCER_SCRATCH_CELLS: &str = "reducer scratch cells";
pub(super) const REDUCER_DENSE_SCAN_WORK: &str = "reducer dense-scan work";
pub(super) const REDUCER_ENTRIES: &str = "reducer U plus L-pattern entries";
pub(super) const TRACE_NODES: &str = "dependency trace nodes";
pub(super) const TRACE_EDGES: &str = "dependency trace edges";
pub(super) const TRACE_SHIFT_COMPONENTS: &str = "retained dependency-trace shift components";
pub(super) const SUPPORT_REQUESTS: &str = "canonical support requests";
pub(super) const NEW_COLUMN_POSITIONS: &str = "new forbidden-column positions";
pub(super) const ROW_VALUES: &str = "streamed modular row values";
pub(super) const ROW_COLUMNS: &str = "streamed modular row columns";
pub(super) const TRACE_WORKSPACE: &str = "dependency-trace workspace";

/// Hard retained-work policy for one private streaming modular lane.
///
/// Symbolica does not expose a cancellation hook inside one sparse forward
/// solve. Width, cumulative scan work, input, and conservative one-row fill
/// growth are therefore checked before native work. Actual native fill is
/// checked immediately afterwards; every post-mutation failure poisons the
/// lane and drops both reducers so partial or over-limit state cannot remain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SpiredModularLimits {
    pub(crate) max_rows: usize,
    pub(crate) max_request_shift_components: usize,
    pub(crate) max_forbidden_columns: usize,
    pub(crate) max_structural_terms_per_row: usize,
    pub(crate) max_structural_terms: usize,
    pub(crate) max_retained_nonzeros: usize,
    /// Simultaneously retained finite-field scratch coordinates across both
    /// reducers. This is a width bound, not a row-work budget.
    pub(crate) max_reducer_scratch_cells: usize,
    /// Conservative cumulative row-reduction work, charging every admitted
    /// row once at the two reducers' width used for that row.
    pub(crate) max_reducer_dense_scan_work: usize,
    pub(crate) max_reducer_entries: usize,
    pub(crate) max_trace_nodes: usize,
    pub(crate) max_trace_edges: usize,
    pub(crate) max_retained_request_shift_components: usize,
    pub(crate) max_support_requests: usize,
}

impl Default for SpiredModularLimits {
    fn default() -> Self {
        Self {
            max_rows: 1_048_576,
            max_request_shift_components: 4_096,
            max_forbidden_columns: 1_048_576,
            max_structural_terms_per_row: 1_048_576,
            max_structural_terms: 67_108_864,
            max_retained_nonzeros: 67_108_864,
            max_reducer_scratch_cells: 2_097_153,
            max_reducer_dense_scan_work: 67_108_864,
            max_reducer_entries: 134_217_728,
            max_trace_nodes: 1_048_576,
            max_trace_edges: 67_108_864,
            max_retained_request_shift_components: 67_108_864,
            max_support_requests: 1_048_576,
        }
    }
}

pub(super) fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SpiredModularError> {
    left.checked_add(right)
        .ok_or(SpiredModularError::ResourceCountOverflow { resource })
}

pub(super) fn maximum_native_row_growth(
    resource: &'static str,
    width: usize,
    rank: usize,
    input_nonzeros: usize,
) -> Result<usize, SpiredModularError> {
    if rank > width {
        return Err(SpiredModularError::Invariant {
            detail: "native reducer rank exceeds its width",
        });
    }
    // Symbolica returns before touching U or L for an empty input or a
    // full-rank reducer. Otherwise one U row can contain at most `width`
    // values, and one L-pattern row can name every prior basis row plus its
    // diagonal if the input is independent.
    if input_nonzeros == 0 || rank == width {
        Ok(0)
    } else {
        checked_add(resource, width, checked_add(resource, rank, 1)?)
    }
}

pub(super) fn checked_u32(resource: &'static str, value: usize) -> Result<u32, SpiredModularError> {
    u32::try_from(value).map_err(|_| SpiredModularError::U32NotRepresentable { resource, value })
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), SpiredModularError> {
    if requested > limit {
        Err(SpiredModularError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

pub(super) fn try_vec<T>(
    resource: &'static str,
    capacity: usize,
) -> Result<Vec<T>, SpiredModularError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| SpiredModularError::AllocationFailure {
            resource,
            requested: capacity,
        })?;
    Ok(values)
}

pub(super) fn try_reserve<T>(
    values: &mut Vec<T>,
    additional: usize,
    resource: &'static str,
) -> Result<(), SpiredModularError> {
    let requested = checked_add(resource, values.len(), additional)?;
    values
        .try_reserve_exact(additional)
        .map_err(|_| SpiredModularError::AllocationFailure {
            resource,
            requested,
        })
}
