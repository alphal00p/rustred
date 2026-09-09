use std::sync::Arc;

use crate::family::IntegralKey;
use crate::foundry::artifact::{
    ClosedTerminalAuthority, derive_one_loop_unit_mass_tadpole_terminal_authority,
};
use crate::foundry::completion::LatticeBox;
use crate::foundry::completion::source_discovery::{
    CampaignLimits, CampaignModularProbe, CanonicalExactOwnerLedger, ExactOwnerCoverDeltaKind,
    ExactOwnerCoverDeltaLimits, ExactOwnerLedgerCoverStatus,
};
use crate::foundry::completion::stratum::{
    DecoratedStratum, ImmutableOwnerSnapshot, StratumRegistryLimits,
};
use crate::identity::{CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator};
use crate::sector::{Mask, OrderingPolicy, SectorMonotoneDomain};

use super::super::{
    SpiredCoordinateCaseEnqueueOutcome, SpiredCoordinateCaseObligation,
    SpiredCoordinateCaseWorklist, SpiredCoordinateCaseWorklistLimits,
};
use super::{
    SpiredEqualityStepIncomplete, SpiredEqualityStepLimits, SpiredEqualityStepOutcome,
    SpiredEqualityStepProbeIncomplete, try_advance_spired_coordinate_case,
};

const PRIME_A: u64 = 1_000_000_007;
const PRIME_B: u64 = 1_000_000_021;

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn root_obligation(
    artifact: &ClosedTerminalAuthority,
    completed: &CompletedIbpSourceRows,
) -> SpiredCoordinateCaseObligation {
    let zero = IntegralShift::try_new([0]).unwrap();
    let structural_shifts = completed.relations()[0]
        .terms()
        .keys()
        .map(|shift| shift.values())
        .collect::<Vec<_>>();
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        Mask::try_new([true]).unwrap(),
        zero.values(),
        &structural_shifts,
    )
    .unwrap();
    let stratum = DecoratedStratum::try_guard_blind(
        artifact.family_fingerprint(),
        artifact.context_fingerprint(),
        domain,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    SpiredCoordinateCaseObligation::try_new_root(stratum).unwrap()
}

fn predecessor(artifact: &Arc<ClosedTerminalAuthority>) -> ImmutableOwnerSnapshot {
    ImmutableOwnerSnapshot::try_from_terminal_authority(
        Arc::clone(artifact),
        StratumRegistryLimits::default(),
    )
    .unwrap()
}

fn worklist(
    root: SpiredCoordinateCaseObligation,
    limits: SpiredCoordinateCaseWorklistLimits,
) -> SpiredCoordinateCaseWorklist {
    let mut worklist = SpiredCoordinateCaseWorklist::new(limits);
    assert_eq!(
        worklist.try_enqueue(root).unwrap(),
        SpiredCoordinateCaseEnqueueOutcome::Inserted {
            removed_subsumed: 0,
        }
    );
    worklist
}

fn ledger(
    generator: &ParametricIbpGenerator<'_>,
    predecessor: ImmutableOwnerSnapshot,
    terminals: impl IntoIterator<Item = IntegralKey>,
) -> CanonicalExactOwnerLedger {
    CanonicalExactOwnerLedger::try_new_with_closure_carrier(
        generator.context(),
        predecessor,
        Mask::try_new([true]).unwrap(),
        OrderingPolicy::default(),
        terminals,
        LatticeBox::try_new([0], [Some(11)]).unwrap(),
        ExactOwnerCoverDeltaLimits::default(),
    )
    .unwrap()
}

fn probe(modulus: u64, chart: u64) -> CampaignModularProbe {
    CampaignModularProbe::try_new(modulus, [37], [chart], CampaignLimits::default()).unwrap()
}

fn closing_limits() -> SpiredEqualityStepLimits {
    let mut limits = SpiredEqualityStepLimits::default();
    limits.target_run.max_depth_inclusive = 1;
    limits.target_run.request_chunk_size = 1;
    limits.target_run.max_post_hit_streamed_rows = 0;
    limits
}

#[test]
fn k_case_is_untouched_when_the_finite_probe_portfolio_has_no_hit() {
    let artifact = derive_one_loop_unit_mass_tadpole_terminal_authority().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let root = root_obligation(&artifact, &completed);
    let expected = root.clone();
    let mut worklist = worklist(root, Default::default());
    let mut ledger = ledger(&generator, predecessor(&artifact), []);
    let worklist_revision = worklist.revision();
    let worklist_census = worklist.census();
    let ledger_revision = ledger.revision();
    let ledger_snapshot = ledger.snapshot();
    let mut limits = closing_limits();
    limits.target_run.max_depth_inclusive = 0;
    let probes = [probe(PRIME_A, 2), probe(PRIME_B, 3)];

    let report = try_advance_spired_coordinate_case(
        &generator,
        &completed,
        &probes,
        &mut worklist,
        &mut ledger,
        limits,
    )
    .unwrap();

    assert_eq!(report.census().probes_offered(), 2);
    assert_eq!(report.census().probes_attempted(), 2);
    assert!(matches!(
        report.outcome(),
        SpiredEqualityStepOutcome::Incomplete(
            SpiredEqualityStepIncomplete::ProbePortfolioExhausted {
                probes_attempted: 2,
                last: Some(SpiredEqualityStepProbeIncomplete::SearchDepthExhausted),
            }
        )
    ));
    assert_eq!(worklist.revision(), worklist_revision);
    assert_eq!(worklist.census(), worklist_census);
    assert_eq!(worklist.current(), Some(&expected));
    assert_eq!(ledger.revision(), ledger_revision);
    assert_eq!(ledger.snapshot(), ledger_snapshot);
}

#[test]
fn admitted_rule_atomically_replaces_parent_by_its_exact_guard_zero_child() {
    let artifact = derive_one_loop_unit_mass_tadpole_terminal_authority().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let root = root_obligation(&artifact, &completed);
    assert_eq!(root.free_dimension(), 1);
    let mut worklist = worklist(root, Default::default());
    let mut ledger = ledger(&generator, predecessor(&artifact), []);

    let report = try_advance_spired_coordinate_case(
        &generator,
        &completed,
        &[probe(PRIME_A, 2)],
        &mut worklist,
        &mut ledger,
        closing_limits(),
    )
    .unwrap();
    let SpiredEqualityStepOutcome::Committed(committed) = report.into_outcome() else {
        panic!("the canonical K1 equality case must commit its exact recurrence")
    };

    assert_eq!(
        committed.owner_delta().kind(),
        ExactOwnerCoverDeltaKind::StrictGeometricShrink
    );
    assert!(!committed.discharged_by_compiler_cover());
    assert_eq!(committed.proposed_guard_children(), 1);
    assert_eq!(committed.retained_pending_cases(), 1);
    assert_eq!(ledger.owners().len(), 1);
    assert!(matches!(
        ledger.snapshot().status(),
        ExactOwnerLedgerCoverStatus::Compiled(_)
    ));
    let child = worklist.current().expect("one guard-zero child remains");
    assert_eq!(child.free_dimension(), 0);
    assert_eq!(child.stratum().domain().bounds()[0].lower(), 1);
    assert_eq!(child.stratum().domain().bounds()[0].upper(), 1);
}

#[test]
fn duplicate_owner_does_not_retire_a_reintroduced_live_case() {
    let artifact = derive_one_loop_unit_mass_tadpole_terminal_authority().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let root = root_obligation(&artifact, &completed);
    let mut worklist = worklist(root.clone(), Default::default());
    let mut ledger = ledger(&generator, predecessor(&artifact), []);

    let first = try_advance_spired_coordinate_case(
        &generator,
        &completed,
        &[probe(PRIME_A, 2)],
        &mut worklist,
        &mut ledger,
        closing_limits(),
    )
    .unwrap();
    assert!(matches!(
        first.outcome(),
        SpiredEqualityStepOutcome::Committed(_)
    ));
    assert_eq!(
        worklist.try_enqueue(root.clone()).unwrap(),
        SpiredCoordinateCaseEnqueueOutcome::Inserted {
            removed_subsumed: 1,
        }
    );
    assert_eq!(worklist.current(), Some(&root));
    let worklist_revision = worklist.revision();
    let worklist_census = worklist.census();
    let ledger_revision = ledger.revision();
    let ledger_snapshot = ledger.snapshot();

    let second = try_advance_spired_coordinate_case(
        &generator,
        &completed,
        &[probe(PRIME_A, 2)],
        &mut worklist,
        &mut ledger,
        closing_limits(),
    )
    .unwrap();

    assert!(matches!(
        second.outcome(),
        SpiredEqualityStepOutcome::Incomplete(
            SpiredEqualityStepIncomplete::OwnerMadeNoGeometricProgress {
                kind: ExactOwnerCoverDeltaKind::Duplicate,
                ..
            }
        )
    ));
    assert_eq!(worklist.revision(), worklist_revision);
    assert_eq!(worklist.census(), worklist_census);
    assert_eq!(worklist.current(), Some(&root));
    assert_eq!(ledger.revision(), ledger_revision);
    assert_eq!(ledger.snapshot(), ledger_snapshot);
    assert_eq!(ledger.owners().len(), 1);
}

#[test]
fn diagnostic_carrier_closure_does_not_discharge_global_guard_children() {
    let artifact = derive_one_loop_unit_mass_tadpole_terminal_authority().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let root = root_obligation(&artifact, &completed);
    let mut worklist = worklist(root, Default::default());
    let mut ledger = ledger(
        &generator,
        predecessor(&artifact),
        [IntegralKey::try_new([1]).unwrap()],
    );

    let report = try_advance_spired_coordinate_case(
        &generator,
        &completed,
        &[probe(PRIME_A, 2)],
        &mut worklist,
        &mut ledger,
        closing_limits(),
    )
    .unwrap();
    let SpiredEqualityStepOutcome::Committed(committed) = report.into_outcome() else {
        panic!("the terminal-backed K1 equality case must commit")
    };

    assert!(
        committed
            .owner_delta()
            .updated()
            .status()
            .is_compiler_closed()
    );
    assert!(!committed.discharged_by_compiler_cover());
    assert_eq!(committed.proposed_guard_children(), 1);
    assert_eq!(worklist.len(), 1);
}

#[test]
fn replacement_preflight_failure_rolls_back_a_fully_prepared_owner_mutation() {
    let artifact = derive_one_loop_unit_mass_tadpole_terminal_authority().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let root = root_obligation(&artifact, &completed);
    let expected = root.clone();
    let mut worklist_limits = SpiredCoordinateCaseWorklistLimits::default();
    worklist_limits.max_retired_cases = 0;
    let mut worklist = worklist(root, worklist_limits);
    let mut ledger = ledger(&generator, predecessor(&artifact), []);
    let worklist_revision = worklist.revision();
    let worklist_census = worklist.census();
    let ledger_revision = ledger.revision();
    let ledger_snapshot = ledger.snapshot();

    let error = try_advance_spired_coordinate_case(
        &generator,
        &completed,
        &[probe(PRIME_A, 2)],
        &mut worklist,
        &mut ledger,
        closing_limits(),
    )
    .unwrap_err();

    assert_eq!(error.census().admitted_candidates(), 1);
    assert_eq!(error.census().exact_guard_cases(), 1);
    assert_eq!(error.census().owner_compile_attempts(), 1);
    assert_eq!(worklist.revision(), worklist_revision);
    assert_eq!(worklist.census(), worklist_census);
    assert_eq!(worklist.current(), Some(&expected));
    assert_eq!(ledger.revision(), ledger_revision);
    assert_eq!(ledger.snapshot(), ledger_snapshot);
    assert!(ledger.owners().is_empty());
}

#[test]
fn probe_census_limit_fails_before_workspace_or_live_state_changes() {
    let artifact = derive_one_loop_unit_mass_tadpole_terminal_authority().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let root = root_obligation(&artifact, &completed);
    let expected = root.clone();
    let mut worklist = worklist(root, Default::default());
    let mut ledger = ledger(&generator, predecessor(&artifact), []);
    let worklist_revision = worklist.revision();
    let ledger_revision = ledger.revision();
    let mut limits = closing_limits();
    limits.max_probes = 0;

    let error = try_advance_spired_coordinate_case(
        &generator,
        &completed,
        &[probe(PRIME_A, 2)],
        &mut worklist,
        &mut ledger,
        limits,
    )
    .unwrap_err();

    assert_eq!(error.census().probes_offered(), 1);
    assert_eq!(error.census().probes_attempted(), 0);
    assert_eq!(worklist.revision(), worklist_revision);
    assert_eq!(worklist.current(), Some(&expected));
    assert_eq!(ledger.revision(), ledger_revision);
}
