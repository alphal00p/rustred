//! One explicit admitted key, identical work limits, two cache policies.
//! Preparation is separate from first/warm application. These are serial
//! handles in one process, not independent cold-process timing measurements.
use std::{path::Path, time::Instant};

use rustred::family::IntegralKey;
use rustred::solver::{CandidateRoutedCachePolicy, CandidateRoutedCacheStatistics};
use rustred_app::{
    PreparedSavedApplication, SavedApplicationOptions, SavedApplicationReduction,
    load_saved_application,
};
use serde_json::{Value, json};

use super::Result;

pub(super) fn run<const N: usize>(args: &[String], arity: usize) -> Result<()> {
    let document: Value = serde_json::from_slice(&std::fs::read(&args[2])?)?;
    let (case, target) = select_case(&document, &args[3], arity)?;
    let bounded_bytes = args[4].parse::<usize>()?;
    let reference_bytes = args
        .get(5)
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(
            SavedApplicationOptions::default()
                .reduction_limits
                .max_cached_coefficient_bytes,
        );
    let mut common = SavedApplicationOptions::default();
    if let Some(path) = args.get(6) {
        super::limits::override_from_json(&mut common, Path::new(path))?;
    }
    // The command's purpose fixes these two differences explicitly. All other
    // options (including term counts and work grants) remain exactly common.
    let (reference_options, bounded_options) = options(common, bounded_bytes, reference_bytes)?;
    println!(
        "{}",
        json!({
            "event":"cache_comparison_begin", "case":case,
            "reference_options":format!("{reference_options:?}"),
            "bounded_options":format!("{bounded_options:?}"),
            "timing_convention":"separate preparation and first/warm applications; one process, serial handles",
            "artifact_writes":false, "closure_claim":false
        })
    );
    let runtime = Path::new(&args[1]);
    let mut reference = load::<N>(runtime, reference_options, "reference")?;
    let fingerprint = reference.family().fingerprint().to_owned();
    let context = reference.family().coefficient_context().clone();
    let metadata = reference.metadata().clone();
    let expected = apply(&mut reference, &target, "reference", "first")?;
    let reference_warm = apply(&mut reference, &target, "reference", "warm")?;
    require_equal(&expected, &reference_warm, "reference warm")?;
    // Preserve only the exact output and small base coefficient context. Do
    // not retain two complete packages/owner spines for this serial comparison.
    drop(reference_warm);
    drop(reference);

    let mut bounded = load::<N>(runtime, bounded_options, "bounded_lru")?;
    if bounded.family().fingerprint() != fingerprint
        || !context.has_same_variable_map(bounded.family().coefficient_context())
        || bounded.metadata() != &metadata
    {
        return Err("package identity or native coefficient context changed between loads".into());
    }
    let actual = apply(&mut bounded, &target, "bounded_lru", "first")?;
    require_equal(&expected, &actual, "bounded first versus reference")?;
    let warm = apply(&mut bounded, &target, "bounded_lru", "warm")?;
    require_equal(&expected, &warm, "bounded warm versus reference")?;
    let evictions = bounded.cache_work_statistics();
    if evictions.evicted_entries == 0 {
        return Err("exact results matched, but the bounded run did not exercise eviction; choose a smaller CACHE_BYTES".into());
    }
    println!(
        "{}",
        json!({
            "event":"cache_comparison_passed", "case":case,
            "exact_terms_equal":true, "exact_ordered_conditions_equal":true,
            "reference_warm_exact_equal":true, "bounded_warm_exact_equal":true,
            "all_outputs_declared_terminals":true, "eviction_observed":true,
            "bounded_cache_work":cache_work(evictions), "closure_claim":false
        })
    );
    Ok(())
}

fn options(
    common: SavedApplicationOptions,
    bounded_bytes: usize,
    reference_bytes: usize,
) -> Result<(SavedApplicationOptions, SavedApplicationOptions)> {
    if bounded_bytes == 0 || reference_bytes == 0 || bounded_bytes >= reference_bytes {
        return Err(
            "require 0 < CACHE_BYTES < REFERENCE_BYTES (reference defaults to 1 GiB)".into(),
        );
    }
    let mut reference = common;
    reference.cache_policy = CandidateRoutedCachePolicy::FailOnLimit;
    reference.reduction_limits.max_cached_coefficient_bytes = reference_bytes;
    let mut bounded = common;
    bounded.cache_policy = CandidateRoutedCachePolicy::EvictLeastRecentlyUsed;
    bounded.reduction_limits.max_cached_coefficient_bytes = bounded_bytes;
    Ok((reference, bounded))
}

fn select_case(document: &Value, id: &str, arity: usize) -> Result<(Value, IntegralKey)> {
    if document["schema"] != "rustred.saved-application-witnesses.v1"
        || document["arity"].as_u64() != Some(arity as u64)
    {
        return Err("witness inventory schema or arity differs".into());
    }
    let mut matches = document["cases"]
        .as_array()
        .ok_or("inventory has no cases")?
        .iter()
        .filter(|row| row["id"].as_str() == Some(id));
    let row = matches
        .next()
        .ok_or("CASE_ID is absent from the inventory")?;
    if matches.next().is_some() {
        return Err("CASE_ID is duplicated in the inventory".into());
    }
    let powers: Vec<i64> = serde_json::from_value(row["powers"].clone())?;
    if powers.len() != arity {
        return Err("selected inventory key has the wrong physical arity".into());
    }
    Ok((row.clone(), IntegralKey::try_new(powers)?))
}

fn load<const N: usize>(
    runtime: &Path,
    options: SavedApplicationOptions,
    label: &str,
) -> Result<PreparedSavedApplication<N>> {
    let started = Instant::now();
    let prepared = match load_saved_application::<N>(runtime, options) {
        Ok(prepared) => prepared,
        Err(error) => {
            println!(
                "{}",
                json!({"event":"cache_comparison_failed", "run":label,
                "phase":"preparation", "seconds":started.elapsed().as_secs_f64(), "error":error.to_string()})
            );
            return Err(error.into());
        }
    };
    let preparation = prepared.preparation_statistics();
    println!(
        "{}",
        json!({"event":"cache_comparison_prepared", "run":label,
        "seconds":started.elapsed().as_secs_f64(), "metadata":prepared.metadata(),
        "preparation_seconds":{
            "total":preparation.total.as_secs_f64(),
            "metadata":preparation.metadata.as_secs_f64(),
            "terminal_maps":preparation.terminal_maps.as_secs_f64(),
            "owner_payloads":preparation.owner_payloads.as_secs_f64(),
            "owner_preparation":preparation.owner_preparation.as_secs_f64(),
            "route_preparation":preparation.route_preparation.as_secs_f64(),
            "other":preparation.other.as_secs_f64()},
        "owner_preparation":super::owner_preparation_json(&prepared)})
    );
    Ok(prepared)
}

fn apply<const N: usize>(
    prepared: &mut PreparedSavedApplication<N>,
    target: &IntegralKey,
    label: &str,
    phase: &str,
) -> Result<SavedApplicationReduction> {
    let one = prepared.family().coefficient_context().one();
    let started = Instant::now();
    let result = match prepared.reduce_linear_combination([(target.clone(), one)]) {
        Ok(result) => result,
        Err(error) => {
            println!(
                "{}",
                json!({"event":"cache_comparison_failed", "run":label,
                "phase":phase, "seconds":started.elapsed().as_secs_f64(), "error":error.to_string(),
                "cache":format!("{:?}",prepared.cache_statistics()),
                "cache_work":cache_work(prepared.cache_work_statistics()),
                "owner_preparation":super::owner_preparation_json(prepared)})
            );
            return Err(error.into());
        }
    };
    // Stop application timing before exact comparisons/diagnostic rendering.
    let seconds = started.elapsed().as_secs_f64();
    let declared = result
        .terms()
        .keys()
        .all(|key| prepared.remaining_terminals().contains(key));
    println!(
        "{}",
        json!({"event":"cache_comparison_reduced", "run":label,
        "phase":phase, "seconds":seconds, "first_is_fresh_process":false,
        "terms":result.terms().len(), "conditions":result.nonzero_conditions().len(),
        "all_outputs_declared_terminals":declared,
        "work":format!("{:?}",result.statistics()),
        "request_cache_work":cache_work(result.statistics().cache),
        "cache_work":cache_work(prepared.cache_work_statistics()),
        "cache":format!("{:?}",prepared.cache_statistics()),
        "terminal_substitution_additions":result.terminal_substitution_additions(),
        "owner_preparation":super::owner_preparation_json(prepared)})
    );
    if !declared {
        return Err(
            "a comparison output is outside the authenticated remaining terminal set".into(),
        );
    }
    Ok(result)
}

fn require_equal(
    expected: &SavedApplicationReduction,
    actual: &SavedApplicationReduction,
    label: &str,
) -> Result<()> {
    let terms_equal = expected.terms() == actual.terms();
    let guards_equal = expected.nonzero_conditions() == actual.nonzero_conditions();
    if !terms_equal || !guards_equal {
        println!(
            "{}",
            json!({"event":"cache_comparison_failed", "phase":"exact_comparison",
            "comparison":label, "exact_terms_equal":terms_equal,
            "exact_ordered_conditions_equal":guards_equal})
        );
        return Err(format!("exact coefficient/guard mismatch: {label}").into());
    }
    Ok(())
}

fn cache_work(stats: CandidateRoutedCacheStatistics) -> Value {
    json!({"evicted_entries":stats.evicted_entries,
        "evicted_coefficient_terms":stats.evicted_coefficient_terms,
        "evicted_coefficient_bytes":stats.evicted_coefficient_bytes,
        "same_request_recomputed_nodes":stats.recomputed_nodes})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparison_changes_only_explicit_cache_policy_and_bytes() {
        let mut common = SavedApplicationOptions::default();
        common.reduction_limits.max_rule_applications = 37;
        common.reduction_limits.max_cached_coefficient_terms = 91;
        common.preparation_workers = 8;
        let (mut reference, mut bounded) = options(common, 1024, 4096).unwrap();
        assert_eq!(
            reference.cache_policy,
            CandidateRoutedCachePolicy::FailOnLimit
        );
        assert_eq!(
            bounded.cache_policy,
            CandidateRoutedCachePolicy::EvictLeastRecentlyUsed
        );
        assert_eq!(
            reference.reduction_limits.max_cached_coefficient_bytes,
            4096
        );
        assert_eq!(bounded.reduction_limits.max_cached_coefficient_bytes, 1024);
        reference.cache_policy = common.cache_policy;
        bounded.cache_policy = common.cache_policy;
        reference.reduction_limits.max_cached_coefficient_bytes =
            common.reduction_limits.max_cached_coefficient_bytes;
        bounded.reduction_limits.max_cached_coefficient_bytes =
            common.reduction_limits.max_cached_coefficient_bytes;
        assert_eq!(reference, common);
        assert_eq!(bounded, common);
        for (bounded, reference) in [(0, 1), (1, 0), (2, 2), (3, 2)] {
            assert!(options(common, bounded, reference).is_err());
        }
    }

    #[test]
    fn inventory_selection_is_exact_and_rejects_duplicate_or_malformed_keys() {
        let document = json!({"schema":"rustred.saved-application-witnesses.v1", "arity":2,
            "cases":[{"id":"point-00163", "powers":[1,-2]}, {"id":"other", "powers":[2,0]}]});
        let (_, key) = select_case(&document, "point-00163", 2).unwrap();
        assert_eq!(key.powers(), [1, -2]);
        assert!(select_case(&document, "163", 2).is_err());
        assert!(select_case(&document, "point-00163", 3).is_err());
        let mut duplicate = document.clone();
        duplicate["cases"]
            .as_array_mut()
            .unwrap()
            .push(document["cases"][0].clone());
        assert!(select_case(&duplicate, "point-00163", 2).is_err());
        let mut malformed = document;
        malformed["cases"][0]["powers"] = json!([1]);
        assert!(select_case(&malformed, "point-00163", 2).is_err());
    }
}
