//! Oracle-disabled K=6 proof that one replayed interior owner removes an
//! exact positive-dimensional part of the initial blind orthant.

use std::sync::Arc;

use crate::family::IntegralKey;
use crate::foundry::artifact::{
    FULL_RANK_ORBITS, canonical_three_loop_family, derive_k6_terminal_authority,
};
use crate::foundry::completion::frame::admission::{
    ExactOwnerCoverObstructionKind, ExactOwnerCoverStatus,
};
use crate::foundry::completion::source_discovery::interior_replay::{
    InteriorReplayRunDisposition, InteriorReplayRunLimits, try_run_interior_replay_task,
};
use crate::foundry::completion::source_discovery::interior_simplex::{
    InteriorSimplexLimits, InteriorSimplexScopePartition, InteriorSimplexTask,
    try_plan_interior_simplex_samples,
};
use crate::foundry::completion::source_discovery::{
    CampaignModularProbe, ExactExecutableOwnerProposal, ExactSemanticExecutableOwner,
    OrdinarySourceIncidenceIndex,
};
use crate::foundry::completion::stratum::{
    DecoratedStratum, ImmutableOwnerSnapshot, MaximalStratumAnchor,
};
use crate::identity::{CompletedIbpSourceRows, ParametricIbpConfig, ParametricIbpGenerator};
use crate::sector::{Mask, OrderingPolicy, SectorMonotoneDomain};

use super::super::{
    CanonicalExactOwnerLedger, ExactOwnerCoverDelta, ExactOwnerCoverDeltaKind,
    ExactOwnerCoverDeltaLimits, ExactOwnerLedgerCoverStatus,
};

const PRIME: u64 = 1_000_000_007;
const DIMENSION_SAMPLE: i64 = 37;

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn bootstrap_anchor(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    task: &InteriorSimplexTask,
    limits: InteriorReplayRunLimits,
) -> MaximalStratumAnchor {
    let zero_sources = generator
        .translate_completed_source_rows(
            completed,
            [crate::identity::IntegralShift::try_new(std::iter::repeat_n(
                0,
                task.key().sector().arity(),
            ))
            .unwrap()],
            limits.scheduler.source_discovery.translation,
        )
        .unwrap();
    let incidence =
        OrdinarySourceIncidenceIndex::try_new(&zero_sources, limits.scheduler.source_discovery)
            .unwrap();
    let nominations = incidence
        .try_nominate_target_unit(task.target_shift(), limits.scheduler.source_discovery)
        .unwrap();
    let selected = generator
        .translate_selected_completed_source_rows(
            completed,
            nominations.requests().iter().cloned(),
            limits.scheduler.campaign.translated_sources,
        )
        .unwrap();
    let physical_shifts = selected
        .sources()
        .iter()
        .flat_map(|source| source.terms().keys())
        .map(|shift| shift.values())
        .collect::<Vec<_>>();
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        task.key().sector().clone(),
        task.target_shift().values(),
        &physical_shifts,
    )
    .unwrap();
    let stratum = DecoratedStratum::try_guard_blind(
        selected.family_fingerprint(),
        generator.context().fingerprint(),
        domain,
        limits.scheduler.campaign.stratum,
    )
    .unwrap();
    MaximalStratumAnchor::try_new(stratum, limits.scheduler.campaign.stratum).unwrap()
}

fn replay_owner(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    task: &InteriorSimplexTask,
    predecessor: ImmutableOwnerSnapshot,
) -> Arc<ExactSemanticExecutableOwner> {
    let mut limits = InteriorReplayRunLimits::default();
    limits.scheduler.max_probes = 1;
    limits.scheduler.max_retained_outcomes = 1;
    limits.scheduler.max_iterations_per_probe = 1;
    limits.scheduler.max_aggregate_epochs = 1;
    limits.scheduler.max_retained_iteration_records = 1;
    limits.scheduler.max_exact_lift_attempts = 1;
    limits.canonical_replay.campaign = limits.scheduler.campaign;
    limits.canonical_replay.source_discovery = limits.scheduler.source_discovery;
    let anchor = bootstrap_anchor(generator, completed, task, limits);
    let probe = CampaignModularProbe::try_new(
        PRIME,
        [DIMENSION_SAMPLE],
        task.lattice_target().iter().copied(),
        limits.scheduler.campaign,
    )
    .unwrap();
    let report = try_run_interior_replay_task(
        generator,
        completed,
        task.target_shift().clone(),
        anchor,
        predecessor,
        OrderingPolicy::default(),
        [probe],
        limits,
    )
    .unwrap();
    let scheduler = report.scheduler();
    assert_eq!(scheduler.epochs(), 1);
    assert_eq!(scheduler.exact_lift_attempts(), 1);
    let outcomes = report.scheduler_outcomes();
    assert_eq!(outcomes.replayed(), 1);
    assert_eq!(outcomes.support_did_not_lift(), 0);
    assert_eq!(outcomes.exact_lift_error(), 0);
    assert_eq!(outcomes.sampled_dual(), 0);
    assert_eq!(outcomes.budget_stop(), 0);
    assert_eq!(outcomes.rejected(), 0);
    assert_eq!(outcomes.stalled(), 0);
    let replay = report
        .replay()
        .expect("the exact proposal must retain replay telemetry");
    assert_eq!(replay.replayed_nominations(), 1);
    assert_eq!(replay.rebase_attempts(), 1);
    assert_eq!(replay.successful_exact_lifts(), 1);
    assert_eq!(replay.unique_candidates(), 1);
    assert_eq!(replay.duplicate_exact_lifts(), 0);
    let InteriorReplayRunDisposition::OwnerProposal {
        proposal:
            ExactExecutableOwnerProposal::Compiled {
                owner,
                obstructions,
            },
        support: Some(support),
    } = report.disposition()
    else {
        panic!("the oracle-disabled interior task must compile an exact owner")
    };
    assert!(obstructions.is_empty());
    assert_eq!(support.census().candidates(), 1);
    owner.clone()
}

fn apply_first_owner(
    generator: &ParametricIbpGenerator<'_>,
    predecessor: ImmutableOwnerSnapshot,
    sector: Mask,
    owner: Arc<ExactSemanticExecutableOwner>,
) -> (CanonicalExactOwnerLedger, ExactOwnerCoverDelta) {
    let terminal = IntegralKey::try_new(sector.corner_indices()).unwrap();
    let arity = sector.arity();
    let mut ledger = CanonicalExactOwnerLedger::try_new(
        generator.context(),
        predecessor,
        sector,
        OrderingPolicy::default(),
        [terminal],
        ExactOwnerCoverDeltaLimits::default(),
    )
    .unwrap();
    let initial = ledger.try_clone_uncovered_partition().unwrap();
    assert_eq!(initial.boxes().len(), 1);
    assert_eq!(initial.boxes()[0].lower(), vec![0; arity]);
    assert_eq!(initial.boxes()[0].upper(), vec![None; arity]);
    let delta = ledger.try_apply_owner(owner).unwrap();
    (ledger, delta)
}

#[test]
fn oracle_disabled_k6_interior_replay_strictly_shrinks_the_exact_blind_orthant() {
    let family = canonical_three_loop_family().unwrap();
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())
            .unwrap();
    let completed = complete_ordinary(&generator);
    assert!(completed.is_complete_ordinary());
    assert_eq!(completed.source_row_count(), 9);

    let sector = Mask::try_from_indices(&FULL_RANK_ORBITS[0].representative).unwrap();
    let predecessor = ImmutableOwnerSnapshot::try_from_terminal_authority(
        derive_k6_terminal_authority().unwrap(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(predecessor.closed_layer_count(), 0);
    let seed_ledger = CanonicalExactOwnerLedger::try_new(
        generator.context(),
        predecessor.clone(),
        sector.clone(),
        OrderingPolicy::default(),
        [IntegralKey::try_new(sector.corner_indices()).unwrap()],
        ExactOwnerCoverDeltaLimits::default(),
    )
    .unwrap();
    let initial = seed_ledger.try_clone_uncovered_partition().unwrap();
    let scope_key = format!(
        "{}|{}|{}|{:?}|{:?}",
        family.fingerprint(),
        generator.context().fingerprint(),
        predecessor.id().as_str(),
        sector.active_bits(),
        OrderingPolicy::default(),
    );
    let plan = try_plan_interior_simplex_samples(
        0,
        [InteriorSimplexScopePartition::new(
            &scope_key, &sector, &initial,
        )],
        2,
        0,
        InteriorSimplexLimits::default(),
    )
    .unwrap();
    assert_eq!(plan.tasks().len(), 1);
    let task = &plan.tasks()[0];
    assert_eq!(task.lattice_target(), &[2; 6]);
    assert_eq!(task.target_shift().values(), &[-2, -2, 2, -2, 2, 2]);

    let first_owner = replay_owner(&generator, &completed, task, predecessor.clone());
    let second_owner = replay_owner(&generator, &completed, task, predecessor.clone());
    assert_eq!(
        first_owner.content_order_key(),
        second_owner.content_order_key()
    );

    let (first_ledger, first_delta) =
        apply_first_owner(&generator, predecessor.clone(), sector.clone(), first_owner);
    let (second_ledger, second_delta) =
        apply_first_owner(&generator, predecessor, sector, second_owner);
    assert_eq!(first_delta, second_delta);
    assert_eq!(
        first_delta.kind(),
        ExactOwnerCoverDeltaKind::StrictGeometricShrink
    );
    assert_eq!(
        first_delta.baseline().status(),
        ExactOwnerLedgerCoverStatus::OwnerFree
    );
    assert_eq!(first_delta.baseline().uncovered_box_count(), 1);
    assert_eq!(
        first_delta.updated().status(),
        ExactOwnerLedgerCoverStatus::Compiled(ExactOwnerCoverStatus::Incomplete(
            ExactOwnerCoverObstructionKind::NonFinite,
        ))
    );
    assert_eq!(first_delta.updated().uncovered_box_count(), 6);
    assert!(!first_delta.updated().uncovered_is_finite());
    assert_eq!(first_delta.updated().missing_terminal_count(), 0);
    assert_eq!(first_delta.updated().guard_incomplete_owner_count(), 0);

    let first_summary = first_ledger
        .proof_owner_summary(0)
        .expect("the exact K=6 cover must retain its one canonical proof owner");
    assert_eq!(first_summary.leading_lattice_point(), &[2; 6]);
    assert!(first_summary.compiled_guard_total());
    let first_dag = first_summary.semantic_dag_census();
    assert_eq!(first_dag.candidates(), 1);
    assert!(first_dag.atoms() > 0);
    assert!(first_dag.candidate_atom_references() >= first_dag.atoms());
    assert!(first_dag.memo_states() > 0);
    assert!(first_dag.nodes() > 0);
    assert_eq!(first_dag.edges(), 2 * first_dag.nodes());
    assert!(first_dag.has_reachable_incomplete());
    assert_eq!(first_ledger.proof_owner_summary(1), None);

    let second_summary = second_ledger
        .proof_owner_summary(0)
        .expect("the repeated exact K=6 cover must retain its proof owner");
    assert_eq!(first_summary, second_summary);

    let first_partition = first_ledger.try_clone_uncovered_partition().unwrap();
    let second_partition = second_ledger.try_clone_uncovered_partition().unwrap();
    assert_eq!(first_partition.boxes(), second_partition.boxes());
    assert_eq!(
        first_partition.split_operations(),
        second_partition.split_operations()
    );
}
