//! Focused semantic adapter regressions.

mod all_full_rank_orbits;
mod boundary;
mod k6;
mod k6_boundary_walk;
mod k6_compact_boundary_walk;

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
use crate::foundry::completion::source_discovery::{
    CampaignModularProbe, OrdinarySourceIncidenceIndex,
};
use crate::foundry::completion::stratum::{ImmutableOwnerSnapshot, StratumRegistryLimits};
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
    let levels =
        usize::BITS as usize - occurrences.max(2).saturating_sub(1).leading_zeros() as usize;
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
