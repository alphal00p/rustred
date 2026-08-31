/// Resource policy for one ordered multi-sample target-evidence run.
///
/// Modular matrices and exact replay retain their own independent policies.
/// These limits bound the probe plan plus scheduler-owned diagnostics,
/// including the selected exact result's diagnostics clone and every retained
/// forbidden-column copy, modular no-hit obstruction sidecars, and canonical
/// trace telemetry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TargetEvidenceLimits {
    pub(crate) max_probes: usize,
    pub(crate) max_discovery_probes: usize,
    pub(crate) max_held_out_probes: usize,
    pub(crate) max_base_parameter_cells: usize,
    pub(crate) max_chart_coordinate_cells: usize,
    pub(crate) max_diagnostic_source_entries: usize,
    pub(crate) max_diagnostic_pivot_entries: usize,
    pub(crate) max_retained_diagnostic_forbidden_column_entries: usize,
    /// Aggregate worst-case logical-column plus sparse-coefficient entries
    /// retained if every admitted probe returns a modular obstruction.
    pub(crate) max_retained_modular_obstruction_entries: usize,
    pub(crate) max_trace_scope_entries: usize,
    pub(crate) max_canonical_source_entries: usize,
    pub(crate) max_canonical_pivot_entries: usize,
    pub(crate) max_trace_groups: usize,
    pub(crate) max_group_members: usize,
}

impl Default for TargetEvidenceLimits {
    fn default() -> Self {
        Self {
            max_probes: 4_096,
            max_discovery_probes: 3_072,
            max_held_out_probes: 1_024,
            max_base_parameter_cells: 1_048_576,
            max_chart_coordinate_cells: 4_194_304,
            max_diagnostic_source_entries: 16_777_216,
            max_diagnostic_pivot_entries: 16_777_216,
            max_retained_diagnostic_forbidden_column_entries: 16_777_216,
            max_retained_modular_obstruction_entries: 33_554_432,
            max_trace_scope_entries: 1_000_000,
            max_canonical_source_entries: 16_777_216,
            max_canonical_pivot_entries: 16_777_216,
            max_trace_groups: 4_096,
            max_group_members: 4_096,
        }
    }
}
