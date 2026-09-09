use std::sync::Arc;

use crate::family::IntegralKey;
use crate::foundry::artifact::derive_one_loop_unit_mass_tadpole_terminal_authority;
use crate::foundry::completion::LatticeBox;
use crate::foundry::completion::source_discovery::{
    CampaignLimits, CampaignModularProbe, CanonicalExactOwnerLedger, ExactExecutableOwnerProposal,
    ExactOwnerCoverDeltaLimits, ExactRuleCellPromotionDisposition, ExactSemanticExecutableOwner,
    try_compile_single_canonical_probe_executable_owner,
};
use crate::foundry::completion::stratum::{
    DecoratedStratum, ImmutableOwnerSnapshot, StratumRegistryLimits,
};
use crate::identity::{CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator};
use crate::sector::{
    CoordinatePriority, CoordinatePriorityLimits, Mask, OrderingPolicy, SectorMonotoneDomain,
};

use super::super::{SpiredCase, SpiredCoordinateFace, SpiredExecutionCase};
use super::{
    SpiredFixedPointConfig, SpiredFixedPointError, SpiredFixedPointStop, SpiredFixedPointTarget,
    SpiredFixedPointTargetRunner, SpiredTargetPortfolioCensus, SpiredTargetPortfolioDisposition,
    SpiredTargetPortfolioIncompleteReason, SpiredTargetPortfolioReport, SpiredTargetRunnerError,
    try_drive_spired_fixed_point,
};

const PRIME: u64 = 1_000_000_007;

#[derive(Default)]
struct RecordingRunner {
    calls: Vec<Vec<i64>>,
    owner: Option<Arc<ExactSemanticExecutableOwner>>,
    reported_work: SpiredTargetPortfolioCensus,
}

impl SpiredFixedPointTargetRunner for RecordingRunner {
    fn try_run_target_portfolio(
        &mut self,
        target: SpiredFixedPointTarget<'_>,
        _budget: super::SpiredTargetPortfolioBudget,
    ) -> Result<SpiredTargetPortfolioReport, SpiredTargetRunnerError> {
        self.calls
            .push(target.task().target_shift().values().to_vec());
        let disposition = match self.owner.take() {
            Some(owner) => SpiredTargetPortfolioDisposition::CompiledOwner(owner),
            None => SpiredTargetPortfolioDisposition::Incomplete(
                SpiredTargetPortfolioIncompleteReason::AlternativePortfolioExhausted,
            ),
        };
        Ok(SpiredTargetPortfolioReport::new(
            self.reported_work,
            disposition,
        ))
    }
}

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn k1_ledger() -> CanonicalExactOwnerLedger {
    let authority = derive_one_loop_unit_mass_tadpole_terminal_authority().unwrap();
    let generator = ParametricIbpGenerator::try_new(authority.family()).unwrap();
    let predecessor = ImmutableOwnerSnapshot::try_from_terminal_authority(
        Arc::clone(&authority),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    CanonicalExactOwnerLedger::try_new_with_closure_carrier(
        generator.context(),
        predecessor,
        Mask::try_new([true]).unwrap(),
        OrderingPolicy::default(),
        [IntegralKey::try_new([1]).unwrap()],
        LatticeBox::try_new([0], [Some(11)]).unwrap(),
        ExactOwnerCoverDeltaLimits::default(),
    )
    .unwrap()
}

fn k1_ledger_and_owner() -> (CanonicalExactOwnerLedger, Arc<ExactSemanticExecutableOwner>) {
    let authority = derive_one_loop_unit_mass_tadpole_terminal_authority().unwrap();
    let generator = ParametricIbpGenerator::try_new(authority.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let predecessor = ImmutableOwnerSnapshot::try_from_terminal_authority(
        Arc::clone(&authority),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let target = IntegralShift::try_new([1]).unwrap();
    let structural_shifts = completed.relations()[0]
        .terms()
        .keys()
        .map(|shift| shift.values())
        .collect::<Vec<_>>();
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        Mask::try_new([true]).unwrap(),
        target.values(),
        &structural_shifts,
    )
    .unwrap();
    let stratum = DecoratedStratum::try_guard_blind(
        predecessor.family_fingerprint(),
        predecessor.context_fingerprint(),
        domain,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let case = SpiredExecutionCase::try_new(
        SpiredCase::CoordinateFace(SpiredCoordinateFace::new(stratum)),
        target,
        OrderingPolicy::default(),
        predecessor.clone(),
    )
    .unwrap();
    let probe = CampaignModularProbe::try_new(PRIME, [37], [2], CampaignLimits::default()).unwrap();
    let mut run_limits = super::super::SpiredTargetRunLimits::default();
    run_limits.max_depth_inclusive = 0;
    run_limits.request_chunk_size = 1;
    let report =
        super::super::try_run_spired_target(&generator, &completed, &case, &probe, run_limits)
            .unwrap();
    let super::super::SpiredTargetRunOutcome::RuleCell(
        ExactRuleCellPromotionDisposition::Admitted(admitted),
    ) = report.into_outcome()
    else {
        panic!("the ordinary K1 depth-zero lane must admit one exact rule")
    };
    let ExactExecutableOwnerProposal::Compiled {
        owner,
        obstructions,
    } = try_compile_single_canonical_probe_executable_owner(
        generator.context(),
        admitted,
        Default::default(),
    )
    .unwrap()
    else {
        panic!("one admitted K1 rule must compile")
    };
    assert!(obstructions.is_empty());
    let ledger = CanonicalExactOwnerLedger::try_new_with_closure_carrier(
        generator.context(),
        predecessor,
        Mask::try_new([true]).unwrap(),
        OrderingPolicy::default(),
        [IntegralKey::try_new([1]).unwrap()],
        LatticeBox::try_new([0], [Some(11)]).unwrap(),
        ExactOwnerCoverDeltaLimits::default(),
    )
    .unwrap();
    (ledger, owner)
}

#[test]
fn bounded_program_skips_the_explicit_terminal_and_reports_incompleteness() {
    let mut ledger = k1_ledger();
    let mut runner = RecordingRunner {
        reported_work: SpiredTargetPortfolioCensus {
            target_lane_runs: 1,
            probe_attempts: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    let report =
        try_drive_spired_fixed_point(&mut ledger, &mut runner, &SpiredFixedPointConfig::default())
            .unwrap();
    assert_eq!(runner.calls, vec![vec![1]]);
    assert!(matches!(
        report.stop(),
        SpiredFixedPointStop::Incomplete { .. }
    ));
    assert_eq!(report.census().epochs(), 1);
    assert_eq!(report.census().plans(), 1);
    assert_eq!(report.census().planned_targets(), 2);
    assert_eq!(report.census().targets_inspected(), 2);
    assert_eq!(report.census().terminal_targets_skipped(), 1);
    assert_eq!(report.census().target_portfolios(), 1);
    assert_eq!(report.census().incomplete_target_portfolios(), 1);
    ledger
        .try_require_current_snapshot(report.stop().authority())
        .unwrap();
}

#[test]
fn real_k1_owner_closes_only_through_the_exact_ledger_compiler() {
    let (mut ledger, owner) = k1_ledger_and_owner();
    let mut runner = RecordingRunner {
        owner: Some(owner),
        reported_work: SpiredTargetPortfolioCensus {
            target_lane_runs: 1,
            probe_attempts: 1,
            modular_hits: 1,
            exact_lift_attempts: 1,
            owner_compile_attempts: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    let report =
        try_drive_spired_fixed_point(&mut ledger, &mut runner, &SpiredFixedPointConfig::default())
            .unwrap();
    assert_eq!(runner.calls, vec![vec![1]]);
    assert!(matches!(
        report.stop(),
        SpiredFixedPointStop::CompilerClosed { .. }
    ));
    assert_eq!(report.census().semantic_owner_mutations(), 1);
    assert_eq!(report.census().strict_geometric_shrinks(), 1);
    ledger
        .try_require_current_snapshot(report.stop().authority())
        .unwrap();
    assert!(report.stop().snapshot().status().is_compiler_closed());
}

#[test]
fn zero_target_portfolio_budget_is_a_typed_incomplete_stop() {
    let mut ledger = k1_ledger();
    let mut runner = RecordingRunner::default();
    let mut config = SpiredFixedPointConfig::default();
    config.limits.max_target_portfolios = 0;
    let report = try_drive_spired_fixed_point(&mut ledger, &mut runner, &config).unwrap();
    assert!(runner.calls.is_empty());
    assert!(matches!(
        report.stop(),
        SpiredFixedPointStop::Incomplete {
            reason: super::SpiredFixedPointIncompleteReason::ResourceLimit {
                resource: "target portfolios",
                requested: 1,
                limit: 0,
            },
            ..
        }
    ));
}

#[test]
fn zero_plan_budget_stops_before_invoking_a_zero_capacity_planner() {
    let mut ledger = k1_ledger();
    let mut runner = RecordingRunner::default();
    let mut config = SpiredFixedPointConfig::default();
    config.limits.max_plans = 0;
    // If fixed-point planning ran before the aggregate preflight, this nested
    // limit would report a different resource first.
    config.leader_walk.max_scopes = 0;
    let report = try_drive_spired_fixed_point(&mut ledger, &mut runner, &config).unwrap();
    assert!(runner.calls.is_empty());
    assert_eq!(report.census().plans(), 0);
    assert_eq!(report.census().planned_targets(), 0);
    assert!(matches!(
        report.stop(),
        SpiredFixedPointStop::Incomplete {
            reason: super::SpiredFixedPointIncompleteReason::ResourceLimit {
                resource: "leader plans",
                requested: 1,
                limit: 0,
            },
            ..
        }
    ));
}

#[test]
fn zero_planned_target_budget_stops_before_geometry_is_frozen() {
    let mut ledger = k1_ledger();
    let mut runner = RecordingRunner::default();
    let mut config = SpiredFixedPointConfig::default();
    config.limits.max_planned_targets = 0;
    // This would fail first if the leader planner were entered.
    config.leader_walk.max_scopes = 0;
    let report = try_drive_spired_fixed_point(&mut ledger, &mut runner, &config).unwrap();
    assert!(runner.calls.is_empty());
    assert_eq!(report.census().plans(), 0);
    assert_eq!(report.census().planned_targets(), 0);
    assert!(matches!(
        report.stop(),
        SpiredFixedPointStop::Incomplete {
            reason: super::SpiredFixedPointIncompleteReason::ResourceLimit {
                resource: "planned targets",
                requested: 1,
                limit: 0,
            },
            ..
        }
    ));
}

#[test]
fn planned_target_cap_is_forwarded_to_the_local_leader_plan() {
    let mut ledger = k1_ledger();
    let mut runner = RecordingRunner::default();
    let mut config = SpiredFixedPointConfig::default();
    config.limits.max_plans = 1;
    // K1's complete lower-corner plus depth-one census contains two tasks.
    config.limits.max_planned_targets = 1;
    let report = try_drive_spired_fixed_point(&mut ledger, &mut runner, &config).unwrap();
    assert!(runner.calls.is_empty());
    assert_eq!(report.census().plans(), 1);
    assert_eq!(report.census().planned_targets(), 0);
    assert!(matches!(
        report.stop(),
        SpiredFixedPointStop::Incomplete {
            reason: super::SpiredFixedPointIncompleteReason::ResourceLimit {
                resource: "tasks across both waves",
                requested: 2,
                limit: 1,
            },
            ..
        }
    ));
}

#[test]
fn exact_plan_and_target_boundaries_admit_the_complete_seed_program() {
    let mut ledger = k1_ledger();
    let mut runner = RecordingRunner::default();
    let mut config = SpiredFixedPointConfig::default();
    config.limits.max_plans = 1;
    config.limits.max_planned_targets = 2;
    let report = try_drive_spired_fixed_point(&mut ledger, &mut runner, &config).unwrap();
    assert_eq!(runner.calls, vec![vec![1]]);
    assert_eq!(report.census().plans(), 1);
    assert_eq!(report.census().planned_targets(), 2);
}

#[test]
fn runner_cannot_hide_work_beyond_the_aggregate_budget() {
    let mut ledger = k1_ledger();
    let mut runner = RecordingRunner {
        reported_work: SpiredTargetPortfolioCensus {
            target_lane_runs: 2,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut config = SpiredFixedPointConfig::default();
    config.limits.max_target_lane_runs = 1;
    assert!(matches!(
        try_drive_spired_fixed_point(&mut ledger, &mut runner, &config),
        Err(SpiredFixedPointError::RunnerExceededBudget {
            resource: "target lane runs",
            reported: 2,
            remaining: 1,
        })
    ));
}

#[test]
fn discovery_priority_is_checked_against_the_live_ledger_arity() {
    let mut ledger = k1_ledger();
    let mut runner = RecordingRunner::default();
    let mut config = SpiredFixedPointConfig::default();
    config.discovery_coordinate_priority =
        Some(CoordinatePriority::try_new(2, &[1, 0], CoordinatePriorityLimits::default()).unwrap());
    assert!(matches!(
        try_drive_spired_fixed_point(&mut ledger, &mut runner, &config),
        Err(SpiredFixedPointError::WrongCoordinatePriorityArity {
            expected: 1,
            actual: 2,
        })
    ));
}
