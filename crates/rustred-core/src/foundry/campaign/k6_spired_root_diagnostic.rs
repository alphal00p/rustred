//! Release-only reality check for the production SpIReD K6 ingress.
//!
//! This deliberately executes one bounded equality-case attempt for every
//! authenticated full-rank orbit.  It is diagnostic evidence, not a closure
//! loop: a hit, a committed owner, or exhaustion of these finite probes cannot
//! publish an artifact or prove that the translated ordinary module is
//! exhausted.

use std::time::{Duration, Instant};

use crate::foundry::artifact::FULL_RANK_ORBITS;
use crate::foundry::completion::LatticeBox;
use crate::foundry::completion::source_discovery::{
    CampaignLimits, CampaignModularProbe, ExactOwnerCoverSnapshot, ProbeCampaignLimits,
};
use crate::foundry::completion::spired::{
    SpiredCoordinateCaseEnqueueOutcome, SpiredCoordinateCaseObligation,
    SpiredCoordinateCaseWorklist, SpiredEqualityStepCensus, SpiredEqualityStepLimits,
    try_advance_spired_coordinate_case,
};
use crate::foundry::completion::stratum::{DecoratedStratum, StratumRegistryLimits};
use crate::identity::{CompletedIbpSourceRows, IntegralShift};
use crate::sector::{
    CoordinatePriority, InteriorBounds, Mask, OrderingPolicy, SectorMonotoneDomain,
};

use super::k6_resource::K6CampaignResourceProfile;
use super::preset_k6::{
    k6_root_predecessor_for_ordering, shared_k6_algebra_inputs,
    try_new_k6_full_rank_ledger_with_profile_and_ordering,
};

const K6_ARITY: usize = 6;
const DIAGNOSTIC_DEPTH: usize = 2;
const DIAGNOSTIC_EXACT_ROW_CAP: usize = 8;
const DIAGNOSTIC_PROBE_COUNT: usize = 4;
const DIAGNOSTIC_POST_HIT_WINDOW: usize = 32;

#[derive(Clone, Copy, Debug)]
struct DiagnosticConfig {
    orbit_ordinal: Option<usize>,
    ordering: OrderingPolicy,
    depth: usize,
    exact_row_cap: usize,
    probe_count: usize,
    post_hit_window: usize,
}

impl DiagnosticConfig {
    fn try_from_env() -> Result<Self, String> {
        let defaults = SpiredEqualityStepLimits::default();
        Ok(Self {
            orbit_ordinal: optional_orbit_from_env("RUSTRED_K6_SPIRED_ORBIT")?,
            ordering: ordering_from_env("RUSTRED_K6_SPIRED_ORDERING")?,
            depth: usize_from_env(
                "RUSTRED_K6_SPIRED_DEPTH",
                DIAGNOSTIC_DEPTH,
                0,
                defaults.target_run.max_depth_inclusive,
            )?,
            exact_row_cap: usize_from_env(
                "RUSTRED_K6_SPIRED_EXACT_ROW_CAP",
                DIAGNOSTIC_EXACT_ROW_CAP,
                0,
                defaults.target_run.compact_lift.exact.max_selected_rows,
            )?,
            probe_count: usize_from_env(
                "RUSTRED_K6_SPIRED_PROBE_COUNT",
                DIAGNOSTIC_PROBE_COUNT,
                1,
                DIAGNOSTIC_PROBE_COUNT,
            )?,
            post_hit_window: usize_from_env(
                "RUSTRED_K6_SPIRED_POST_HIT_WINDOW",
                DIAGNOSTIC_POST_HIT_WINDOW,
                0,
                defaults.target_run.max_alternative_streamed_rows,
            )?,
        })
    }
}

fn ordering_from_env(name: &str) -> Result<OrderingPolicy, String> {
    let Some(raw) = std::env::var_os(name) else {
        return Ok(OrderingPolicy::default());
    };
    let raw = raw
        .to_str()
        .ok_or_else(|| format!("{name} is not valid UTF-8"))?;
    if raw.eq_ignore_ascii_case("natural") {
        return Ok(OrderingPolicy::default());
    }
    if let Ok(policy) = OrderingPolicy::try_from_stable_id(raw) {
        return Ok(policy);
    }
    let ranks = raw
        .split(',')
        .map(str::trim)
        .map(|rank| {
            rank.parse::<usize>()
                .map_err(|error| format!("invalid rank {rank:?} in {name}: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let priority = CoordinatePriority::try_new(K6_ARITY, &ranks, Default::default())
        .map_err(|error| format!("invalid {name}={raw:?}: {error}"))?;
    OrderingPolicy::try_with_coordinate_priority(&priority)
        .map_err(|error| format!("invalid {name}={raw:?}: {error}"))
}

fn optional_orbit_from_env(name: &str) -> Result<Option<usize>, String> {
    let Some(raw) = std::env::var_os(name) else {
        return Ok(None);
    };
    let raw = raw
        .to_str()
        .ok_or_else(|| format!("{name} is not valid UTF-8"))?;
    if raw.eq_ignore_ascii_case("all") {
        return Ok(None);
    }
    let ordinal = raw
        .parse::<usize>()
        .map_err(|error| format!("invalid {name}={raw:?}: {error}"))?;
    if ordinal >= FULL_RANK_ORBITS.len() {
        return Err(format!(
            "{name}={ordinal} is outside 0..{}",
            FULL_RANK_ORBITS.len()
        ));
    }
    Ok(Some(ordinal))
}

fn usize_from_env(name: &str, default: usize, min: usize, max: usize) -> Result<usize, String> {
    let Some(raw) = std::env::var_os(name) else {
        return Ok(default);
    };
    let raw = raw
        .to_str()
        .ok_or_else(|| format!("{name} is not valid UTF-8"))?;
    let value = raw
        .parse::<usize>()
        .map_err(|error| format!("invalid {name}={raw:?}: {error}"))?;
    if !(min..=max).contains(&value) {
        return Err(format!("{name}={value} is outside {min}..={max}"));
    }
    Ok(value)
}

fn diagnostic_probes(
    stratum: &DecoratedStratum,
) -> Result<Vec<CampaignModularProbe>, Box<dyn std::error::Error>> {
    // These points lie close to the origin of every source-safe K6 carrier,
    // but deliberately exercise more than the all-corner specialization.
    // Each point is authenticated against the exact case before it is handed
    // to discovery.
    let specifications = [
        (998_244_353, 29, [0, 0, 0, 0, 0, 0]),
        (998_244_353, 43, [1, 1, 1, 1, 1, 1]),
        (1_000_000_007, 29, [2, 3, 5, 7, 11, 13]),
        (1_000_000_009, 43, [13, 11, 7, 5, 3, 2]),
    ];
    let mut probes = Vec::with_capacity(specifications.len());
    for (modulus, dimension, chart) in specifications {
        let probe =
            CampaignModularProbe::try_new(modulus, [dimension], chart, CampaignLimits::default())?;
        let anchor = probe.try_index_anchor_for_stratum(stratum)?;
        assert!(stratum.domain().contains(&anchor)?);
        probes.push(probe);
    }
    Ok(probes)
}

fn root_stratum_from_exact_ledger_carrier(
    completed: &CompletedIbpSourceRows,
    sector: &Mask,
    carrier: &LatticeBox,
) -> Result<DecoratedStratum, Box<dyn std::error::Error>> {
    assert_eq!(sector.arity(), K6_ARITY);
    assert_eq!(carrier.arity(), K6_ARITY);
    let mut bounds = Vec::with_capacity(K6_ARITY);
    for ((&local_lower, &local_upper), &active) in carrier
        .lower()
        .iter()
        .zip(carrier.upper())
        .zip(sector.active_bits())
    {
        let local_upper = local_upper.expect("a K6 diagnostic ledger carrier is finite");
        let (lower, upper) = if active {
            (i128::from(local_lower) + 1, i128::from(local_upper) + 1)
        } else {
            (-i128::from(local_upper), -i128::from(local_lower))
        };
        bounds.push(InteriorBounds::new(
            i64::try_from(lower)?,
            i64::try_from(upper)?,
        ));
    }

    let zero = IntegralShift::try_new([0; K6_ARITY])?;
    let structural_shifts = completed
        .relations()
        .iter()
        .flat_map(|relation| relation.terms().keys())
        .map(|shift| shift.values())
        .collect::<Vec<_>>();
    let domain = SectorMonotoneDomain::try_new_for_rule(
        sector.clone(),
        bounds,
        zero.values(),
        &structural_shifts,
    )?;
    Ok(DecoratedStratum::try_guard_blind(
        completed.family_fingerprint(),
        completed.context_fingerprint(),
        domain,
        StratumRegistryLimits::default(),
    )?)
}

fn diagnostic_limits(config: DiagnosticConfig) -> SpiredEqualityStepLimits {
    let mut limits = SpiredEqualityStepLimits::default();
    // Depth two is already 765 translated K6 source rows per probe.  This
    // short release sweep is intentionally repeatable; deeper resumable and
    // fixed-point campaigns remain separate milestones.
    limits.target_run.max_depth_inclusive = config.depth;
    limits.target_run.request_chunk_size = 512;
    limits.target_run.max_post_hit_streamed_rows = config.post_hit_window;
    limits.target_run.max_search_branches = 8;
    limits.target_run.max_distinct_compact_lift_attempts = 8;
    // The unconstrained first release attempt spent more than seven minutes
    // and grew beyond 700 MiB inside the first orbit's exact materialization.
    // Keep this repeatable sweep operationally bounded before entering
    // Symbolica's native rational-polynomial reducer. Hitting the cap is an
    // explicit incomplete/error result, never evidence against existence of
    // a larger exact witness.
    limits.target_run.compact_lift.exact.max_selected_rows = config.exact_row_cap;
    limits.max_probes = config.probe_count;
    limits
}

fn report_line(
    representative: [i64; K6_ARITY],
    elapsed: Duration,
    census: SpiredEqualityStepCensus,
    outcome: &str,
    snapshot: ExactOwnerCoverSnapshot,
    pending_cases: usize,
) {
    eprintln!(
        "K6 SpIReD root orbit={representative:?} elapsed_ms={:.3} outcome={outcome} \
         probes={}/{} singular={} requests={} rows={} hits={} exact_lifts={} admitted={} \
         guards={} owner_compiles={} commits={} pending_cases={} cover_status={:?} owners={} \
         terminals={} uncovered_boxes={} uncovered_finite={} missing_terminals={} guard_incomplete={}",
        elapsed.as_secs_f64() * 1_000.0,
        census.probes_attempted(),
        census.probes_offered(),
        census.singular_probes_skipped(),
        census.scheduled_requests(),
        census.streamed_rows(),
        census.modular_hits(),
        census.exact_lift_attempts(),
        census.admitted_candidates(),
        census.exact_guard_cases(),
        census.owner_compile_attempts(),
        census.committed_steps(),
        pending_cases,
        snapshot.status(),
        snapshot.owner_count(),
        snapshot.terminal_count(),
        snapshot.uncovered_box_count(),
        snapshot.uncovered_is_finite(),
        snapshot.missing_terminal_count(),
        snapshot.guard_incomplete_owner_count(),
    );
}

/// Run with:
///
/// ```text
/// cargo test --release -p rustred-core \
///   k6_spired_root_attempt_reports_every_full_rank_orbit -- --ignored --nocapture
/// ```
///
/// Optional controls are `RUSTRED_K6_SPIRED_ORBIT` (`all` or `0..5`),
/// `RUSTRED_K6_SPIRED_ORDERING` (an exact stable ID, `natural`, or a six-rank
/// CSV such as `5,3,4,2,0,1`), `RUSTRED_K6_SPIRED_DEPTH`,
/// `RUSTRED_K6_SPIRED_EXACT_ROW_CAP`, `RUSTRED_K6_SPIRED_PROBE_COUNT`, and
/// `RUSTRED_K6_SPIRED_POST_HIT_WINDOW`. They alter only this ignored test and
/// are validated against its production resource policies before setup.
#[test]
#[ignore = "release-only bounded K6 SpIReD diagnostic; this is not a closure claim"]
fn k6_spired_root_attempt_reports_every_full_rank_orbit() {
    let config = DiagnosticConfig::try_from_env().expect("valid bounded K6 diagnostic controls");
    let started_all = Instant::now();
    let setup_started = Instant::now();
    let inputs = shared_k6_algebra_inputs().expect("the authenticated K6 algebra must build");
    assert_eq!(inputs.completed().source_row_count(), 9);
    let setup_elapsed = setup_started.elapsed();
    eprintln!(
        "K6 SpIReD bounded root sweep setup_ms={:.3} orbit={:?} ordering={} depth={} \
         exact_selected_row_cap={} probes_per_orbit={} post_hit_window={}",
        setup_elapsed.as_secs_f64() * 1_000.0,
        config.orbit_ordinal,
        config.ordering.stable_id(),
        config.depth,
        config.exact_row_cap,
        config.probe_count,
        config.post_hit_window,
    );
    let mut attempted = 0usize;

    for (orbit_ordinal, orbit) in FULL_RANK_ORBITS.into_iter().enumerate() {
        if config
            .orbit_ordinal
            .is_some_and(|requested| requested != orbit_ordinal)
        {
            continue;
        }
        let predecessor = k6_root_predecessor_for_ordering(config.ordering)
            .expect("the K6 root predecessor must build");
        let profile = K6CampaignResourceProfile::try_for_task_report_ceiling(1)
            .expect("the one-step K6 resource profile must build");
        let mut ledger = try_new_k6_full_rank_ledger_with_profile_and_ordering(
            inputs,
            orbit.representative,
            predecessor,
            config.ordering,
            profile,
            ProbeCampaignLimits::default(),
        )
        .expect("the source-safe K6 ledger must build");
        let stratum = root_stratum_from_exact_ledger_carrier(
            inputs.completed(),
            ledger.sector(),
            ledger.closure_carrier(),
        )
        .expect("the exact ledger carrier must define its coordinate root");
        let probes = diagnostic_probes(&stratum).expect("diagnostic probes must be case-valid");
        let root = SpiredCoordinateCaseObligation::try_new_root(stratum)
            .expect("the exact ledger carrier must be an admissible logical root");
        let mut worklist = SpiredCoordinateCaseWorklist::new(Default::default());
        assert_eq!(
            worklist.try_enqueue(root).unwrap(),
            SpiredCoordinateCaseEnqueueOutcome::Inserted {
                removed_subsumed: 0,
            }
        );

        let started = Instant::now();
        match try_advance_spired_coordinate_case(
            inputs.generator(),
            inputs.completed(),
            &probes[..config.probe_count],
            &mut worklist,
            &mut ledger,
            diagnostic_limits(config),
        ) {
            Ok(report) => {
                let outcome = format!("{:?}", report.outcome());
                report_line(
                    orbit.representative,
                    started.elapsed(),
                    report.census(),
                    &outcome,
                    ledger.snapshot(),
                    worklist.len(),
                );
            }
            Err(error) => report_line(
                orbit.representative,
                started.elapsed(),
                error.census(),
                &format!("error:{error}"),
                ledger.snapshot(),
                worklist.len(),
            ),
        }
        attempted += 1;
    }

    assert_eq!(
        attempted,
        config.orbit_ordinal.map_or(FULL_RANK_ORBITS.len(), |_| 1)
    );
    eprintln!(
        "K6 SpIReD bounded root sweep depth={} exact_selected_row_cap={} \
         total_elapsed_ms={:.3}; no closure or publication inferred",
        config.depth,
        config.exact_row_cap,
        started_all.elapsed().as_secs_f64() * 1_000.0,
    );
}
