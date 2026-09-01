//! Oracle-disabled K=6 inputs shared by exact discovery regressions.

use std::sync::{Arc, OnceLock};

use crate::family::{IntegralFamily, IntegralKey};
use crate::foundry::artifact::{
    FULL_RANK_ORBITS, canonical_three_loop_family, derive_k6_terminal_authority,
};
use crate::foundry::completion::source_discovery::cover_delta::{
    CanonicalExactOwnerLedger, ExactOwnerCoverDeltaLimits,
};
use crate::foundry::completion::source_discovery::interior_replay::{
    InteriorReplayRunDisposition, InteriorReplayRunLimits, try_run_interior_replay_task,
};
use crate::foundry::completion::source_discovery::interior_simplex::{
    InteriorSimplexLimits, InteriorSimplexPlan, InteriorSimplexScopePartition, InteriorSimplexTask,
    try_plan_interior_simplex_samples,
};
use crate::foundry::completion::source_discovery::{
    CampaignModularProbe, ExactExecutableOwnerProposal, ExactSemanticExecutableOwner,
    OrdinarySourceIncidenceIndex,
};
use crate::foundry::completion::stratum::{
    DecoratedStratum, ImmutableOwnerSnapshot, MaximalStratumAnchor,
};
use crate::identity::{
    CompletedIbpSourceRows, IntegralShift, ParametricIbpConfig, ParametricIbpGenerator,
    TranslatedSourceBatch,
};
use crate::sector::{Mask, OrderingPolicy, SectorMonotoneDomain};

const PRIME: u64 = 1_000_000_007;
const DIMENSION_SAMPLE: i64 = 37;
static K6_FAMILY: OnceLock<IntegralFamily> = OnceLock::new();
static K6_FIXTURE: OnceLock<OracleDisabledK6Fixture> = OnceLock::new();

/// Typed K=6 family, complete ordinary module, zero-source incidence input,
/// and terminal-only predecessor authority. No reduction-oracle rule,
/// coefficient, support, or topology name enters this fixture.
pub(crate) struct OracleDisabledK6Fixture {
    generator: ParametricIbpGenerator<'static>,
    completed: CompletedIbpSourceRows,
    zero_sources: TranslatedSourceBatch,
    predecessor: ImmutableOwnerSnapshot,
    sector: Mask,
    ordering: OrderingPolicy,
}

impl OracleDisabledK6Fixture {
    pub(crate) fn shared() -> &'static Self {
        K6_FIXTURE.get_or_init(Self::build)
    }

    fn build() -> Self {
        let family = K6_FAMILY.get_or_init(|| canonical_three_loop_family().unwrap());
        let generator =
            ParametricIbpGenerator::try_new_with_config(family, ParametricIbpConfig::default())
                .unwrap();
        let prepared = generator.prepare_ordinary_ibp().unwrap();
        let rows = (0..prepared.len())
            .map(|ordinal| prepared.generate(ordinal))
            .collect();
        let completed = prepared.complete(rows).unwrap();
        assert!(completed.is_complete_ordinary());
        assert_eq!(completed.source_row_count(), 9);

        let sector = Mask::try_from_indices(&FULL_RANK_ORBITS[0].representative).unwrap();
        let predecessor = ImmutableOwnerSnapshot::try_from_terminal_authority(
            derive_k6_terminal_authority().unwrap(),
            Default::default(),
        )
        .unwrap();
        assert_eq!(predecessor.closed_layer_count(), 0);
        let replay = InteriorReplayRunLimits::default();
        let zero_sources = generator
            .translate_completed_source_rows(
                &completed,
                [IntegralShift::try_new(std::iter::repeat_n(0, sector.arity())).unwrap()],
                replay.scheduler.source_discovery.translation,
            )
            .unwrap();
        // Authenticate the zero-offset complete ordinary batch at the shared
        // fixture boundary; consumers retain their own policy-bound index.
        OrdinarySourceIncidenceIndex::try_new(&zero_sources, replay.scheduler.source_discovery)
            .unwrap();

        Self {
            generator,
            completed,
            zero_sources,
            predecessor,
            sector,
            ordering: OrderingPolicy::default(),
        }
    }

    pub(crate) const fn generator(&self) -> &ParametricIbpGenerator<'static> {
        &self.generator
    }

    pub(crate) const fn completed(&self) -> &CompletedIbpSourceRows {
        &self.completed
    }

    pub(crate) const fn zero_sources(&self) -> &TranslatedSourceBatch {
        &self.zero_sources
    }

    pub(crate) const fn predecessor(&self) -> &ImmutableOwnerSnapshot {
        &self.predecessor
    }

    pub(crate) const fn sector(&self) -> &Mask {
        &self.sector
    }

    pub(crate) fn new_ledger(&self) -> CanonicalExactOwnerLedger {
        CanonicalExactOwnerLedger::try_new(
            self.generator.context(),
            self.predecessor.clone(),
            self.sector.clone(),
            self.ordering,
            [IntegralKey::try_new(self.sector.corner_indices()).unwrap()],
            ExactOwnerCoverDeltaLimits::default(),
        )
        .unwrap()
    }

    pub(crate) fn plan(
        &self,
        ledger: &CanonicalExactOwnerLedger,
        interior_margin: u64,
        polynomial_degree_ceiling: usize,
    ) -> InteriorSimplexPlan {
        let partition = ledger.try_clone_uncovered_partition().unwrap();
        let scope = format!(
            "{}|{}|{}|{:?}|{:?}|{}",
            self.predecessor.family_fingerprint(),
            self.predecessor.context_fingerprint(),
            self.predecessor.id().as_str(),
            self.sector.active_bits(),
            self.ordering,
            ledger.revision().get(),
        );
        try_plan_interior_simplex_samples(
            ledger.revision().get(),
            [InteriorSimplexScopePartition::new(
                &scope,
                &self.sector,
                &partition,
            )],
            interior_margin,
            polynomial_degree_ceiling,
            InteriorSimplexLimits::default(),
        )
        .unwrap()
    }

    /// Reproduce one exact owner from a single task-local modular probe. This
    /// helper intentionally retains the original replay/support assertions so
    /// refactored tests do not weaken the oracle-disabled first-shrink proof.
    pub(crate) fn replay_owner(
        &self,
        task: &InteriorSimplexTask,
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

        let incidence = OrdinarySourceIncidenceIndex::try_new(
            &self.zero_sources,
            limits.scheduler.source_discovery,
        )
        .unwrap();
        let nominations = incidence
            .try_nominate_target_unit(task.target_shift(), limits.scheduler.source_discovery)
            .unwrap();
        let selected = self
            .generator
            .translate_selected_completed_source_rows(
                &self.completed,
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
            selected.context_fingerprint(),
            domain,
            limits.scheduler.campaign.stratum,
        )
        .unwrap();
        let anchor =
            MaximalStratumAnchor::try_new(stratum, limits.scheduler.campaign.stratum).unwrap();
        let probe = CampaignModularProbe::try_new(
            PRIME,
            [DIMENSION_SAMPLE],
            task.lattice_target().iter().copied(),
            limits.scheduler.campaign,
        )
        .unwrap();
        let report = try_run_interior_replay_task(
            &self.generator,
            &self.completed,
            task.target_shift().clone(),
            anchor,
            self.predecessor.clone(),
            self.ordering,
            [probe],
            limits,
        )
        .unwrap();
        assert_eq!(report.scheduler().epochs(), 1);
        assert_eq!(report.scheduler().exact_lift_attempts(), 1);
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
}
