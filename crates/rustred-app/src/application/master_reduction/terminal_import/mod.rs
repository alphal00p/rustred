//! Import a finite, independently replayed symmetry recipe into a normal
//! application publication. No campaign traversal or new ordinary IBP search.
mod maps;
mod recipe;
pub(super) mod rows;
mod state;

use super::*;
use rustred::family::IntegralKey;
use rustred::reduction::terminal_normalization::TerminalNumeratorSymmetryEquations;
use rustred::reduction::terminal_relations::TerminalEquation;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;

const IMPORT_SCHEMA: &str = "rustred.master-refinement-proof-import.v1";

/// Import explicitly nominated, exact finite symmetry identities into a new
/// portable publication. The source is immutable. Resume uses authenticated
/// copied input/certificate state after preparation, not research output maps.
pub fn master_import_proved_refinement(
    source: &Path,
    proof_recipe: &Path,
    options: &MasterReductionOptions,
    cancel: &AtomicBool,
    observer: impl Fn(Value),
) -> Result<Value, AppError> {
    validate_options(options)?;
    if source.exists() {
        let resolved_source = source.canonicalize().map_err(io)?;
        let source_directory = if resolved_source.is_dir() {
            resolved_source
        } else {
            resolved_source.parent().unwrap().to_path_buf()
        };
        storage::reject_overlapping_directories(&source_directory, &options.directory)?;
    }
    preflight_inputs(proof_recipe, options)?;
    let _lock = storage::WriterLock::acquire(&options.directory)?;
    let started = Instant::now();
    let mut progress = state::open(source, proof_recipe, options)?;
    let result = execute(source, options, cancel, &observer, started, &mut progress);
    if result.is_err() && cancel.load(Ordering::Relaxed) {
        return paused(options, &mut progress, &observer, started);
    }
    if let Err(error) = &result {
        progress["status"] = json!("failed");
        progress["last_error"] = json!(error.to_string());
        let _ = state::save(options, &progress);
        observer(event(&progress, "master_reduction_error", started));
    }
    result
}

fn preflight_inputs(path: &Path, options: &MasterReductionOptions) -> Result<(), AppError> {
    let bytes = storage::read_bounded(path, profile::native_io_limits(None).max_program_bytes)?;
    let recipe = recipe::Recipe::parse(&bytes)?;
    let prior = if options.resume {
        Some(storage::read_json_with_policy(
            &options.directory.join("import-state.json"),
            options.finite_policy.as_ref(),
        )?)
    } else {
        None
    };
    if let Some(prior) = &prior {
        // A prepared resume is self-contained. Authenticate its exact recipe
        // and native state before allowing its archived recipe inside OUT.
        if prior["proof_import"]["schema"] == IMPORT_SCHEMA
            && prior["proof_import"]["recipe"]["blake3"].as_str()
                == Some(blake3::hash(&bytes).to_hex().as_str())
            && matches!(
                prior["proof_import"]["stage"].as_str(),
                Some("executing" | "flattening" | "published")
            )
        {
            if state::archived(options, &prior["proof_import"]["recipe"])? != bytes {
                return Err(AppError::input("prepared import recipe changed"));
            }
            let _ = state::archived(options, &prior["proof_import"]["work_state"])?;
            return Ok(());
        }
    }
    let original_base = prior
        .as_ref()
        .and_then(|value| value["proof_import"]["recipe_base"].as_str())
        .map(PathBuf::from)
        .unwrap_or(
            std::path::absolute(path)
                .map_err(io)?
                .parent()
                .unwrap()
                .to_path_buf(),
        );
    let baseline = recipe::resolve(&original_base, &recipe.baseline.artifact)
        .canonicalize()
        .map_err(io)?;
    let baseline_directory = if baseline.is_dir() {
        baseline
    } else {
        baseline.parent().unwrap().to_path_buf()
    };
    storage::reject_overlapping_directories(&baseline_directory, &options.directory)?;
    let mut files = vec![recipe::resolve(
        &original_base,
        &recipe.numerator_symmetry.path,
    )];
    if !options.resume {
        files.push(path.to_path_buf());
    }
    if let Some(expected) = recipe.expected_result {
        files.push(recipe::resolve(&original_base, &expected.path));
    }
    for file in files {
        storage::reject_overlapping_directories(
            &file.canonicalize().map_err(io)?,
            &options.directory,
        )?;
    }
    Ok(())
}

fn validate_options(options: &MasterReductionOptions) -> Result<(), AppError> {
    if options.operation != MasterReductionOperation::Publish
        || options.fresh_search
        || options.previous_artifact.is_some()
        || !options.collection_artifacts.is_empty()
        || options.saved_rule_assistance
        || options.circuit_symmetry_assistance
        || options.numerator_symmetry_assistance
        || options.sector_symmetry_assistance
        || options.sector_symmetry_policy.is_some()
        || options.sector_symmetry_saved_projection.is_some()
        || options.seed_depth != 0
        || options.containing_sector_depth != 0
        || options.deep_verification.is_some()
        || options.refinement_strategy.is_some()
        || options.modular_policy.is_some()
        || options.finite_feedback == Some(true)
        || options.threads == 0
        || options.checkpoint_interval.is_zero()
    {
        return Err(AppError::input(
            "proof import requires Publish and no new search/verification policy",
        ));
    }
    Ok(())
}

fn paused(
    options: &MasterReductionOptions,
    progress: &mut Value,
    observer: &impl Fn(Value),
    started: Instant,
) -> Result<Value, AppError> {
    progress["status"] = json!("paused");
    progress["import_complete"] = json!(false);
    state::save(options, progress)?;
    observer(event(progress, "master_reduction_checkpoint", started));
    Ok(progress.clone())
}

fn read_session(
    options: &MasterReductionOptions,
    descriptor: &Value,
    normalization: MasterNormalizationProfile,
) -> Result<TerminalRelationSession, AppError> {
    TerminalRelationSession::from_native_bytes(
        &state::archived(options, descriptor)?,
        normalization.relation_limits(),
        profile::native_io_limits(None),
    )
    .map_err(io)
}

fn execute(
    source: &Path,
    options: &MasterReductionOptions,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
    started: Instant,
    report: &mut Value,
) -> Result<Value, AppError> {
    if report["import_complete"] == true && options.directory.join("artifact.json").exists() {
        let _ = load_master_reduction_with_optional_policy(&options.directory, options)?;
        return Ok(inspect_with_policy(
            &options.directory,
            options.finite_policy.as_ref(),
        )?);
    }
    if cancel.load(Ordering::Relaxed) {
        return paused(options, report, observer, started);
    }
    if report["proof_import"]["stage"] == "preparing" {
        prepare(source, options, cancel, observer, started, report)?;
        if cancel.load(Ordering::Relaxed) {
            return paused(options, report, observer, started);
        }
    }
    let normalization = profile::from_report(report)?;
    let baseline = read_session(
        options,
        &report["proof_import"]["baseline_compact"],
        normalization,
    )?;
    let current = read_session(
        options,
        &report["proof_import"]["current_compact"],
        normalization,
    )?;
    let mut working = read_session(
        options,
        &report["proof_import"]["work_state"],
        normalization,
    )?;
    let binding = report["proof_import"]["work_binding"]
        .as_str()
        .ok_or_else(|| AppError::input("import work binding absent"))?
        .to_owned();
    if working.assistance_binding() != Some(binding.as_str()) {
        return Err(AppError::input("import native assistance binding changed"));
    }
    let all_rows = rows::decode(
        &state::archived(options, &report["proof_import"]["equations"])?,
        working.family_owner(),
        &binding,
    )?;
    let first = working
        .raw_terminals()
        .first()
        .cloned()
        .ok_or_else(|| AppError::input("empty import terminal inventory"))?;
    let mut provider = |key: &IntegralKey| {
        Ok(if key == &first {
            all_rows.clone()
        } else {
            Vec::new()
        })
    };
    let mut last_checkpoint = Instant::now();
    let mut last_event = Instant::now();
    report["proof_import"]["stage"] = json!("executing");
    while working.statistics().pending_assistance_keys != 0
        || working.statistics().pending_assistance_rows != 0
    {
        if cancel.load(Ordering::Relaxed) {
            state::checkpoint(options, &working, report)?;
            return paused(options, report, observer, started);
        }
        working
            .step_with_provider(cancel, &mut provider)
            .map_err(io)?;
        if working.statistics().completed_source_rows != 0 {
            return Err(AppError::input(
                "proof import consumed an ordinary IBP source",
            ));
        }
        if last_event.elapsed() >= Duration::from_millis(250) {
            update_stats(report, &working);
            report["stage"] = json!("importing_exact_terminal_relations");
            observer(event(report, "master_reduction_progress", started));
            last_event = Instant::now();
        }
        if last_checkpoint.elapsed() >= options.checkpoint_interval {
            state::checkpoint(options, &working, report)?;
            observer(event(report, "master_reduction_checkpoint", started));
            last_checkpoint = Instant::now();
        }
    }
    report["proof_import"]["stage"] = json!("flattening");
    state::checkpoint(options, &working, report)?;
    maps::contains(&working, &baseline)?;
    maps::contains(&working, &current)?;
    if report["proof_import"]["expected_result"].is_object() {
        let expected = read_session(
            options,
            &report["proof_import"]["expected_result"],
            normalization,
        )?;
        // A research output is only a coefficient-map assertion. Its extra
        // intermediate flattening poles are not imported proof assumptions.
        maps::contains_maps(&working, &expected)?;
    }
    let archive = report["proof_import"]["archive"].clone();
    let source_binding = json!({
        "manifest_blake3":report["proof_import"]["source_manifest_blake3"],
        "native_state_blake3":report["proof_import"]["current_source_native_blake3"],
        "collection_state_blake3":report["proof_import"]["current_source_collection_blake3"],
        "family_fingerprint":working.family_owner().fingerprint(),
        "program_binding":report["inputs"]["program_binding"],"scope_binding":report["scope_binding"],
        "relation_authority":report["proof_import"]["current_source_relation_authority"],"normalization_profile":normalization.name(),
        "proof_import_archive":archive,
        "meaning":"exact full maps inherited from the current publication and finite witnessed symmetry/source-rule consequences replayed at import; no independent ordinary-IBP certification"});
    let final_binding = fresh::authority(&source_binding)?;
    let flattened = maps::flatten(&working, &final_binding, normalization)?;
    maps::contains(&flattened, &baseline)?;
    maps::contains(&flattened, &current)?;
    let capsule = flattened
        .to_native_bytes(profile::native_io_limits(None))
        .map_err(io)?;
    report["retained_relations"] = json!({"recipe":fresh::RECIPE,"binding":final_binding,"source":source_binding,
        "native_state":fresh::write_state(&options.directory,&capsule)?,"raw_terminals":flattened.raw_terminals().len(),
        "normalization_profile":normalization.name(),"ordinary_source_rows_included":0,"auxiliary_search_rows_included":0});
    for field in [
        "inherited_collection",
        "inherited_equation_authority",
        "modular_state",
        "modular_refinement",
        "modular_search_complete",
        "modular_policy",
        "numerator_symmetry_certificate",
        "sector_symmetry_preparation",
        "sector_symmetry_policy",
        "sector_symmetry_saved_projection",
        "sector_symmetry_reduction",
        "sector_symmetry_certificate",
        "sector_symmetry_normalization_certificate",
        "sector_symmetry_task_certificate",
        "sector_symmetry_reduced_certificate",
        "sector_symmetry_projection_checkpoint",
        "refinement_binding",
        "partial_refinement_binding",
        "partial_refinement_source",
    ] {
        report.as_object_mut().unwrap().remove(field);
    }
    report["collection_members"] = json!([]);
    report["collection_state"] = Value::Null;
    report["collection"] = json!({"strategy":collection::STRATEGY,"status":"pending"});
    report["operation"] = json!("publish");
    report["seed_depth"] = json!(0);
    report["containing_sector_depth"] = json!(0);
    report["requested_containing_sector_depth"] = json!(0);
    report["fresh_search"] = json!(true);
    report["fresh_search_recipe"] = json!(fresh::RECIPE);
    report["saved_rule_assistance"] = json!(false);
    report["circuit_symmetry_assistance"] = json!(false);
    report["numerator_symmetry_assistance"] = json!(false);
    report["sector_symmetry_assistance"] = json!(false);
    report["relation_authority"] = json!(
        "exact retained published maps and finite replayed numerator-momentum-change identities composed with supplied candidate rules; imported rule provenance is not upgraded to independent ordinary-IBP certification"
    );
    modular::record(report, MasterRefinementStrategy::Exact, options)?;
    report["completion_policy"] =
        json!("finite proof recipe replay only; no new ordinary or modular source search");
    fresh::record_authority(report);
    collection::record_feedback(report, false);
    report["proof_import"]["basis_before"] = json!(current.remaining_terminals().len());
    report["proof_import"]["basis_after"] = json!(flattened.remaining_terminals().len());
    report["proof_import"]["all_current_maps_and_guards_retained"] = json!(true);
    report["proof_import"]["new_ordinary_source_rows"] = json!(0);
    save(options, &flattened, report, "running", started, observer)?;
    collection::complete(options, &flattened, report, cancel, observer, started)?;
    if cancel.load(Ordering::Relaxed) {
        return paused(options, report, observer, started);
    }
    report["proof_import"]["stage"] = json!("published");
    report["import_complete"] = json!(true);
    save(
        options,
        &flattened,
        report,
        "published_unrefined",
        started,
        observer,
    )?;
    publish_final(options, report)?;
    state::save(options, report)?;
    let _ = load_master_reduction_with_optional_policy(&options.directory, options)?;
    observer(event(report, "master_reduction_finished", started));
    Ok(report.clone())
}

fn load_master_reduction_with_optional_policy(
    path: &Path,
    options: &MasterReductionOptions,
) -> Result<MasterReductionArtifact, AppError> {
    collection::load_with_policy(path, options.finite_policy.as_ref())
}

fn publication(
    path: &Path,
    options: &MasterReductionOptions,
) -> Result<(PathBuf, Vec<u8>, Value), AppError> {
    let file = if path.is_dir() {
        path.join("artifact.json")
    } else {
        path.to_path_buf()
    };
    let bytes = storage::read_bounded(&file, profile::native_io_limits(None).max_program_bytes)?;
    let report = inspect_with_policy(&file, options.finite_policy.as_ref())?;
    if !matches!(
        report["status"].as_str(),
        Some("published_unrefined" | "completed_nonminimal")
    ) || report["collection"]["status"] != "completed"
    {
        return Err(AppError::input(
            "proof import needs a completed portable source publication",
        ));
    }
    Ok((file, bytes, report))
}

fn prepare(
    source: &Path,
    options: &MasterReductionOptions,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
    started: Instant,
    report: &mut Value,
) -> Result<(), AppError> {
    let recipe_bytes = state::archived(options, &report["proof_import"]["recipe"])?;
    let recipe = recipe::Recipe::parse(&recipe_bytes)?;
    let recipe_base = PathBuf::from(
        report["proof_import"]["recipe_base"]
            .as_str()
            .ok_or_else(|| AppError::input("proof recipe source base missing"))?,
    );
    let (current_path, current_bytes, current_report) = publication(source, options)?;
    let current_hash = blake3::hash(&current_bytes).to_hex().to_string();
    if report["proof_import"]["source_manifest_blake3"]
        .as_str()
        .is_some_and(|prior| prior != current_hash)
    {
        return Err(AppError::input(
            "current publication changed while preparing proof import",
        ));
    }
    report["proof_import"]["source_manifest_blake3"] = json!(current_hash);
    state::save(options, report)?;
    let base_path = recipe::resolve(&recipe_base, &recipe.baseline.artifact);
    let (base_path, base_bytes, base_report) = publication(&base_path, options)?;
    if blake3::hash(&base_bytes).to_hex().as_str() != recipe.baseline.blake3 {
        return Err(AppError::input("proof baseline publication digest differs"));
    }
    storage::reject_overlapping_directories(
        &base_path.parent().unwrap().canonicalize().map_err(io)?,
        &options.directory,
    )?;
    for field in ["scope_binding", "normalization_profile"] {
        if current_report[field] != base_report[field] {
            return Err(AppError::input(format!(
                "proof baseline and current publication differ in {field}"
            )));
        }
    }
    if current_report["inputs"]["program_binding"] != base_report["inputs"]["program_binding"] {
        return Err(AppError::input(
            "proof baseline and current saved-program identities differ",
        ));
    }
    let normalization = profile::from_report(&current_report)?;
    profile::for_resume(options.normalization_profile, &current_report)?;
    report["stage"] = json!("loading_current_proved_maps");
    observer(event(report, "master_reduction_progress", started));
    let current_full = load_master_reduction_with_optional_policy(&current_path, options)?;
    let current = maps::compact(
        &current_full,
        &format!("proof-import-current:{current_hash}"),
        normalization,
    )?;
    drop(current_full);
    if cancel.load(Ordering::Relaxed) {
        return Ok(());
    }
    report["stage"] = json!("loading_baseline_proved_maps");
    observer(event(report, "master_reduction_progress", started));
    let baseline_full = load_master_reduction_with_optional_policy(&base_path, options)?;
    let baseline = maps::compact(
        &baseline_full,
        &format!("proof-import-baseline:{}", recipe.baseline.blake3),
        normalization,
    )?;
    drop(baseline_full);
    if current.family_owner().fingerprint() != baseline.family_owner().fingerprint()
        || current.raw_terminals() != baseline.raw_terminals()
    {
        return Err(AppError::input(
            "proof baseline and current family/raw retained inventory differ",
        ));
    }
    let symmetry_path = recipe::resolve(&recipe_base, &recipe.numerator_symmetry.path);
    let symmetry_bytes = recipe::checked(&symmetry_path, &recipe.numerator_symmetry.blake3)?;
    let plan = TerminalNumeratorSymmetryEquations::from_native_bytes(
        &symmetry_bytes,
        Arc::clone(current.family_owner()),
        Default::default(),
        profile::native_io_limits(None),
    )
    .map_err(io)?;
    let keys: BTreeSet<_> = current
        .raw_terminals()
        .union(current.normalization().canonical_terminals())
        .cloned()
        .collect();
    if plan.input_keys() != &keys {
        return Err(AppError::input(
            "symmetry witness inventory differs from the exact published finite inventory",
        ));
    }
    for selected in &recipe.selected_saved_rows {
        let key = IntegralKey::try_new(selected.source.clone()).map_err(io)?;
        if plan.equations(&key).get(selected.witness).is_none() {
            return Err(AppError::input(
                "selected source witness is absent from the replayed native capsule",
            ));
        }
    }
    if cancel.load(Ordering::Relaxed) {
        return Ok(());
    }
    storage::clone_inputs_with_policy(
        current_path.parent().unwrap(),
        &options.directory,
        &current_report,
        options.finite_policy.as_ref(),
    )?;
    let mut output = current_report.clone();
    census_cache::inherit_with_cancel(
        current_path.parent().unwrap(),
        &options.directory,
        &mut output,
        cancel,
        options.finite_policy.as_ref(),
    )?;
    let imported = report["proof_import"].clone();
    output["proof_import"] = imported;
    output["operation"] = json!("publish");
    output["status"] = json!("running");
    output["import_complete"] = json!(false);
    let mut archive = Vec::new();
    archive.push(output["proof_import"]["recipe"].clone());
    for (name, bytes) in [
        ("source-current.json", current_bytes.as_slice()),
        ("source-baseline.json", base_bytes.as_slice()),
        ("symmetry.rrbin", symmetry_bytes.as_slice()),
    ] {
        archive.push(state::archive(options, name, bytes)?);
    }
    let current_bytes_compact = current
        .to_native_bytes(profile::native_io_limits(None))
        .map_err(io)?;
    let baseline_bytes_compact = baseline
        .to_native_bytes(profile::native_io_limits(None))
        .map_err(io)?;
    output["proof_import"]["current_compact"] =
        state::archive(options, "current.rrbin", &current_bytes_compact)?;
    output["proof_import"]["baseline_compact"] =
        state::archive(options, "baseline.rrbin", &baseline_bytes_compact)?;
    archive.push(output["proof_import"]["current_compact"].clone());
    archive.push(output["proof_import"]["baseline_compact"].clone());
    let mut equations = Vec::new();
    let context = current.family_owner().coefficient_context();
    // Retain the old baseline span as well as every newer current map.
    for (target, terms) in baseline.apply_all_terminals().map_err(io)? {
        let mut row = BTreeMap::from([(target, context.one())]);
        for (key, value) in terms {
            let old = row.remove(&key).unwrap_or_else(|| context.zero());
            let value = context
                .try_sub(&old, &value, profile::native_io_limits(None).exact_algebra)
                .map_err(io)?;
            if !value.is_zero() {
                row.insert(key, value);
            }
        }
        if !row.is_empty() {
            equations.push(TerminalEquation {
                terms: row,
                nonzero_conditions: Vec::new(),
            });
        }
    }
    // Preserve all baseline guards once, even if every baseline map is identity.
    equations.push(TerminalEquation {
        terms: BTreeMap::new(),
        nonzero_conditions: baseline.nonzero_conditions().to_vec(),
    });
    if recipe.include_raw_symmetry {
        for key in plan.input_keys() {
            equations.extend(plan.equations(key).iter().cloned());
        }
    }
    let mut selected_receipts = Vec::new();
    let work_binding = format!(
        "finite-proof-import-v1:{}:{current_hash}",
        output["proof_import"]["recipe"]["blake3"].as_str().unwrap()
    );
    if !recipe.selected_saved_rows.is_empty() {
        use crate::application::routed_campaign::{
            RoutedCampaignRequest, prepare_saved_integral_equations,
        };
        let selection_path = storage::safe_child(
            &options.directory,
            current_report["inputs"]["selection"]
                .as_str()
                .ok_or_else(|| AppError::input("source selection missing"))?,
        )?;
        let selection = storage::read_bounded(&selection_path, usize::MAX)?;
        if blake3::hash(&selection).to_hex().as_str()
            != current_report["inputs"]["program_binding"]
                .as_str()
                .unwrap_or("")
        {
            return Err(AppError::input("cloned import selection content differs"));
        }
        let mut request =
            RoutedCampaignRequest::new(String::from_utf8(selection).map_err(io)?, String::new());
        request.owner_base = selection_path.parent().unwrap().to_path_buf();
        request.workers = options.threads;
        numerator_symmetry::configure_saved_resources(&mut request);
        let digests = current_report["inputs"]["payloads"]
            .as_array()
            .ok_or_else(|| AppError::input("payload descriptors absent"))?
            .iter()
            .map(|row| {
                row["blake3"]
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| AppError::input("payload digest absent"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        output["stage"] = json!("preparing_import_saved_rules");
        observer(event(&output, "master_reduction_progress", started));
        let cached = output["proof_import"]["selected_saved_prefix"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if cached.len() > recipe.selected_saved_rows.len() {
            return Err(AppError::input("saved import prefix exceeds recipe"));
        }
        let mut retained_prefix = cached.clone();
        let mut saved = None;
        for (index, selected) in recipe.selected_saved_rows.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                *report = output;
                return Ok(());
            }
            let key = IntegralKey::try_new(selected.source.clone()).map_err(io)?;
            let equation = plan.equations(&key).get(selected.witness).ok_or_else(|| {
                AppError::input(
                    "selected source witness is absent from the replayed native capsule",
                )
            })?;
            let selected_binding = format!("{work_binding}:selected:{index}");
            if let Some(record) = cached.get(index) {
                if record["source"] != json!(selected.source)
                    || record["witness"] != json!(selected.witness)
                {
                    return Err(AppError::input("cached saved source differs from recipe"));
                }
                let mut cached_rows = rows::decode(
                    &state::archived(options, &record["native_row"])?,
                    current.family_owner(),
                    &selected_binding,
                )?;
                if cached_rows.len() != 1 {
                    return Err(AppError::input(
                        "cached saved source is not one complete identity",
                    ));
                }
                equations.push(cached_rows.pop().unwrap());
                selected_receipts.push(record["receipt"].clone());
                continue;
            }
            if saved.is_none() {
                saved = Some(prepare_saved_integral_equations(
                    &request,
                    current.family_owner().fingerprint(),
                    &digests,
                    cancel,
                    &|native| {
                        let mut event = event(&output, "master_reduction_progress", started);
                        event["preparation_event"] = native;
                        observer(event);
                    },
                )?);
            }
            let result = saved.as_mut().unwrap().reduce_equation(equation);
            let (row, failure) = match result {
                Ok(row) => (row, Value::Null),
                Err(error) => (
                    equation.clone(),
                    json!({"kind":error.kind(),"message":error.to_string(),"fallback":"complete original witnessed equation; unresolved auxiliaries are not masters"}),
                ),
            };
            let receipt = json!({"source":selected.source,"witness":selected.witness,"terms":row.terms.len(),"conditions":row.nonzero_conditions.len(),"saved_application_failure":failure});
            let encoded = rows::encode(
                current.family_owner(),
                std::slice::from_ref(&row),
                &selected_binding,
            )?;
            let descriptor = state::archive(options, &format!("selected-{index}.rrbin"), &encoded)?;
            retained_prefix.push(json!({"source":selected.source,"witness":selected.witness,"native_row":descriptor,"receipt":receipt}));
            output["proof_import"]["selected_saved_prefix"] = json!(retained_prefix);
            state::save(options, &output)?;
            *report = output.clone();
            observer(event(&output, "master_reduction_checkpoint", started));
            selected_receipts.push(receipt);
            equations.push(row);
        }
    }
    if let Some(prefix) = output["proof_import"]["selected_saved_prefix"].as_array() {
        for record in prefix {
            archive.push(record["native_row"].clone());
        }
    }
    let mut conditions: HashSet<_> = current.nonzero_conditions().iter().cloned().collect();
    for row in &mut equations {
        row.nonzero_conditions
            .retain(|guard| conditions.insert(guard.clone()));
    }
    let row_bytes = rows::encode(current.family_owner(), &equations, &work_binding)?;
    output["proof_import"]["equations"] = state::archive(options, "equations.rrbin", &row_bytes)?;
    archive.push(output["proof_import"]["equations"].clone());
    let receipt_bytes=serde_json::to_vec(&json!({"selected_saved_rows":selected_receipts,"raw_symmetry_rows":if recipe.include_raw_symmetry{plan.statistics().equations}else{0},
        "baseline_manifest_blake3":recipe.baseline.blake3,"current_manifest_blake3":current_hash,
        "native_symmetry_blake3":recipe.numerator_symmetry.blake3,"program_binding":current_report["inputs"]["program_binding"],"new_ordinary_source_rows":0})).map_err(io)?;
    archive.push(state::archive(
        options,
        "replay-receipt.json",
        &receipt_bytes,
    )?);
    if let Some(expected) = &recipe.expected_result {
        let expected_bytes = recipe::checked(
            &recipe::resolve(&recipe_base, &expected.path),
            &expected.blake3,
        )?;
        output["proof_import"]["expected_result"] =
            state::archive(options, "expected-result.rrbin", &expected_bytes)?;
        archive.push(output["proof_import"]["expected_result"].clone());
    }
    if blake3::hash(&storage::read_bounded(&current_path, usize::MAX)?)
        .to_hex()
        .as_str()
        != current_hash
        || blake3::hash(&storage::read_bounded(&base_path, usize::MAX)?)
            .to_hex()
            .as_str()
            != recipe.baseline.blake3
    {
        return Err(AppError::input(
            "a source publication changed during proof import preparation",
        ));
    }
    output["proof_import"]["archive"] = json!(archive);
    output["proof_import"]["work_binding"] = json!(work_binding);
    output["proof_import"]["current_source_native_blake3"] =
        current_report["native_state"]["blake3"].clone();
    output["proof_import"]["current_source_collection_blake3"] =
        current_report["collection_state"]["blake3"].clone();
    output["proof_import"]["current_source_relation_authority"] =
        current_report["relation_authority"].clone();
    output["proof_import"]["stage"] = json!("executing");
    let mut working = current;
    working.enable_assistance(work_binding).map_err(io)?;
    state::checkpoint(options, &working, &mut output)?;
    *report = output;
    Ok(())
}

/// Imported maps use the same trusted-generated retained-map boundary as a
/// fresh round. Its finite import certificates are content-bound and portable;
/// cold application never reopens original research paths or repeats a solve.
pub(super) fn validate_archive(directory: &Path, report: &Value) -> Result<(), AppError> {
    let descriptors = &report["retained_relations"]["source"]["proof_import_archive"];
    if descriptors.is_null() {
        return Ok(());
    }
    for descriptor in descriptors
        .as_array()
        .ok_or_else(|| AppError::input("invalid retained proof-import archive"))?
    {
        let name = descriptor["path"]
            .as_str()
            .ok_or_else(|| AppError::input("import provenance path absent"))?;
        if !name.starts_with("proof-import/") {
            return Err(AppError::input("import provenance escapes its archive"));
        }
        let bytes = recipe::checked(
            &storage::safe_child(directory, name)?,
            descriptor["blake3"]
                .as_str()
                .ok_or_else(|| AppError::input("import provenance digest absent"))?,
        )?;
        if descriptor["bytes"].as_u64() != Some(bytes.len() as u64) {
            return Err(AppError::input("import provenance archive size differs"));
        }
    }
    Ok(())
}

pub(super) fn inherit_archive(
    source: &Path,
    output: &Path,
    report: &Value,
) -> Result<(), AppError> {
    validate_archive(source, report)?;
    if let Some(descriptors) =
        report["retained_relations"]["source"]["proof_import_archive"].as_array()
    {
        for descriptor in descriptors {
            let name = descriptor["path"].as_str().unwrap();
            let bytes = storage::read_bounded(&storage::safe_child(source, name)?, usize::MAX)?;
            let path = storage::safe_child(output, name)?;
            std::fs::create_dir_all(path.parent().unwrap()).map_err(io)?;
            write_file_atomically(&path, &bytes, true).map_err(io)?;
        }
    }
    Ok(())
}
