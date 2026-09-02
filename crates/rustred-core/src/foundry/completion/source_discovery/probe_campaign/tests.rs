//! Focused semantic adapter regressions.

mod all_full_rank_orbits;
mod boundary;
mod k6;
mod k6_alphaloop_lhs_diagnostic;
mod k6_boundary_walk;
mod k6_compact_boundary_walk;
mod k6_line301_guard_diagnostic;

use std::sync::Arc;

use crate::family::IntegralKey;
use crate::foundry::artifact::{
    derive_one_loop_unit_mass_tadpole, derive_two_loop_unit_mass_sunset,
};
use crate::foundry::completion::source_discovery::cover_delta::{
    CanonicalExactOwnerLedger, ExactOwnerCoverDeltaError, ExactOwnerCoverDeltaKind,
    ExactOwnerCoverDeltaLimits,
};
use crate::foundry::completion::source_discovery::interior_simplex::{
    InteriorSimplexLimits, InteriorSimplexPlan, InteriorSimplexScopePartition,
    try_plan_interior_simplex_samples,
};
use crate::foundry::completion::source_discovery::leader_walk::{
    LeaderWalkLimits, RequestedDomain, RequestedDomainScopePartition, try_plan_requested_domains,
};
use crate::foundry::completion::source_discovery::{
    AccumulatedSourceRequests, CampaignModularProbe, ExactExecutableOwnerProposal,
    InteriorReplayRunDisposition, OrdinarySourceIncidenceIndex, RequestedDomainSupportLimits,
    RequestedDomainSupportProposal, RequestedSupportProposalOrigin,
    RequestedSupportProposalProvenanceInput,
};
use crate::foundry::completion::stratum::{ImmutableOwnerSnapshot, StratumRegistryLimits};
use crate::foundry::completion::{LatticeBox, LatticePoint};
use crate::identity::{
    CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator, TranslatedSourceBatch,
};
use crate::sector::{Mask, OrderingPolicy};

use super::{
    ProbeCampaignAdapter, ProbeCampaignError, ProbeCampaignLimits, ProbeCampaignOutcome,
    ProbeCampaignOwnerEffect,
};

const PRIME: u64 = 1_000_000_007;

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn zero_sources(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    limits: ProbeCampaignLimits,
) -> TranslatedSourceBatch {
    generator
        .translate_completed_source_rows(
            completed,
            [
                IntegralShift::try_new(std::iter::repeat_n(0, generator.context().index_count()))
                    .unwrap(),
            ],
            limits.replay.scheduler.source_discovery.translation,
        )
        .unwrap()
}

fn plan(
    ledger: &CanonicalExactOwnerLedger,
    sector: &Mask,
    margin: u64,
    degree: usize,
) -> InteriorSimplexPlan {
    let partition = ledger.try_clone_uncovered_partition().unwrap();
    let scope = format!(
        "{}|{}|{}|{:?}|{:?}|{}",
        ledger.predecessor_snapshot().family_fingerprint(),
        ledger.predecessor_snapshot().context_fingerprint(),
        ledger.predecessor_snapshot().id().as_str(),
        sector.active_bits(),
        ledger.ordering(),
        ledger.revision().get(),
    );
    try_plan_interior_simplex_samples(
        ledger.revision().get(),
        [InteriorSimplexScopePartition::new(
            &scope, sector, &partition,
        )],
        margin,
        degree,
        InteriorSimplexLimits::default(),
    )
    .unwrap()
}

fn probe(
    coordinates: impl IntoIterator<Item = u64>,
    limits: ProbeCampaignLimits,
) -> CampaignModularProbe {
    CampaignModularProbe::try_new(PRIME, [37], coordinates, limits.replay.scheduler.campaign)
        .unwrap()
}

#[test]
fn adapter_rejects_zero_sources_from_a_different_completed_row_chronology() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let mut reordered = complete_ordinary(&generator);
    assert!(reordered.swap_source_rows_for_test(0, 1));
    let limits = ProbeCampaignLimits::default();
    let reordered_zero_sources = zero_sources(&generator, &reordered, limits);

    let error =
        ProbeCampaignAdapter::try_new(&generator, &completed, &reordered_zero_sources, limits)
            .unwrap_err();
    assert!(matches!(
        error,
        ProbeCampaignError::Scope {
            detail: "zero-source incidence is not the exact translation of the completed source barrier"
        }
    ));
}

#[test]
fn one_loop_task_closes_only_through_the_exact_ledger_compiler() {
    let artifact = Arc::new(derive_one_loop_unit_mass_tadpole().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    assert_eq!(completed.source_row_count(), 1);
    let limits = ProbeCampaignLimits::default();
    let zero_sources = zero_sources(&generator, &completed, limits);
    let adapter =
        ProbeCampaignAdapter::try_new(&generator, &completed, &zero_sources, limits).unwrap();
    let predecessor = ImmutableOwnerSnapshot::try_from_closed_artifact(
        Arc::clone(&artifact),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let sector = Mask::try_new([true]).unwrap();
    // The one-loop translated frame carries a physical +2 column, so its
    // executable recurrence has an exact finite representability ceiling.
    // Exercise genuine closure on an explicit supported-root carrier instead
    // of declaring the isolated i64::MAX fringe integral terminal.
    let mut ledger = CanonicalExactOwnerLedger::try_new_with_closure_carrier(
        generator.context(),
        predecessor,
        sector.clone(),
        OrderingPolicy::default(),
        [IntegralKey::try_new(sector.corner_indices()).unwrap()],
        LatticeBox::try_new([0], [Some(11)]).unwrap(),
        ExactOwnerCoverDeltaLimits::default(),
    )
    .unwrap();
    let plan = plan(&ledger, &sector, 1, 0);
    let task = &plan.tasks()[0];
    assert_eq!(task.target_shift().values(), &[1]);
    let binding = adapter.try_bind_task(&plan, task, &ledger).unwrap();
    let stale_binding = adapter.try_bind_task(&plan, task, &ledger).unwrap();
    let report = adapter
        .try_run_task(
            binding,
            &mut ledger,
            [probe([2], limits), probe([3], limits)],
        )
        .unwrap();
    assert_eq!(report.canonical_task_ordinal(), 0);
    assert_eq!(report.planned_ledger_revision().get(), 0);
    assert!(report.census().bootstrap().requests() > 0);
    assert_eq!(report.census().scheduler_outcomes().replayed(), 2);
    let ProbeCampaignOutcome::Closed { effect, applied } = report.outcome() else {
        panic!("the one-loop campaign task must close through the exact compiler")
    };
    assert_eq!(effect, ProbeCampaignOwnerEffect::StrictGeometricShrink);
    assert_eq!(
        applied.delta().kind(),
        ExactOwnerCoverDeltaKind::StrictGeometricShrink
    );
    assert!(applied.delta().updated().status().is_compiler_closed());
    assert_eq!(applied.delta().updated().revision().get(), 1);
    assert!(applied.obstructions().is_empty());

    let stale = adapter
        .try_run_task(stale_binding, &mut ledger, [probe([2], limits)])
        .unwrap_err();
    assert!(matches!(
        stale,
        ProbeCampaignError::CoverDelta(
            ExactOwnerCoverDeltaError::StaleLedgerSnapshotIdentity { expected, actual }
        ) if expected.get() == 1 && actual.get() == 0
    ));
}

#[test]
fn two_loop_planned_dot_task_strictly_shrinks_without_a_closure_claim() {
    let artifact = Arc::new(derive_two_loop_unit_mass_sunset().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    assert_eq!(completed.source_row_count(), 4);
    let limits = ProbeCampaignLimits::default();
    let zero_sources = zero_sources(&generator, &completed, limits);
    let adapter =
        ProbeCampaignAdapter::try_new(&generator, &completed, &zero_sources, limits).unwrap();
    let predecessor = ImmutableOwnerSnapshot::try_from_closed_artifact(
        Arc::clone(&artifact),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let sector = Mask::try_new([true, true, true]).unwrap();
    let mut ledger = CanonicalExactOwnerLedger::try_new(
        generator.context(),
        predecessor,
        sector.clone(),
        OrderingPolicy::default(),
        [IntegralKey::try_new(sector.corner_indices()).unwrap()],
        ExactOwnerCoverDeltaLimits::default(),
    )
    .unwrap();
    let plan = plan(&ledger, &sector, 1, 1);
    let task = plan
        .tasks()
        .iter()
        .find(|task| task.target_shift().values() == [2, 1, 1])
        .expect("the complete degree-one simplex must contain the first dot");
    let binding = adapter.try_bind_task(&plan, task, &ledger).unwrap();
    let report = adapter
        .try_run_task(
            binding,
            &mut ledger,
            [probe([2, 3, 5], limits), probe([3, 5, 7], limits)],
        )
        .unwrap();
    let ProbeCampaignOutcome::StrictGeometricShrink(applied) = report.outcome() else {
        panic!("the two-loop dot task must strictly shrink but remain incomplete")
    };
    assert_eq!(
        applied.delta().kind(),
        ExactOwnerCoverDeltaKind::StrictGeometricShrink
    );
    assert!(!applied.delta().updated().status().is_compiler_closed());
    assert_eq!(applied.delta().updated().revision().get(), 1);
    assert!(applied.obstructions().is_empty());
    assert_eq!(report.census().support().unwrap().candidates(), 1);
}

#[test]
fn requested_parent_support_enters_only_through_incidence_and_exact_replay() {
    let artifact = Arc::new(derive_one_loop_unit_mass_tadpole().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ProbeCampaignLimits::default();
    let zero_sources = zero_sources(&generator, &completed, limits);
    let adapter =
        ProbeCampaignAdapter::try_new(&generator, &completed, &zero_sources, limits).unwrap();
    let predecessor = ImmutableOwnerSnapshot::try_from_closed_artifact(
        Arc::clone(&artifact),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let sector = Mask::try_new([true]).unwrap();
    let mut ledger = CanonicalExactOwnerLedger::try_new_with_closure_carrier(
        generator.context(),
        predecessor,
        sector.clone(),
        OrderingPolicy::default(),
        [IntegralKey::try_new(sector.corner_indices()).unwrap()],
        LatticeBox::try_new([0], [Some(11)]).unwrap(),
        ExactOwnerCoverDeltaLimits::default(),
    )
    .unwrap();
    let partition = ledger.try_clone_uncovered_partition().unwrap();
    let requested = [RequestedDomain::new(
        LatticePoint::try_new([1]).unwrap(),
        [0],
    )];
    let plan = try_plan_requested_domains(
        ledger.revision().get(),
        [RequestedDomainScopePartition::new(
            "assisted-one-loop",
            &sector,
            &partition,
            &requested,
        )],
        LeaderWalkLimits::default(),
    )
    .unwrap();
    let task = &plan.tasks()[0];
    assert_eq!(task.target_shift().values(), &[1]);
    let support = [IntegralShift::try_new([2]).unwrap()];
    let incidence = OrdinarySourceIncidenceIndex::try_new(
        &zero_sources,
        limits.replay.scheduler.source_discovery,
    )
    .unwrap();
    let bootstrap = incidence
        .try_nominate_target_unit(
            task.target_shift(),
            limits.replay.scheduler.source_discovery,
        )
        .unwrap();
    let nominated_support = incidence
        .try_nominate_initial_parent_support(
            &completed,
            &support,
            limits.replay.scheduler.source_discovery,
        )
        .unwrap();
    assert!(
        nominated_support
            .requests()
            .iter()
            .any(|request| { bootstrap.requests().binary_search(request).is_err() })
    );
    let expected_epoch_zero_requests = AccumulatedSourceRequests::try_new(
        incidence.arity(),
        bootstrap
            .requests()
            .iter()
            .cloned()
            .chain(nominated_support.requests().iter().cloned()),
        limits.replay.scheduler.campaign,
    )
    .unwrap();
    let proposal = RequestedDomainSupportProposal::try_new(
        task.key().stable_scope_key(),
        task.key().sector(),
        task.key().requested_domain_lower(),
        task.key().symbolic_axes(),
        &support,
        RequestedSupportProposalProvenanceInput::new(
            1,
            1,
            0,
            "default-order",
            "one-loop-prolongation",
            RequestedSupportProposalOrigin::InvolutiveProlongation,
        ),
        RequestedDomainSupportLimits::default(),
    )
    .unwrap();
    let binding = adapter.try_bind_task(&plan, task, &ledger).unwrap();
    let stale_binding = adapter.try_bind_task(&plan, task, &ledger).unwrap();
    let baseline_identity = ledger.snapshot_identity();
    let evaluated = adapter
        .try_evaluate_requested_task_with_parent_support(
            binding,
            &ledger,
            &proposal,
            [probe([2], limits), probe([3], limits)],
        )
        .unwrap();
    let stale_evaluated = adapter
        .try_evaluate_requested_task_with_parent_support(
            stale_binding,
            &ledger,
            &proposal,
            [probe([2], limits), probe([3], limits)],
        )
        .unwrap();
    assert_eq!(evaluated.canonical_task_ordinal(), task.canonical_ordinal());
    let census = evaluated.census();
    assert_eq!(
        census.bootstrap().requests(),
        expected_epoch_zero_requests.len()
    );
    assert_eq!(
        census.bootstrap().selected_sources(),
        expected_epoch_zero_requests.len()
    );
    assert_eq!(census.scheduler().epochs(), 2);
    let scheduler = census.scheduler_outcomes();
    assert_eq!(scheduler.replayed(), 2);
    assert_eq!(scheduler.support_did_not_lift(), 0);
    assert_eq!(scheduler.exact_lift_error(), 0);
    assert_eq!(scheduler.sampled_dual(), 0);
    assert_eq!(scheduler.budget_stop(), 0);
    assert_eq!(scheduler.rejected(), 0);
    assert_eq!(scheduler.stalled(), 0);
    assert!(census.first_scheduler_rejection().is_none());
    let attempts = census.canonical_attempts();
    assert_eq!(attempts.replayed(), 2);
    assert_eq!(attempts.no_modular_hit(), 0);
    assert_eq!(attempts.query_rejected(), 0);
    assert_eq!(attempts.support_did_not_lift(), 0);
    let replay = census
        .replay()
        .expect("both probes must reach exact replay");
    assert_eq!(replay.replayed_nominations(), 2);
    assert_eq!(replay.union_requests(), expected_epoch_zero_requests.len());
    assert_eq!(replay.rebase_attempts(), 2);
    assert_eq!(replay.successful_exact_lifts(), 2);
    assert_eq!(replay.unique_candidates(), 1);
    assert_eq!(replay.duplicate_exact_lifts(), 1);

    let InteriorReplayRunDisposition::OwnerProposal {
        proposal:
            ExactExecutableOwnerProposal::Compiled {
                owner,
                obstructions,
            },
        support: Some(replay_support),
    } = evaluated.replay_disposition()
    else {
        panic!("the assisted one-loop task must compile an exact owner")
    };
    assert!(obstructions.is_empty());
    assert_eq!(owner.epoch().target_shift(), task.target_shift());
    assert_eq!(
        owner.epoch().requests().requests(),
        expected_epoch_zero_requests.requests()
    );
    assert!(
        owner
            .executable_candidates()
            .iter()
            .all(|candidate| candidate.circuit().target_shift() == task.target_shift())
    );
    assert_eq!(replay_support.candidates().len(), 1);
    assert!(!replay_support.candidates()[0].sources().is_empty());
    for source in replay_support
        .candidates()
        .iter()
        .flat_map(|candidate| candidate.sources())
    {
        assert_eq!(
            completed.source_row_id(source.source_ordinal()),
            Some(source.source_row())
        );
        assert_eq!(source.relative_offset().len(), task.target_shift().len());
    }
    assert!(
        ledger
            .snapshot_identity()
            .same_snapshot_as(&baseline_identity)
    );

    let report = adapter
        .try_apply_evaluated_task(evaluated, &mut ledger)
        .unwrap();
    let ProbeCampaignOutcome::Closed { effect, applied } = report.outcome() else {
        panic!("the assisted one-loop owner must close only after ledger application")
    };
    assert_eq!(effect, ProbeCampaignOwnerEffect::StrictGeometricShrink);
    assert_eq!(
        applied.delta().kind(),
        ExactOwnerCoverDeltaKind::StrictGeometricShrink
    );
    assert_eq!(ledger.revision().get(), 1);

    let stale = adapter
        .try_apply_evaluated_task(stale_evaluated, &mut ledger)
        .unwrap_err();
    assert!(matches!(
        stale,
        ProbeCampaignError::CoverDelta(
            ExactOwnerCoverDeltaError::StaleLedgerSnapshotIdentity { expected, actual }
        ) if expected.get() == 1 && actual.get() == 0
    ));
}

#[test]
fn requested_parent_support_rejects_every_semantic_key_mismatch_before_replay() {
    let artifact = Arc::new(derive_one_loop_unit_mass_tadpole().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ProbeCampaignLimits::default();
    let zero_sources = zero_sources(&generator, &completed, limits);
    let adapter =
        ProbeCampaignAdapter::try_new(&generator, &completed, &zero_sources, limits).unwrap();
    let predecessor = ImmutableOwnerSnapshot::try_from_closed_artifact(
        Arc::clone(&artifact),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let sector = Mask::try_new([true]).unwrap();
    let ledger = CanonicalExactOwnerLedger::try_new(
        generator.context(),
        predecessor,
        sector.clone(),
        OrderingPolicy::default(),
        [IntegralKey::try_new(sector.corner_indices()).unwrap()],
        ExactOwnerCoverDeltaLimits::default(),
    )
    .unwrap();
    let partition = ledger.try_clone_uncovered_partition().unwrap();
    let requested = [RequestedDomain::new(
        LatticePoint::try_new([1]).unwrap(),
        [0],
    )];
    let plan = try_plan_requested_domains(
        ledger.revision().get(),
        [RequestedDomainScopePartition::new(
            "expected-scope",
            &sector,
            &partition,
            &requested,
        )],
        LeaderWalkLimits::default(),
    )
    .unwrap();
    let task = &plan.tasks()[0];
    let support = [IntegralShift::try_new([2]).unwrap()];
    let foreign_sector = Mask::try_new([false]).unwrap();
    let expected_point = [1];
    let foreign_point = [2];
    let expected_axes = [0];
    let foreign_axes = [];
    let cases: [(&str, &str, &Mask, &[u64], &[usize]); 4] = [
        (
            "scope",
            "foreign-scope",
            &sector,
            &expected_point,
            &expected_axes,
        ),
        (
            "sector",
            "expected-scope",
            &foreign_sector,
            &expected_point,
            &expected_axes,
        ),
        (
            "point",
            "expected-scope",
            &sector,
            &foreign_point,
            &expected_axes,
        ),
        (
            "symbolic axes",
            "expected-scope",
            &sector,
            &expected_point,
            &foreign_axes,
        ),
    ];
    let baseline_identity = ledger.snapshot_identity();
    for (case, scope, proposal_sector, point, axes) in cases {
        let proposal = RequestedDomainSupportProposal::try_new(
            scope,
            proposal_sector,
            point,
            axes,
            &support,
            RequestedSupportProposalProvenanceInput::new(
                1,
                1,
                0,
                "default-order",
                case,
                RequestedSupportProposalOrigin::InvolutiveProlongation,
            ),
            RequestedDomainSupportLimits::default(),
        )
        .unwrap();
        let binding = adapter.try_bind_task(&plan, task, &ledger).unwrap();
        let error = adapter
            .try_evaluate_requested_task_with_parent_support(
                binding,
                &ledger,
                &proposal,
                std::iter::empty(),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            ProbeCampaignError::Scope {
                detail: "parent-support proposal and requested task have different semantic domains",
            }
        ));
        assert!(
            ledger
                .snapshot_identity()
                .same_snapshot_as(&baseline_identity),
            "{case} mismatch must not mutate the ledger"
        );
    }
}

#[test]
fn bootstrap_sort_work_one_below_fails_before_ledger_mutation() {
    let artifact = Arc::new(derive_one_loop_unit_mass_tadpole().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let mut limits = ProbeCampaignLimits::default();
    let zero_sources = zero_sources(&generator, &completed, limits);
    let predecessor = ImmutableOwnerSnapshot::try_from_closed_artifact(
        Arc::clone(&artifact),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let sector = Mask::try_new([true]).unwrap();
    let mut ledger = CanonicalExactOwnerLedger::try_new(
        generator.context(),
        predecessor,
        sector.clone(),
        OrderingPolicy::default(),
        [IntegralKey::try_new(sector.corner_indices()).unwrap()],
        ExactOwnerCoverDeltaLimits::default(),
    )
    .unwrap();
    let plan = plan(&ledger, &sector, 1, 0);
    let task = &plan.tasks()[0];

    let incidence = OrdinarySourceIncidenceIndex::try_new(
        &zero_sources,
        limits.replay.scheduler.source_discovery,
    )
    .unwrap();
    let nominations = incidence
        .try_nominate_target_unit(
            task.target_shift(),
            limits.replay.scheduler.source_discovery,
        )
        .unwrap();
    let selected = generator
        .translate_selected_completed_source_rows(
            &completed,
            nominations.requests().iter().cloned(),
            limits.replay.scheduler.campaign.translated_sources,
        )
        .unwrap();
    let occurrences: usize = selected
        .sources()
        .iter()
        .map(|source| source.terms().len())
        .sum();
    let normalized = occurrences.max(2);
    let levels = usize::BITS as usize - (normalized - 1).leading_zeros() as usize;
    let exact_sort_work = occurrences.checked_mul(levels).unwrap();
    assert!(exact_sort_work > 0);
    limits.max_bootstrap_physical_shift_sort_work = exact_sort_work - 1;

    let adapter =
        ProbeCampaignAdapter::try_new(&generator, &completed, &zero_sources, limits).unwrap();
    let binding = adapter.try_bind_task(&plan, task, &ledger).unwrap();
    let baseline = ledger.snapshot();
    let baseline_identity = ledger.snapshot_identity();
    let error = adapter
        .try_run_task(binding, &mut ledger, [probe([2], limits)])
        .unwrap_err();
    assert!(matches!(
        error,
        ProbeCampaignError::ResourceLimit {
            resource: "bootstrap physical shift logical sort work reservation",
            requested,
            limit,
        } if requested == exact_sort_work && limit + 1 == exact_sort_work
    ));
    assert_eq!(ledger.snapshot(), baseline);
    assert!(
        ledger
            .snapshot_identity()
            .same_snapshot_as(&baseline_identity)
    );
}
