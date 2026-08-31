/// Resource policy for one decorated-stratum physical-column registry.
///
/// The limits cover only retained Rust metadata and exact target-sector
/// partition work. Symbolica matrix limits remain owned by the modular and
/// exact-lift layers which consume this registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StratumRegistryLimits {
    pub(crate) max_guard_branches: usize,
    pub(crate) max_guard_identity_bytes: usize,
    pub(crate) max_stratum_identity_bytes: usize,
    pub(crate) max_owner_regions: usize,
    pub(crate) max_owner_coordinate_cells: usize,
    pub(crate) max_owner_routes: usize,
    pub(crate) max_owner_route_coordinate_cells: usize,
    pub(crate) max_owner_identity_bytes: usize,
    pub(crate) max_physical_columns: usize,
    pub(crate) max_column_coordinate_cells: usize,
    pub(crate) max_target_sector_cells: usize,
    pub(crate) max_owner_probes: usize,
    pub(crate) max_retained_owner_witnesses: usize,
}

impl Default for StratumRegistryLimits {
    fn default() -> Self {
        Self {
            max_guard_branches: 4_096,
            max_guard_identity_bytes: 1_048_576,
            max_stratum_identity_bytes: 67_108_864,
            max_owner_regions: 1_048_576,
            max_owner_coordinate_cells: 16_777_216,
            max_owner_routes: 4_194_304,
            max_owner_route_coordinate_cells: 134_217_728,
            max_owner_identity_bytes: 67_108_864,
            max_physical_columns: 4_000_000,
            max_column_coordinate_cells: 64_000_000,
            max_target_sector_cells: 16_777_216,
            max_owner_probes: 268_435_456,
            max_retained_owner_witnesses: 16_777_216,
        }
    }
}
