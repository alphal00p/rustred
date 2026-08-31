//! Bounded semantic-owner-input sweeps for canonical three-loop sectors.
//!
//! These tests are discovery telemetry, not production artifact authority.
//! Every retained semantic input survives exact lift, full replay, semantic
//! DAG compilation, and a cold outer-extension proof. It does not pass
//! guarded executable `RuleCell` promotion. Modular no-hits remain inconclusive.

mod expected;
mod limits;
mod model;
mod sweep;

use crate::sector::Mask;

use self::expected::{
    EXPECTED_FULL_RANK_DEGREE_ONE_SWEEP, EXPECTED_RANK_THREE_ROOT_AUTHORITY_DEGREE_ONE_SWEEP,
    assert_expected_mixed_s4a_sweep, assert_expected_sweep,
};
use self::sweep::{
    assert_structural_accounting, is_closed, sweep_sector, sweep_sector_against_k6_terminals,
};
use super::manifest::FULL_RANK_ORBITS;

const DEGREE_ONE: &[usize] = &[1];
const DEGREE_ONE_AND_TWO: &[usize] = &[1, 2];
const CANONICAL_S4A: [i64; 6] = [0, 1, 1, 1, 1, 0];

#[test]
fn first_rank_three_frontier_uses_exact_root_owners_without_claiming_closure() {
    for expected in EXPECTED_RANK_THREE_ROOT_AUTHORITY_DEGREE_ONE_SWEEP {
        let report = sweep_sector_against_k6_terminals(
            Mask::try_from_indices(&expected.representative).unwrap(),
            DEGREE_ONE,
        )
        .unwrap();
        eprintln!(
            "K6 rank-three root-authority sweep for {:?}: {report:#?}",
            expected.representative
        );
        assert_structural_accounting(&report);
        assert_expected_sweep(&report, &expected);
        assert!(
            !is_closed(&report),
            "bounded semantic replay must not masquerade as executable all-rank closure"
        );
    }
}

#[test]
fn canonical_s4a_degree_one_reports_an_exact_owner_cover_obstruction() {
    let sector = Mask::try_from_indices(&CANONICAL_S4A).unwrap();
    let report = sweep_sector(sector.clone(), DEGREE_ONE).unwrap();
    let repeated = sweep_sector(sector, DEGREE_ONE).unwrap();
    eprintln!("K6 S4a degree-one semantic owner-input sweep: {report:#?}");
    assert_eq!(report, repeated);
    assert_structural_accounting(&report);
    let degree = &report.degrees[0];
    assert_eq!(
        (
            degree.frame_offsets,
            degree.frame_rows,
            degree.frame_columns,
            degree.frame_entries,
        ),
        (7, 63, 157, 630)
    );
    assert_expected_sweep(&report, &EXPECTED_FULL_RANK_DEGREE_ONE_SWEEP[3]);
    assert!(!is_closed(&report));
}

#[test]
fn every_full_rank_orbit_has_bounded_semantic_owner_input_sweep_telemetry() {
    assert_eq!(
        FULL_RANK_ORBITS.map(|orbit| orbit.representative),
        EXPECTED_FULL_RANK_DEGREE_ONE_SWEEP.map(|expected| expected.representative),
        "the typed sweep baseline must cover the complete orbit manifest in canonical order"
    );
    for expected in EXPECTED_FULL_RANK_DEGREE_ONE_SWEEP {
        let report = sweep_sector(
            Mask::try_from_indices(&expected.representative).unwrap(),
            DEGREE_ONE,
        )
        .unwrap();
        eprintln!(
            "K6 orbit {:?} degree-one semantic owner-input sweep: {report:#?}",
            expected.representative
        );
        assert_structural_accounting(&report);
        assert_expected_sweep(&report, &expected);
        assert!(!is_closed(&report));
    }
}

#[test]
fn canonical_s4a_mixed_degree_reports_the_exact_owner_cover_obstruction() {
    let sector = Mask::try_from_indices(&CANONICAL_S4A).unwrap();
    let report = sweep_sector(sector.clone(), DEGREE_ONE_AND_TWO).unwrap();
    let repeated = sweep_sector(sector, DEGREE_ONE_AND_TWO).unwrap();
    eprintln!("K6 S4a degree-one plus degree-two semantic owner-input sweep: {report:#?}");
    assert_eq!(report, repeated);
    assert_structural_accounting(&report);
    assert_eq!(report.degrees.len(), 2);
    assert_eq!(
        (
            report.degrees[0].frame_offsets,
            report.degrees[0].frame_rows,
            report.degrees[0].frame_columns,
            report.degrees[0].frame_entries,
        ),
        (7, 63, 157, 630)
    );
    assert_eq!(
        (
            report.degrees[1].frame_offsets,
            report.degrees[1].frame_rows,
            report.degrees[1].frame_columns,
            report.degrees[1].frame_entries,
        ),
        (28, 252, 488, 2_520)
    );
    assert_expected_mixed_s4a_sweep(&report);
    assert!(!is_closed(&report));
}
