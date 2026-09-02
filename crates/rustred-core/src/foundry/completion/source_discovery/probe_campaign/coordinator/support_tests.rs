use std::sync::Arc;

use crate::family::IntegralKey;
use crate::foundry::artifact::derive_one_loop_unit_mass_tadpole;
use crate::foundry::completion::source_discovery::cover_delta::{
    CanonicalExactOwnerLedger, ExactOwnerCoverDeltaLimits, ExactOwnerCoverSnapshot,
};
use crate::foundry::completion::source_discovery::leader_walk::{
    LeaderWalkLimits, RequestedDomain, RequestedDomainPlan, RequestedDomainScopePartition,
    try_plan_requested_domains,
};
use crate::foundry::completion::source_discovery::test_fixtures::OracleDisabledK6Fixture;
use crate::foundry::completion::source_discovery::{
    ProbeCampaignAdapter, ProbeCampaignError, ProbeCampaignLimits, ProbeCoordinatorCensus,
    RequestedDomainSupportLimits, RequestedDomainSupportProposal, RequestedDomainSupportUnion,
    RequestedSupportProposalOrigin, RequestedSupportProposalProvenanceInput,
    try_union_requested_domain_support,
};
use crate::foundry::completion::stratum::{ImmutableOwnerSnapshot, StratumRegistryLimits};
use crate::foundry::completion::{LatticeBox, LatticePoint, UncoveredPartition};
use crate::identity::{CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator};
use crate::sector::{Mask, OrderingPolicy};

use super::support::requested_support_for_task;
use super::{
    BoundaryProbeCoordinator, ProbeCoordinatorConfig, ProbeCoordinatorFailure,
    ProbeCoordinatorLimits, RequestedProbeCoordinatorStop, TaskRelativeModularProbe,
};

const PRIME: u64 = 1_000_000_007;
const ONE_LOOP_SCOPE: &str = "support-assisted-one-loop";

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn proposal(
    scope: &str,
    sector: &Mask,
    point: &[u64],
    symbolic_axes: &[usize],
    support: &[IntegralShift],
    obligation: &str,
) -> RequestedDomainSupportProposal {
    RequestedDomainSupportProposal::try_new(
        scope,
        sector,
        point,
        symbolic_axes,
        support,
        RequestedSupportProposalProvenanceInput::new(
            1,
            1,
            0,
            "support-test-order",
            obligation,
            RequestedSupportProposalOrigin::InvolutiveProlongation,
        ),
        RequestedDomainSupportLimits::default(),
    )
    .unwrap()
}

fn requested_plan(
    revision: u64,
    scope: &str,
    sector: &Mask,
    partition: &UncoveredPartition,
    requests: &[RequestedDomain],
) -> RequestedDomainPlan {
    try_plan_requested_domains(
        revision,
        [RequestedDomainScopePartition::new(
            scope, sector, partition, requests,
        )],
        LeaderWalkLimits::default(),
    )
    .unwrap()
}

fn one_loop_support() -> RequestedDomainSupportUnion {
    let sector = Mask::try_new([true]).unwrap();
    try_union_requested_domain_support(
        vec![proposal(
            ONE_LOOP_SCOPE,
            &sector,
            &[1],
            &[0],
            &[IntegralShift::try_new([2]).unwrap()],
            "one-loop-parent-two",
        )],
        RequestedDomainSupportLimits::default(),
    )
    .unwrap()
}

fn run_one_loop(
    support: Option<&RequestedDomainSupportUnion>,
    campaign_limits: ProbeCampaignLimits,
) -> (
    CanonicalExactOwnerLedger,
    ProbeCoordinatorCensus,
    RequestedProbeCoordinatorStop,
) {
    let artifact = Arc::new(derive_one_loop_unit_mass_tadpole().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let zero_sources = generator
        .translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0]).unwrap()],
            campaign_limits
                .replay
                .scheduler
                .source_discovery
                .translation,
        )
        .unwrap();
    let adapter =
        ProbeCampaignAdapter::try_new(&generator, &completed, &zero_sources, campaign_limits)
            .unwrap();
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
    let requests = [RequestedDomain::new(
        LatticePoint::try_new([1]).unwrap(),
        [0],
    )];
    let plan = requested_plan(
        ledger.revision().get(),
        ONE_LOOP_SCOPE,
        &sector,
        &partition,
        &requests,
    );
    let config = ProbeCoordinatorConfig::try_new(
        [
            TaskRelativeModularProbe::try_new(
                PRIME,
                [37],
                [1],
                campaign_limits.replay.scheduler.campaign,
            )
            .unwrap(),
            TaskRelativeModularProbe::try_new(
                PRIME,
                [37],
                [2],
                campaign_limits.replay.scheduler.campaign,
            )
            .unwrap(),
        ],
        1,
        0,
        ProbeCoordinatorLimits::default(),
    )
    .unwrap();
    let mut coordinator = BoundaryProbeCoordinator::try_new(config, adapter, &ledger).unwrap();
    let stop = match support {
        Some(support) => {
            coordinator.try_run_requested_plan_with_support(&plan, &mut ledger, support)
        }
        None => coordinator.try_run_requested_plan(&plan, &mut ledger),
    };
    (ledger, coordinator.census(), stop)
}

#[test]
fn same_domain_support_union_is_canonical_and_lookup_is_deterministic() {
    let sector = Mask::try_new([true, true]).unwrap();
    let point = [0, 0];
    let axes = [0, 1];
    let first = proposal(
        "same-domain",
        &sector,
        &point,
        &axes,
        &[IntegralShift::try_new([1, 0]).unwrap()],
        "first",
    );
    let second = proposal(
        "same-domain",
        &sector,
        &point,
        &axes,
        &[
            IntegralShift::try_new([0, 1]).unwrap(),
            IntegralShift::try_new([1, 0]).unwrap(),
        ],
        "second",
    );
    let limits = RequestedDomainSupportLimits::default();
    let forward =
        try_union_requested_domain_support(vec![first.clone(), second.clone()], limits).unwrap();
    let reverse = try_union_requested_domain_support(vec![second, first], limits).unwrap();
    assert_eq!(forward, reverse);
    assert_eq!(forward.proposals().len(), 1);
    assert_eq!(
        forward.proposals()[0].parent_support(),
        [
            IntegralShift::try_new([0, 1]).unwrap(),
            IntegralShift::try_new([1, 0]).unwrap(),
        ]
    );

    let partition =
        UncoveredPartition::new(vec![LatticeBox::try_new([0, 0], [None, None]).unwrap()], 0);
    let requests = [RequestedDomain::new(
        LatticePoint::try_new(point).unwrap(),
        axes,
    )];
    let plan = requested_plan(0, "same-domain", &sector, &partition, &requests);
    assert_eq!(
        requested_support_for_task(&forward, &plan.tasks()[0])
            .unwrap()
            .parent_support(),
        reverse.proposals()[0].parent_support()
    );
}

#[test]
fn residual_task_lookup_uses_original_request_pivot_not_residual_leader() {
    let sector = Mask::try_new([true]).unwrap();
    let partition = UncoveredPartition::new(vec![LatticeBox::try_new([2], [Some(5)]).unwrap()], 0);
    let requests = [RequestedDomain::new(
        LatticePoint::try_new([0]).unwrap(),
        [0],
    )];
    let plan = requested_plan(4, "residual-original-pivot", &sector, &partition, &requests);
    assert_eq!(plan.tasks().len(), 1);
    let task = &plan.tasks()[0];
    assert_eq!(task.leader(), [2]);
    assert_eq!(task.key().requested_domain_lower(), [0]);
    assert_eq!(task.target_shift().values(), [0]);

    let union = try_union_requested_domain_support(
        vec![proposal(
            "residual-original-pivot",
            &sector,
            &[0],
            &[0],
            &[IntegralShift::try_new([3]).unwrap()],
            "residual",
        )],
        RequestedDomainSupportLimits::default(),
    )
    .unwrap();
    assert_eq!(
        requested_support_for_task(&union, task)
            .unwrap()
            .parent_support()[0]
            .values(),
        [3]
    );
}

#[test]
fn support_assisted_and_ordinary_coordinator_paths_compile_the_same_exact_owner() {
    let support = one_loop_support();
    let (ordinary, ordinary_census, ordinary_stop) =
        run_one_loop(None, ProbeCampaignLimits::default());
    let (assisted, assisted_census, assisted_stop) =
        run_one_loop(Some(&support), ProbeCampaignLimits::default());
    assert!(matches!(
        ordinary_stop,
        RequestedProbeCoordinatorStop::CompilerClosed { .. }
    ));
    assert!(matches!(
        assisted_stop,
        RequestedProbeCoordinatorStop::CompilerClosed { .. }
    ));
    assert_eq!(ordinary.snapshot(), assisted.snapshot());
    assert_eq!(ordinary_census.task_reports(), 1);
    assert_eq!(assisted_census.task_reports(), 1);
    assert_eq!(ordinary_census.requested_support_assisted(), 0);
    assert_eq!(ordinary_census.requested_support_fallback(), 0);
    assert_eq!(assisted_census.requested_support_assisted(), 1);
    assert_eq!(assisted_census.requested_support_fallback(), 0);
    assert_eq!(ordinary.owners().len(), 1);
    assert_eq!(assisted.owners().len(), 1);
    let ordinary_rule = ordinary.owners()[0].executable_candidates()[0]
        .cell()
        .rule();
    let assisted_rule = assisted.owners()[0].executable_candidates()[0]
        .cell()
        .rule();
    assert_eq!(ordinary_rule.domain(), assisted_rule.domain());
    assert_eq!(ordinary_rule.ordering(), assisted_rule.ordering());
    assert_eq!(ordinary_rule.pivot(), assisted_rule.pivot());
    assert_eq!(
        ordinary_rule.right_hand_side().len(),
        assisted_rule.right_hand_side().len()
    );
    for (ordinary_term, assisted_term) in ordinary_rule
        .right_hand_side()
        .iter()
        .zip(assisted_rule.right_hand_side())
    {
        assert_eq!(ordinary_term.shift(), assisted_term.shift());
        assert_eq!(ordinary_term.coefficient(), assisted_term.coefficient());
    }
    assert_eq!(
        ordinary_rule
            .elimination_pivot_guards()
            .iter()
            .map(|guard| guard.nonzero_polynomial())
            .collect::<Vec<_>>(),
        assisted_rule
            .elimination_pivot_guards()
            .iter()
            .map(|guard| guard.nonzero_polynomial())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        ordinary_rule
            .nonzero_guards()
            .iter()
            .map(|guard| guard.polynomial())
            .collect::<Vec<_>>(),
        assisted_rule
            .nonzero_guards()
            .iter()
            .map(|guard| guard.polynomial())
            .collect::<Vec<_>>()
    );
}

#[test]
fn same_scope_per_domain_miss_falls_back_to_the_ordinary_path() {
    let sector = Mask::try_new([true]).unwrap();
    let unrelated = try_union_requested_domain_support(
        vec![proposal(
            ONE_LOOP_SCOPE,
            &sector,
            &[2],
            &[0],
            &[IntegralShift::try_new([2]).unwrap()],
            "unrelated",
        )],
        RequestedDomainSupportLimits::default(),
    )
    .unwrap();
    let (ordinary, _, ordinary_stop) = run_one_loop(None, ProbeCampaignLimits::default());
    let (fallback, fallback_census, fallback_stop) =
        run_one_loop(Some(&unrelated), ProbeCampaignLimits::default());
    assert!(matches!(
        ordinary_stop,
        RequestedProbeCoordinatorStop::CompilerClosed { .. }
    ));
    assert!(matches!(
        fallback_stop,
        RequestedProbeCoordinatorStop::CompilerClosed { .. }
    ));
    assert_eq!(ordinary.snapshot(), fallback.snapshot());
    assert_eq!(fallback_census.requested_support_assisted(), 0);
    assert_eq!(fallback_census.requested_support_fallback(), 1);
    assert_eq!(
        ordinary.owners()[0].content_order_key(),
        fallback.owners()[0].content_order_key()
    );
}

#[test]
fn globally_unmatched_support_scope_is_rejected_before_work_or_mutation() {
    let sector = Mask::try_new([true]).unwrap();
    let unrelated = try_union_requested_domain_support(
        vec![proposal(
            "different-scope",
            &sector,
            &[1],
            &[0],
            &[IntegralShift::try_new([2]).unwrap()],
            "unrelated-scope",
        )],
        RequestedDomainSupportLimits::default(),
    )
    .unwrap();
    let (ledger, census, stop) = run_one_loop(Some(&unrelated), ProbeCampaignLimits::default());
    assert!(matches!(
        stop,
        RequestedProbeCoordinatorStop::Failed(ref failure)
            if matches!(
                failure.failure(),
                ProbeCoordinatorFailure::UnmatchedRequestedSupportScope {
                    support_domains: 1,
                    declared_scopes: 1,
                }
            )
    ));
    assert_eq!(ledger.snapshot().revision().get(), 0);
    assert_eq!(ledger.snapshot().owner_count(), 0);
    assert_eq!(census.epochs_started(), 0);
    assert_eq!(census.plans_built(), 0);
    assert_eq!(census.task_reports(), 0);
    assert_eq!(census.requested_support_assisted(), 0);
    assert_eq!(census.requested_support_fallback(), 0);
}

#[test]
fn matching_scope_with_fully_covered_requests_completes_with_zero_support_hits() {
    let artifact = Arc::new(derive_one_loop_unit_mass_tadpole().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ProbeCampaignLimits::default();
    let zero_sources = generator
        .translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0]).unwrap()],
            limits.replay.scheduler.source_discovery.translation,
        )
        .unwrap();
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
    let already_covered_geometry =
        UncoveredPartition::new(vec![LatticeBox::try_new([0], [Some(0)]).unwrap()], 0);
    let requests = [RequestedDomain::new(
        LatticePoint::try_new([1]).unwrap(),
        [0],
    )];
    let plan = requested_plan(
        ledger.revision().get(),
        ONE_LOOP_SCOPE,
        &sector,
        &already_covered_geometry,
        &requests,
    );
    assert!(plan.tasks().is_empty());
    assert_eq!(plan.fully_covered_domain_count(), 1);
    assert!(plan.declares_scope(ONE_LOOP_SCOPE, &sector));

    let support = one_loop_support();
    let config = ProbeCoordinatorConfig::try_new(
        [
            TaskRelativeModularProbe::try_new(PRIME, [37], [1], limits.replay.scheduler.campaign)
                .unwrap(),
        ],
        1,
        0,
        ProbeCoordinatorLimits::default(),
    )
    .unwrap();
    let baseline = ledger.snapshot();
    let mut coordinator = BoundaryProbeCoordinator::try_new(config, adapter, &ledger).unwrap();
    let RequestedProbeCoordinatorStop::PhaseCompleted {
        census,
        completed_tasks,
        ..
    } = coordinator.try_run_requested_plan_with_support(&plan, &mut ledger, &support)
    else {
        panic!("fully covered support domains must complete without replay")
    };
    assert_eq!(completed_tasks, 0);
    assert_eq!(census.requested_support_assisted(), 0);
    assert_eq!(census.requested_support_fallback(), 0);
    assert_eq!(ledger.snapshot(), baseline);
}

#[test]
fn stale_supported_plan_is_rejected_without_mutating_the_ledger() {
    let fixture = OracleDisabledK6Fixture::shared();
    let limits = ProbeCampaignLimits::default();
    let adapter = ProbeCampaignAdapter::try_new(
        fixture.generator(),
        fixture.completed(),
        fixture.zero_sources(),
        limits,
    )
    .unwrap();
    let mut ledger = fixture.new_ledger();
    let partition = ledger.try_clone_uncovered_partition().unwrap();
    let requests = [RequestedDomain::new(
        LatticePoint::try_new([0, 0, 0, 0, 0, 0]).unwrap(),
        [0, 1, 2, 3, 4, 5],
    )];
    let plan = requested_plan(
        ledger.revision().get(),
        "stale-supported-plan",
        fixture.sector(),
        &partition,
        &requests,
    );
    let union = try_union_requested_domain_support(
        vec![proposal(
            "stale-supported-plan",
            fixture.sector(),
            &[0, 0, 0, 0, 0, 0],
            &[0, 1, 2, 3, 4, 5],
            &[IntegralShift::try_new([0, 0, 0, 0, 0, 0]).unwrap()],
            "stale",
        )],
        RequestedDomainSupportLimits::default(),
    )
    .unwrap();
    let discovery_plan = fixture.plan(&ledger, 2, 0);
    let owner = fixture.replay_owner(&discovery_plan.tasks()[0]);
    ledger.try_apply_owner(owner).unwrap();
    let baseline = ledger.snapshot();
    let config = ProbeCoordinatorConfig::try_new(
        [TaskRelativeModularProbe::try_new(
            PRIME,
            [37],
            [0, 0, 0, 0, 0, 0],
            limits.replay.scheduler.campaign,
        )
        .unwrap()],
        1,
        0,
        ProbeCoordinatorLimits::default(),
    )
    .unwrap();
    let mut coordinator = BoundaryProbeCoordinator::try_new(config, adapter, &ledger).unwrap();
    assert!(matches!(
        coordinator.try_run_requested_plan_with_support(&plan, &mut ledger, &union),
        RequestedProbeCoordinatorStop::Failed(ref stop)
            if matches!(
                stop.failure(),
                ProbeCoordinatorFailure::Campaign(
                    ProbeCampaignError::StaleLedgerRevision { planned: 0, current: 1 }
                )
            )
    ));
    assert_eq!(ledger.snapshot(), baseline);
}

#[test]
fn support_resource_failure_is_transactional() {
    let support = one_loop_support();
    let mut limits = ProbeCampaignLimits::default();
    limits
        .replay
        .scheduler
        .source_discovery
        .max_incidence_visits = 1;
    let (ledger, census, stop) = run_one_loop(Some(&support), limits);
    let initial = ExactOwnerCoverSnapshot::clone(&ledger.snapshot());
    assert!(matches!(stop, RequestedProbeCoordinatorStop::Failed(_)));
    assert_eq!(initial.revision().get(), 0);
    assert_eq!(initial.owner_count(), 0);
    assert_eq!(census.task_reports(), 0);
}
