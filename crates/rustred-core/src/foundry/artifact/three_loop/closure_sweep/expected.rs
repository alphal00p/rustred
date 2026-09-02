//! Typed regression baseline for the fixed degree-one K6 experiment.
//!
//! These counts do not turn modular no-hits into negative evidence. They make
//! the exact, deterministic experiment reviewable: losing an exactly replayed
//! semantic owner input, changing its guard-total proof, or changing the
//! measured obstruction geometry must be accepted as an explicit baseline
//! update instead of drifting silently behind printed telemetry.

use crate::foundry::completion::frame::admission::{
    ExactOwnerCoverObstructionKind, ExactOwnerCoverStatus,
};
use crate::sector::Mask;

use super::model::{DegreeSweepTelemetry, SectorSweepTelemetry, SweepCoverTelemetry};

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
    semantic_owner_inputs: usize,
    cover: ExpectedSweepCoverTelemetry,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExpectedSweepCoverTelemetry {
    NoSemanticOwnerInputs {
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
        semantic_owner_inputs: 0,
        cover: ExpectedSweepCoverTelemetry::NoSemanticOwnerInputs {
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
        semantic_owner_inputs: 3,
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
        semantic_owner_inputs: 2,
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
        semantic_owner_inputs: 2,
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
        semantic_owner_inputs: 3,
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
        semantic_owner_inputs: 7,
        cover: ExpectedSweepCoverTelemetry::Compiled {
            // Symbolica-backed separable-locus replay proves every retained
            // full-rank owner guard-total on its exact region. The residual
            // five-dimensional box is therefore geometric, not a hidden
            // guard-incomplete branch.
            guard_total_owners: 7,
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

/// Degree-one semantic replay for the two nonzero rank-three representatives
/// after proper-subsector classification is joined to the exact installed K6
/// terminal authority. These are stronger discovery inputs than the empty-
/// snapshot baseline, but their nonfinite complements remain explicit.
pub(super) const EXPECTED_RANK_THREE_ROOT_AUTHORITY_DEGREE_ONE_SWEEP:
    [ExpectedSectorSweepTelemetry; 2] = [
    ExpectedSectorSweepTelemetry {
        representative: [0, 0, 1, 0, 1, 1],
        frame_columns: 159,
        partitioned_targets: 85,
        inactive_activation_targets: 74,
        modular_hits: 9,
        modular_no_hits: 76,
        exact_replayed: 9,
        exact_support_did_not_lift: 0,
        semantic_owner_inputs: 9,
        cover: ExpectedSweepCoverTelemetry::Compiled {
            guard_total_owners: 4,
            status: ExactOwnerCoverStatus::Incomplete(ExactOwnerCoverObstructionKind::NonFinite),
            uncovered_boxes: 10,
            uncovered_free_dimension_histogram: [0, 0, 0, 1, 8, 1, 0],
            maximum_uncovered_free_dimension: 5,
            maximum_uncovered_varying_dimension: 5,
            missing_terminal_points: 0,
            guard_incomplete_owners: 0,
        },
    },
    ExpectedSectorSweepTelemetry {
        representative: [0, 0, 1, 1, 0, 1],
        frame_columns: 163,
        partitioned_targets: 91,
        inactive_activation_targets: 72,
        modular_hits: 22,
        modular_no_hits: 69,
        exact_replayed: 22,
        exact_support_did_not_lift: 0,
        semantic_owner_inputs: 22,
        cover: ExpectedSweepCoverTelemetry::Compiled {
            guard_total_owners: 12,
            status: ExactOwnerCoverStatus::Incomplete(ExactOwnerCoverObstructionKind::NonFinite),
            uncovered_boxes: 4,
            uncovered_free_dimension_histogram: [0, 0, 0, 0, 1, 3, 0],
            maximum_uncovered_free_dimension: 5,
            maximum_uncovered_varying_dimension: 5,
            missing_terminal_points: 0,
            guard_incomplete_owners: 0,
        },
    },
];

const EXPECTED_CANONICAL_S4A_MIXED_DEGREES: [DegreeSweepTelemetry; 2] = [
    DegreeSweepTelemetry {
        degree: 1,
        frame_offsets: 7,
        frame_rows: 63,
        frame_columns: 157,
        frame_entries: 630,
        partitioned_targets: 107,
        inactive_activation_targets: 50,
        modular_hits: 2,
        modular_no_hits: 105,
        exact_replayed: 2,
        exact_support_did_not_lift: 0,
        exact_content_duplicates: 0,
        semantic_owner_inputs: 2,
    },
    DegreeSweepTelemetry {
        degree: 2,
        frame_offsets: 28,
        frame_rows: 252,
        frame_columns: 488,
        frame_entries: 2_520,
        partitioned_targets: 328,
        inactive_activation_targets: 160,
        modular_hits: 22,
        modular_no_hits: 306,
        exact_replayed: 22,
        exact_support_did_not_lift: 0,
        exact_content_duplicates: 0,
        semantic_owner_inputs: 22,
    },
];

pub(super) fn assert_expected_mixed_s4a_sweep(actual: &SectorSweepTelemetry) {
    assert_eq!(
        actual.sector,
        Mask::try_from_indices(&[0, 1, 1, 1, 1, 0]).unwrap()
    );
    assert_eq!(actual.ordinary_sources, 9);
    assert_eq!(
        actual.degrees.as_ref(),
        EXPECTED_CANONICAL_S4A_MIXED_DEGREES.as_slice()
    );
    assert_eq!(actual.semantic_owner_inputs, 24);
    match &actual.cover {
        SweepCoverTelemetry::Compiled {
            guard_total_owners,
            status,
            uncovered_boxes,
            uncovered_free_dimension_histogram,
            maximum_uncovered_free_dimension,
            maximum_uncovered_varying_dimension,
            missing_terminal_points,
            guard_incomplete_owners,
        } => {
            assert_eq!(*guard_total_owners, 1);
            assert_eq!(
                *status,
                ExactOwnerCoverStatus::Incomplete(ExactOwnerCoverObstructionKind::NonFinite)
            );
            assert_eq!(*uncovered_boxes, 3);
            assert_eq!(
                uncovered_free_dimension_histogram.as_ref(),
                [0, 0, 0, 0, 0, 3, 0]
            );
            assert_eq!(*maximum_uncovered_free_dimension, 5);
            assert_eq!(*maximum_uncovered_varying_dimension, 5);
            assert_eq!(*missing_terminal_points, 0);
            assert_eq!(*guard_incomplete_owners, 0);
        }
        SweepCoverTelemetry::NoSemanticOwnerInputs { .. } => {
            panic!("the pinned mixed-degree S4a sweep lost every semantic owner input")
        }
    }
}

pub(super) fn assert_expected_sweep(
    actual: &SectorSweepTelemetry,
    expected: &ExpectedSectorSweepTelemetry,
) {
    assert_eq!(
        actual.sector,
        Mask::try_from_indices(&expected.representative).unwrap()
    );
    assert_eq!(actual.degrees.len(), 1);
    let degree = &actual.degrees[0];
    assert_eq!(degree.degree, 1);
    assert_eq!(degree.frame_offsets, 7);
    assert_eq!(degree.frame_rows, 63);
    assert_eq!(degree.frame_columns, expected.frame_columns);
    assert_eq!(degree.frame_entries, 630);
    assert_eq!(degree.partitioned_targets, expected.partitioned_targets);
    assert_eq!(
        degree.inactive_activation_targets,
        expected.inactive_activation_targets
    );
    assert_eq!(degree.modular_hits, expected.modular_hits);
    assert_eq!(degree.modular_no_hits, expected.modular_no_hits);
    assert_eq!(degree.exact_replayed, expected.exact_replayed);
    assert_eq!(
        degree.exact_support_did_not_lift,
        expected.exact_support_did_not_lift
    );
    assert_eq!(degree.exact_content_duplicates, 0);
    assert_eq!(degree.semantic_owner_inputs, expected.semantic_owner_inputs);
    assert_eq!(actual.semantic_owner_inputs, expected.semantic_owner_inputs);
    match (&actual.cover, expected.cover) {
        (
            SweepCoverTelemetry::NoSemanticOwnerInputs {
                full_orthant_free_dimension: actual,
            },
            ExpectedSweepCoverTelemetry::NoSemanticOwnerInputs {
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
