// Integration tests for the serial equality-case driver live here.
use std::sync::Arc;

use crate::family::IntegralKey;
use crate::foundry::artifact::{
    ClosedTerminalAuthority, derive_one_loop_unit_mass_tadpole_terminal_authority,
};
use crate::foundry::completion::LatticeBox;
use crate::foundry::completion::source_discovery::{
    CanonicalExactOwnerLedger, ExactOwnerCoverDeltaLimits,
};
use crate::foundry::completion::stratum::{
    DecoratedStratum, ImmutableOwnerSnapshot, StratumRegistryLimits,
};
use crate::identity::{CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator};
use crate::sector::{InteriorBounds, Mask, OrderingPolicy, SectorMonotoneDomain};

use super::super::{SpiredCoordinateCaseObligation, SpiredEqualityStepIncomplete};
use super::{
    SpiredExistingTerminalAuthority, SpiredSerialCaseIncomplete, SpiredSerialCaseOutcome,
    SpiredSerialDriverConfig, SpiredSerialDriverStop, SpiredSerialFiniteResidualReason,
    SpiredSerialProbePortfolio, SpiredSerialProbePortfolioLimits, SpiredSerialResumePolicy,
    try_drive_spired_serial_coordinate_cases, try_initialize_spired_serial_worklist,
};

const PRIME: u64 = 1_000_000_007;

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn predecessor(authority: &Arc<ClosedTerminalAuthority>) -> ImmutableOwnerSnapshot {
    ImmutableOwnerSnapshot::try_from_terminal_authority(
        Arc::clone(authority),
        StratumRegistryLimits::default(),
    )
    .unwrap()
}

fn root(
    authority: &ClosedTerminalAuthority,
    completed: &CompletedIbpSourceRows,
) -> SpiredCoordinateCaseObligation {
    let zero = IntegralShift::try_new([0]).unwrap();
    let structural = completed.relations()[0]
        .terms()
        .keys()
        .map(|shift| shift.values())
        .collect::<Vec<_>>();
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        Mask::try_new([true]).unwrap(),
        zero.values(),
        &structural,
    )
    .unwrap();
    SpiredCoordinateCaseObligation::try_new_root(
        DecoratedStratum::try_guard_blind(
            authority.family_fingerprint(),
            authority.context_fingerprint(),
            domain,
            StratumRegistryLimits::default(),
        )
        .unwrap(),
    )
    .unwrap()
}

fn singleton(authority: &ClosedTerminalAuthority, value: i64) -> SpiredCoordinateCaseObligation {
    let domain = SectorMonotoneDomain::try_new_for_rule(
        Mask::try_new([true]).unwrap(),
        [InteriorBounds::new(value, value)],
        &[0],
        &[] as &[&[i64]],
    )
    .unwrap();
    SpiredCoordinateCaseObligation::try_new_root(
        DecoratedStratum::try_guard_blind(
            authority.family_fingerprint(),
            authority.context_fingerprint(),
            domain,
            StratumRegistryLimits::default(),
        )
        .unwrap(),
    )
    .unwrap()
}

fn ledger(
    generator: &ParametricIbpGenerator<'_>,
    predecessor: ImmutableOwnerSnapshot,
) -> CanonicalExactOwnerLedger {
    CanonicalExactOwnerLedger::try_new(
        generator.context(),
        predecessor,
        Mask::try_new([true]).unwrap(),
        OrderingPolicy::default(),
        [],
        ExactOwnerCoverDeltaLimits::default(),
    )
    .unwrap()
}

fn source_safe_ledger(
    generator: &ParametricIbpGenerator<'_>,
    predecessor: ImmutableOwnerSnapshot,
) -> CanonicalExactOwnerLedger {
    // This deliberately retained finite carrier is wholly inside the K1
    // source-translation interior. Closure below is therefore a claim about
    // exactly this carrier, not about the remote i64 representability edge.
    CanonicalExactOwnerLedger::try_new_with_closure_carrier(
        generator.context(),
        predecessor,
        Mask::try_new([true]).unwrap(),
        OrderingPolicy::default(),
        [],
        LatticeBox::try_new([0], [Some(11)]).unwrap(),
        ExactOwnerCoverDeltaLimits::default(),
    )
    .unwrap()
}

fn portfolio() -> SpiredSerialProbePortfolio {
    SpiredSerialProbePortfolio::try_new(
        [PRIME],
        [[37]],
        [[2]],
        SpiredSerialProbePortfolioLimits::default(),
    )
    .unwrap()
}

fn closing_config() -> SpiredSerialDriverConfig {
    let mut config = SpiredSerialDriverConfig::default();
    config.equality_step.target_run.max_depth_inclusive = 1;
    config.equality_step.target_run.request_chunk_size = 1;
    config.equality_step.target_run.max_post_hit_streamed_rows = 0;
    config
}

#[test]
fn k1_root_closes_end_to_end_through_the_serial_driver() {
    let authority = derive_one_loop_unit_mass_tadpole_terminal_authority().unwrap();
    let generator = ParametricIbpGenerator::try_new(authority.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let root = root(&authority, &completed);
    let mut worklist = try_initialize_spired_serial_worklist([root], Default::default()).unwrap();
    let mut ledger = source_safe_ledger(&generator, predecessor(&authority));

    let report = try_drive_spired_serial_coordinate_cases(
        &generator,
        &completed,
        &portfolio(),
        &mut worklist,
        &mut ledger,
        closing_config(),
    )
    .unwrap();

    assert!(
        matches!(
            report.stop(),
            SpiredSerialDriverStop::BoundedCarrierClosed { .. }
        ),
        "unexpected K1 stop: {:?}",
        report.stop()
    );
    assert_eq!(report.census().rule_commits(), 1);
    assert_eq!(report.census().positive_dimensional_case_attempts(), 1);
    assert_eq!(report.census().generated_probes(), 1);
    assert!(ledger.snapshot().status().is_compiler_closed());
    assert!(worklist.is_empty());
    assert!(matches!(
        report.cases()[0].outcome(),
        SpiredSerialCaseOutcome::RuleCommitted(_)
    ));
}

#[test]
fn full_i64_carrier_boundary_is_incomplete_and_never_invented_as_a_terminal() {
    let authority = derive_one_loop_unit_mass_tadpole_terminal_authority().unwrap();
    let generator = ParametricIbpGenerator::try_new(authority.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let root = root(&authority, &completed);
    let mut worklist = try_initialize_spired_serial_worklist([root], Default::default()).unwrap();
    let mut ledger = ledger(&generator, predecessor(&authority));

    let report = try_drive_spired_serial_coordinate_cases(
        &generator,
        &completed,
        &portfolio(),
        &mut worklist,
        &mut ledger,
        closing_config(),
    )
    .unwrap();

    let remote_boundary = IntegralKey::try_new([i64::MAX]).unwrap();
    assert!(worklist.is_empty());
    assert!(!ledger.snapshot().status().is_compiler_closed());
    assert!(!ledger.has_explicit_terminal(&remote_boundary));
    assert_eq!(report.census().finite_residual_reconciliations(), 1);
    assert!(
        matches!(
            report.stop(),
            SpiredSerialDriverStop::FiniteResidualIncomplete {
                integrals,
                reason: SpiredSerialFiniteResidualReason::OutsideRetainedDeclaredCarrier,
                ..
            } if integrals.contains(&remote_boundary)
        ),
        "unexpected full-carrier stop: {:?}",
        report.stop()
    );
}

#[test]
fn bounded_no_hit_retains_the_exact_current_case_for_explicit_restart() {
    let authority = derive_one_loop_unit_mass_tadpole_terminal_authority().unwrap();
    let generator = ParametricIbpGenerator::try_new(authority.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let root = root(&authority, &completed);
    let expected = root.clone();
    let mut worklist = try_initialize_spired_serial_worklist([root], Default::default()).unwrap();
    let worklist_revision = worklist.revision();
    let mut ledger = ledger(&generator, predecessor(&authority));
    let ledger_revision = ledger.revision();
    let mut config = closing_config();
    config.equality_step.target_run.max_depth_inclusive = 0;

    let report = try_drive_spired_serial_coordinate_cases(
        &generator,
        &completed,
        &portfolio(),
        &mut worklist,
        &mut ledger,
        config,
    )
    .unwrap();

    assert_eq!(worklist.current(), Some(&expected));
    assert_eq!(worklist.revision(), worklist_revision);
    assert_eq!(ledger.revision(), ledger_revision);
    assert!(matches!(
        report.stop(),
        SpiredSerialDriverStop::CurrentCaseIncomplete {
            reason: SpiredSerialCaseIncomplete::EqualityStep(
                SpiredEqualityStepIncomplete::ProbePortfolioExhausted { .. }
            ),
            resume:
                SpiredSerialResumePolicy::RestartCurrentCaseFromFirstConfiguredDiagonalAndDepthZero,
            ..
        }
    ));
}

#[test]
fn authenticated_zero_dimensional_terminal_is_retired_but_not_invented() {
    let authority = derive_one_loop_unit_mass_tadpole_terminal_authority().unwrap();
    let generator = ParametricIbpGenerator::try_new(authority.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let mut worklist =
        try_initialize_spired_serial_worklist([singleton(&authority, 1)], Default::default())
            .unwrap();
    let mut ledger = ledger(&generator, predecessor(&authority));

    let report = try_drive_spired_serial_coordinate_cases(
        &generator,
        &completed,
        &portfolio(),
        &mut worklist,
        &mut ledger,
        closing_config(),
    )
    .unwrap();

    assert!(worklist.is_empty());
    assert_eq!(report.census().existing_terminal_retirements(), 1);
    assert_eq!(report.census().predecessor_terminals_retained(), 1);
    assert!(ledger.has_explicit_terminal(&IntegralKey::try_new([1]).unwrap()));
    assert!(matches!(
        report.cases()[0].outcome(),
        SpiredSerialCaseOutcome::ExistingTerminalRetired {
            authority: SpiredExistingTerminalAuthority::AuthenticatedByPredecessor,
            ..
        }
    ));
}

#[test]
fn unknown_zero_dimensional_case_remains_pending_without_terminal_invention() {
    let authority = derive_one_loop_unit_mass_tadpole_terminal_authority().unwrap();
    let generator = ParametricIbpGenerator::try_new(authority.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let pending = singleton(&authority, 2);
    let mut worklist =
        try_initialize_spired_serial_worklist([pending.clone()], Default::default()).unwrap();
    let mut ledger = ledger(&generator, predecessor(&authority));

    let report = try_drive_spired_serial_coordinate_cases(
        &generator,
        &completed,
        &portfolio(),
        &mut worklist,
        &mut ledger,
        closing_config(),
    )
    .unwrap();

    assert_eq!(worklist.current(), Some(&pending));
    assert!(!ledger.has_explicit_terminal(&IntegralKey::try_new([2]).unwrap()));
    assert!(matches!(
        report.stop(),
        SpiredSerialDriverStop::CurrentCaseIncomplete {
            reason: SpiredSerialCaseIncomplete::ZeroDimensionalCaseNeedsDeclaredTerminal {
                integral
            },
            ..
        } if integral == &IntegralKey::try_new([2]).unwrap()
    ));
}
