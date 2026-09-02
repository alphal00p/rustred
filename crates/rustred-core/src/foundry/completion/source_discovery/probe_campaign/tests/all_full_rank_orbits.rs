//! Exact predecessor/discovery coverage across every typed K=6 full-rank orbit.

use crate::family::IntegralKey;
use crate::foundry::artifact::FULL_RANK_ORBITS;
use crate::foundry::completion::frame::admission::{
    ExactOwnerCoverObstructionKind, ExactOwnerCoverStatus,
};
use crate::foundry::completion::source_discovery::cover_delta::{
    CanonicalExactOwnerLedger, ExactOwnerCoverDeltaKind, ExactOwnerCoverDeltaLimits,
    ExactOwnerLedgerCoverStatus,
};
use crate::foundry::completion::source_discovery::test_fixtures::OracleDisabledK6Fixture;
use crate::sector::{InteriorBounds, Mask, OrderingPolicy, SectorInteriorDomain};

use super::super::{ProbeCampaignAdapter, ProbeCampaignLimits, ProbeCampaignOutcome};
use super::{plan, probe};

const PRODUCT_INTERIOR_WITNESSES: [[i64; 6]; 3] = [
    [-2, -4, 2, -2, 3, 4],
    [-2, -4, 2, 3, -2, 4],
    [-4, -2, 2, 3, 4, 2],
];

#[test]
fn every_k6_orbit_retains_its_exact_fringe_and_reports_its_first_proposal_truthfully() {
    let fixture = OracleDisabledK6Fixture::shared();
    assert_eq!(fixture.completed().source_row_count(), 9);
    assert!(fixture.completed().is_complete_ordinary());
    assert_eq!(fixture.predecessor().closed_layer_count(), 0);

    let limits = ProbeCampaignLimits::default();
    let adapter = ProbeCampaignAdapter::try_new(
        fixture.generator(),
        fixture.completed(),
        fixture.zero_sources(),
        limits,
    )
    .unwrap();

    let mut authenticated_product_witnesses = 0usize;
    let mut discovery_shrunk = 0usize;
    for (orbit_ordinal, orbit) in FULL_RANK_ORBITS.iter().enumerate() {
        let sector = Mask::try_from_indices(&orbit.representative).unwrap();
        let complete_sector = SectorInteriorDomain::try_new(
            sector.clone(),
            sector.active_bits().iter().map(|&active| {
                if active {
                    InteriorBounds::new(1, i64::MAX)
                } else {
                    InteriorBounds::new(i64::MIN, 0)
                }
            }),
        )
        .unwrap();
        assert!(
            !fixture
                .predecessor()
                .authenticates_same_sector_domain(OrderingPolicy::default(), &complete_sector),
            "orbit {orbit_ordinal} must retain a genuine discovery fringe",
        );
        if let Some(&powers) = PRODUCT_INTERIOR_WITNESSES.get(orbit_ordinal) {
            let witness = IntegralKey::try_new(powers).unwrap();
            assert_eq!(Mask::try_from_indices(witness.powers()).unwrap(), sector);
            assert!(
                fixture
                    .predecessor()
                    .authenticates_explicit_terminal(&witness)
                    .unwrap(),
                "product orbit {orbit_ordinal} must retain its sparse exact predecessor authority",
            );
            authenticated_product_witnesses += 1;
        }
        let mut ledger = CanonicalExactOwnerLedger::try_new_with_closure_carrier(
            fixture.generator().context(),
            fixture.predecessor().clone(),
            sector.clone(),
            OrderingPolicy::default(),
            std::iter::empty::<IntegralKey>(),
            fixture.source_safe_closure_carrier(&sector),
            ExactOwnerCoverDeltaLimits::default(),
        )
        .unwrap();
        let baseline = ledger.snapshot();
        assert_eq!(baseline.revision().get(), 0);
        // Product ownership is an exact sparse preimage, not the enclosing
        // sector hull.  Its coupled endpoint fringe therefore remains an
        // ordinary discovery obligation, just like the three irreducible
        // full-rank sectors.
        assert_eq!(baseline.status(), ExactOwnerLedgerCoverStatus::OwnerFree);
        assert_eq!(baseline.owner_count(), 0);
        assert_eq!(baseline.terminal_count(), 0);
        assert_eq!(baseline.uncovered_box_count(), 1);
        assert!(!baseline.uncovered_is_finite());
        let baseline_partition = ledger.try_clone_uncovered_partition().unwrap();
        let [baseline_box] = baseline_partition.boxes() else {
            panic!("a fresh owner-free ledger must retain one discovery box")
        };
        assert_eq!(baseline_box.lower(), &[0; 6]);
        assert_eq!(baseline_box.upper(), &[None; 6]);

        let proposal_plan = plan(&ledger, &sector, 2, 0);
        let [task] = proposal_plan.tasks() else {
            panic!("an owner-free K=6 orthant must produce exactly one degree-zero task")
        };
        assert_eq!(task.key().sector(), &sector);
        assert_eq!(task.lattice_target(), &[2; 6]);
        let binding = adapter
            .try_bind_task(&proposal_plan, task, &ledger)
            .unwrap();
        let report = adapter
            .try_run_task(
                binding,
                &mut ledger,
                [probe(task.lattice_target().iter().copied(), limits)],
            )
            .unwrap();
        assert_eq!(report.planned_ledger_revision().get(), 0);
        let ProbeCampaignOutcome::StrictGeometricShrink(applied) = report.outcome() else {
            panic!(
                "orbit {orbit_ordinal} did not produce its expected first exact shrink: {:?}",
                report.outcome(),
            )
        };
        assert!(applied.obstructions().is_empty());
        let delta = applied.delta();
        assert_eq!(
            delta.kind(),
            ExactOwnerCoverDeltaKind::StrictGeometricShrink
        );
        assert_eq!(delta.baseline(), baseline);
        assert_eq!(delta.updated().revision().get(), 1);
        assert_eq!(delta.updated().owner_count(), 1);
        assert_eq!(delta.updated().terminal_count(), 0);
        assert_eq!(delta.updated().uncovered_box_count(), 6);
        assert!(!delta.updated().uncovered_is_finite());
        assert_eq!(delta.updated().missing_terminal_count(), 0);
        assert_eq!(delta.updated().guard_incomplete_owner_count(), 0);
        assert_eq!(
            delta.updated().status(),
            ExactOwnerLedgerCoverStatus::Compiled(ExactOwnerCoverStatus::Incomplete(
                ExactOwnerCoverObstructionKind::NonFinite,
            ))
        );
        assert_eq!(ledger.snapshot(), delta.updated());
        let updated_partition = ledger.try_clone_uncovered_partition().unwrap();
        assert_eq!(updated_partition.boxes().len(), 6);
        for (pivot, cell) in updated_partition.boxes().iter().enumerate() {
            let mut expected_lower = [0; 6];
            expected_lower[..pivot].fill(2);
            let mut expected_upper = [None; 6];
            expected_upper[pivot] = Some(1);
            assert_eq!(cell.lower(), expected_lower);
            assert_eq!(cell.upper(), expected_upper);
        }
        discovery_shrunk += 1;
    }
    assert_eq!(authenticated_product_witnesses, 3);
    assert_eq!(discovery_shrunk, FULL_RANK_ORBITS.len());
}
