//! `litered-reduce`: the UNCERTIFIED LiteRed lane over a saved candidate
//! bundle (RR-8). Loads the bundle under the parametric-vacuum admission,
//! builds the RustRed sector-symmetry census, expands the requested targets
//! into a Zp application DAG, compresses the candidate terminals with census
//! rows and returns exact coefficients onto the compressed basis at rational
//! parameter points (multi-prime CRT + MQRR, verified at a fresh prime).
//!
//! The output is candidate arithmetic: no source replay, no cover proof, no
//! minimality proof. It is labelled `status = "uncertified-litered"`.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use rustred::family::IntegralKey;
use rustred::reduction::ReductionLimits;
use rustred::sector::symmetry::{
    SectorSymmetryCensus, SectorSymmetryCensusControl, SectorSymmetryCensusOptions,
};
use rustred::sector::{Mask, zero};
use rustred::solver::CandidateFamilyAdmission;
use rustred::solver::litered::{
    ColumnOrder, LiteRedOptions, LiteRedSession, SymmetryRows, ZpSample,
};
use serde::Deserialize;
use serde_json::{Value, json};
use symbolica::domains::integer::Integer;
use symbolica::domains::rational::Rational;

use super::AppError;
use super::candidate_bundle::{
    CandidateBundleLimits, load_generated_candidate_bundle_with_admission,
};

pub const LITERED_REDUCE_SPEC_SCHEMA: &str = "rustred.litered-reduce-spec.v1";
pub const LITERED_REDUCE_OUTPUT_SCHEMA: &str = "rustred.litered-reduce-output.v1";

fn default_true() -> bool {
    true
}

/// JSON request of `rustred litered-reduce`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiteRedReduceSpec {
    pub schema: String,
    /// Path of a `family-candidates` bundle.
    pub bundle: std::path::PathBuf,
    /// `"parametric-vacuum"` (default) or `"unit-mass-vacuum"`.
    #[serde(default)]
    pub admission: Option<String>,
    /// Requests, in the bundle family's physical index order.
    pub targets: Vec<Vec<i64>>,
    /// Further integrals to reduce onto the same basis (e.g. a foreign basis
    /// for a basis-change check). Not counted in the target DAG statistics.
    #[serde(default)]
    pub extra_targets: Vec<Vec<i64>>,
    /// Preferred basis integrals, forced to the easy end of the column order.
    #[serde(default)]
    pub preferred: Vec<Vec<i64>>,
    #[serde(default = "default_true")]
    pub census: bool,
    /// `"group-orbit"` (default) or `"generators"`.
    #[serde(default)]
    pub rows: Option<String>,
    /// `"lines-dots-rank"` (default) or `"program"`.
    #[serde(default)]
    pub order: Option<String>,
    #[serde(default)]
    pub seed: Option<u64>,
    /// Rational points, each a map parameter name -> "p/q".
    #[serde(default)]
    pub points: Vec<BTreeMap<String, String>>,
    /// Additional random rational points (small numerators/denominators).
    #[serde(default)]
    pub random_points: usize,
    #[serde(default)]
    pub random_point_seed: Option<u64>,
    #[serde(default)]
    pub max_primes: Option<usize>,
    #[serde(default)]
    pub workers: Option<usize>,
    /// Number of targets also reduced by the exact candidate applier and
    /// compared with the Zp forward sweep at the structure sample.
    #[serde(default)]
    pub exact_cross_check: usize,
    /// Also report the compression with generator rows only.
    #[serde(default)]
    pub compare_generator_rows: bool,
    /// Raise the bundle byte and total-coefficient byte input limits.
    #[serde(default)]
    pub bundle_max_bytes: Option<usize>,
    /// Univariate reductions: each map fixes all parameters but one to
    /// rationals; the coefficients are reconstructed exactly in the free one.
    #[serde(default)]
    pub univariate: Vec<BTreeMap<String, String>>,
}

fn bundle_limits(spec: &LiteRedReduceSpec) -> CandidateBundleLimits {
    let mut limits = CandidateBundleLimits::default();
    if let Some(bytes) = spec.bundle_max_bytes {
        limits.max_bundle_bytes = limits.max_bundle_bytes.max(bytes);
        limits.max_total_coefficient_bytes = limits.max_total_coefficient_bytes.max(bytes);
    }
    limits
}

/// Deterministic SplitMix64 stream for reproducible sample points.
struct SplitMix(u64);

impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

fn key(powers: &[i64]) -> Result<IntegralKey, AppError> {
    IntegralKey::try_new(powers.iter().copied()).map_err(|e| AppError::input(e.to_string()))
}

fn parse_rational(text: &str) -> Result<Rational, AppError> {
    let parse = |part: &str| {
        part.trim()
            .parse::<i64>()
            .map_err(|_| AppError::input(format!("invalid rational {text:?}")))
    };
    let (numerator, denominator) = match text.split_once('/') {
        Some((n, d)) => (parse(n)?, parse(d)?),
        None => (parse(text)?, 1),
    };
    if denominator == 0 {
        return Err(AppError::input(format!("zero denominator in {text:?}")));
    }
    Ok(Rational::from((
        Integer::from(numerator),
        Integer::from(denominator),
    )))
}

fn keys_json(keys: &[IntegralKey]) -> Value {
    Value::Array(keys.iter().map(|k| json!(k.powers())).collect())
}

/// Run one `litered-reduce` request and return the JSON report.
pub fn litered_reduce(spec: &LiteRedReduceSpec) -> Result<Value, AppError> {
    if spec.schema != LITERED_REDUCE_SPEC_SCHEMA {
        return Err(AppError::input(format!(
            "litered-reduce spec schema must be {LITERED_REDUCE_SPEC_SCHEMA}"
        )));
    }
    let bytes = std::fs::read(&spec.bundle)
        .map_err(|e| AppError::input(format!("cannot read bundle: {e}")))?;
    let arity =
        super::candidate_bundle::inspect_generated_candidate_bundle(&bytes, bundle_limits(spec))?
            .arity;
    macro_rules! arms {
        ($($n:literal),* $(,)?) => {
            match rustred::campaign_storage_arity(arity) {
                $( $n => run::<$n>(spec, &bytes), )*
                _ => Err(AppError::input(format!("bundle arity {arity} is not compiled"))),
            }
        };
    }
    crate::ensure_runtime_arity(arity)?;
    rustred::with_app_runtime_arities!(arms)
}

fn run<const N: usize>(spec: &LiteRedReduceSpec, bytes: &[u8]) -> Result<Value, AppError> {
    let total = Instant::now();
    let admission = match spec.admission.as_deref() {
        None | Some("parametric-vacuum") => CandidateFamilyAdmission::ParametricVacuum,
        Some("unit-mass-vacuum") => CandidateFamilyAdmission::UnitMassVacuum,
        Some(other) => return Err(AppError::input(format!("unknown admission {other:?}"))),
    };
    let rows = match spec.rows.as_deref() {
        None | Some("group-orbit") => SymmetryRows::GroupOrbit,
        Some("generators") => SymmetryRows::Generators,
        Some(other) => return Err(AppError::input(format!("unknown rows {other:?}"))),
    };
    let order = match spec.order.as_deref() {
        None | Some("lines-dots-rank") => ColumnOrder::LinesDotsRank,
        Some("program") => ColumnOrder::Program,
        Some(other) => return Err(AppError::input(format!("unknown order {other:?}"))),
    };
    let start = Instant::now();
    let (family, reducer) = load_generated_candidate_bundle_with_admission::<N>(
        bytes,
        bundle_limits(spec),
        ReductionLimits::default(),
        admission,
    )?;
    let load_seconds = start.elapsed().as_secs_f64();
    let root = reducer.root_sector();
    let family = Arc::new(family);
    let parameters: Vec<String> = family.coefficient_context().parameter_names().to_vec();
    let start = Instant::now();
    let census = if spec.census {
        let analyzer = zero::Analyzer::try_unrestricted(&family)
            .map_err(|e| AppError::execution(e.to_string()))?;
        let top = Mask::try_new(root[..family.denominator_count()].iter().copied())
            .map_err(|e| AppError::input(e.to_string()))?;
        Some(
            SectorSymmetryCensus::discover_subsectors(
                Arc::clone(&family),
                &top,
                &analyzer,
                SectorSymmetryCensusOptions::default(),
                &SectorSymmetryCensusControl::default(),
            )
            .map_err(|e| AppError::execution(e.to_string()))?,
        )
    } else {
        None
    };
    let census_seconds = start.elapsed().as_secs_f64();
    let census = census.map(Arc::new);
    let reducer = Arc::new(reducer);
    let mut options = LiteRedOptions {
        order,
        rows,
        ..Default::default()
    };
    if let Some(seed) = spec.seed {
        options.seed = seed;
    }
    if let Some(max_primes) = spec.max_primes {
        options.max_primes = max_primes;
    }
    if let Some(workers) = spec.workers {
        options.workers = workers;
    }
    let mut session = LiteRedSession::new(Arc::clone(&reducer), census.clone(), options)
        .map_err(|e| AppError::execution(e.to_string()))?;
    let targets: Vec<IntegralKey> = spec
        .targets
        .iter()
        .map(|t| key(t))
        .collect::<Result<_, _>>()?;
    let extra: Vec<IntegralKey> = spec
        .extra_targets
        .iter()
        .map(|t| key(t))
        .collect::<Result<_, _>>()?;
    let preferred: Vec<IntegralKey> = spec
        .preferred
        .iter()
        .map(|t| key(t))
        .collect::<Result<_, _>>()?;
    let target_ids = session
        .add_targets(&targets)
        .map_err(|e| AppError::execution(e.to_string()))?;
    let target_dag_seconds = session.build_seconds();
    let target_stats = session.statistics(&target_ids);
    let sample_stats_targets = session.sample_statistics();
    let extra_ids = session
        .add_targets(&extra)
        .map_err(|e| AppError::execution(e.to_string()))?;
    // Forward versus adjoint at the structure sample.
    let forward = session
        .forward()
        .map_err(|e| AppError::execution(e.to_string()))?;
    let adjoint = session
        .adjoint(&target_ids)
        .map_err(|e| AppError::execution(e.to_string()))?;
    let mut sweeps_agree = true;
    for (row, &id) in adjoint.iter().zip(&target_ids) {
        let mut dense = vec![0u64; row.len()];
        if let Some(vector) = &forward[id as usize] {
            for &(terminal, value) in vector {
                dense[terminal as usize] = value;
            }
        } else {
            sweeps_agree = false;
        }
        if &dense != row {
            sweeps_agree = false;
        }
    }
    let reached: std::collections::BTreeSet<u32> = target_ids
        .iter()
        .filter_map(|&id| forward[id as usize].as_ref())
        .flatten()
        .map(|&(terminal, _)| terminal)
        .collect();
    // Exact candidate applier versus the Zp DAG on the first targets.
    let mut exact_checks = Vec::new();
    if spec.exact_cross_check > 0 {
        let mut exact = reducer.fork_empty();
        let sample = session.structure_sample().clone();
        let terminal_ids: BTreeMap<&IntegralKey, usize> = session
            .terminals()
            .iter()
            .enumerate()
            .map(|(id, key)| (key, id))
            .collect();
        for (target, &id) in targets.iter().zip(&target_ids).take(spec.exact_cross_check) {
            let start = Instant::now();
            let decomposition = exact
                .reduce_unit_mass(target)
                .map_err(|e| AppError::execution(e.to_string()))?;
            let mut dense = vec![0u64; session.terminals().len()];
            for (terminal, coefficient) in decomposition.terms() {
                let value = sample
                    .coefficient(coefficient)
                    .map_err(|e| AppError::execution(e.to_string()))?
                    .ok_or_else(|| AppError::execution("unlucky structure sample"))?;
                dense[terminal_ids[terminal]] = value;
            }
            let mut zp = vec![0u64; dense.len()];
            if let Some(vector) = &forward[id as usize] {
                for &(terminal, value) in vector {
                    zp[terminal as usize] = value;
                }
            }
            exact_checks.push(json!({
                "target": target.powers(),
                "exact_terms": decomposition.terms().len(),
                "agree": dense == zp,
                "exact_seconds": start.elapsed().as_secs_f64(),
            }));
        }
    }
    let mut generator_report = Value::Null;
    if spec.compare_generator_rows && census.is_some() {
        let mut other = LiteRedSession::new(
            Arc::clone(&reducer),
            census.clone(),
            LiteRedOptions {
                rows: SymmetryRows::Generators,
                ..options
            },
        )
        .map_err(|e| AppError::execution(e.to_string()))?;
        other
            .add_targets(&targets)
            .map_err(|e| AppError::execution(e.to_string()))?;
        let report = other
            .compress(&preferred)
            .map_err(|e| AppError::execution(e.to_string()))?;
        generator_report = json!({
            "rows": report.rows.rows,
            "rank": report.rank,
            "masters": report.masters.len(),
            "same_masters_as_main": Value::Null,
            "master_keys": keys_json(&report.masters),
        });
    }
    let compression = session
        .compress(&preferred)
        .map_err(|e| AppError::execution(e.to_string()))?;
    if let Value::Object(map) = &mut generator_report {
        let same = map["master_keys"] == keys_json(&compression.masters);
        map.insert("same_masters_as_main".into(), json!(same));
    }
    // Rational points.
    let mut points: Vec<Vec<Rational>> = Vec::new();
    for point in &spec.points {
        let mut values = Vec::new();
        for name in &parameters {
            let text = point
                .get(name)
                .ok_or_else(|| AppError::input(format!("point lacks parameter {name:?}")))?;
            values.push(parse_rational(text)?);
        }
        if point.len() != parameters.len() {
            return Err(AppError::input("point names unknown parameters"));
        }
        points.push(values);
    }
    let mut rng = SplitMix(spec.random_point_seed.unwrap_or(20261024));
    for _ in 0..spec.random_points {
        points.push(
            parameters
                .iter()
                .map(|_| {
                    let numerator = (rng.next() % 195) as i64 - 97;
                    let denominator = (rng.next() % 89) as i64 + 1;
                    Rational::from((Integer::from(numerator), Integer::from(denominator)))
                })
                .collect(),
        );
    }
    let all_ids: Vec<u32> = target_ids.iter().chain(&extra_ids).copied().collect();
    let all_keys: Vec<IntegralKey> = targets.iter().chain(&extra).cloned().collect();
    let mut point_reports = Vec::new();
    for point in &points {
        let reduction = session
            .reduce_exact_at_point(point, &all_ids)
            .map_err(|e| AppError::execution(e.to_string()))?;
        let rows: Vec<Value> = all_keys
            .iter()
            .zip(&reduction.coefficients)
            .map(|(target, row)| {
                let terms: Vec<Value> = row
                    .iter()
                    .enumerate()
                    .filter(|(_, value)| !value.is_zero())
                    .map(|(master, value)| json!([master, value.to_string()]))
                    .collect();
                json!({"target": target.powers(), "terms": terms})
            })
            .collect();
        point_reports.push(json!({
            "point": parameters.iter().zip(point).map(|(n, v)| (n.clone(), json!(v.to_string()))).collect::<serde_json::Map<_, _>>(),
            "primes_used": reduction.primes_used,
            "skipped_primes": reduction.skipped_primes,
            "seconds": reduction.seconds,
            "reductions": rows,
        }));
    }
    let mut univariate_reports = Vec::new();
    for fixed in &spec.univariate {
        let mut values = Vec::new();
        for name in &parameters {
            values.push(
                fixed
                    .get(name)
                    .map(|text| parse_rational(text))
                    .transpose()?,
            );
        }
        let reduction = session
            .reduce_univariate(&values, &all_ids)
            .map_err(|e| AppError::execution(e.to_string()))?;
        let rows: Vec<Value> = all_keys
            .iter()
            .zip(&reduction.coefficients)
            .map(|(target, row)| {
                let terms: Vec<Value> = row
                    .iter()
                    .enumerate()
                    .filter(|(_, function)| !function.is_zero())
                    .map(|(master, function)| {
                        json!([
                            master,
                            function
                                .numerator
                                .iter()
                                .map(|c| c.to_string())
                                .collect::<Vec<_>>(),
                            function
                                .denominator
                                .iter()
                                .map(|c| c.to_string())
                                .collect::<Vec<_>>(),
                        ])
                    })
                    .collect();
                json!({"target": target.powers(), "terms": terms})
            })
            .collect();
        univariate_reports.push(json!({
            "fixed": fixed,
            "free_parameter": parameters[reduction.free_parameter],
            "primes_used": reduction.primes_used,
            "skipped_primes": reduction.skipped_primes,
            "max_points": reduction.max_points,
            "seconds": reduction.seconds,
            "reductions": rows,
        }));
    }
    // A modular reduction at a second random Zp point (pivot stability).
    let second = {
        let field = rustred::solver::litered::PrimeField::new(
            rustred::solver::litered::reconstruction_primes(1)[0],
        )
        .map_err(|e| AppError::execution(e.to_string()))?;
        let mut rng = SplitMix(options.seed ^ 0x9e37_79b9);
        let base = parameters
            .iter()
            .map(|_| 1 + rng.next() % (field.prime() - 1))
            .collect();
        session
            .reduce_at(&ZpSample::new(field, base), &all_ids)
            .is_ok()
    };
    let classes: Vec<Value> = compression
        .classes
        .iter()
        .map(|class| {
            json!({
                "representative": class.representative.active_bits(),
                "representative_id": class.representative_id.to_string(),
                "members": class.members,
                "masters": class.masters,
            })
        })
        .collect();
    let census_json = census.as_ref().map(|census| {
        json!({
            "classes": census.classes().len(),
            "members": census.members().len(),
            "seconds": census_seconds,
        })
    });
    Ok(json!({
        "schema": LITERED_REDUCE_OUTPUT_SCHEMA,
        "status": "uncertified-litered",
        "note": "candidate formulas without source replay or cover proof; basis not proved minimal",
        "family_fingerprint": family.fingerprint(),
        "bundle_blake3": blake3::hash(bytes).to_hex().to_string(),
        "parameters": parameters,
        "admission": format!("{admission:?}"),
        "options": {
            "rows": format!("{rows:?}"),
            "order": format!("{order:?}"),
            "seed": options.seed,
            "max_primes": options.max_primes,
            "workers": options.workers,
        },
        "terminals": session.terminals().len(),
        "target_dag": {
            "targets": target_stats.targets,
            "nodes": target_stats.nodes,
            "rule_nodes": target_stats.rule_nodes,
            "edges": target_stats.edges,
            "terminals_reached": target_stats.terminals_reached,
            "uncovered": target_stats.uncovered,
            "zero_nodes": target_stats.zero_nodes,
            "sectors_touched": target_stats.sectors_touched,
            "max_rank_visited": target_stats.max_rank_visited,
            "max_dots_visited": target_stats.max_dots_visited,
            "term_applications": target_stats.term_applications,
            "residuals_in_result": reached.len(),
            "build_seconds": target_dag_seconds,
            "compiled_rules": sample_stats_targets.compiled_rules,
            "compiled_monomials": sample_stats_targets.compiled_monomials,
            "exact_zero_confirmations": sample_stats_targets.exact_zero_confirmations,
            "point_zeros": sample_stats_targets.point_zeros,
        },
        "forward_adjoint_agree": sweeps_agree,
        "exact_cross_check": exact_checks,
        "census": census_json,
        "compression": {
            "terminals": compression.terminals,
            "columns": compression.columns,
            "preferred_columns": compression.preferred_columns,
            "rows": compression.rows.rows,
            "route_rows": compression.rows.route_rows,
            "automorphism_rows": compression.rows.automorphism_rows,
            "orbit_rows": compression.rows.orbit_rows,
            "transports": compression.rows.transports,
            "max_orbit": compression.rows.max_orbit,
            "row_terms": compression.rows.row_terms,
            "rank": compression.rank,
            "masters": compression.masters.len(),
            "master_keys": keys_json(&compression.masters),
            "classes": classes,
            "dag_nodes_total": compression.dag_nodes,
            "row_seconds": compression.row_seconds,
            "elimination_seconds": compression.elimination_seconds,
        },
        "generator_rows_comparison": generator_report,
        "second_sample_pivots_stable": second,
        "points": point_reports,
        "univariate": univariate_reports,
        "seconds": {
            "load": load_seconds,
            "census": census_seconds,
            "total": total.elapsed().as_secs_f64(),
        },
    }))
}
