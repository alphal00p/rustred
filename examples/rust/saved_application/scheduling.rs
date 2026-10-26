//! A single cold sample and a separate, reduction-free exact comparison.
//!
//! Request JSON: {"schema":"rustred.saved-application-scheduling-request.v1",
//! "id":"tuning-id","arity":1,"entry_mode":"direct",
//! "terms":[{"powers":[3],"coefficient":"(d+1)/(d-4)"}]}.
//! entry_mode defaults to direct; routed explicitly uses the saved entry maps.
//! Orders: canonical, numerator-rank-descending, positive-power-sum-descending,
//! propagator-count-descending, corner-distance-descending, coefficient-terms-descending.
//! Samples contain full exact terminal coefficients and raw nonzero conditions;
//! they are trusted generated receipts, not independently proved identities.
//! No command here grants scientific/campaign admission or launches another job.
//! Actual 5L sampling and checking both require external admission of Native
//! data loading before invocation; no generic CLI gate can authorize that work.
//! Optional compile-time RUSTRED_SCHEDULING_SOURCE_REVISION and
//! RUSTRED_SCHEDULING_COHORT_LABEL are labels, not executable authentication.
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::Instant;

use rustred::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};
use rustred::family::IntegralKey;
use rustred::solver::{CandidateRoutedScheduling, CandidateRoutedWorkOrder, UnresolvedPolicy};
use rustred_app::{PreparedSavedApplication, SavedApplicationOptions, load_saved_application};
use serde::Deserialize;
use serde_json::{Value, json};
use symbolica::prelude::*;

use super::Result;

const REQUEST_SCHEMA: &str = "rustred.saved-application-scheduling-request.v1";
const SAMPLE_SCHEMA: &str = "rustred.saved-application-scheduling-sample.v1";

fn build_identity() -> Value {
    json!({
        "source_revision":option_env!("RUSTRED_SCHEDULING_SOURCE_REVISION")
            .filter(|value| !value.trim().is_empty()),
        "cohort_label":option_env!("RUSTRED_SCHEDULING_COHORT_LABEL")
            .filter(|value| !value.trim().is_empty()),
        "provenance_requirement":"External ELF/source manifest required when a label is absent; supplied labels also require external authentication.",
    })
}

fn parse_peak_rss_bytes(status: &str) -> Option<u64> {
    let mut fields = status
        .lines()
        .find_map(|line| line.strip_prefix("VmHWM:"))?
        .split_whitespace();
    let kib = fields.next()?.parse::<u64>().ok()?;
    if fields.next()? != "kB" || fields.next().is_some() {
        return None;
    }
    kib.checked_mul(1024)
}

fn peak_process_rss_bytes() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|status| parse_peak_rss_bytes(&status))
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum EntryMode {
    #[default]
    Direct,
    Routed,
}
impl EntryMode {
    fn name(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Routed => "routed",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema: String,
    id: String,
    arity: usize,
    #[serde(default)]
    entry_mode: EntryMode,
    terms: Vec<InputTerm>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InputTerm {
    powers: Vec<i64>,
    coefficient: String,
}

fn parse_request(bytes: &[u8], arity: usize) -> Result<Request> {
    let request: Request = serde_json::from_slice(bytes)?;
    if request.schema != REQUEST_SCHEMA || request.arity != arity {
        return Err("scheduling request schema or physical arity differs".into());
    }
    if request.id.trim().is_empty() || request.terms.is_empty() {
        return Err("scheduling request needs a nonempty id and input term list".into());
    }
    if request
        .terms
        .iter()
        .any(|term| term.powers.len() != arity || term.coefficient.trim().is_empty())
    {
        return Err("scheduling input has incorrect arity or an empty coefficient".into());
    }
    Ok(request)
}

fn order(name: &str) -> Result<CandidateRoutedWorkOrder> {
    Ok(match name {
        "canonical" => CandidateRoutedWorkOrder::Canonical,
        "numerator-rank-descending" => CandidateRoutedWorkOrder::NumeratorRankDescending,
        "positive-power-sum-descending" => CandidateRoutedWorkOrder::PositivePowerSumDescending,
        "propagator-count-descending" => CandidateRoutedWorkOrder::PropagatorCountDescending,
        "corner-distance-descending" => CandidateRoutedWorkOrder::CornerDistanceDescending,
        "coefficient-terms-descending" => CandidateRoutedWorkOrder::CoefficientTermsDescending,
        _ => return Err(format!("unknown scheduling order {name}").into()),
    })
}

fn coefficient(
    text: &str,
    context: &CoefficientContext,
    limits: ExactAlgebraLimits,
) -> Result<Coefficient> {
    let variables = context.one().get_variables().clone();
    let atom = try_parse!(text, default_namespace = "rustred")
        .map_err(|error| format!("invalid exact coefficient: {error}"))?;
    let value = atom
        .as_view()
        .try_to_rational_polynomial(&Q, &Z, Some(variables))
        .map_err(|error| format!("coefficient is not an exact rational polynomial: {error}"))?;
    context.validate_with_limits(&value, limits)?;
    Ok(value)
}

fn load<const N: usize>(
    path: &Path,
    options: SavedApplicationOptions,
) -> Result<(PreparedSavedApplication<N>, String)> {
    let manifest = if path.is_dir() {
        path.join("application.json")
    } else {
        path.to_path_buf()
    };
    let before = std::fs::read(&manifest)?;
    let prepared = load_saved_application::<N>(path, options)?;
    if std::fs::read(&manifest)? != before {
        return Err("saved application manifest changed during authenticated load".into());
    }
    Ok((prepared, blake3::hash(&before).to_hex().to_string()))
}

fn package_identity<const N: usize>(
    prepared: &PreparedSavedApplication<N>,
    manifest_hash: &str,
) -> Value {
    json!({
        "family_fingerprint":prepared.family().fingerprint(),
        "physical_arity":prepared.family().denominator_count(),
        "parameters":prepared.family().coefficient_context().parameter_names(),
        "manifest_blake3":manifest_hash,
        "queries_blake3":blake3::hash(prepared.queries_json().as_bytes()).to_hex().to_string(),
        "selection_blake3":blake3::hash(prepared.selection_json().as_bytes()).to_hex().to_string(),
    })
}

pub(super) fn sample<const N: usize>(args: &[String], arity: usize) -> Result<()> {
    let request_bytes = std::fs::read(&args[2])?;
    let request = parse_request(&request_bytes, arity)?;
    let mut options = SavedApplicationOptions::default();
    if let Some(path) = args.get(6) {
        super::limits::override_from_json(&mut options, Path::new(path))?;
    }
    options.unresolved_policy = UnresolvedPolicy::Reject;
    options.scheduling = CandidateRoutedScheduling {
        roots: order(&args[3])?,
        dependencies: order(&args[4])?,
    };
    let mut common_options = options;
    common_options.scheduling = CandidateRoutedScheduling::default();
    let output = Path::new(&args[5]);
    // Reject an unusable destination before consuming the one-shot request.
    // create_new also protects existing receipts against concurrent launches.
    let output_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    let started = Instant::now();
    let (mut prepared, manifest_hash) = load::<N>(Path::new(&args[1]), options)?;
    let load_seconds = started.elapsed().as_secs_f64();
    let preparation = prepared.preparation_statistics();
    let preparation_json = json!({
        "total":preparation.total.as_secs_f64(),
        "metadata":preparation.metadata.as_secs_f64(),
        "terminal_maps":preparation.terminal_maps.as_secs_f64(),
        "owner_payloads":preparation.owner_payloads.as_secs_f64(),
        "owner_preparation":preparation.owner_preparation.as_secs_f64(),
        "route_preparation":preparation.route_preparation.as_secs_f64(),
        "other":preparation.other.as_secs_f64(),
    });
    let ingress_started = Instant::now();
    let inputs = request
        .terms
        .iter()
        .map(|term| {
            Ok((
                IntegralKey::try_new(term.powers.iter().copied())?,
                coefficient(
                    &term.coefficient,
                    prepared.family().coefficient_context(),
                    options.reduction_limits.exact_algebra,
                )?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let ingress_seconds = ingress_started.elapsed().as_secs_f64();
    let initial_cache = prepared.cache_statistics();
    if initial_cache.cached_integrals() != 0 {
        return Err("a fresh scheduling sample must start with an empty application cache".into());
    }
    // Exactly one application, without warm-up, retries, backing or capture.
    let started = Instant::now();
    let reduced = match request.entry_mode {
        EntryMode::Direct => prepared.reduce_linear_combination(inputs),
        EntryMode::Routed => prepared.reduce_routed_linear_combination(inputs),
    };
    let application_seconds = started.elapsed().as_secs_f64();
    let peak_process_rss_bytes = peak_process_rss_bytes();
    // Full validation, coefficient formatting and JSON writing are outside it.
    let identity = package_identity(&prepared, &manifest_hash);
    let request_hash = blake3::hash(&request_bytes).to_hex().to_string();
    let mut document = json!({
        "schema":SAMPLE_SCHEMA,"status":"failed", "error":null,
        "request_id":request.id,"request_blake3":request_hash,
        "entry_mode":request.entry_mode.name(),"package_identity":identity,
        "package_metadata":prepared.metadata(),
        "scheduling":{"roots":args[3],"dependencies":args[4]},
        "operational_options":format!("{options:?}"),
        "common_operational_options":format!("{common_options:?}"),
        "build_identity":build_identity(),
        "defaults":{"scheduling":{"roots":"canonical","dependencies":"canonical"},
            "entry_mode":"direct","unresolved_policy":"Reject",
            "cache_policy":format!("{:?}",SavedApplicationOptions::default().cache_policy)},
        "fresh_application":true,"reduction_calls":1,"persistent_backing":false,
        "phases_seconds":{
            "package_load":load_seconds,"package_preparation":preparation_json,
            "coefficient_ingress":ingress_seconds,
            "application_including_terminal_substitutions":application_seconds,
            "native_application_only":null,"terminal_substitutions_only":null,
            "serialization":null,"independent_exact_check":null,"cpu":null},
        "peak_process_rss_bytes":peak_process_rss_bytes,
        "peak_process_rss_scope":"Linux VmHWM process-lifetime highwater through application return; includes setup, excludes later serialization/checks; null if unavailable",
        "route_nodes":null,"apply_nodes":null,
        "owner_preparation":super::owner_preparation_json(&prepared),
        "cache":cache_json(&prepared),"work":null,
        "terms":[],"guards":[],"unresolved":[],
        "guard_context_family":prepared.family().fingerprint(),
    });
    match reduced {
        Ok(reduced) => {
            if !reduced.unresolved().is_empty()
                || !reduced
                    .terms()
                    .keys()
                    .all(|key| prepared.remaining_terminals().contains(key))
            {
                return Err("strict sample returned unresolved or undeclared output terms".into());
            }
            for (key, value) in reduced.terms() {
                let family = prepared
                    .output_family(key.family_fingerprint())
                    .ok_or("sample output family is not authenticated")?;
                family
                    .coefficient_context()
                    .validate_with_limits(value, options.reduction_limits.exact_algebra)?;
                if value.is_zero() {
                    return Err("sample returned an explicit zero output term".into());
                }
            }
            for guard in reduced.nonzero_conditions() {
                prepared
                    .family()
                    .coefficient_context()
                    .validate_with_limits(guard, options.reduction_limits.exact_algebra)?;
                if guard.is_zero() {
                    return Err("sample returned an identically zero nonzero-condition".into());
                }
            }
            let terms: Vec<_> = reduced
                .terms()
                .iter()
                .map(|(key, value)| {
                    json!({"family_fingerprint":key.family_fingerprint(),
                        "powers":key.integral().powers(),"coefficient":value.to_string()})
                })
                .collect();
            let stats = reduced.statistics();
            document["status"] = json!("complete");
            document["terms"] = json!(terms);
            document["guards"] = json!(
                reduced
                    .nonzero_conditions()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            );
            document["work"] = json!({
                "input_terms":stats.input_terms,"requested_targets":stats.requested_targets,
                "operational_nodes":stats.operational_nodes,
                "transport_calls":stats.transport_calls,"transport_operations":stats.transport_operations,
                "transport_endpoints":stats.transport_endpoints,
                "rule_applications":stats.work.rule_applications(),"cache_hits":stats.work.cache_hits(),
                "coalescing_additions":stats.work.coalescing_additions(),
                "terminal_substitution_additions":reduced.terminal_substitution_additions(),
                "evicted_entries":stats.cache.evicted_entries,
                "evicted_coefficient_terms":stats.cache.evicted_coefficient_terms,
                "evicted_coefficient_bytes":stats.cache.evicted_coefficient_bytes,
                "same_request_recomputed_nodes":stats.cache.recomputed_nodes,
            });
        }
        Err(error) => document["error"] = json!(error.to_string()),
    }
    let mut writer = BufWriter::new(output_file);
    serde_json::to_writer(&mut writer, &document)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    println!(
        "{}",
        json!({"event":"scheduling_sample","status":document["status"],
        "request_id":request.id,"application_seconds":application_seconds,"output":output})
    );
    if document["status"] != "complete" {
        return Err("the single scheduling sample failed; see its output receipt".into());
    }
    Ok(())
}

fn cache_json<const N: usize>(prepared: &PreparedSavedApplication<N>) -> Value {
    let cache = prepared.cache_statistics();
    let work = prepared.cache_work_statistics();
    json!({"integrals":cache.cached_integrals(),"coefficient_terms":cache.cached_coefficient_terms(),
        "coefficient_bytes":cache.cached_coefficient_bytes(),"cache_hits":cache.cache_hits(),
        "rule_applications":cache.rule_applications(),"coalescing_additions":cache.coalescing_additions(),
        "evicted_entries":work.evicted_entries,"evicted_coefficient_terms":work.evicted_coefficient_terms,
        "evicted_coefficient_bytes":work.evicted_coefficient_bytes,"same_request_recomputed_nodes":work.recomputed_nodes})
}

type OutputTerms = BTreeMap<(String, IntegralKey), Coefficient>;
fn parse_output<const N: usize>(
    prepared: &PreparedSavedApplication<N>,
    request: &Request,
    request_hash: &str,
    manifest_hash: &str,
    document: &Value,
) -> Result<(OutputTerms, Vec<Coefficient>)> {
    if document["schema"] != SAMPLE_SCHEMA
        || document["status"] != "complete"
        || document["error"] != Value::Null
        || document["request_id"].as_str() != Some(request.id.as_str())
        || document["request_blake3"].as_str() != Some(request_hash)
        || document["entry_mode"].as_str() != Some(request.entry_mode.name())
        || document["package_identity"] != package_identity(prepared, manifest_hash)
        || document["guard_context_family"].as_str() != Some(prepared.family().fingerprint())
        || document["fresh_application"] != true
        || document["reduction_calls"].as_u64() != Some(1)
        || document["persistent_backing"] != false
        || !document["common_operational_options"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    {
        return Err("sample is incomplete or its request/package/context identity differs".into());
    }
    if !document["unresolved"].as_array().is_some_and(Vec::is_empty) {
        return Err("sample contains unresolved outputs or omits completeness evidence".into());
    }
    order(
        document["scheduling"]["roots"]
            .as_str()
            .ok_or("missing root order")?,
    )?;
    order(
        document["scheduling"]["dependencies"]
            .as_str()
            .ok_or("missing dependency order")?,
    )?;
    let declared: BTreeSet<_> = prepared
        .remaining_terminals()
        .iter()
        .map(|key| (key.family_fingerprint().to_owned(), key.integral().clone()))
        .collect();
    let limits = ExactAlgebraLimits::default();
    let mut terms = BTreeMap::new();
    for row in document["terms"]
        .as_array()
        .ok_or("sample omits full exact terms")?
    {
        let family_id = row["family_fingerprint"]
            .as_str()
            .ok_or("output family is missing")?;
        let family = prepared
            .output_family(family_id)
            .ok_or("output family is not authenticated")?;
        if !family
            .coefficient_context()
            .has_same_variable_map(prepared.family().coefficient_context())
        {
            return Err("output coefficients do not share the authenticated guard context".into());
        }
        let powers: Vec<i64> = serde_json::from_value(row["powers"].clone())?;
        if powers.len() != family.denominator_count() {
            return Err("output term has incorrect physical arity".into());
        }
        let key = (family_id.to_owned(), IntegralKey::try_new(powers)?);
        if !declared.contains(&key) {
            return Err("output is not a declared authenticated terminal".into());
        }
        let value = coefficient(
            row["coefficient"]
                .as_str()
                .ok_or("output coefficient is missing")?,
            family.coefficient_context(),
            limits,
        )?;
        if value.is_zero() || terms.insert(key, value).is_some() {
            return Err("output has an explicit zero or duplicate terminal".into());
        }
    }
    let guards = document["guards"]
        .as_array()
        .ok_or("sample omits raw guards")?
        .iter()
        .map(|guard| {
            let value = coefficient(
                guard
                    .as_str()
                    .ok_or("guard is not an exact coefficient string")?,
                prepared.family().coefficient_context(),
                limits,
            )?;
            if value.is_zero() {
                return Err("a nonzero guard is identically zero".into());
            }
            Ok(value)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((terms, guards))
}

pub(super) fn check<const N: usize>(args: &[String], arity: usize) -> Result<()> {
    let bytes = std::fs::read(&args[2])?;
    let request = parse_request(&bytes, arity)?;
    let request_hash = blake3::hash(&bytes).to_hex().to_string();
    let (prepared, manifest_hash) =
        load::<N>(Path::new(&args[1]), SavedApplicationOptions::default())?;
    // Validate ingress on the authenticated map, without evaluating any rule.
    for term in &request.terms {
        coefficient(
            &term.coefficient,
            prepared.family().coefficient_context(),
            ExactAlgebraLimits::default(),
        )?;
    }
    let expected: Value = serde_json::from_slice(&std::fs::read(&args[3])?)?;
    let actual: Value = serde_json::from_slice(&std::fs::read(&args[4])?)?;
    let (expected_terms, expected_guards) = parse_output(
        &prepared,
        &request,
        &request_hash,
        &manifest_hash,
        &expected,
    )?;
    let (actual_terms, actual_guards) =
        parse_output(&prepared, &request, &request_hash, &manifest_hash, &actual)?;
    let terms_equal = expected_terms == actual_terms;
    let guards_equal = same_guard_set(&expected_guards, &actual_guards);
    let ordered_guards_equal = expected_guards == actual_guards;
    let ordered_guards_required = expected["scheduling"]["roots"] == actual["scheduling"]["roots"];
    let common_options_equal =
        expected["common_operational_options"] == actual["common_operational_options"];
    let build_labels_equal = expected["build_identity"] == actual["build_identity"];
    println!(
        "{}",
        json!({"event":"scheduling_exact_check","request_id":request.id,
        "exact_full_terms_equal":terms_equal,"exact_guard_sets_equal":guards_equal,
        "exact_ordered_guards_equal":ordered_guards_equal,"ordered_guard_delta":!ordered_guards_equal,
        "ordered_guards_required":ordered_guards_required,
        "common_operational_options_equal":common_options_equal,"build_labels_equal":build_labels_equal,
        "checker_build_identity":build_identity(),
        "expected_term_count":expected_terms.len(),"actual_term_count":actual_terms.len(),
        "reduction_calls":0,"package_authenticated_by_loader":true})
    );
    if !terms_equal
        || !guards_equal
        || !common_options_equal
        || !build_labels_equal
        || (ordered_guards_required && !ordered_guards_equal)
    {
        return Err("full exact coefficients, nonzero guard obligations, common operational options or build labels differ".into());
    }
    Ok(())
}

fn same_guard_set(expected: &[Coefficient], actual: &[Coefficient]) -> bool {
    expected.iter().collect::<HashSet<_>>() == actual.iter().collect::<HashSet<_>>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_contract_rejects_ambiguous_modes_schemas_and_arity() {
        let mut value = json!({"schema":REQUEST_SCHEMA,"id":"unit","arity":2,
            "terms":[{"powers":[1,-2],"coefficient":"1"}]});
        let parse = |v: &Value| parse_request(&serde_json::to_vec(v).unwrap(), 2);
        assert_eq!(parse(&value).unwrap().entry_mode, EntryMode::Direct);
        value["entry_mode"] = json!("routed");
        assert_eq!(parse(&value).unwrap().entry_mode, EntryMode::Routed);
        value["entry_mode"] = json!("warm");
        assert!(parse(&value).is_err());
        value["entry_mode"] = json!("direct");
        value["terms"][0]["powers"] = json!([1]);
        assert!(parse(&value).is_err());
        value["terms"][0]["powers"] = json!([1, -2]);
        value["schema"] = json!("unversioned");
        assert!(parse(&value).is_err());
    }

    #[test]
    fn exact_ingress_uses_only_the_declared_variable_map() {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let limits = ExactAlgebraLimits::default();
        let expected = coefficient("(d+1)/(d-4)", &context, limits).unwrap();
        assert_eq!(
            coefficient(&expected.to_string(), &context, limits).unwrap(),
            expected
        );
        assert!(coefficient("undeclared+1", &context, limits).is_err());
        assert!(coefficient("1/0", &context, limits).is_err());
        assert!(order("largest-first").is_err());
    }

    #[test]
    fn guard_set_comparison_ignores_order_and_duplicates_but_never_missing_obligations() {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let a = coefficient("d-1", &context, ExactAlgebraLimits::default()).unwrap();
        let b = coefficient("d-2", &context, ExactAlgebraLimits::default()).unwrap();
        assert!(same_guard_set(
            &[a.clone(), b.clone()],
            &[b.clone(), a.clone(), a.clone()]
        ));
        assert!(!same_guard_set(&[a.clone(), b.clone()], &[a]));
        assert!(!same_guard_set(&[b], &[]));
    }

    #[test]
    fn process_highwater_requires_linux_kib_and_checked_byte_conversion() {
        assert_eq!(
            parse_peak_rss_bytes("Name:\ttest\nVmHWM:\t123 kB\n"),
            Some(123 * 1024)
        );
        assert_eq!(parse_peak_rss_bytes("VmRSS: 123 kB"), None);
        assert_eq!(parse_peak_rss_bytes("VmHWM: 123 MB"), None);
        assert_eq!(parse_peak_rss_bytes("VmHWM: 18446744073709551615 kB"), None);
    }
}
