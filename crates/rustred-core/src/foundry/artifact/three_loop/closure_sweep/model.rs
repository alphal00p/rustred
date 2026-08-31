//! Deterministic telemetry for bounded semantic-owner-input sweeps.

use crate::foundry::completion::frame::admission::ExactOwnerCoverStatus;
use crate::sector::Mask;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DegreeSweepTelemetry {
    pub(super) degree: usize,
    pub(super) frame_offsets: usize,
    pub(super) frame_rows: usize,
    pub(super) frame_columns: usize,
    pub(super) frame_entries: usize,
    pub(super) partitioned_targets: usize,
    pub(super) inactive_activation_targets: usize,
    pub(super) modular_hits: usize,
    pub(super) modular_no_hits: usize,
    pub(super) exact_replayed: usize,
    pub(super) exact_support_did_not_lift: usize,
    /// Exact-content duplicates removed only among candidates for this
    /// degree-local target. Modular support and sample identity never enter
    /// this count.
    pub(super) exact_content_duplicates: usize,
    /// Exactly replayed circuits compiled into semantic owner-cover inputs.
    /// This does not imply guarded executable `RuleCell` promotion.
    pub(super) semantic_owner_inputs: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SectorSweepTelemetry {
    pub(super) sector: Mask,
    pub(super) ordinary_sources: usize,
    pub(super) degrees: Box<[DegreeSweepTelemetry]>,
    pub(super) semantic_owner_inputs: usize,
    pub(super) cover: SweepCoverTelemetry,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum SweepCoverTelemetry {
    /// The bounded modular discovery budget produced no replayed semantic
    /// owner input. The cover compiler returned its typed empty-input error.
    NoSemanticOwnerInputs {
        /// With no owner orthant, the entire sector chart remains uncovered.
        full_orthant_free_dimension: usize,
    },
    Compiled {
        guard_total_owners: usize,
        status: ExactOwnerCoverStatus,
        uncovered_boxes: usize,
        uncovered_free_dimension_histogram: Box<[usize]>,
        maximum_uncovered_free_dimension: usize,
        maximum_uncovered_varying_dimension: usize,
        missing_terminal_points: usize,
        guard_incomplete_owners: usize,
    },
}
