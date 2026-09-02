//! Offline-only replay of the 55 AlphaLoop LHS domains through RustRed IBPs.

use std::collections::BTreeMap;
use std::time::Instant;

use crate::foundry::artifact::{
    MaterializedAlphaLoopLhsAnchor, materialize_alpha_loop_lhs_anchors,
    materialize_alpha_loop_lhs_anchors_with_ordering,
};
use crate::foundry::completion::LatticePoint;
use crate::foundry::completion::source_discovery::leader_walk::{
    LeaderWalkLimits, RequestedDomain, RequestedDomainScopePartition, RequestedDomainTask,
    try_plan_requested_domains,
};
use crate::foundry::completion::source_discovery::scheduler::{
    ProbeLocalBudgetCause, ProbeLocalBudgetScope,
};
use crate::foundry::completion::source_discovery::test_fixtures::OracleDisabledK6Fixture;
use crate::foundry::completion::source_discovery::{
    CampaignModularProbe, CanonicalExactOwnerLedger, ExactExecutableCandidateObstruction,
    ExactExecutableOwnerObstruction, ExactRuleCellGuardObstruction, ProbeCampaignOutcome,
};
use crate::identity::RowId;
use crate::sector::{Mask, OrderingPolicy};

use super::super::{ProbeCampaignAdapter, ProbeCampaignLimits, ProbeCampaignTaskReport};
const RAW_DOMAIN_COUNT: usize = 55;
const DEFAULT_PROBE_COUNT: usize = 6;
const MAX_PROBE_COUNT: usize = 6;
const MAX_RESIDUAL_ATTEMPTS_PER_DOMAIN: usize = 64;
const MAX_ADAPTIVE_WIDENING_ROUNDS: usize = 4;
const AGGREGATE_RESIDUAL_CANDIDATE_WORK: &str = "probe-local aggregate residual candidate work";
const AGGREGATE_RESIDUAL_SOURCE_TERM_WORK: &str = "probe-local aggregate residual source-term work";
const AGGREGATE_OBSTRUCTION_BLOCK_CANDIDATE_WORK: &str =
    "probe-local aggregate obstruction-block candidate work";
const AGGREGATE_OBSTRUCTION_BLOCK_SOURCE_TERM_WORK: &str =
    "probe-local aggregate obstruction-block source-term work";
const ALPHALOOP_WINNER_ORDERING_ID: &str = "rustred.unshifted-sector-order.v1;priority=rustred.coordinate-priority.v1;k=6;rank-by-slot=5,3,4,2,0,1";
const PROBE_SAMPLES: [(u64, i64); MAX_PROBE_COUNT] = [
    (1_000_000_007, 31),
    (1_000_000_007, 37),
    (1_000_000_009, 31),
    (1_000_000_009, 37),
    (998_244_353, 31),
    (998_244_353, 37),
];

#[derive(Default)]
struct DiagnosticCensus {
    domains: usize,
    fully_covered: usize,
    attempted_residuals: usize,
    strict_shrinks: usize,
    adaptive_widenings: usize,
    closed_sectors: usize,
    stalled_domains: usize,
    hard_failures: Vec<String>,
}

struct CanonicalDomainGroup<'a> {
    representative: &'a MaterializedAlphaLoopLhsAnchor,
    source_line_aliases: Vec<u16>,
}

#[test]
fn lhs_hint_itinerary_can_only_drive_regenerated_complete_ordinary_sources() {
    let fixture = OracleDisabledK6Fixture::shared();
    let completed = fixture.completed();
    assert!(completed.is_complete_ordinary());
    assert_eq!(completed.source_row_count(), 9);
    for ordinal in 0..completed.source_row_count() {
        assert_eq!(
            completed.source_row_id(ordinal),
            Some(&RowId::OrdinaryIbp {
                contraction_momentum: ordinal / 3,
                differentiated_loop: ordinal % 3,
            })
        );
    }

    // The hints bind only a requested domain. Candidate algebra remains
    // bound to the generator-owned complete ordinary barrier above.
    let adapter = ProbeCampaignAdapter::try_new(
        fixture.generator(),
        completed,
        fixture.zero_sources(),
        ProbeCampaignLimits::default(),
    )
    .unwrap();
    drop(adapter);
}

/// Run with, for example:
///
/// ```text
/// RUSTRED_ALPHA_LHS_LIMIT=3 cargo test --release -p rustred \
///   k6_alphaloop_lhs_domains_replay_through_regenerated_ordinary_ibps \
///   --lib -- --ignored --nocapture
/// ```
///
/// The runner first semantic-deduplicates all 55 patterns after authenticated
/// canonicalization. `RUSTRED_ALPHA_LHS_START` and
/// `RUSTRED_ALPHA_LHS_LIMIT` select a consecutive zero-based range of unique
/// domains; `RUSTRED_ALPHA_LHS_DOMAIN` selects exactly one unique ordinal.
/// Every report retains all original source-line aliases.
///
/// `RUSTRED_ALPHA_LHS_ORDERING` accepts a persisted `OrderingPolicy` stable
/// identity. It defaults to the exhaustive AlphaLoop ordering-sweep winner
/// `[5,3,4,2,0,1]`; use `rustred.unshifted-sector-order.v1` to reproduce the
/// frozen natural-order census.
///
/// `RUSTRED_ALPHA_LHS_PROBES=1..=6` bounds a deterministic portfolio over
/// three prime fields, two dimension samples, and several symbolic-axis chart
/// offsets. This only changes modular discovery; exact regeneration, replay,
/// guard analysis, and cover admission remain mandatory.
/// `RUSTRED_ALPHA_LHS_ADAPTIVE_WIDENING_ROUNDS=0..=4` deterministically
/// retries a typed budget stop by doubling only its named exhausted resource.
/// A different stop resource or any algebraic outcome ends that retry lane.
///
/// The input is LHS-domain metadata only. No AlphaLoop RHS shift, source row,
/// support, coefficient, or rule enters this primary diagnostic.
///
/// This first executable lane keeps one terminal-only ledger per canonical
/// sector. It does not yet transactionally publish closed lower-rank sibling
/// waves into the immutable predecessor of higher sectors. Consequently a
/// stall is scheduling/feedback evidence, never proof that no IBP exists.
#[test]
#[ignore = "offline AlphaLoop LHS itinerary; run explicitly in release mode"]
fn k6_alphaloop_lhs_domains_replay_through_regenerated_ordinary_ibps() {
    let probe_count = std::env::var("RUSTRED_ALPHA_LHS_PROBES")
        .ok()
        .map(|raw| {
            raw.parse::<usize>()
                .expect("probe count must be an integer")
        })
        .unwrap_or(DEFAULT_PROBE_COUNT);
    assert!((1..=MAX_PROBE_COUNT).contains(&probe_count));
    let adaptive_widening_rounds =
        parse_optional_usize("RUSTRED_ALPHA_LHS_ADAPTIVE_WIDENING_ROUNDS").unwrap_or(0);
    assert!(adaptive_widening_rounds <= MAX_ADAPTIVE_WIDENING_ROUNDS);
    let ordering_id = std::env::var("RUSTRED_ALPHA_LHS_ORDERING")
        .unwrap_or_else(|_| ALPHALOOP_WINNER_ORDERING_ID.to_owned());
    let ordering = OrderingPolicy::try_from_stable_id(&ordering_id)
        .expect("RUSTRED_ALPHA_LHS_ORDERING must be a canonical persisted policy identity");

    let fixture = OracleDisabledK6Fixture::shared();
    let limits = ProbeCampaignLimits::default();
    let adapter = ProbeCampaignAdapter::try_new(
        fixture.generator(),
        fixture.completed(),
        fixture.zero_sources(),
        limits,
    )
    .unwrap();
    let anchors = materialize_alpha_loop_lhs_anchors_with_ordering(ordering);
    assert_eq!(anchors.len(), RAW_DOMAIN_COUNT);
    let domains = semantic_domain_groups(&anchors);
    let predecessor = fixture.predecessor_for_ordering(ordering);
    let selected_domain = parse_optional_usize("RUSTRED_ALPHA_LHS_DOMAIN");
    let requested_start = parse_optional_usize("RUSTRED_ALPHA_LHS_START");
    let requested_limit = parse_optional_usize("RUSTRED_ALPHA_LHS_LIMIT");
    assert!(
        selected_domain.is_none() || (requested_start.is_none() && requested_limit.is_none()),
        "RUSTRED_ALPHA_LHS_DOMAIN cannot be combined with START or LIMIT"
    );
    let (start, limit) = if let Some(ordinal) = selected_domain {
        assert!(
            ordinal < domains.len(),
            "selected unique domain is out of range"
        );
        (ordinal, 1)
    } else {
        let start = requested_start.unwrap_or(0);
        assert!(start < domains.len(), "unique-domain start is out of range");
        let available = domains.len() - start;
        let limit = requested_limit.unwrap_or(available);
        assert!(
            (1..=available).contains(&limit),
            "unique-domain limit is out of range"
        );
        (start, limit)
    };
    let mut ledgers: BTreeMap<Mask, CanonicalExactOwnerLedger> = BTreeMap::new();
    let mut census = DiagnosticCensus::default();

    for (domain_ordinal, domain) in domains.iter().enumerate().skip(start).take(limit) {
        census.domains += 1;
        let sector = domain.representative.canonical_sector.clone();
        let ledger = ledgers.entry(sector.clone()).or_insert_with(|| {
            fixture.new_ledger_for_sector_with_ordering_and_predecessor(
                &sector,
                ordering,
                &predecessor,
            )
        });
        run_one_domain(
            domain_ordinal,
            domain,
            &adapter,
            ledger,
            limits,
            probe_count,
            adaptive_widening_rounds,
            &mut census,
        );
    }

    eprintln!(
        "ALPHA-LHS aggregate mode=independent-terminal-only-ledgers ordering={} raw_domains={} unique_domains={} selected_start={} selected_count={} probes_per_residual={} adaptive_widening_rounds={} adaptive_widenings={} covered={} residual_attempts={} strict_shrinks={} closed_sectors={} stalled={} hard_failures={}",
        ordering.stable_id(),
        anchors.len(),
        domains.len(),
        start,
        census.domains,
        probe_count,
        adaptive_widening_rounds,
        census.adaptive_widenings,
        census.fully_covered,
        census.attempted_residuals,
        census.strict_shrinks,
        census.closed_sectors,
        census.stalled_domains,
        census.hard_failures.len(),
    );
    for failure in &census.hard_failures {
        eprintln!("ALPHA-LHS failure {failure}");
    }
    assert_eq!(census.domains, limit);
    // Algebraic non-proposals are the purpose of this diagnostic and remain
    // typed output. Hard transaction/scope failures indicate a broken seam.
    assert!(
        census.hard_failures.is_empty(),
        "the LHS itinerary hit a hard planner/adapter failure"
    );
}

fn run_one_domain(
    domain_ordinal: usize,
    domain: &CanonicalDomainGroup<'_>,
    adapter: &ProbeCampaignAdapter<'_, '_, '_>,
    ledger: &mut CanonicalExactOwnerLedger,
    limits: ProbeCampaignLimits,
    probe_count: usize,
    adaptive_widening_rounds: usize,
    census: &mut DiagnosticCensus,
) {
    let domain_started = Instant::now();
    let anchor = domain.representative;
    let aliases = &domain.source_line_aliases;
    let request = [RequestedDomain::new(
        LatticePoint::try_new(anchor.canonical_point.coordinates().iter().copied()).unwrap(),
        anchor.canonical_symbolic_axes.iter().copied(),
    )];
    let fixed = anchor
        .canonical_integral
        .powers()
        .iter()
        .enumerate()
        .filter(|(position, _)| {
            anchor
                .canonical_symbolic_axes
                .binary_search(position)
                .is_err()
        })
        .map(|(position, &power)| (position, power))
        .collect::<Vec<_>>();
    let mut attempt = 0usize;
    loop {
        if attempt == MAX_RESIDUAL_ATTEMPTS_PER_DOMAIN {
            census.stalled_domains += 1;
            eprintln!(
                "ALPHA-LHS domain={domain_ordinal} aliases={aliases:?} sector={:?} axes={:?} fixed={fixed:?} result=residual-attempt-cap elapsed_ms={}",
                anchor.canonical_sector.active_bits(),
                anchor.canonical_symbolic_axes,
                domain_started.elapsed().as_millis(),
            );
            return;
        }
        let partition = match ledger.try_clone_uncovered_partition() {
            Ok(partition) => partition,
            Err(error) => {
                census.hard_failures.push(format!(
                    "domain={domain_ordinal} aliases={aliases:?} clone-uncovered: {error}"
                ));
                return;
            }
        };
        let scope = format!(
            "alphaloop-lhs|domain={domain_ordinal}|line={}|sector={:?}|revision={}",
            aliases[0],
            anchor.canonical_sector.active_bits(),
            ledger.revision().get(),
        );
        let plan = match try_plan_requested_domains(
            ledger.revision().get(),
            [RequestedDomainScopePartition::new(
                &scope,
                &anchor.canonical_sector,
                &partition,
                &request,
            )],
            LeaderWalkLimits::default(),
        ) {
            Ok(plan) => plan,
            Err(error) => {
                census.hard_failures.push(format!(
                    "domain={domain_ordinal} aliases={aliases:?} plan: {error}"
                ));
                return;
            }
        };
        let Some(task) = plan.tasks().first() else {
            assert_eq!(plan.fully_covered_domain_count(), 1);
            census.fully_covered += 1;
            eprintln!(
                "ALPHA-LHS domain={domain_ordinal} aliases={aliases:?} representative_raw_sector={} canonical_sector={:?} axes={:?} fixed={fixed:?} result=covered attempts={attempt} cover_boxes={} elapsed_ms={}",
                anchor.source.sector_bits,
                anchor.canonical_sector.active_bits(),
                anchor.canonical_symbolic_axes,
                ledger.snapshot().uncovered_box_count(),
                domain_started.elapsed().as_millis(),
            );
            return;
        };
        attempt += 1;
        census.attempted_residuals += 1;
        let before = ledger.snapshot();
        let base_probe = task.base_probe_chart_origin().collect::<Vec<_>>();
        let mut task_limits = limits;
        let mut widening_ordinal = 0usize;
        let mut first_widened_resource = None;
        let report = loop {
            let rebound_adapter = if task_limits == limits {
                None
            } else {
                match adapter.try_with_limits(task_limits) {
                    Ok(adapter) => Some(adapter),
                    Err(error) => {
                        census.hard_failures.push(format!(
                            "domain={domain_ordinal} aliases={aliases:?} rebind-limits residual={:?}..{:?}: {error}",
                            task.leader(),
                            task.key().residual_domain_upper(),
                        ));
                        return;
                    }
                }
            };
            let active_adapter = rebound_adapter.as_ref().unwrap_or(adapter);
            let binding = match active_adapter.try_bind_task(&plan, task, ledger) {
                Ok(binding) => binding,
                Err(error) => {
                    census.hard_failures.push(format!(
                        "domain={domain_ordinal} aliases={aliases:?} bind residual={:?}..{:?}: {error}",
                        task.leader(),
                        task.key().residual_domain_upper(),
                    ));
                    return;
                }
            };
            let probes = probe_portfolio(task, task_limits, probe_count);
            let report = match active_adapter.try_run_task(binding, ledger, probes) {
                Ok(report) => report,
                Err(error) => {
                    census.hard_failures.push(format!(
                        "domain={domain_ordinal} aliases={aliases:?} replay residual={:?}..{:?} base={base_probe:?}: {error}",
                        task.leader(),
                        task.key().residual_domain_upper(),
                    ));
                    return;
                }
            };
            if widening_ordinal == adaptive_widening_rounds
                || !matches!(report.outcome(), ProbeCampaignOutcome::NoProposal(_))
                || report.budget_stops().len() != 1
            {
                break report;
            }
            let stop = &report.budget_stops()[0];
            if first_widened_resource.is_some_and(|resource| resource != stop.resource()) {
                break report;
            }
            let Some(widening) = try_widen_named_budget(task_limits, stop.cause()) else {
                break report;
            };
            first_widened_resource.get_or_insert(widening.resource);
            widening_ordinal += 1;
            census.adaptive_widenings += 1;
            eprintln!(
                "ALPHA-LHS domain={domain_ordinal} aliases={aliases:?} adaptive-widening={widening_ordinal}/{adaptive_widening_rounds} stage={:?} resource={} requested={} limit={} next_limit={}",
                stop.stage(),
                widening.resource,
                widening.requested,
                widening.old_limit,
                widening.new_limit,
            );
            task_limits = widening.limits;
        };
        let (reason, strictly_shrank, closed) = summarize(&report);
        let after = ledger.snapshot();
        eprintln!(
            "ALPHA-LHS domain={domain_ordinal} aliases={aliases:?} representative_raw_sector={} canonical_sector={:?} axes={:?} fixed={fixed:?} attempt={} residual={:?}..{:?} base={base_probe:?} reason={reason} bootstrap=requests:{}/sources:{} replay={:?} exact_attempts={:?} exact_obstructions={} cover=r{}:{}->r{}:{} elapsed_ms={}",
            anchor.source.sector_bits,
            anchor.canonical_sector.active_bits(),
            anchor.canonical_symbolic_axes,
            attempt,
            task.leader(),
            task.key().residual_domain_upper(),
            report.census().bootstrap().requests(),
            report.census().bootstrap().selected_sources(),
            report.census().scheduler_outcomes(),
            report.census().canonical_attempts(),
            report.census().exact_obstructions(),
            before.revision().get(),
            before.uncovered_box_count(),
            after.revision().get(),
            after.uncovered_box_count(),
            domain_started.elapsed().as_millis(),
        );
        if strictly_shrank {
            census.strict_shrinks += 1;
            if closed {
                census.closed_sectors += 1;
            }
            // Rebuild the same requested recurrence against fresh exact
            // geometry. Residual lower endpoints remain cover state and are
            // never promoted into a different translated recurrence.
            continue;
        }
        census.stalled_domains += 1;
        return;
    }
}

fn parse_optional_usize(name: &str) -> Option<usize> {
    std::env::var(name).ok().map(|raw| {
        raw.parse::<usize>()
            .unwrap_or_else(|_| panic!("{name} must be an integer"))
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct NamedBudgetWidening {
    limits: ProbeCampaignLimits,
    resource: &'static str,
    requested: usize,
    old_limit: usize,
    new_limit: usize,
}

/// Widen exactly one supported aggregate resource named by an authoritative
/// scheduler stop. Other fields remain byte-for-byte equal. Count overflows,
/// allocation failures, nested-campaign stops, and unknown resources are not
/// adaptive instructions and deliberately return `None`.
fn try_widen_named_budget(
    limits: ProbeCampaignLimits,
    cause: &ProbeLocalBudgetCause,
) -> Option<NamedBudgetWidening> {
    let ProbeLocalBudgetCause::Outer {
        scope: ProbeLocalBudgetScope::Aggregate,
        resource,
        requested,
        limit,
    } = cause
    else {
        return None;
    };
    let scheduler = &limits.replay.scheduler;
    let configured_limit = match *resource {
        AGGREGATE_RESIDUAL_CANDIDATE_WORK => scheduler.max_aggregate_residual_candidate_work,
        AGGREGATE_RESIDUAL_SOURCE_TERM_WORK => scheduler.max_aggregate_residual_source_term_work,
        AGGREGATE_OBSTRUCTION_BLOCK_CANDIDATE_WORK => {
            scheduler.max_aggregate_obstruction_block_candidate_work
        }
        AGGREGATE_OBSTRUCTION_BLOCK_SOURCE_TERM_WORK => {
            scheduler.max_aggregate_obstruction_block_source_term_work
        }
        _ => return None,
    };
    if configured_limit != *limit {
        return None;
    }
    let next_limit = configured_limit.saturating_mul(2).max(*requested);
    if next_limit == configured_limit {
        return None;
    }
    let mut widened = limits;
    match *resource {
        AGGREGATE_RESIDUAL_CANDIDATE_WORK => {
            widened
                .replay
                .scheduler
                .max_aggregate_residual_candidate_work = next_limit;
        }
        AGGREGATE_RESIDUAL_SOURCE_TERM_WORK => {
            widened
                .replay
                .scheduler
                .max_aggregate_residual_source_term_work = next_limit;
        }
        AGGREGATE_OBSTRUCTION_BLOCK_CANDIDATE_WORK => {
            widened
                .replay
                .scheduler
                .max_aggregate_obstruction_block_candidate_work = next_limit;
        }
        AGGREGATE_OBSTRUCTION_BLOCK_SOURCE_TERM_WORK => {
            widened
                .replay
                .scheduler
                .max_aggregate_obstruction_block_source_term_work = next_limit;
        }
        _ => unreachable!("the supported resource was matched above"),
    }
    Some(NamedBudgetWidening {
        limits: widened,
        resource,
        requested: *requested,
        old_limit: configured_limit,
        new_limit: next_limit,
    })
}

fn semantic_domain_groups(
    anchors: &[MaterializedAlphaLoopLhsAnchor],
) -> Vec<CanonicalDomainGroup<'_>> {
    let mut groups: Vec<CanonicalDomainGroup<'_>> = Vec::new();
    for anchor in anchors {
        if let Some(existing) = groups.iter_mut().find(|group| {
            group.representative.canonical_sector == anchor.canonical_sector
                && group.representative.canonical_point == anchor.canonical_point
                && group.representative.canonical_symbolic_axes == anchor.canonical_symbolic_axes
        }) {
            existing.source_line_aliases.push(anchor.source.source_line);
        } else {
            groups.push(CanonicalDomainGroup {
                representative: anchor,
                source_line_aliases: vec![anchor.source.source_line],
            });
        }
    }
    groups
}

#[test]
fn canonical_lhs_domain_alias_census_is_semantically_deduplicated() {
    let anchors = materialize_alpha_loop_lhs_anchors();
    let domains = semantic_domain_groups(&anchors);
    assert_eq!(
        domains
            .iter()
            .map(|domain| domain.source_line_aliases.len())
            .sum::<usize>(),
        RAW_DOMAIN_COUNT
    );
    assert!(domains.len() < RAW_DOMAIN_COUNT);
    assert_eq!(&domains[0].source_line_aliases[..3], &[301, 313, 325]);
    for (left_ordinal, left) in domains.iter().enumerate() {
        assert!(
            left.source_line_aliases
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        );
        for right in domains.iter().skip(left_ordinal + 1) {
            assert!(
                left.representative.canonical_sector != right.representative.canonical_sector
                    || left.representative.canonical_point != right.representative.canonical_point
                    || left.representative.canonical_symbolic_axes
                        != right.representative.canonical_symbolic_axes
            );
        }
    }
    eprintln!(
        "ALPHA-LHS semantic census raw={} unique={} aliases={:?}",
        anchors.len(),
        domains.len(),
        domains
            .iter()
            .map(|domain| &domain.source_line_aliases)
            .collect::<Vec<_>>()
    );
}

#[test]
fn winner_priority_lhs_domain_alias_census_uses_winner_representatives() {
    let ordering = OrderingPolicy::try_from_stable_id(ALPHALOOP_WINNER_ORDERING_ID).unwrap();
    let anchors = materialize_alpha_loop_lhs_anchors_with_ordering(ordering);
    let domains = semantic_domain_groups(&anchors);
    assert_eq!(
        domains
            .iter()
            .map(|domain| domain.source_line_aliases.len())
            .sum::<usize>(),
        RAW_DOMAIN_COUNT
    );
    eprintln!(
        "ALPHA-LHS winner semantic census raw={} unique={} aliases={:?}",
        anchors.len(),
        domains.len(),
        domains
            .iter()
            .map(|domain| &domain.source_line_aliases)
            .collect::<Vec<_>>()
    );
}

fn summarize(report: &ProbeCampaignTaskReport) -> (String, bool, bool) {
    match report.outcome() {
        ProbeCampaignOutcome::NoProposal(reason) => (
            format!(
                "no-proposal:{reason:?};budget-stops={}",
                compact_budget_stops(report)
            ),
            false,
            false,
        ),
        ProbeCampaignOutcome::IncompleteProposal(proposal) => (
            format!(
                "incomplete:obstructions={}:first={}",
                proposal.obstructions().len(),
                proposal
                    .obstructions()
                    .first()
                    .map(compact_obstruction)
                    .unwrap_or_else(|| "none".to_owned())
            ),
            false,
            false,
        ),
        ProbeCampaignOutcome::Duplicate(applied) => (
            format!("duplicate:obstructions={}", applied.obstructions().len()),
            false,
            false,
        ),
        ProbeCampaignOutcome::ChangedWithoutGeometricShrink(applied) => (
            format!(
                "changed-no-shrink:obstructions={}",
                applied.obstructions().len()
            ),
            false,
            false,
        ),
        ProbeCampaignOutcome::StrictGeometricShrink(applied) => (
            format!(
                "strict-shrink:obstructions={}",
                applied.obstructions().len()
            ),
            true,
            false,
        ),
        ProbeCampaignOutcome::Closed { applied, .. } => (
            format!("closed:obstructions={}", applied.obstructions().len()),
            true,
            true,
        ),
    }
}

fn compact_budget_stops(report: &ProbeCampaignTaskReport) -> String {
    if report.budget_stops().is_empty() {
        return "none".to_owned();
    }
    report
        .budget_stops()
        .iter()
        .map(|stop| {
            format!(
                "p{}:e{}:{:?}:{:?}:{}:{:?}",
                stop.probe_ordinal(),
                stop.epoch_ordinal(),
                stop.stage(),
                stop.scope(),
                stop.resource(),
                stop.cause(),
            )
        })
        .collect::<Vec<_>>()
        .join("|")
}

fn probe_portfolio(
    task: &RequestedDomainTask,
    limits: ProbeCampaignLimits,
    probe_count: usize,
) -> Vec<CampaignModularProbe> {
    let origin = task.base_probe_chart_origin().collect::<Vec<_>>();
    PROBE_SAMPLES
        .iter()
        .take(probe_count)
        .enumerate()
        .map(|(probe_ordinal, &(modulus, dimension))| {
            let coordinates = origin
                .iter()
                .enumerate()
                .map(|(position, &base)| {
                    if task.key().symbolic_axes().binary_search(&position).is_err() {
                        return 0;
                    }
                    let requested_offset = ((probe_ordinal + position) % 3) as u64;
                    let available_from_leader = task.key().residual_domain_upper()[position]
                        .map_or(u64::MAX, |upper| upper - task.leader()[position]);
                    base.saturating_add(
                        requested_offset.min(available_from_leader.saturating_sub(base)),
                    )
                })
                .collect::<Vec<_>>();
            CampaignModularProbe::try_new(
                modulus,
                [dimension],
                coordinates,
                limits.replay.scheduler.campaign,
            )
            .unwrap()
        })
        .collect()
}

fn compact_obstruction(entry: &ExactExecutableCandidateObstruction) -> String {
    match entry.obstruction() {
        ExactExecutableOwnerObstruction::BlockedByKnownZero {
            required_predicate_ordinal,
            first_circuit_guard_ordinal,
            ..
        } => format!(
            "candidate={}:known-zero:required-predicate={required_predicate_ordinal}:first-circuit-guard={first_circuit_guard_ordinal}",
            entry.candidate_ordinal(),
        ),
        ExactExecutableOwnerObstruction::NeedsGuardedStratum {
            refinement,
            obstruction,
        } => {
            let detail = match obstruction {
                ExactRuleCellGuardObstruction::IntegerRoot {
                    guard_ordinal,
                    position,
                    value,
                } => {
                    format!("integer-root:guard={guard_ordinal}:position={position}:value={value}")
                }
                ExactRuleCellGuardObstruction::UnsupportedMultivariate { guard_ordinal } => {
                    format!("unsupported-multivariate:guard={guard_ordinal}")
                }
            };
            format!(
                "candidate={}:guarded-stratum:{detail}:required={}:new-splits={}:exceptional={}",
                entry.candidate_ordinal(),
                refinement.required_predicates().len(),
                refinement.newly_split_predicate_ordinals().len(),
                refinement.exceptional_strata().len(),
            )
        }
        ExactExecutableOwnerObstruction::AnchorOnGuardWall {
            refinement,
            guard_ordinal,
        } => format!(
            "candidate={}:anchor-on-guard-wall:guard={guard_ordinal}:required={}:new-splits={}:exceptional={}",
            entry.candidate_ordinal(),
            refinement.required_predicates().len(),
            refinement.newly_split_predicate_ordinals().len(),
            refinement.exceptional_strata().len(),
        ),
        ExactExecutableOwnerObstruction::ExceptionalGuardDomain { refinement, split } => format!(
            "candidate={}:exceptional-guard-domain:guard={}:position={}:value={}:required={}:new-splits={}:exceptional={}",
            entry.candidate_ordinal(),
            split.guard_ordinal(),
            split.position(),
            split.value(),
            refinement.required_predicates().len(),
            refinement.newly_split_predicate_ordinals().len(),
            refinement.exceptional_strata().len(),
        ),
    }
}

#[test]
fn adaptive_widening_changes_only_the_typed_named_resource() {
    for resource in [
        AGGREGATE_RESIDUAL_CANDIDATE_WORK,
        AGGREGATE_RESIDUAL_SOURCE_TERM_WORK,
        AGGREGATE_OBSTRUCTION_BLOCK_CANDIDATE_WORK,
        AGGREGATE_OBSTRUCTION_BLOCK_SOURCE_TERM_WORK,
    ] {
        let limits = ProbeCampaignLimits::default();
        let old_limit = configured_named_budget(limits, resource);
        let requested = old_limit + 17;
        let cause = ProbeLocalBudgetCause::Outer {
            scope: ProbeLocalBudgetScope::Aggregate,
            resource,
            requested,
            limit: old_limit,
        };
        let widening = try_widen_named_budget(limits, &cause).unwrap();
        assert_eq!(widening.resource, resource);
        assert_eq!(widening.requested, requested);
        assert_eq!(widening.old_limit, old_limit);
        assert_eq!(widening.new_limit, old_limit * 2);
        assert_eq!(
            configured_named_budget(widening.limits, resource),
            old_limit * 2
        );

        let mut expected = limits;
        set_named_budget(&mut expected, resource, old_limit * 2);
        assert_eq!(widening.limits, expected);
    }
}

#[test]
fn adaptive_widening_rejects_non_authoritative_or_stale_causes() {
    let limits = ProbeCampaignLimits::default();
    let resource = AGGREGATE_RESIDUAL_CANDIDATE_WORK;
    let old_limit = configured_named_budget(limits, resource);
    for cause in [
        ProbeLocalBudgetCause::Outer {
            scope: ProbeLocalBudgetScope::Probe,
            resource,
            requested: old_limit + 1,
            limit: old_limit,
        },
        ProbeLocalBudgetCause::Outer {
            scope: ProbeLocalBudgetScope::Aggregate,
            resource: "unrecognized aggregate resource",
            requested: old_limit + 1,
            limit: old_limit,
        },
        ProbeLocalBudgetCause::Outer {
            scope: ProbeLocalBudgetScope::Aggregate,
            resource,
            requested: old_limit + 1,
            limit: old_limit - 1,
        },
        ProbeLocalBudgetCause::CountOverflow {
            scope: ProbeLocalBudgetScope::Aggregate,
            resource,
        },
    ] {
        assert_eq!(try_widen_named_budget(limits, &cause), None);
    }

    let mut saturated = limits;
    set_named_budget(&mut saturated, resource, usize::MAX);
    assert_eq!(
        try_widen_named_budget(
            saturated,
            &ProbeLocalBudgetCause::Outer {
                scope: ProbeLocalBudgetScope::Aggregate,
                resource,
                requested: usize::MAX,
                limit: usize::MAX,
            },
        ),
        None
    );
}

fn configured_named_budget(limits: ProbeCampaignLimits, resource: &str) -> usize {
    match resource {
        AGGREGATE_RESIDUAL_CANDIDATE_WORK => {
            limits
                .replay
                .scheduler
                .max_aggregate_residual_candidate_work
        }
        AGGREGATE_RESIDUAL_SOURCE_TERM_WORK => {
            limits
                .replay
                .scheduler
                .max_aggregate_residual_source_term_work
        }
        AGGREGATE_OBSTRUCTION_BLOCK_CANDIDATE_WORK => {
            limits
                .replay
                .scheduler
                .max_aggregate_obstruction_block_candidate_work
        }
        AGGREGATE_OBSTRUCTION_BLOCK_SOURCE_TERM_WORK => {
            limits
                .replay
                .scheduler
                .max_aggregate_obstruction_block_source_term_work
        }
        _ => panic!("unknown named budget in test"),
    }
}

fn set_named_budget(limits: &mut ProbeCampaignLimits, resource: &str, value: usize) {
    match resource {
        AGGREGATE_RESIDUAL_CANDIDATE_WORK => {
            limits
                .replay
                .scheduler
                .max_aggregate_residual_candidate_work = value;
        }
        AGGREGATE_RESIDUAL_SOURCE_TERM_WORK => {
            limits
                .replay
                .scheduler
                .max_aggregate_residual_source_term_work = value;
        }
        AGGREGATE_OBSTRUCTION_BLOCK_CANDIDATE_WORK => {
            limits
                .replay
                .scheduler
                .max_aggregate_obstruction_block_candidate_work = value;
        }
        AGGREGATE_OBSTRUCTION_BLOCK_SOURCE_TERM_WORK => {
            limits
                .replay
                .scheduler
                .max_aggregate_obstruction_block_source_term_work = value;
        }
        _ => panic!("unknown named budget in test"),
    }
}
