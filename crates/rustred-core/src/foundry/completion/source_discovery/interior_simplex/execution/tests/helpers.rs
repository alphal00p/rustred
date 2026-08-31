use crate::foundry::completion::source_discovery::interior_simplex::{
    InteriorSimplexExecutionLimits, InteriorSimplexExecutionReport, InteriorSimplexLimits,
    InteriorSimplexPlan, InteriorSimplexProbeExecutor, InteriorSimplexScopePartition,
    InteriorSimplexTaskExecutionReport, try_plan_interior_simplex_samples,
};
use crate::foundry::completion::source_discovery::{CampaignLimits, CampaignModularProbe};
use crate::foundry::completion::stratum::ImmutableOwnerSnapshot;
use crate::foundry::completion::{LatticeBox, UncoveredPartition};
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};
use crate::sector::{Mask, OrderingPolicy};

const PRIME: u64 = 1_000_000_007;

pub(super) fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

pub(super) fn bounded_execution_limits() -> InteriorSimplexExecutionLimits {
    let mut limits = InteriorSimplexExecutionLimits::default();
    limits.scheduler.max_probes = 1;
    limits.scheduler.max_retained_outcomes = 1;
    limits.scheduler.max_iterations_per_probe = 1;
    limits.scheduler.max_aggregate_epochs = 1;
    limits.scheduler.max_retained_iteration_records = 1;
    limits.scheduler.max_exact_lift_attempts = 1;
    limits
}

pub(super) fn declared_probe(
    generator: &ParametricIbpGenerator<'_>,
    campaign: CampaignLimits,
) -> CampaignModularProbe {
    CampaignModularProbe::try_new(
        PRIME,
        std::iter::repeat_n(37, generator.context().base().parameter_names().len()),
        (1..=generator.context().index_count()).map(|value| value as u64),
        campaign,
    )
    .unwrap()
}

pub(super) fn lattice_box(lower: &[u64], upper: &[Option<u64>]) -> LatticeBox {
    LatticeBox::try_new(lower.iter().copied(), upper.iter().copied()).unwrap()
}

pub(super) fn one_scope_plan(
    epoch: u64,
    sector: &Mask,
    partition: &UncoveredPartition,
    degree: usize,
) -> InteriorSimplexPlan {
    try_plan_interior_simplex_samples(
        epoch,
        [InteriorSimplexScopePartition::new(
            "typed-test-scope",
            sector,
            partition,
        )],
        1,
        degree,
        InteriorSimplexLimits::default(),
    )
    .unwrap()
}

pub(super) fn execute(
    plan: InteriorSimplexPlan,
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    owners: ImmutableOwnerSnapshot,
    limits: InteriorSimplexExecutionLimits,
) -> InteriorSimplexExecutionReport {
    let probe = declared_probe(generator, limits.scheduler.campaign);
    InteriorSimplexProbeExecutor::try_new(
        plan,
        generator,
        completed,
        owners,
        OrderingPolicy::default(),
        [probe],
        limits,
    )
    .unwrap()
    .run()
    .unwrap()
}

pub(super) fn summarize(
    report: &InteriorSimplexExecutionReport,
) -> Vec<InteriorSimplexTaskExecutionReport> {
    report.tasks().to_vec()
}

pub(super) fn assert_structural_accounting(report: &InteriorSimplexExecutionReport) {
    for (ordinal, task) in report.tasks().iter().enumerate() {
        assert_eq!(task.canonical_ordinal(), ordinal);
        let bootstrap = task.bootstrap();
        assert!(bootstrap.raw_incidence_visits() > 0);
        assert!(bootstrap.unique_nominated_requests() > 0);
        assert_eq!(bootstrap.excluded_existing_requests(), 0);
        assert_eq!(
            bootstrap.selected_sources(),
            bootstrap.unique_nominated_requests()
        );
        assert!(bootstrap.physical_shift_occurrences() > 0);
        assert!(bootstrap.distinct_physical_shifts() > 0);
        assert!(bootstrap.distinct_physical_shifts() <= bootstrap.physical_shift_occurrences());
        assert_eq!(task.probes().len(), 1);
        assert_eq!(task.probes()[0].probe_ordinal(), 0);
        assert_eq!(task.probes()[0].iterations().len(), 1);
        assert_eq!(task.census().epochs(), 1);
        assert_eq!(task.census().retained_iteration_records(), 1);
    }
    let retained = report.retained_payload();
    assert_eq!(retained.task_reports(), report.tasks().len());
    assert_eq!(retained.task_probe_reports(), report.tasks().len());
    assert_eq!(retained.iteration_records(), report.tasks().len());
    assert_eq!(retained.retained_exact_circuits(), 0);
    assert_eq!(retained.retained_support_entries(), 0);
}
