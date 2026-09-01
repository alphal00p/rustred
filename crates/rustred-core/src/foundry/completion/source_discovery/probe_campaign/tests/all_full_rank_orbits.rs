//! Oracle-disabled first-owner coverage across every typed K=6 full-rank orbit.

use crate::family::IntegralKey;
use crate::foundry::artifact::FULL_RANK_ORBITS;
use crate::foundry::completion::frame::admission::{
    ExactOwnerCoverObstructionKind, ExactOwnerCoverStatus,
};
use crate::foundry::completion::source_discovery::OrdinarySourceIncidenceIndex;
use crate::foundry::completion::source_discovery::cover_delta::{
    CanonicalExactOwnerLedger, ExactOwnerCoverDeltaKind, ExactOwnerCoverDeltaLimits,
    ExactOwnerLedgerCoverStatus,
};
use crate::foundry::completion::source_discovery::test_fixtures::OracleDisabledK6Fixture;
use crate::sector::{Mask, OrderingPolicy};

use super::super::{ProbeCampaignAdapter, ProbeCampaignLimits, ProbeCampaignOutcome};
use super::{plan, probe};

#[test]
fn every_typed_k6_full_rank_orbit_has_an_obstruction_free_first_exact_shrink() {
    let fixture = OracleDisabledK6Fixture::shared();
    assert_eq!(fixture.completed().source_row_count(), 9);
    assert!(fixture.completed().is_complete_ordinary());
    assert_eq!(fixture.predecessor().closed_layer_count(), 0);

    let limits = ProbeCampaignLimits::default();
    let incidence = OrdinarySourceIncidenceIndex::try_new(
        fixture.zero_sources(),
        limits.replay.scheduler.source_discovery,
    )
    .unwrap();
    let adapter =
        ProbeCampaignAdapter::try_new(fixture.generator(), fixture.completed(), &incidence, limits)
            .unwrap();

    for orbit in FULL_RANK_ORBITS {
        let sector = Mask::try_from_indices(&orbit.representative).unwrap();
        let mut ledger = CanonicalExactOwnerLedger::try_new(
            fixture.generator().context(),
            fixture.predecessor().clone(),
            sector.clone(),
            OrderingPolicy::default(),
            std::iter::empty::<IntegralKey>(),
            ExactOwnerCoverDeltaLimits::default(),
        )
        .unwrap();
        let baseline = ledger.snapshot();
        assert_eq!(baseline.revision().get(), 0);
        assert_eq!(baseline.status(), ExactOwnerLedgerCoverStatus::OwnerFree);
        assert_eq!(baseline.owner_count(), 0);
        assert_eq!(baseline.terminal_count(), 0);
        assert_eq!(baseline.uncovered_box_count(), 1);

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
            panic!("every typed K=6 full-rank orbit must produce a strict first exact shrink")
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
    }
}
