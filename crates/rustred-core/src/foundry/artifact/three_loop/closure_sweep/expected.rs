//! Typed regression baseline for the fixed degree-one K6 experiment.
//!
//! These counts do not turn modular no-hits into negative evidence. They make
//! the exact, deterministic experiment reviewable: losing a nominated and
//! exactly admitted owner, changing its guard-total proof, or changing the
//! measured obstruction geometry must be accepted as an explicit baseline
//! update instead of drifting silently behind printed telemetry.

use crate::foundry::completion::frame::admission::{
    ExactOwnerCoverObstructionKind, ExactOwnerCoverStatus,
};
use crate::sector::Mask;

use super::{SectorSweepTelemetry, SweepCoverTelemetry};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ExpectedSectorSweepTelemetry {
    pub(super) representative: [i64; 6],
    frame_columns: usize,
    partitioned_targets: usize,
    inactive_activation_targets: usize,
    modular_hits: usize,
    modular_no_hits: usize,
    exact_replayed: usize,
    exact_support_did_not_lift: usize,
    admitted_owners: usize,
    cover: ExpectedSweepCoverTelemetry,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExpectedSweepCoverTelemetry {
    NoAdmittedOwners {
        full_orthant_free_dimension: usize,
    },
    Compiled {
        guard_total_owners: usize,
        status: ExactOwnerCoverStatus,
        uncovered_boxes: usize,
        uncovered_free_dimension_histogram: [usize; 7],
        maximum_uncovered_free_dimension: usize,
        maximum_uncovered_varying_dimension: usize,
        missing_terminal_points: usize,
        guard_incomplete_owners: usize,
    },
}

pub(super) const EXPECTED_FULL_RANK_DEGREE_ONE_SWEEP: [ExpectedSectorSweepTelemetry; 6] = [
    ExpectedSectorSweepTelemetry {
        representative: [0, 0, 1, 0, 1, 1],
        frame_columns: 159,
        partitioned_targets: 85,
        inactive_activation_targets: 74,
        modular_hits: 0,
        modular_no_hits: 85,
        exact_replayed: 0,
        exact_support_did_not_lift: 0,
        admitted_owners: 0,
        cover: ExpectedSweepCoverTelemetry::NoAdmittedOwners {
            full_orthant_free_dimension: 6,
        },
    },
    ExpectedSectorSweepTelemetry {
        representative: [0, 0, 1, 1, 0, 1],
        frame_columns: 163,
        partitioned_targets: 91,
        inactive_activation_targets: 72,
        modular_hits: 3,
        modular_no_hits: 88,
        exact_replayed: 3,
        exact_support_did_not_lift: 0,
        admitted_owners: 3,
        cover: ExpectedSweepCoverTelemetry::Compiled {
            guard_total_owners: 1,
            status: ExactOwnerCoverStatus::Incomplete(ExactOwnerCoverObstructionKind::NonFinite),
            uncovered_boxes: 3,
            uncovered_free_dimension_histogram: [0, 0, 0, 0, 0, 3, 0],
            maximum_uncovered_free_dimension: 5,
            maximum_uncovered_varying_dimension: 5,
            missing_terminal_points: 0,
            guard_incomplete_owners: 0,
        },
    },
    ExpectedSectorSweepTelemetry {
        representative: [0, 0, 1, 1, 1, 1],
        frame_columns: 161,
        partitioned_targets: 111,
        inactive_activation_targets: 50,
        modular_hits: 2,
        modular_no_hits: 109,
        exact_replayed: 2,
        exact_support_did_not_lift: 0,
        admitted_owners: 2,
        cover: ExpectedSweepCoverTelemetry::Compiled {
            guard_total_owners: 0,
            status: ExactOwnerCoverStatus::Incomplete(ExactOwnerCoverObstructionKind::NonFinite),
            uncovered_boxes: 1,
            uncovered_free_dimension_histogram: [0, 0, 0, 0, 0, 0, 1],
            maximum_uncovered_free_dimension: 6,
            maximum_uncovered_varying_dimension: 6,
            missing_terminal_points: 0,
            guard_incomplete_owners: 0,
        },
    },
    ExpectedSectorSweepTelemetry {
        representative: [0, 1, 1, 1, 1, 0],
        frame_columns: 157,
        partitioned_targets: 107,
        inactive_activation_targets: 50,
        modular_hits: 2,
        modular_no_hits: 105,
        exact_replayed: 2,
        exact_support_did_not_lift: 0,
        admitted_owners: 2,
        cover: ExpectedSweepCoverTelemetry::Compiled {
            guard_total_owners: 0,
            status: ExactOwnerCoverStatus::Incomplete(ExactOwnerCoverObstructionKind::NonFinite),
            uncovered_boxes: 1,
            uncovered_free_dimension_histogram: [0, 0, 0, 0, 0, 0, 1],
            maximum_uncovered_free_dimension: 6,
            maximum_uncovered_varying_dimension: 6,
            missing_terminal_points: 0,
            guard_incomplete_owners: 0,
        },
    },
    ExpectedSectorSweepTelemetry {
        representative: [0, 1, 1, 1, 1, 1],
        frame_columns: 153,
        partitioned_targets: 128,
        inactive_activation_targets: 25,
        modular_hits: 3,
        modular_no_hits: 125,
        exact_replayed: 3,
        exact_support_did_not_lift: 0,
        admitted_owners: 3,
        cover: ExpectedSweepCoverTelemetry::Compiled {
            guard_total_owners: 1,
            status: ExactOwnerCoverStatus::Incomplete(ExactOwnerCoverObstructionKind::NonFinite),
            uncovered_boxes: 2,
            uncovered_free_dimension_histogram: [0, 0, 0, 0, 0, 2, 0],
            maximum_uncovered_free_dimension: 5,
            maximum_uncovered_varying_dimension: 5,
            missing_terminal_points: 0,
            guard_incomplete_owners: 0,
        },
    },
    ExpectedSectorSweepTelemetry {
        representative: [1, 1, 1, 1, 1, 1],
        frame_columns: 136,
        partitioned_targets: 136,
        inactive_activation_targets: 0,
        modular_hits: 7,
        modular_no_hits: 129,
        exact_replayed: 7,
        exact_support_did_not_lift: 0,
        admitted_owners: 7,
        cover: ExpectedSweepCoverTelemetry::Compiled {
            guard_total_owners: 3,
            status: ExactOwnerCoverStatus::Incomplete(ExactOwnerCoverObstructionKind::NonFinite),
            uncovered_boxes: 1,
            uncovered_free_dimension_histogram: [0, 0, 0, 0, 0, 1, 0],
            maximum_uncovered_free_dimension: 5,
            maximum_uncovered_varying_dimension: 5,
            missing_terminal_points: 0,
            guard_incomplete_owners: 0,
        },
    },
];

pub(super) fn assert_expected_sweep(
    actual: &SectorSweepTelemetry,
    expected: &ExpectedSectorSweepTelemetry,
) {
    assert_eq!(
        actual.sector,
        Mask::try_from_indices(&expected.representative).unwrap()
    );
    assert_eq!(actual.frame_columns, expected.frame_columns);
    assert_eq!(actual.partitioned_targets, expected.partitioned_targets);
    assert_eq!(
        actual.inactive_activation_targets,
        expected.inactive_activation_targets
    );
    assert_eq!(actual.modular_hits, expected.modular_hits);
    assert_eq!(actual.modular_no_hits, expected.modular_no_hits);
    assert_eq!(actual.exact_replayed, expected.exact_replayed);
    assert_eq!(
        actual.exact_support_did_not_lift,
        expected.exact_support_did_not_lift
    );
    assert_eq!(actual.admitted_owners, expected.admitted_owners);
    match (&actual.cover, expected.cover) {
        (
            SweepCoverTelemetry::NoAdmittedOwners {
                full_orthant_free_dimension: actual,
            },
            ExpectedSweepCoverTelemetry::NoAdmittedOwners {
                full_orthant_free_dimension: expected,
            },
        ) => assert_eq!(*actual, expected),
        (
            SweepCoverTelemetry::Compiled {
                guard_total_owners: actual_guard_total,
                status: actual_status,
                uncovered_boxes: actual_boxes,
                uncovered_free_dimension_histogram: actual_histogram,
                maximum_uncovered_free_dimension: actual_max_free,
                maximum_uncovered_varying_dimension: actual_max_varying,
                missing_terminal_points: actual_missing_terminals,
                guard_incomplete_owners: actual_guard_incomplete,
            },
            ExpectedSweepCoverTelemetry::Compiled {
                guard_total_owners: expected_guard_total,
                status: expected_status,
                uncovered_boxes: expected_boxes,
                uncovered_free_dimension_histogram: expected_histogram,
                maximum_uncovered_free_dimension: expected_max_free,
                maximum_uncovered_varying_dimension: expected_max_varying,
                missing_terminal_points: expected_missing_terminals,
                guard_incomplete_owners: expected_guard_incomplete,
            },
        ) => {
            assert_eq!(*actual_guard_total, expected_guard_total);
            assert_eq!(*actual_status, expected_status);
            assert_eq!(*actual_boxes, expected_boxes);
            assert_eq!(actual_histogram.as_ref(), expected_histogram.as_slice());
            assert_eq!(*actual_max_free, expected_max_free);
            assert_eq!(*actual_max_varying, expected_max_varying);
            assert_eq!(*actual_missing_terminals, expected_missing_terminals);
            assert_eq!(*actual_guard_incomplete, expected_guard_incomplete);
        }
        (actual, expected) => panic!(
            "K6 degree-one sweep cover kind changed: actual {actual:?}, expected {expected:?}"
        ),
    }
}
