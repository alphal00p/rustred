//! Release-only bounded end-to-end drive of the production SpIReD equality
//! case coordinator on an authenticated K6 full-rank orbit.
//!
//! This test never publishes an artifact and never interprets a finite stop
//! as closure. Its sole purpose is to keep the real K6 path runnable while
//! exposing exact case, search, and owner-cover telemetry.

use std::time::Instant;

use crate::foundry::artifact::FULL_RANK_ORBITS;
use crate::foundry::completion::LatticeBox;
use crate::foundry::completion::source_discovery::ProbeCampaignLimits;
use crate::foundry::completion::spired::{
    SpiredCoordinateCaseObligation, SpiredSerialCaseOutcome, SpiredSerialDriverConfig,
    SpiredSerialProbePortfolio, SpiredSerialProbePortfolioLimits,
    try_drive_spired_serial_coordinate_cases, try_initialize_spired_serial_worklist,
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
const MODULI: [u64; 4] = [998_244_353, 1_000_000_007, 1_000_000_009, 1_000_000_021];

#[derive(Clone, Debug)]
struct SerialDiagnosticConfig {
    orbit_ordinal: Option<usize>,
    ordering: OrderingPolicy,
    depth: usize,
    exact_row_cap: usize,
    probe_count_per_diagonal: usize,
    base_parameter_values: Box<[i64]>,
    chart_rank_points: Box<[Box<[u64]>]>,
    post_hit_window: usize,
    case_limit: usize,
    row_limit: usize,
}

impl SerialDiagnosticConfig {
    fn try_from_env() -> Result<Self, String> {
        Ok(Self {
            orbit_ordinal: optional_orbit("RUSTRED_K6_SPIRED_SERIAL_ORBIT")?,
            ordering: ordering("RUSTRED_K6_SPIRED_SERIAL_ORDERING")?,
            depth: bounded_usize("RUSTRED_K6_SPIRED_SERIAL_DEPTH", 1, 0, 64)?,
            exact_row_cap: bounded_usize("RUSTRED_K6_SPIRED_SERIAL_EXACT_ROW_CAP", 4, 1, 4_096)?,
            probe_count_per_diagonal: bounded_usize(
                "RUSTRED_K6_SPIRED_SERIAL_PROBE_COUNT",
                1,
                1,
                MODULI.len(),
            )?,
            base_parameter_values: scalar_points("RUSTRED_K6_SPIRED_SERIAL_BASE_VALUES")?,
            chart_rank_points: chart_rank_points("RUSTRED_K6_SPIRED_SERIAL_CHART_POINTS")?,
            post_hit_window: bounded_usize(
                "RUSTRED_K6_SPIRED_SERIAL_POST_HIT_WINDOW",
                0,
                0,
                65_536,
            )?,
            case_limit: bounded_usize("RUSTRED_K6_SPIRED_SERIAL_CASE_LIMIT", 2, 1, 65_536)?,
            row_limit: bounded_usize("RUSTRED_K6_SPIRED_SERIAL_ROW_LIMIT", 2_048, 1, 67_108_864)?,
        })
    }
}

fn optional_orbit(name: &str) -> Result<Option<usize>, String> {
    let Some(raw) = std::env::var_os(name) else {
        return Ok(Some(0));
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

fn ordering(name: &str) -> Result<OrderingPolicy, String> {
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

fn bounded_usize(
    name: &str,
    default: usize,
    minimum: usize,
    maximum: usize,
) -> Result<usize, String> {
    let Some(raw) = std::env::var_os(name) else {
        return Ok(default);
    };
    let raw = raw
        .to_str()
        .ok_or_else(|| format!("{name} is not valid UTF-8"))?;
    let value = raw
        .parse::<usize>()
        .map_err(|error| format!("invalid {name}={raw:?}: {error}"))?;
    if !(minimum..=maximum).contains(&value) {
        return Err(format!("{name}={value} is outside {minimum}..={maximum}"));
    }
    Ok(value)
}

fn scalar_points(name: &str) -> Result<Box<[i64]>, String> {
    let Some(raw) = std::env::var_os(name) else {
        return Ok(vec![29, 43].into_boxed_slice());
    };
    let raw = raw
        .to_str()
        .ok_or_else(|| format!("{name} is not valid UTF-8"))?;
    let values = raw
        .split(',')
        .map(str::trim)
        .map(|value| {
            value
                .parse::<i64>()
                .map_err(|error| format!("invalid scalar point {value:?} in {name}: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if values.is_empty() || values.len() > 256 {
        return Err(format!("{name} must contain 1..=256 scalar points"));
    }
    Ok(values.into_boxed_slice())
}

fn chart_rank_points(name: &str) -> Result<Box<[Box<[u64]>]>, String> {
    let Some(raw) = std::env::var_os(name) else {
        let mut points = Vec::with_capacity(K6_ARITY + 1);
        points.push(vec![1_u64; K6_ARITY].into_boxed_slice());
        for axis in 0..K6_ARITY {
            let mut point = vec![1_u64; K6_ARITY];
            point[axis] = 2;
            points.push(point.into_boxed_slice());
        }
        return Ok(points.into_boxed_slice());
    };
    let raw = raw
        .to_str()
        .ok_or_else(|| format!("{name} is not valid UTF-8"))?;
    let points = raw
        .split(';')
        .map(str::trim)
        .map(|point| {
            let coordinates = point
                .split(',')
                .map(str::trim)
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|error| format!("invalid chart rank {value:?} in {name}: {error}"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            if coordinates.len() != K6_ARITY {
                return Err(format!(
                    "each {name} point must contain {K6_ARITY} ranks, got {}",
                    coordinates.len()
                ));
            }
            Ok(coordinates.into_boxed_slice())
        })
        .collect::<Result<Vec<_>, String>>()?;
    if points.is_empty() || points.len() > 256 {
        return Err(format!("{name} must contain 1..=256 chart-rank points"));
    }
    Ok(points.into_boxed_slice())
}

fn root_stratum(
    completed: &CompletedIbpSourceRows,
    sector: &Mask,
    carrier: &LatticeBox,
) -> Result<DecoratedStratum, Box<dyn std::error::Error>> {
    let mut bounds = Vec::with_capacity(sector.arity());
    for ((&local_lower, &local_upper), &active) in carrier
        .lower()
        .iter()
        .zip(carrier.upper())
        .zip(sector.active_bits())
    {
        let local_upper = local_upper.expect("K6 source-safe carrier must be finite");
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
    let zero = IntegralShift::try_new(std::iter::repeat_n(0, sector.arity()))?;
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

fn portfolio(config: &SerialDiagnosticConfig) -> SpiredSerialProbePortfolio {
    SpiredSerialProbePortfolio::try_new(
        MODULI[..config.probe_count_per_diagonal].iter().copied(),
        config
            .base_parameter_values
            .iter()
            .copied()
            .map(|value| [value]),
        config
            .chart_rank_points
            .iter()
            .map(|point| point.iter().copied()),
        SpiredSerialProbePortfolioLimits::default(),
    )
    .expect("the bounded deterministic K6 probe portfolio must build")
}

fn driver_config(
    config: &SerialDiagnosticConfig,
    portfolio: &SpiredSerialProbePortfolio,
) -> SpiredSerialDriverConfig {
    let mut limits = SpiredSerialDriverConfig::default();
    let probe_templates = portfolio.probe_template_count();
    limits.equality_step.target_run.max_depth_inclusive = config.depth;
    limits.equality_step.target_run.request_chunk_size = 512;
    limits.equality_step.target_run.max_post_hit_streamed_rows = config.post_hit_window;
    limits.equality_step.target_run.max_total_streamed_rows = config.row_limit;
    limits
        .equality_step
        .target_run
        .compact_lift
        .exact
        .max_selected_rows = config.exact_row_cap;
    limits.equality_step.max_probes = probe_templates;
    limits.equality_step.max_total_streamed_rows = config.row_limit;
    limits.limits.max_case_attempts = config.case_limit;
    limits.limits.max_case_reports = config.case_limit;
    limits.limits.max_existing_terminal_retirements = config.case_limit;
    limits.limits.max_streamed_rows = config.row_limit;
    limits.limits.max_generated_probes = config
        .case_limit
        .checked_mul(probe_templates)
        .expect("bounded aggregate generated-probe count");
    limits
}

/// Run with:
///
/// ```text
/// cargo test --release -p rustred k6_spired_serial_driver_attempt -- \
///   --ignored --nocapture
/// ```
///
/// Controls use the `RUSTRED_K6_SPIRED_SERIAL_` prefix: `ORBIT` (`0..5` or
/// `all`), `ORDERING` (`natural`, stable ID, or rank CSV), `DEPTH`,
/// `EXACT_ROW_CAP`, `PROBE_COUNT`, `BASE_VALUES` (dimension-value CSV),
/// `CHART_POINTS` (semicolon-separated rank vectors), `POST_HIT_WINDOW`,
/// `CASE_LIMIT`, and `ROW_LIMIT`.
#[test]
#[ignore = "release-only bounded K6 serial SpIReD diagnostic; never a closure claim"]
fn k6_spired_serial_driver_attempt() {
    assert!(
        !cfg!(debug_assertions),
        "the K6 serial diagnostic must run with --release"
    );
    let config = SerialDiagnosticConfig::try_from_env().expect("valid K6 serial controls");
    let started_all = Instant::now();
    let inputs = shared_k6_algebra_inputs().expect("authenticated K6 algebra must build");
    assert_eq!(inputs.completed().source_row_count(), 9);
    let portfolio = portfolio(&config);
    eprintln!(
        "K6 SpIReD serial config={config:?} templates_per_case={} source_rows={}",
        portfolio.probe_template_count(),
        inputs.completed().source_row_count(),
    );

    let mut attempted = 0usize;
    for (orbit_ordinal, orbit) in FULL_RANK_ORBITS.into_iter().enumerate() {
        if config
            .orbit_ordinal
            .is_some_and(|requested| requested != orbit_ordinal)
        {
            continue;
        }
        let started = Instant::now();
        let predecessor = k6_root_predecessor_for_ordering(config.ordering)
            .expect("K6 root predecessor must build");
        let profile = K6CampaignResourceProfile::try_for_task_report_ceiling(config.case_limit)
            .expect("coherent K6 resource profile must build");
        let mut ledger = try_new_k6_full_rank_ledger_with_profile_and_ordering(
            inputs,
            orbit.representative,
            predecessor,
            config.ordering,
            profile,
            ProbeCampaignLimits::default(),
        )
        .expect("source-safe K6 ledger must build");
        let carrier = ledger.closure_carrier();
        let root = SpiredCoordinateCaseObligation::try_new_root(
            root_stratum(inputs.completed(), ledger.sector(), carrier)
                .expect("source-safe K6 carrier must define a root"),
        )
        .expect("K6 root must be an equality-case obligation");
        let run_config = driver_config(&config, &portfolio);
        let mut worklist = try_initialize_spired_serial_worklist([root], run_config.worklist)
            .expect("K6 equality worklist must initialize");

        let report = try_drive_spired_serial_coordinate_cases(
            inputs.generator(),
            inputs.completed(),
            &portfolio,
            &mut worklist,
            &mut ledger,
            run_config,
        )
        .expect("the bounded K6 serial driver must not hard-fail");

        for (case_ordinal, case) in report.cases().iter().enumerate() {
            let disposition = match case.outcome() {
                SpiredSerialCaseOutcome::RuleCommitted(committed) => format!(
                    "rule-committed delta={:?} guard_children={} pending={}",
                    committed.owner_delta().kind(),
                    committed.proposed_guard_children(),
                    committed.retained_pending_cases(),
                ),
                SpiredSerialCaseOutcome::ExistingTerminalRetired {
                    integral,
                    authority,
                    delta,
                } => format!(
                    "existing-terminal integral={integral:?} authority={authority:?} delta={:?}",
                    delta.kind(),
                ),
                SpiredSerialCaseOutcome::Incomplete(reason) => {
                    format!("incomplete={reason:?}")
                }
            };
            eprintln!(
                "K6 SpIReD serial orbit={orbit_ordinal} case={case_ordinal} carrier={} id={} \
                 free_dimension={} probe_census={:?} equality_census={:?} outcome={disposition}",
                case.declared_carrier_id().as_str(),
                case.case_id().as_str(),
                case.free_dimension(),
                case.probe_census(),
                case.equality_census(),
            );
        }
        eprintln!(
            "K6 SpIReD serial orbit={orbit_ordinal} representative={:?} elapsed_ms={:.3} \
             census={:?} stop={:?} snapshot={:?} pending_cases={} closure_carrier={:?}",
            orbit.representative,
            started.elapsed().as_secs_f64() * 1_000.0,
            report.census(),
            report.stop(),
            ledger.snapshot(),
            worklist.len(),
            ledger.closure_carrier(),
        );
        attempted += 1;
    }
    assert_eq!(
        attempted,
        config.orbit_ordinal.map_or(FULL_RANK_ORBITS.len(), |_| 1)
    );
    eprintln!(
        "K6 SpIReD serial total_elapsed_ms={:.3}; no closure/publication inferred from bounded stops",
        started_all.elapsed().as_secs_f64() * 1_000.0,
    );
}
