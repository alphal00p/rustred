//! Generation-free saved application: inventory, export, and timed reductions.
//!
//! `saved_application inventory PUBLICATION`
//! `saved_application terminals APPLICATION`
//! `saved_application export PUBLICATION NEW_DIRECTORY`
//! `saved_application suite APPLICATION INVENTORY_JSON [OFFSET] [COUNT] [REPEATS] [CACHE_MODE] [CACHE_BYTES] [CACHE_REPRESENTATION] [LIMITS_JSON]`
//!
//! Inventory writes a small JSON document to stdout. Suite emits JSONL, one
//! completed receipt per concrete point, and never claims exhaustive closure.
//! Optional OFFSET/COUNT divide validation work; they do not change the package
//! or its mathematical scope. REPEATS defaults to three warm applications.
//! CACHE_MODE is independent (default, clear between cases) or shared.
//! CACHE_BYTES explicitly replaces the default cache byte/aggregate-term policy;
//! all other operational limits and the exact input scope stay unchanged.
//! CACHE_REPRESENTATION is sparse (default) or factorized (Symbolica-native).
//! LIMITS_JSON optionally overrides named operational work/cardinality limits;
//! it never changes admission or the mathematical rank/power scope.
//! `preparation_workers` in that object controls only native route preparation.
//! A failed request is recorded and its partial memoization cleared, so one
//! exhausted cache cannot manufacture failures for all later inventory cases.
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

use rustred::family::IntegralKey;
use rustred_app::{
    SavedApplicationOptions, export_saved_application, load_saved_application,
    saved_application_arity, saved_application_boundary_witnesses, saved_application_domains,
};
use serde_json::{Value, json};

#[path = "saved_application/limits.rs"]
mod limits;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        return Err("expected inventory SOURCE, export SOURCE DEST, or suite PACKAGE INVENTORY [OFFSET COUNT REPEATS]".into());
    }
    let arity = saved_application_arity(Path::new(&args[1]))?;
    macro_rules! dispatch {
        ($($n:literal),*) => {
            match rustred::campaign_storage_arity(arity) {
                $($n => run::<$n>(&args, arity),)*
                _ => Err("input arity is not compiled".into()),
            }
        }
    }
    rustred::with_app_runtime_arities!(dispatch)
}

fn run<const N: usize>(args: &[String], arity: usize) -> Result<()> {
    match args[0].as_str() {
        "inventory" if args.len() == 2 => inventory::<N>(Path::new(&args[1]), arity),
        "terminals" if args.len() == 2 => terminals::<N>(Path::new(&args[1])),
        "export" if args.len() == 3 => {
            let started = Instant::now();
            let report = export_saved_application::<N>(
                Path::new(&args[1]),
                Path::new(&args[2]),
                SavedApplicationOptions::default(),
            )?;
            println!(
                "{}",
                json!({"event":"exported", "wall_seconds":started.elapsed().as_secs_f64(), "report":report})
            );
            Ok(())
        }
        "suite" if (3..=10).contains(&args.len()) => suite::<N>(args, arity),
        _ => Err("invalid saved_application arguments".into()),
    }
}

fn terminals<const N: usize>(runtime: &Path) -> Result<()> {
    let started = Instant::now();
    let prepared = load_saved_application::<N>(runtime, SavedApplicationOptions::default())?;
    let mut family_ids = BTreeMap::new();
    let mut families = Vec::new();
    let rows: Vec<_> = prepared
        .remaining_terminals()
        .iter()
        .map(|terminal| {
            let next = families.len();
            let family_id = *family_ids
                .entry(terminal.family_fingerprint().to_owned())
                .or_insert_with(|| {
                    families.push(terminal.family_fingerprint().to_owned());
                    next
                });
            json!({"family":family_id,"powers":terminal.integral().powers()})
        })
        .collect();
    println!(
        "{}",
        json!({"schema":"rustred.saved-application-terminal-inventory.v1",
            "families":families,"terminals":rows,
            "seconds":started.elapsed().as_secs_f64(),"minimality_claim":false})
    );
    Ok(())
}

fn inventory<const N: usize>(source: &Path, arity: usize) -> Result<()> {
    let started = Instant::now();
    let domains = saved_application_domains::<N>(source)?;
    let mut cases: BTreeMap<Vec<i64>, Vec<Value>> = BTreeMap::new();
    for domain in &domains {
        if domain.is_empty {
            continue;
        }
        for witness in saved_application_boundary_witnesses(&domain.region)? {
            let powers = witness.integral.powers()[..arity].to_vec();
            cases.entry(powers).or_default().push(json!({
                "domain":domain.id, "boundary":format!("{:?}",witness.kind),
            }));
        }
    }
    let rows: Vec<_> = cases
        .into_iter()
        .enumerate()
        .map(|(i, (powers, origins))| {
            let positive: i128 = powers
                .iter()
                .filter(|&&n| n > 0)
                .map(|&n| i128::from(n))
                .sum();
            let numerator: i128 = powers
                .iter()
                .filter(|&&n| n < 0)
                .map(|&n| -i128::from(n))
                .sum();
            json!({"id":format!("point-{i:05}"), "powers":powers, "origins":origins,
            "positive_power":positive, "negative_index_degree":numerator,
            "power_difference":positive-numerator,
            "mass_dimension_of_inverse_denominator_numerator":2*numerator})
        })
        .collect();
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "schema":"rustred.saved-application-witnesses.v1", "arity":arity,
        "source":source, "recorded_domains":domains.len(), "distinct_cases":rows.len(),
        "empty_domains":domains.iter().filter(|domain|domain.is_empty).map(|domain|&domain.id).collect::<Vec<_>>(),
        "nonempty_domains":domains.iter().filter(|domain|!domain.is_empty).count(),
            "exhaustive_point_enumeration":false, "closure_claim":false,
            "wall_seconds":started.elapsed().as_secs_f64(), "cases":rows,
        }))?
    );
    Ok(())
}

fn suite<const N: usize>(args: &[String], arity: usize) -> Result<()> {
    let mut options = SavedApplicationOptions::default();
    if let Some(value) = args.get(7) {
        let bytes = value.parse::<usize>()?;
        if bytes == 0 {
            return Err("cache byte allowance must be positive".into());
        }
        options.reduction_limits.max_cached_coefficient_bytes = bytes;
        options.reduction_limits.max_cached_coefficient_terms = usize::MAX;
    }
    options.cache_representation = match args.get(8).map(String::as_str).unwrap_or("sparse") {
        "sparse" => rustred::solver::CandidateCacheRepresentation::Sparse,
        "factorized" => rustred::solver::CandidateCacheRepresentation::Factorized,
        _ => return Err("cache representation must be sparse or factorized".into()),
    };
    if let Some(path) = args.get(9) {
        limits::override_from_json(&mut options, Path::new(path))?;
    }
    let cache_mode = args.get(6).map(String::as_str).unwrap_or("independent");
    if !matches!(cache_mode, "independent" | "shared") {
        return Err("cache mode must be independent or shared".into());
    }
    let offset = args
        .get(3)
        .map(|s| s.parse::<usize>())
        .transpose()?
        .unwrap_or(0);
    let count = args
        .get(4)
        .map(|s| s.parse::<usize>())
        .transpose()?
        .unwrap_or(usize::MAX);
    let repeats = args
        .get(5)
        .map(|s| s.parse::<usize>())
        .transpose()?
        .unwrap_or(3);
    let document: Value = serde_json::from_slice(&std::fs::read(&args[2])?)?;
    if document["schema"] != "rustred.saved-application-witnesses.v1"
        || document["arity"].as_u64() != Some(arity as u64)
    {
        return Err("witness inventory schema or arity differs".into());
    }
    let rows = document["cases"]
        .as_array()
        .ok_or("inventory has no cases")?;
    let started = Instant::now();
    let mut prepared = load_saved_application::<N>(Path::new(&args[1]), options)?;
    let preparation = prepared.preparation_statistics();
    println!(
        "{}",
        json!({"event":"prepared", "seconds":started.elapsed().as_secs_f64(),
        "metadata":prepared.metadata(), "cases":rows.len(), "offset":offset,"requested_count":count,
        "cache_max_bytes":options.reduction_limits.max_cached_coefficient_bytes,
        "cache_representation":format!("{:?}",options.cache_representation),
        "operational_options":format!("{:?}",options),
        "owner_preparation":owner_preparation_json(&prepared),
        "preparation_seconds": {
            "total":preparation.total.as_secs_f64(),
            "metadata":preparation.metadata.as_secs_f64(),
            "terminal_maps":preparation.terminal_maps.as_secs_f64(),
            "owner_payloads":preparation.owner_payloads.as_secs_f64(),
            "owner_preparation":preparation.owner_preparation.as_secs_f64(),
            "route_preparation":preparation.route_preparation.as_secs_f64(),
            "other":preparation.other.as_secs_f64()
        },
        "cache_max_coefficient_terms":options.reduction_limits.max_cached_coefficient_terms})
    );
    let mut failures = 0;
    let mut completed = 0;
    for row in rows.iter().skip(offset).take(count) {
        if cache_mode == "independent" {
            prepared.clear_cache()?;
        }
        let powers: Vec<i64> = serde_json::from_value(row["powers"].clone())?;
        let target = IntegralKey::try_new(powers)?;
        let one = prepared.family().coefficient_context().one();
        let first_started = Instant::now();
        let first = match prepared.reduce_linear_combination([(target.clone(), one.clone())]) {
            Ok(value) => value,
            Err(error) => {
                failures += 1;
                println!(
                    "{}",
                    json!({"event":"failed", "case":row,
                    "seconds":first_started.elapsed().as_secs_f64(), "error":error.to_string(),
                    "cache_before_clear":format!("{:?}",prepared.cache_statistics()),
                    "owner_preparation":owner_preparation_json(&prepared),
                    "cache_cleared_after_failure":true})
                );
                prepared.clear_cache()?;
                continue;
            }
        };
        let first_seconds = first_started.elapsed().as_secs_f64();
        let mut warm = Vec::with_capacity(repeats);
        for _ in 0..repeats {
            let repeated_started = Instant::now();
            let repeated = prepared.reduce_linear_combination([(target.clone(), one.clone())])?;
            if repeated.terms() != first.terms()
                || repeated.nonzero_conditions() != first.nonzero_conditions()
            {
                return Err(format!("warm-cache exact mismatch at {}", row["id"]).into());
            }
            warm.push(json!({"seconds":repeated_started.elapsed().as_secs_f64(),
                "work":format!("{:?}",repeated.statistics())}));
        }
        completed += 1;
        println!(
            "{}",
            json!({"event":"reduced", "case":row, "first_seconds":first_seconds,
            "terms":first.terms().len(), "conditions":first.nonzero_conditions().len(),
            "cache_mode":cache_mode, "first_is_fresh_process":false,
            "terminal_substitution_additions":first.terminal_substitution_additions(),
            "first_work":format!("{:?}",first.statistics()),
            "owner_preparation":owner_preparation_json(&prepared),
            "cache":format!("{:?}",prepared.cache_statistics()),"warm_exact_equal":true,"warm":warm})
        );
    }
    println!(
        "{}",
        json!({"event":"finished", "completed":completed,"failed":failures,
        "wall_seconds":started.elapsed().as_secs_f64(),"closure_claim":false})
    );
    if failures != 0 {
        return Err(format!("{failures} admitted witness reductions failed").into());
    }
    Ok(())
}

fn owner_preparation_json<const N: usize>(
    prepared: &rustred_app::PreparedSavedApplication<N>,
) -> Value {
    let stats = prepared.owner_preparation_statistics();
    json!({"declared_owners":stats.declared_owners,
        "prepared_owners":stats.prepared_owners,"failed_owners":stats.failed_owners,
        "preparation_attempts":stats.preparation_attempts,
        "cumulative_seconds":stats.preparation_time.as_secs_f64()})
}
