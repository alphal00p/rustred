//! Bounded support-only falsifier for one natural S4a matcher chart.
//!
//! The foreign chart is allowed to nominate canonical parent support only.
//! Every selected source is regenerated from the sealed parent K6 ordinary
//! chronology before either modular or exact work, and both lanes use the
//! same target, probe, lower-owner authority, ordering, and resource policy.
//! A measured miss is therefore only a falsification of this proposal lane;
//! it is never a no-relation or closure claim.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::family::{IntegralFamily, IntegralKey};
use crate::foundry::cell::{FixedIndexRestriction, RuleCell, SourceViewConstruction};
use crate::foundry::completion::SectorChart;
use crate::foundry::completion::frame::admission::{
    ExactCircuitOuterExtensionWitness, ExactCircuitOwnerCover, ExactCircuitOwnerInput,
    ExactCircuitSemanticDag, ExactOwnerCoverSelection, ExactOwnerCoverStatus,
};
use crate::foundry::completion::frame::exact::{
    ExactCircuitLift, try_lift_exact_circuit, try_lift_exact_circuit_over_complete_frame,
    try_lower_exact_circuit,
};
use crate::foundry::completion::frame::modular::ModularTargetQuery;
use crate::foundry::completion::source_discovery::scheduler::ProbeLocalRunCensus;
use crate::foundry::completion::source_discovery::test_fixtures::OracleDisabledK6Fixture;
use crate::foundry::completion::source_discovery::{
    AccumulatedSourceRequests, CampaignModularProbe, CanonicalExactOwnerLedger,
    CanonicalReplayTelemetry, ExactExecutableOwnerProposal, ExactOwnerCoverDelta,
    ExactOwnerCoverDeltaKind, ExactOwnerCoverDeltaLimits, FreshTaskEpoch,
    InitialParentSourceProposalTelemetry, InteriorReplayRunDisposition, InteriorReplayRunLimits,
    InteriorReplaySchedulerOutcomeCensus, InteriorReplayTaskReport, OrdinarySourceIncidenceIndex,
    try_run_interior_replay_task, try_run_interior_replay_task_with_initial_parent_proposal,
};
use crate::foundry::completion::stratum::{DecoratedStratum, MaximalStratumAnchor};
use crate::foundry::parametric::ParametricRuleTermDescent;
use crate::identity::{IntegralShift, TranslatedSourceRequest};
use crate::sector::symmetry::{Canonicalizer, RoutingWitness};
use crate::sector::{InteriorBounds, Mask, OrderingPolicy, SectorMonotoneDomain};

use super::super::{canonical_family, canonical_s4};
use super::transport::{
    FixedMatcherChartRowTransportLimits, try_transport_fixed_matcher_chart_row,
};
use super::{MatcherSeedChart, MatcherSeedPortfolio};

const CHART_LABEL: &str = "I3L_pinch_1_6";
const PRIME: u64 = 1_000_000_007;
const DIMENSION_SAMPLE: i64 = 37;
const ORDINARY_ROW_COUNT: usize = 9;
const RAY_SAMPLES: [(i64, [i64; 6]); 2] = [(4, [1, 1, 2, 4, 0, 0]), (5, [1, 1, 2, 5, 0, 0])];
const MIXED_DOT_RAY_TARGET_SHIFT: [i64; 6] = [0, 0, 1, 1, 2, 0];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReplayOutcome {
    NoReplayedNominations,
    NoRebasedCircuits,
    IncompleteOwner,
    CompiledOwner,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ParentSourceSpan {
    requests: usize,
    translated_sources: usize,
    translated_term_occurrences: usize,
    distinct_physical_shifts: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ReplayObservation {
    scheduler: ProbeLocalRunCensus,
    scheduler_outcomes: InteriorReplaySchedulerOutcomeCensus,
    canonical_replay: Option<CanonicalReplayTelemetry>,
    outcome: ReplayOutcome,
    cover_delta: Option<ExactOwnerCoverDelta>,
}

fn s4a_chart(portfolio: &MatcherSeedPortfolio) -> &MatcherSeedChart {
    portfolio
        .charts
        .iter()
        .find(|chart| chart.diagnostic_label == CHART_LABEL)
        .expect("the frozen matcher portfolio contains the natural S4a chart")
}

fn bounded_replay_limits() -> InteriorReplayRunLimits {
    let mut limits = InteriorReplayRunLimits::default();
    limits.scheduler.max_probes = 1;
    limits.scheduler.max_retained_outcomes = 1;
    limits.scheduler.max_iterations_per_probe = 1;
    limits.scheduler.max_aggregate_epochs = 1;
    limits.scheduler.max_retained_iteration_records = 1;
    limits.scheduler.max_exact_lift_attempts = 1;
    limits.canonical_replay.campaign = limits.scheduler.campaign;
    limits.canonical_replay.source_discovery = limits.scheduler.source_discovery;
    limits
}

fn adaptive_replay_limits(
    max_epochs_per_probe: usize,
    probe_count: usize,
) -> InteriorReplayRunLimits {
    let mut limits = InteriorReplayRunLimits::default();
    limits.scheduler.max_probes = probe_count;
    limits.scheduler.max_retained_probe_coordinate_cells = probe_count * 7;
    limits.scheduler.max_retained_outcomes = probe_count;
    limits.scheduler.max_iterations_per_probe = max_epochs_per_probe;
    limits.scheduler.max_aggregate_epochs = max_epochs_per_probe * probe_count;
    limits.scheduler.max_retained_iteration_records = max_epochs_per_probe * probe_count;
    limits.scheduler.max_exact_lift_attempts = 2 * probe_count;
    limits.canonical_replay.campaign = limits.scheduler.campaign;
    limits.canonical_replay.source_discovery = limits.scheduler.source_discovery;
    limits
}

fn transport_parent_support(
    parent: &IntegralFamily,
    chart: &MatcherSeedChart,
    canonicalizer: &Canonicalizer,
    target: &IntegralShift,
    local_sample: [i64; 6],
) -> Vec<IntegralShift> {
    let mut support = BTreeSet::new();
    let mut common_route: Option<RoutingWitness> = None;
    for source_ordinal in 0..ORDINARY_ROW_COUNT {
        let row = try_transport_fixed_matcher_chart_row(
            parent,
            chart,
            canonicalizer,
            source_ordinal,
            IntegralShift::try_new(local_sample).unwrap(),
            FixedMatcherChartRowTransportLimits::default(),
        )
        .unwrap();
        assert_eq!(row.provenance().source_ordinal(), source_ordinal);
        assert_eq!(
            row.provenance().source_row(),
            chart.ordinary.source_row_id(source_ordinal).unwrap()
        );
        assert_eq!(row.provenance().raw_target().powers(), target.values());
        assert_eq!(
            row.provenance().canonical_target().powers(),
            target.values()
        );
        assert_eq!(
            row.provenance().parent_family_fingerprint(),
            parent.fingerprint()
        );
        assert_ne!(
            row.provenance().local_family_fingerprint(),
            parent.fingerprint()
        );
        if let Some(route) = &common_route {
            assert_eq!(row.provenance().common_route(), route);
        } else {
            common_route = Some(row.provenance().common_route().clone());
        }
        assert!(row.terms().windows(2).all(|pair| pair[0].0 < pair[1].0));
        assert!(
            row.terms()
                .iter()
                .all(|(_, coefficient)| !coefficient.is_zero())
        );
        for key in row.support() {
            support.insert(IntegralShift::try_new(key.powers().iter().copied()).unwrap());
        }
    }
    assert_eq!(chart.ordinary.source_row_count(), ORDINARY_ROW_COUNT);
    assert!(!support.is_empty());
    let support = support.into_iter().collect::<Vec<_>>();
    assert!(support.windows(2).all(|pair| pair[0] < pair[1]));
    support
}

/// Reconstruct the inverse-incidence request set independently for test
/// telemetry and anchor construction. These requests never enter discovery:
/// the scheduler receives only the sealed proposal and regenerates it from
/// the exact `CompletedIbpSourceRows` owner to which that proposal is bound.
fn expected_parent_requests(
    fixture: &OracleDisabledK6Fixture,
    support: &[IntegralShift],
    limits: InteriorReplayRunLimits,
) -> Vec<TranslatedSourceRequest> {
    let term_occurrences = fixture
        .zero_sources()
        .sources()
        .iter()
        .map(|source| source.terms().len())
        .sum::<usize>();
    let raw_requests = support
        .len()
        .checked_mul(term_occurrences)
        .expect("the bounded inverse-incidence request census fits usize");
    assert!(raw_requests <= limits.scheduler.source_discovery.max_raw_requests);
    let coordinate_cells = raw_requests
        .checked_mul(fixture.generator().context().index_count())
        .expect("the bounded inverse-incidence coordinate census fits usize");
    assert!(
        coordinate_cells
            <= limits
                .scheduler
                .source_discovery
                .max_candidate_coordinate_cells
    );

    let mut requests = Vec::new();
    requests
        .try_reserve_exact(raw_requests)
        .expect("the bounded test request census is allocatable");
    for supported in support {
        for (source_ordinal, source) in fixture.zero_sources().sources().iter().enumerate() {
            for source_shift in source.terms().keys() {
                let offset =
                    IntegralShift::try_new(
                        supported.values().iter().zip(source_shift.values()).map(
                            |(&left, &right)| {
                                left.checked_sub(right)
                                    .expect("the declared K6 support difference fits i64")
                            },
                        ),
                    )
                    .unwrap();
                requests.push(TranslatedSourceRequest::new(source_ordinal, offset));
            }
        }
    }
    assert_eq!(requests.len(), raw_requests);
    requests.sort_unstable();
    requests.dedup();
    assert!(requests.len() <= limits.scheduler.source_discovery.max_unique_requests);
    requests
}

fn merge_requests(
    baseline: &[TranslatedSourceRequest],
    proposal: &[TranslatedSourceRequest],
) -> Vec<TranslatedSourceRequest> {
    let capacity = baseline
        .len()
        .checked_add(proposal.len())
        .expect("the bounded merged request census fits usize");
    let mut merged = Vec::new();
    merged
        .try_reserve_exact(capacity)
        .expect("the bounded merged request census is allocatable");
    merged.extend_from_slice(baseline);
    merged.extend_from_slice(proposal);
    merged.sort_unstable();
    merged.dedup();
    merged
}

fn parent_anchor_and_span(
    fixture: &OracleDisabledK6Fixture,
    target: &IntegralShift,
    sector: &Mask,
    requests: &[TranslatedSourceRequest],
    limits: InteriorReplayRunLimits,
) -> (MaximalStratumAnchor, ParentSourceSpan) {
    let selected = fixture
        .generator()
        .translate_selected_completed_source_rows(
            fixture.completed(),
            requests.iter().cloned(),
            limits.scheduler.campaign.translated_sources,
        )
        .unwrap();
    assert_eq!(selected.requests(), requests);
    assert_eq!(
        selected.family_fingerprint(),
        fixture.completed().family_fingerprint()
    );
    assert_eq!(
        selected.context_fingerprint(),
        fixture.completed().context_fingerprint()
    );
    for (request, source) in selected.requests().iter().zip(selected.sources()) {
        assert_eq!(
            source.provenance().source_ordinal(),
            request.source_ordinal()
        );
        assert_eq!(source.provenance().offset(), request.offset());
        assert_eq!(
            source.provenance().source_row(),
            fixture
                .completed()
                .source_row_id(request.source_ordinal())
                .unwrap()
        );
    }

    let translated_term_occurrences = selected
        .sources()
        .iter()
        .map(|source| source.terms().len())
        .sum::<usize>();
    let mut physical_shifts = selected
        .sources()
        .iter()
        .flat_map(|source| source.terms().keys())
        .map(|shift| shift.values())
        .collect::<Vec<_>>();
    physical_shifts.sort_unstable();
    physical_shifts.dedup();
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        sector.clone(),
        target.values(),
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
    let anchor = MaximalStratumAnchor::try_new(stratum, limits.scheduler.campaign.stratum).unwrap();
    (
        anchor,
        ParentSourceSpan {
            requests: requests.len(),
            translated_sources: selected.len(),
            translated_term_occurrences,
            distinct_physical_shifts: physical_shifts.len(),
        },
    )
}

fn fixed_mixed_dot_ray_epoch(
    fixture: &OracleDisabledK6Fixture,
    limits: InteriorReplayRunLimits,
) -> FreshTaskEpoch {
    let target = IntegralShift::try_new(MIXED_DOT_RAY_TARGET_SHIFT).unwrap();
    let sector = Mask::try_from_indices(&[0, 1, 1, 1, 1, 0]).unwrap();
    let incidence = OrdinarySourceIncidenceIndex::try_new(
        fixture.zero_sources(),
        limits.scheduler.source_discovery,
    )
    .unwrap();
    let nominations = incidence
        .try_nominate_target_unit(&target, limits.scheduler.source_discovery)
        .unwrap();
    let requests = AccumulatedSourceRequests::try_new(
        target.len(),
        nominations.requests().iter().cloned(),
        limits.scheduler.campaign,
    )
    .unwrap();
    let selected = fixture
        .generator()
        .translate_selected_completed_source_rows(
            fixture.completed(),
            requests.requests().iter().cloned(),
            limits.scheduler.campaign.translated_sources,
        )
        .unwrap();
    let physical_shifts = selected
        .sources()
        .iter()
        .flat_map(|source| source.terms().keys())
        .map(|shift| shift.values())
        .collect::<Vec<_>>();
    let maximal = SectorMonotoneDomain::try_maximal_for_rule(
        sector.clone(),
        target.values(),
        &physical_shifts,
    )
    .unwrap();
    let mut bounds = maximal.bounds().to_vec();
    bounds[0] = InteriorBounds::new(0, 0);
    bounds[1] = InteriorBounds::new(1, 1);
    bounds[2] = InteriorBounds::new(1, 1);
    bounds[3] = InteriorBounds::new(1, 1);
    bounds[4] = InteriorBounds::new(1, bounds[4].upper());
    bounds[5] = InteriorBounds::new(0, 0);
    let domain =
        SectorMonotoneDomain::try_new_for_rule(sector, bounds, target.values(), &physical_shifts)
            .unwrap();
    let stratum = DecoratedStratum::try_guard_blind(
        selected.family_fingerprint(),
        selected.context_fingerprint(),
        domain,
        limits.scheduler.campaign.stratum,
    )
    .unwrap();
    FreshTaskEpoch::try_new(
        0,
        fixture.generator(),
        fixture.completed(),
        requests,
        target,
        stratum,
        fixture.predecessor().clone(),
        OrderingPolicy::default(),
        limits.scheduler.campaign,
    )
    .unwrap()
}

fn outcome(report: &InteriorReplayTaskReport) -> ReplayOutcome {
    match report.disposition() {
        InteriorReplayRunDisposition::NoReplayedNominations => ReplayOutcome::NoReplayedNominations,
        InteriorReplayRunDisposition::NoRebasedCircuits { .. } => ReplayOutcome::NoRebasedCircuits,
        InteriorReplayRunDisposition::OwnerProposal {
            proposal: ExactExecutableOwnerProposal::Incomplete(_),
            ..
        } => ReplayOutcome::IncompleteOwner,
        InteriorReplayRunDisposition::OwnerProposal {
            proposal: ExactExecutableOwnerProposal::Compiled { .. },
            ..
        } => ReplayOutcome::CompiledOwner,
    }
}

fn cover_delta(
    fixture: &OracleDisabledK6Fixture,
    sector: &Mask,
    report: &InteriorReplayTaskReport,
) -> Option<ExactOwnerCoverDelta> {
    let InteriorReplayRunDisposition::OwnerProposal {
        proposal: ExactExecutableOwnerProposal::Compiled { owner, .. },
        ..
    } = report.disposition()
    else {
        return None;
    };
    // This comparison ledger intentionally invents no master terminal. It
    // measures only the exact geometric effect of the replayed owner on the
    // same owner-free S4a orthant.
    let mut ledger = CanonicalExactOwnerLedger::try_new(
        fixture.generator().context(),
        fixture.predecessor().clone(),
        sector.clone(),
        OrderingPolicy::default(),
        std::iter::empty::<IntegralKey>(),
        ExactOwnerCoverDeltaLimits::default(),
    )
    .unwrap();
    Some(ledger.try_apply_owner(owner.clone()).unwrap())
}

fn observe(
    fixture: &OracleDisabledK6Fixture,
    sector: &Mask,
    report: &InteriorReplayTaskReport,
) -> ReplayObservation {
    ReplayObservation {
        scheduler: report.scheduler(),
        scheduler_outcomes: report.scheduler_outcomes(),
        canonical_replay: report.replay(),
        outcome: outcome(report),
        cover_delta: cover_delta(fixture, sector, report),
    }
}

fn is_strict_shrink(observation: ReplayObservation) -> bool {
    matches!(
        observation.cover_delta.map(ExactOwnerCoverDelta::kind),
        Some(ExactOwnerCoverDeltaKind::StrictGeometricShrink)
    )
}

#[test]
#[ignore = "offline release-only falsifier: the deliberately broad 1,350-request lane is not part of the default correctness suite"]
fn natural_s4a_support_only_proposals_are_measured_against_target_unit_at_n4_and_n5() {
    let parent = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&parent).unwrap();
    let portfolio = MatcherSeedPortfolio::try_compile().unwrap();
    let chart = s4a_chart(&portfolio);
    let fixture = OracleDisabledK6Fixture::shared();
    let limits = bounded_replay_limits();
    let incidence = OrdinarySourceIncidenceIndex::try_new(
        fixture.zero_sources(),
        limits.scheduler.source_discovery,
    )
    .unwrap();
    assert!(incidence.exactly_replays_completed(fixture.completed()));

    for (n, local_sample) in RAY_SAMPLES {
        let target = IntegralShift::try_new([0, 1, 1, 2, n, 0]).unwrap();
        let sector = Mask::try_from_indices(target.values()).unwrap();
        assert_eq!(sector.active_bits(), [false, true, true, true, true, false]);
        let lattice_target = SectorChart::new(sector.clone())
            .to_lattice(&IntegralKey::try_new(target.values().iter().copied()).unwrap())
            .unwrap();
        assert_eq!(
            lattice_target.coordinates(),
            [0, 0, 0, 1, (n - 1) as u64, 0]
        );

        let support =
            transport_parent_support(&parent, chart, &canonicalizer, &target, local_sample);
        let expected_proposal_requests = expected_parent_requests(fixture, &support, limits);
        let proposal = incidence
            .try_nominate_initial_parent_support(
                fixture.completed(),
                &support,
                limits.scheduler.source_discovery,
            )
            .unwrap();
        let proposal_telemetry: InitialParentSourceProposalTelemetry = proposal.telemetry();
        assert_eq!(
            proposal.family_fingerprint(),
            fixture.completed().family_fingerprint()
        );
        assert_eq!(
            proposal.context_fingerprint(),
            fixture.completed().context_fingerprint()
        );
        assert_eq!(
            proposal_telemetry.ordinary_source_rows(),
            ORDINARY_ROW_COUNT
        );
        assert_eq!(proposal_telemetry.parent_support_entries(), support.len());
        assert_eq!(
            proposal_telemetry.request_count(),
            expected_proposal_requests.len()
        );
        assert_eq!(proposal.requests(), expected_proposal_requests.as_slice());
        assert_eq!(
            proposal_telemetry.request_coordinate_cells(),
            expected_proposal_requests.len() * target.len()
        );

        let baseline_nominations = incidence
            .try_nominate_target_unit(&target, limits.scheduler.source_discovery)
            .unwrap();
        let baseline_requests = baseline_nominations.requests().to_vec();
        let assisted_requests = merge_requests(&baseline_requests, &expected_proposal_requests);
        let (_, baseline_span) =
            parent_anchor_and_span(fixture, &target, &sector, &baseline_requests, limits);
        let (common_anchor, assisted_span) =
            parent_anchor_and_span(fixture, &target, &sector, &assisted_requests, limits);
        assert_eq!(baseline_span.requests, baseline_requests.len());
        assert_eq!(assisted_span.requests, assisted_requests.len());
        assert!(assisted_span.requests >= baseline_span.requests);
        eprintln!(
            "K6 S4a matcher-chart preflight N={n}: support={}; proposal_requests={}; baseline_span={baseline_span:?}; assisted_span={assisted_span:?}",
            support.len(),
            expected_proposal_requests.len(),
        );

        let probe = CampaignModularProbe::try_new(
            PRIME,
            [DIMENSION_SAMPLE],
            lattice_target.coordinates().iter().copied(),
            limits.scheduler.campaign,
        )
        .unwrap();
        let baseline_owners = fixture.predecessor().clone();
        let assisted_owners = fixture.predecessor().clone();
        assert!(baseline_owners.same_authority_as(&assisted_owners));
        let baseline = try_run_interior_replay_task(
            fixture.generator(),
            fixture.completed(),
            target.clone(),
            common_anchor.clone(),
            baseline_owners,
            OrderingPolicy::default(),
            [probe.clone()],
            limits,
        )
        .unwrap();
        eprintln!("K6 S4a matcher-chart baseline N={n} completed");
        let assisted = try_run_interior_replay_task_with_initial_parent_proposal(
            fixture.generator(),
            fixture.completed(),
            target,
            common_anchor,
            assisted_owners,
            OrderingPolicy::default(),
            proposal,
            [probe],
            limits,
        )
        .unwrap();
        eprintln!("K6 S4a matcher-chart assisted N={n} completed");
        let baseline = observe(fixture, &sector, &baseline);
        let assisted = observe(fixture, &sector, &assisted);
        assert_eq!(baseline.scheduler.epochs(), 1);
        assert_eq!(assisted.scheduler.epochs(), 1);
        assert_eq!(
            baseline.scheduler.epoch_request_work(),
            baseline_span.requests
        );
        assert_eq!(
            assisted.scheduler.epoch_request_work(),
            assisted_span.requests
        );
        assert_eq!(
            baseline.scheduler.materialized_source_terms(),
            baseline_span.translated_term_occurrences
        );
        assert_eq!(
            assisted.scheduler.materialized_source_terms(),
            assisted_span.translated_term_occurrences
        );

        let chart_only_strict_shrink = is_strict_shrink(assisted) && !is_strict_shrink(baseline);
        eprintln!(
            "K6 S4a matcher-chart support falsifier N={n}: \
             support={support:?}; proposal={proposal_telemetry:?}; \
             baseline_span={baseline_span:?}; baseline={baseline:?}; \
             assisted_span={assisted_span:?}; assisted={assisted:?}; \
             request_delta={}; modular_entry_delta={}; \
             chart_only_strict_shrink={chart_only_strict_shrink}",
            assisted_span.requests as i128 - baseline_span.requests as i128,
            assisted.scheduler.modular_entry_work() as i128
                - baseline.scheduler.modular_entry_work() as i128,
        );
    }
}

#[test]
fn broad_inverse_incidence_is_rejected_by_a_fast_request_census() {
    let parent = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&parent).unwrap();
    let portfolio = MatcherSeedPortfolio::try_compile().unwrap();
    let chart = s4a_chart(&portfolio);
    let fixture = OracleDisabledK6Fixture::shared();
    let limits = bounded_replay_limits();
    let incidence = OrdinarySourceIncidenceIndex::try_new(
        fixture.zero_sources(),
        limits.scheduler.source_discovery,
    )
    .unwrap();

    for (_, local_sample) in RAY_SAMPLES {
        let target = IntegralShift::try_new([
            0,
            local_sample[0],
            local_sample[1],
            local_sample[2],
            local_sample[3],
            0,
        ])
        .unwrap();
        let support =
            transport_parent_support(&parent, chart, &canonicalizer, &target, local_sample);
        let broad = expected_parent_requests(fixture, &support, limits);
        let baseline = incidence
            .try_nominate_target_unit(&target, limits.scheduler.source_discovery)
            .unwrap();
        assert_eq!(support.len(), 21);
        assert_eq!(baseline.requests().len(), 90);
        assert_eq!(broad.len(), 1_350);
        assert!(broad.len() >= 15 * baseline.requests().len());
    }
}

#[test]
#[ignore = "release-only calibration of the target-unit K6 baseline"]
fn release_only_n4_target_unit_baseline_reports_its_exact_disposition() {
    assert!(
        !cfg!(debug_assertions),
        "runtime calibration must use an optimized release binary"
    );
    let fixture = OracleDisabledK6Fixture::shared();
    let target = IntegralShift::try_new([0, 1, 1, 2, 4, 0]).unwrap();
    let sector = Mask::try_from_indices(target.values()).unwrap();
    let lattice_target = SectorChart::new(sector.clone())
        .to_lattice(&IntegralKey::try_new(target.values().iter().copied()).unwrap())
        .unwrap();
    let limits = bounded_replay_limits();
    let incidence = OrdinarySourceIncidenceIndex::try_new(
        fixture.zero_sources(),
        limits.scheduler.source_discovery,
    )
    .unwrap();
    let requests = incidence
        .try_nominate_target_unit(&target, limits.scheduler.source_discovery)
        .unwrap()
        .requests()
        .to_vec();
    let (anchor, span) = parent_anchor_and_span(fixture, &target, &sector, &requests, limits);
    let probe = CampaignModularProbe::try_new(
        PRIME,
        [DIMENSION_SAMPLE],
        lattice_target.coordinates().iter().copied(),
        limits.scheduler.campaign,
    )
    .unwrap();
    let report = try_run_interior_replay_task(
        fixture.generator(),
        fixture.completed(),
        target,
        anchor,
        fixture.predecessor().clone(),
        OrderingPolicy::default(),
        [probe],
        limits,
    )
    .unwrap();
    let observation = observe(fixture, &sector, &report);
    eprintln!(
        "K6 S4a N=4 target-unit release baseline: span={span:?}; observation={observation:?}"
    );
    assert_eq!(observation.scheduler.epochs(), 1);
    assert_eq!(observation.scheduler.epoch_request_work(), span.requests);
}

#[test]
#[ignore = "release-only bounded adaptive K6 upward walk"]
fn release_only_n4_obstruction_guided_upward_walk_reports_progress() {
    assert!(
        !cfg!(debug_assertions),
        "runtime calibration must use an optimized release binary"
    );
    const MAX_EPOCHS: usize = 8;
    let fixture = OracleDisabledK6Fixture::shared();
    let target = IntegralShift::try_new([0, 1, 1, 2, 4, 0]).unwrap();
    let sector = Mask::try_from_indices(target.values()).unwrap();
    let lattice_target = SectorChart::new(sector.clone())
        .to_lattice(&IntegralKey::try_new(target.values().iter().copied()).unwrap())
        .unwrap();
    let limits = adaptive_replay_limits(MAX_EPOCHS, 1);
    let incidence = OrdinarySourceIncidenceIndex::try_new(
        fixture.zero_sources(),
        limits.scheduler.source_discovery,
    )
    .unwrap();
    let requests = incidence
        .try_nominate_target_unit(&target, limits.scheduler.source_discovery)
        .unwrap()
        .requests()
        .to_vec();
    let (anchor, initial_span) =
        parent_anchor_and_span(fixture, &target, &sector, &requests, limits);
    let probe = CampaignModularProbe::try_new(
        PRIME,
        [DIMENSION_SAMPLE],
        lattice_target.coordinates().iter().copied(),
        limits.scheduler.campaign,
    )
    .unwrap();
    let report = try_run_interior_replay_task(
        fixture.generator(),
        fixture.completed(),
        target,
        anchor,
        fixture.predecessor().clone(),
        OrderingPolicy::default(),
        [probe],
        limits,
    )
    .unwrap();
    let observation = observe(fixture, &sector, &report);
    eprintln!(
        "K6 S4a N=4 release adaptive walk: initial_span={initial_span:?}; observation={observation:?}"
    );
    assert!(observation.scheduler.epochs() <= MAX_EPOCHS);
    assert!(observation.scheduler.epochs() > 1);
}

#[test]
#[ignore = "release-only bounded multi-probe K6 upward walk"]
fn release_only_n4_multi_probe_walk_seeks_an_exactly_liftable_support() {
    assert!(
        !cfg!(debug_assertions),
        "runtime calibration must use an optimized release binary"
    );
    const MAX_EPOCHS: usize = 8;
    const SAMPLES: [(u64, i64); 6] = [
        (1_000_000_007, 31),
        (1_000_000_007, 37),
        (1_000_000_009, 31),
        (1_000_000_009, 37),
        (998_244_353, 31),
        (998_244_353, 37),
    ];
    let fixture = OracleDisabledK6Fixture::shared();
    let target = IntegralShift::try_new([0, 1, 1, 2, 4, 0]).unwrap();
    let sector = Mask::try_from_indices(target.values()).unwrap();
    let lattice_target = SectorChart::new(sector.clone())
        .to_lattice(&IntegralKey::try_new(target.values().iter().copied()).unwrap())
        .unwrap();
    let limits = adaptive_replay_limits(MAX_EPOCHS, SAMPLES.len());
    let incidence = OrdinarySourceIncidenceIndex::try_new(
        fixture.zero_sources(),
        limits.scheduler.source_discovery,
    )
    .unwrap();
    let requests = incidence
        .try_nominate_target_unit(&target, limits.scheduler.source_discovery)
        .unwrap()
        .requests()
        .to_vec();
    let (anchor, initial_span) =
        parent_anchor_and_span(fixture, &target, &sector, &requests, limits);
    let probes = SAMPLES.map(|(prime, dimension)| {
        CampaignModularProbe::try_new(
            prime,
            [dimension],
            lattice_target.coordinates().iter().copied(),
            limits.scheduler.campaign,
        )
        .unwrap()
    });
    let report = try_run_interior_replay_task(
        fixture.generator(),
        fixture.completed(),
        target,
        anchor,
        fixture.predecessor().clone(),
        OrderingPolicy::default(),
        probes,
        limits,
    )
    .unwrap();
    let observation = observe(fixture, &sector, &report);
    eprintln!(
        "K6 S4a N=4 release multi-probe walk: initial_span={initial_span:?}; observation={observation:?}"
    );
    assert!(observation.scheduler.epochs() <= MAX_EPOCHS * SAMPLES.len());
    assert!(observation.scheduler.epochs() > SAMPLES.len());
}

#[test]
#[ignore = "release-only complete-frame exact fallback for one bounded K6 epoch"]
fn release_only_n4_complete_frame_exact_fallback_tests_the_epoch_two_span() {
    assert!(
        !cfg!(debug_assertions),
        "runtime calibration must use an optimized release binary"
    );
    const MAX_EPOCHS: usize = 8;
    let fixture = OracleDisabledK6Fixture::shared();
    let target = IntegralShift::try_new([0, 1, 1, 2, 4, 0]).unwrap();
    let sector = Mask::try_from_indices(target.values()).unwrap();
    let lattice_target = SectorChart::new(sector.clone())
        .to_lattice(&IntegralKey::try_new(target.values().iter().copied()).unwrap())
        .unwrap();
    let mut limits = adaptive_replay_limits(MAX_EPOCHS, 1);
    limits.scheduler.max_complete_frame_exact_fallback_rows = 256;
    let incidence = OrdinarySourceIncidenceIndex::try_new(
        fixture.zero_sources(),
        limits.scheduler.source_discovery,
    )
    .unwrap();
    let requests = incidence
        .try_nominate_target_unit(&target, limits.scheduler.source_discovery)
        .unwrap()
        .requests()
        .to_vec();
    let (anchor, initial_span) =
        parent_anchor_and_span(fixture, &target, &sector, &requests, limits);
    let probe = CampaignModularProbe::try_new(
        PRIME,
        [DIMENSION_SAMPLE],
        lattice_target.coordinates().iter().copied(),
        limits.scheduler.campaign,
    )
    .unwrap();
    let report = try_run_interior_replay_task(
        fixture.generator(),
        fixture.completed(),
        target,
        anchor,
        fixture.predecessor().clone(),
        OrderingPolicy::default(),
        [probe],
        limits,
    )
    .unwrap();
    let observation = observe(fixture, &sector, &report);
    eprintln!(
        "K6 S4a N=4 release complete-frame fallback: initial_span={initial_span:?}; observation={observation:?}"
    );
    assert_eq!(observation.scheduler.epochs(), 2);
    assert_eq!(observation.scheduler.exact_lift_attempts(), 2);
}

#[test]
#[ignore = "release-only generic-index K6 upward walk"]
fn release_only_n4_generic_index_probe_walks_past_point_specialized_hits() {
    assert!(
        !cfg!(debug_assertions),
        "runtime calibration must use an optimized release binary"
    );
    const MAX_EPOCHS: usize = 8;
    // Deliberately move off the scalar face on both inactive axes.  Keeping
    // either coordinate at zero evaluates only the n_i = 0 slice and can
    // produce a modular circuit which is not a relation over the complete
    // inactive-index sector tested by the exact lift.
    const GENERIC_CHART_SAMPLE: [u64; 6] = [2, 2, 3, 5, 7, 3];
    let fixture = OracleDisabledK6Fixture::shared();
    let target = IntegralShift::try_new([0, 1, 1, 2, 4, 0]).unwrap();
    let sector = Mask::try_from_indices(target.values()).unwrap();
    let mut limits = adaptive_replay_limits(MAX_EPOCHS, 1);
    limits.scheduler.max_complete_frame_exact_fallback_rows = 512;
    let incidence = OrdinarySourceIncidenceIndex::try_new(
        fixture.zero_sources(),
        limits.scheduler.source_discovery,
    )
    .unwrap();
    let requests = incidence
        .try_nominate_target_unit(&target, limits.scheduler.source_discovery)
        .unwrap()
        .requests()
        .to_vec();
    let (anchor, initial_span) =
        parent_anchor_and_span(fixture, &target, &sector, &requests, limits);
    let probe = CampaignModularProbe::try_new(
        PRIME,
        [DIMENSION_SAMPLE],
        GENERIC_CHART_SAMPLE,
        limits.scheduler.campaign,
    )
    .unwrap();
    let report = try_run_interior_replay_task(
        fixture.generator(),
        fixture.completed(),
        target,
        anchor,
        fixture.predecessor().clone(),
        OrderingPolicy::default(),
        [probe],
        limits,
    )
    .unwrap();
    let observation = observe(fixture, &sector, &report);
    eprintln!(
        "K6 S4a N=4 release generic-index walk at {GENERIC_CHART_SAMPLE:?}: initial_span={initial_span:?}; observation={observation:?}"
    );
    assert!(observation.scheduler.epochs() <= MAX_EPOCHS);
}

#[test]
#[ignore = "release-only exact quotient control for the established K6 mixed-dot ray"]
fn release_only_fixed_mixed_dot_ray_lifts_in_its_declared_stratum() {
    assert!(
        !cfg!(debug_assertions),
        "runtime calibration must use an optimized release binary"
    );
    let fixture = OracleDisabledK6Fixture::shared();
    let limits = adaptive_replay_limits(1, 1);
    let epoch = fixed_mixed_dot_ray_epoch(fixture, limits);
    assert_eq!(
        epoch
            .fixed_stratum()
            .domain()
            .bounds()
            .iter()
            .enumerate()
            .filter_map(|(position, bounds)| {
                (bounds.lower() == bounds.upper()).then_some((position, bounds.lower()))
            })
            .collect::<Vec<_>>(),
        vec![(0, 0), (1, 1), (2, 1), (3, 1), (5, 0)]
    );
    let probe = CampaignModularProbe::try_new(
        PRIME,
        [DIMENSION_SAMPLE],
        [0, 0, 0, 0, 1, 0],
        limits.scheduler.campaign,
    )
    .unwrap();
    let query = epoch
        .try_query(
            fixture.generator().context(),
            &probe,
            limits.scheduler.campaign,
        )
        .unwrap();
    let ModularTargetQuery::Hit(hit) = query.query() else {
        panic!("the fixed mixed-dot ray target must have a modular hit")
    };
    let lift = try_lift_exact_circuit(
        fixture.generator().context(),
        hit,
        query.partition(),
        limits.scheduler.exact_circuit,
    )
    .unwrap();
    let (lift, complete_frame_fallback) = match lift {
        ExactCircuitLift::Replayed(circuit) => (ExactCircuitLift::Replayed(circuit), false),
        ExactCircuitLift::ModularSupportDidNotLift(_) => (
            try_lift_exact_circuit_over_complete_frame(
                fixture.generator().context(),
                hit,
                query.partition(),
                limits.scheduler.exact_circuit,
            )
            .unwrap(),
            true,
        ),
    };
    let ExactCircuitLift::Replayed(circuit) = lift else {
        panic!("the complete fixed-ray frame must lift exactly")
    };
    let fixed = [(0, 0), (1, 1), (2, 1), (3, 1), (5, 0)];
    let mut participating_rows = circuit
        .source_combination()
        .iter()
        .map(|source| source.frame_row_ordinal())
        .chain(
            circuit
                .pivot_guards()
                .iter()
                .map(|pivot| pivot.frame_row_ordinal()),
        )
        .collect::<Vec<_>>();
    participating_rows.sort_unstable();
    participating_rows.dedup();
    let specialized_zero_terms = participating_rows
        .iter()
        .flat_map(|&row| epoch.plan().source_for_row(row).unwrap().terms().values())
        .filter(|coefficient| {
            fixture
                .generator()
                .context()
                .specialize_fixed_indices(coefficient, &fixed, Default::default())
                .unwrap()
                .0
                .is_zero()
        })
        .count();
    assert_eq!(circuit.stratum_id(), epoch.fixed_stratum().id());
    assert_eq!(circuit.target_shift().values(), MIXED_DOT_RAY_TARGET_SHIFT);
    assert!(!circuit.source_combination().is_empty());
    let anchor = epoch.try_anchor_for_probe(&probe).unwrap();
    let lowered = try_lower_exact_circuit(
        fixture.generator().context(),
        epoch.plan(),
        &circuit,
        &anchor,
        Default::default(),
    )
    .unwrap();
    let SourceViewConstruction::FixedIndexSpecialization(evidence) =
        lowered.sources().construction()
    else {
        panic!("a fixed-ray exact circuit must retain its quotient specialization")
    };
    assert_eq!(
        evidence
            .fixed_restrictions()
            .iter()
            .map(|restriction| (restriction.position(), restriction.value()))
            .collect::<Vec<_>>(),
        fixed
    );
    assert_eq!(
        lowered.rule().domain().bounds(),
        epoch.fixed_stratum().domain().bounds()
    );
    assert!(
        lowered
            .rule()
            .right_hand_side()
            .iter()
            .zip(circuit.residual_terms())
            .all(|(lowered, exact)| matches!(
                lowered.descent(),
                ParametricRuleTermDescent::SectorMonotone(witness)
                    if witness == exact.descent()
            ))
    );
    let (rule, sources) = lowered.into_parts();
    let cell = RuleCell::try_refined(
        fixture.generator().context(),
        rule,
        sources,
        epoch.fixed_stratum().domain().clone(),
        fixed
            .iter()
            .map(|&(position, value)| FixedIndexRestriction::new(position, value)),
        [],
        Default::default(),
    )
    .unwrap();
    let circuit = Arc::new(circuit);
    let semantic = Arc::new(
        ExactCircuitSemanticDag::try_compile(
            fixture.generator().context(),
            query.partition(),
            std::slice::from_ref(&circuit),
            Default::default(),
        )
        .unwrap(),
    );
    let extension =
        ExactCircuitOuterExtensionWitness::try_prove(query.partition(), semantic).unwrap();
    assert_eq!(extension.region().lower(), [0, 0, 1, 1, 2, 0]);
    assert_eq!(
        extension.region().upper(),
        [Some(0), Some(0), Some(1), Some(1), None, Some(0)]
    );
    let cover = ExactCircuitOwnerCover::try_compile(
        fixture.generator().context(),
        [ExactCircuitOwnerInput::new(query.partition(), extension)],
        [],
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        cover.status(),
        ExactOwnerCoverStatus::Incomplete(
            crate::foundry::completion::frame::admission::ExactOwnerCoverObstructionKind::NonFinite,
        )
    );
    let target = IntegralKey::try_new(
        anchor
            .iter()
            .zip(MIXED_DOT_RAY_TARGET_SHIFT)
            .map(|(&index, shift)| index.checked_add(shift).unwrap()),
    )
    .unwrap();
    assert!(cell.assignment_for_target(&target).unwrap().is_some());
    assert!(matches!(
        cover
            .try_select_at(fixture.generator().context(), &target, Default::default())
            .unwrap(),
        ExactOwnerCoverSelection::Descending { .. }
    ));
    eprintln!(
        "K6 fixed mixed-dot ray release lift: rows={}; columns={}; selected_sources={}; participating_rows={}; residuals={}; specialized_zero_terms={specialized_zero_terms}; complete_frame_fallback={complete_frame_fallback}",
        epoch.plan().row_count(),
        epoch.plan().columns().len(),
        circuit.source_combination().len(),
        participating_rows.len(),
        circuit.residual_terms().len(),
    );
    assert!(
        circuit
            .residual_terms()
            .iter()
            .all(|term| term.descent().domain() == epoch.fixed_stratum().domain())
    );
}
