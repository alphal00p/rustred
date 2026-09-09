/// Resource policy for one exact coordinate-case worklist.
///
/// Pending-payload limits describe live retained work. Enqueue attempts and
/// subsumption checks are cumulative, so repeatedly proposing redundant
/// cases cannot evade the campaign envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SpiredCoordinateCaseWorklistLimits {
    pub(crate) max_pending_cases: usize,
    /// Two inclusive endpoints are charged for every coordinate.
    pub(crate) max_pending_coordinate_cells: usize,
    pub(crate) max_pending_identity_bytes: usize,
    pub(crate) max_enqueue_attempts: usize,
    pub(crate) max_subsumption_checks: usize,
    /// Maximum cumulative cases retired by atomic driver transitions.
    pub(crate) max_retired_cases: usize,
}

impl Default for SpiredCoordinateCaseWorklistLimits {
    fn default() -> Self {
        Self {
            max_pending_cases: 1_048_576,
            max_pending_coordinate_cells: 67_108_864,
            max_pending_identity_bytes: 1_073_741_824,
            max_enqueue_attempts: 16_777_216,
            max_subsumption_checks: 1_073_741_824,
            max_retired_cases: 16_777_216,
        }
    }
}
