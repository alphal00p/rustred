//! Portable terminal collection after the resumable finite source search.
//! Algebra, guards, replay and composition are owned by the core plan.
use super::*;
use rustred::family::{IntegralFamily, IntegralKey};
use rustred::reduction::terminal_relations::collection::{
    GuardedVacuumReduction, TerminalCollectionLimits, TerminalCollectionPlan,
    VacuumDiagonalCollectionLimits,
};
use std::collections::BTreeMap;

pub(super) const STRATEGY: &str = "full-u-diagonal-v1";
pub(super) const FEEDBACK_RECIPE: &str = "finite-row-feedback-v1";

pub(super) fn feedback_for_phase(
    options: &MasterReductionOptions,
    resumed: Option<&Value>,
) -> Result<bool, AppError> {
    if options.operation == MasterReductionOperation::Publish {
        return Ok(false);
    }
    let Some(report) = resumed else {
        return Ok(options.finite_feedback.unwrap_or(true));
    };
    let recorded = match report.get("finite_feedback") {
        None => false, // Existing checkpoints predate finite feedback.
        Some(Value::Bool(value)) if report["finite_feedback_recipe"] == FEEDBACK_RECIPE => *value,
        _ => {
            return Err(AppError::input(
                "invalid finite feedback configuration snapshot",
            ));
        }
    };
    if options
        .finite_feedback
        .is_some_and(|requested| requested != recorded)
    {
        return Err(AppError::input(
            "cannot change finite feedback on resume; start a new refinement phase",
        ));
    }
    Ok(recorded)
}

pub(super) fn record_feedback(report: &mut Value, enabled: bool) {
    report["finite_feedback"] = json!(enabled);
    report["finite_feedback_recipe"] = json!(FEEDBACK_RECIPE);
}

fn limits(report: &Value) -> Result<TerminalCollectionLimits, AppError> {
    let recipe = &report["collection_limits"];
    let selected = if recipe.is_object() {
        if recipe["recipe"] != "terminal-collection-limits-v1"
            && recipe["recipe"] != "terminal-collection-limits-v2"
        {
            return Err(AppError::input(
                "unknown terminal collection resource recipe",
            ));
        }
        profile::from_report(recipe)?
    } else {
        let mut selected = profile::from_report(report)?;
        for member in report["collection_members"]
            .as_array()
            .into_iter()
            .flatten()
        {
            if profile::from_report(member)? == MasterNormalizationProfile::StandardV1 {
                selected = MasterNormalizationProfile::StandardV1;
            }
        }
        if report["inherited_collection"]["collection_limits"].is_object()
            && profile::from_report(&report["inherited_collection"]["collection_limits"])?
                == MasterNormalizationProfile::StandardV1
        {
            selected = MasterNormalizationProfile::StandardV1;
        }
        selected
    };
    let mut limits = TerminalCollectionLimits::default();
    limits.diagonal.aliases.parametric = selected.normalization_limits().parametric;
    limits.finite_feedback.aliases.parametric = selected.normalization_limits().parametric;
    let mut expected = limit_snapshot(limits);
    if recipe["recipe"] == "terminal-collection-limits-v1" {
        expected.as_object_mut().unwrap().remove("finite_feedback");
    }
    if recipe.is_object() && recipe["bounds"] != expected {
        return Err(AppError::input(
            "terminal collection limits differ from their versioned recipe",
        ));
    }
    Ok(limits)
}

fn limit_snapshot(limits: TerminalCollectionLimits) -> Value {
    json!({"max_composed_terms":limits.max_composed_terms,"max_proof_layers":limits.max_proof_layers,
        "diagonal":budget_snapshot(limits.diagonal),
        "finite_feedback":budget_snapshot(limits.finite_feedback)})
}

fn budget_snapshot(d: VacuumDiagonalCollectionLimits) -> Value {
    json!({"max_corner_seeds":d.max_corner_seeds,"max_source_rows":d.max_source_rows,
        "max_source_terms":d.max_source_terms,"max_columns":d.max_columns,
        "max_reducer_nonzeros":d.max_reducer_nonzeros,"max_replay_operations":d.max_replay_operations,
        "max_flat_map_terms":d.max_flat_map_terms,"max_conditions":d.max_conditions,
        "max_coefficient_terms":d.max_coefficient_terms,
        "aliases":{"max_families":d.aliases.max_families,"max_terminals":d.aliases.max_terminals},
        "algebra":{"max_specialization_power_operations":d.algebra.max_specialization_power_operations,
        "max_specialization_integer_bits":d.algebra.max_specialization_integer_bits,
        "exact":{"max_exponent":d.algebra.exact_algebra.max_exponent,
        "max_polynomial_terms":d.algebra.exact_algebra.max_polynomial_terms,
        "max_term_operations":d.algebra.exact_algebra.max_term_operations}}})
}

fn record_limits(report: &mut Value) -> Result<(), AppError> {
    if report["collection_limits"].is_object() {
        limits(report)?;
        return Ok(());
    }
    let value = limits(report)?;
    let selected = if value
        .diagonal
        .aliases
        .parametric
        .symanzik
        .max_polynomial_terms
        == MasterNormalizationProfile::StandardV1
            .normalization_limits()
            .parametric
            .symanzik
            .max_polynomial_terms
    {
        MasterNormalizationProfile::StandardV1
    } else {
        MasterNormalizationProfile::ConservativeV1
    };
    let mut recipe =
        json!({"recipe":"terminal-collection-limits-v2","bounds":limit_snapshot(value)});
    profile::record(&mut recipe, selected);
    report["collection_limits"] = recipe;
    Ok(())
}

/// Application-aware publication. `primary_session()` is the finite search
/// cursor; applications must use `apply_terminal()` to include collected maps.
pub struct MasterReductionArtifact {
    primary: TerminalRelationSession,
    members: Vec<TerminalRelationSession>,
    collection: TerminalCollectionPlan,
}

impl MasterReductionArtifact {
    pub fn primary_session(&self) -> &TerminalRelationSession {
        &self.primary
    }
    pub fn member_sessions(&self) -> &[TerminalRelationSession] {
        &self.members
    }
    pub fn collection(&self) -> &TerminalCollectionPlan {
        &self.collection
    }
    pub fn apply_terminal(
        &self,
        family: &IntegralFamily,
        key: &IntegralKey,
    ) -> Result<&GuardedVacuumReduction, AppError> {
        self.collection.apply(family, key).map_err(io)
    }
}

/// Cold-load and authenticate the complete terminal application, including
/// family-qualified collection maps. No algebraic proof is repeated on apply.
pub fn load_master_reduction(path: &Path) -> Result<MasterReductionArtifact, AppError> {
    let report = master_reduction_inspect(path)?;
    let directory = directory(path);
    let primary = load_master_relation_session(path)?;
    let members = load_members(directory, &report)?;
    let sessions = session_refs(&primary, &members);
    let collection = if report["collection_state"].is_object() {
        let bytes = read_state(directory, &report["collection_state"])?;
        TerminalCollectionPlan::from_native_bytes(
            &bytes,
            &sessions,
            limits(&report)?,
            BinaryIoLimits::default(),
        )
        .map_err(io)?
    } else {
        if report["collection"]["status"] == "completed" {
            return Err(AppError::input(
                "completed terminal collection has no native payload",
            ));
        }
        TerminalCollectionPlan::from_sessions(&sessions, limits(&report)?).map_err(io)?
    };
    Ok(MasterReductionArtifact {
        primary,
        members,
        collection,
    })
}

fn directory(path: &Path) -> &Path {
    if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(Path::new("."))
    }
}

fn session_refs<'a>(
    primary: &'a TerminalRelationSession,
    members: &'a [TerminalRelationSession],
) -> Vec<&'a TerminalRelationSession> {
    std::iter::once(primary).chain(members.iter()).collect()
}

fn read_state(directory: &Path, state: &Value) -> Result<Vec<u8>, AppError> {
    let name = state["file"]
        .as_str()
        .ok_or_else(|| AppError::input("native collection member missing"))?;
    let bytes = storage::read_bounded(
        &storage::safe_child(directory, name)?,
        BinaryIoLimits::default().max_program_bytes,
    )?;
    if state["bytes"].as_u64() != Some(bytes.len() as u64)
        || state["blake3"].as_str() != Some(blake3::hash(&bytes).to_hex().as_str())
    {
        return Err(AppError::input(
            "native collection member digest/size mismatch",
        ));
    }
    Ok(bytes)
}

fn write_state(directory: &Path, bytes: &[u8], prefix: &str) -> Result<Value, AppError> {
    let digest = blake3::hash(bytes).to_hex().to_string();
    let name = format!("{prefix}-{digest}.rrbin");
    write_file_atomically(&directory.join(&name), bytes, true).map_err(io)?;
    Ok(json!({"file":name,"bytes":bytes.len(),"blake3":digest}))
}

fn load_member(directory: &Path, member: &Value) -> Result<TerminalRelationSession, AppError> {
    TerminalRelationSession::from_native_bytes(
        &read_state(directory, &member["native_state"])?,
        profile::from_report(member)?.relation_limits(),
        BinaryIoLimits::default(),
    )
    .map_err(io)
}

fn load_members(
    directory: &Path,
    report: &Value,
) -> Result<Vec<TerminalRelationSession>, AppError> {
    report["collection_members"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|member| load_member(directory, member))
        .collect()
}

fn copy_member(source: &Path, output: &Path, member: &Value) -> Result<Value, AppError> {
    let mut member = member.clone();
    member["native_state"] = write_state(
        output,
        &read_state(source, &member["native_state"])?,
        "collection-member",
    )?;
    Ok(member)
}

fn primary_member(report: &Value) -> Result<Value, AppError> {
    let mut result = json!({"native_state":report["native_state"]});
    for field in [
        "relation_authority",
        "saved_rule_assistance",
        "circuit_symmetry_assistance",
        "inherited_equation_authority",
    ] {
        if let Some(value) = report.get(field) {
            result[field] = value.clone();
        }
    }
    result["source_manifest_blake3"] = json!(
        blake3::hash(&serde_json::to_vec(report).map_err(io)?)
            .to_hex()
            .to_string()
    );
    profile::record(&mut result, profile::from_report(report)?);
    Ok(result)
}

/// Preserve prior proof as a restartable capsule before any primary session is
/// extended. Its old finite sessions are only needed until the new rebind is
/// durably written; the final package has no external source-path dependency.
pub(super) fn inherit(source: &Path, output: &Path, report: &mut Value) -> Result<(), AppError> {
    let source_report = master_reduction_inspect(source)?;
    let source = directory(source);
    let members = source_report["collection_members"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|member| copy_member(source, output, member))
        .collect::<Result<Vec<_>, _>>()?;
    report["collection_members"] = json!(members);
    if source_report["collection_state"].is_object() {
        // Transport content-addressed bytes now; complete() validates the
        // entire inherited proof against these exact frozen sessions once
        // before any completed publication. Pending metadata makes no claim
        // that this old collection is already rebound/applied.
        let state = write_state(
            output,
            &read_state(source, &source_report["collection_state"])?,
            "collection-inherited",
        )?;
        let mut inputs = vec![copy_member(
            source,
            output,
            &primary_member(&source_report)?,
        )?];
        inputs.extend(members);
        report["inherited_collection"] = json!({"native_state":state,"sessions":inputs,"collection_limits":source_report["collection_limits"]});
    }
    report["collection_state"] = Value::Null;
    report.as_object_mut().unwrap().remove("collection_limits");
    report["collection"] = json!({"strategy":STRATEGY,"status":"pending"});
    Ok(())
}

/// Add compatible published finite inventories, retaining the already packaged
/// peers on plain repeated refinement. Fingerprint collisions must be byte-
/// identical sessions; a partial inventory is never silently overwritten.
pub(super) fn add_members(
    options: &MasterReductionOptions,
    primary: &TerminalRelationSession,
    report: &mut Value,
) -> Result<(), AppError> {
    if options.collection_artifacts.is_empty()
        && report["collection_members"]
            .as_array()
            .is_none_or(Vec::is_empty)
    {
        report["collection_members"] = json!([]);
        return Ok(());
    }
    let primary_bytes = primary
        .to_native_bytes(BinaryIoLimits::default())
        .map_err(io)?;
    let mut known = BTreeMap::from([(
        primary.family_owner().fingerprint().to_owned(),
        blake3::hash(&primary_bytes).to_hex().to_string(),
    )]);
    let mut members = report["collection_members"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for member in &members {
        let loaded = load_member(&options.directory, member)?;
        let fingerprint = loaded.family_owner().fingerprint().to_owned();
        let digest = member["native_state"]["blake3"]
            .as_str()
            .ok_or_else(|| AppError::input("member digest missing"))?
            .to_owned();
        if known.insert(fingerprint, digest).is_some() {
            return Err(AppError::input(
                "packaged collection contains duplicate family sessions",
            ));
        }
    }
    for artifact in &options.collection_artifacts {
        storage::reject_overlapping_directories(
            &directory(artifact).canonicalize().map_err(io)?,
            &options.directory,
        )?;
        let source_report = master_reduction_inspect(artifact)?;
        if !matches!(
            source_report["status"].as_str(),
            Some("published_unrefined" | "completed_nonminimal")
        ) {
            return Err(AppError::input(
                "collection input must be a completed publication",
            ));
        }
        let source_directory = directory(artifact);
        let mut candidates = vec![primary_member(&source_report)?];
        candidates.extend(
            source_report["collection_members"]
                .as_array()
                .into_iter()
                .flatten()
                .cloned(),
        );
        for member in candidates {
            let loaded = load_member(source_directory, &member)?;
            let fingerprint = loaded.family_owner().fingerprint().to_owned();
            let digest = member["native_state"]["blake3"]
                .as_str()
                .ok_or_else(|| AppError::input("member digest missing"))?
                .to_owned();
            if let Some(previous) = known.get(&fingerprint) {
                if previous != &digest {
                    return Err(AppError::input(
                        "collection has different finite sessions for the same family; refine or merge them explicitly first",
                    ));
                }
                continue;
            }
            known.insert(fingerprint, digest);
            members.push(copy_member(source_directory, &options.directory, &member)?);
        }
    }
    report["collection_members"] = json!(members);
    Ok(())
}

pub(super) fn input_identity(
    options: &MasterReductionOptions,
    resumed: Option<&Value>,
) -> Result<Value, AppError> {
    if let Some(report) = resumed {
        if report["collection_requested_paths"] != json!(options.collection_artifacts) {
            return Err(AppError::input(
                "cannot change collection inputs while resuming a phase",
            ));
        }
        let identity = report["collection_input_identity"].clone();
        if !identity.is_array() {
            return Err(AppError::input(
                "collection checkpoint lacks input identity",
            ));
        }
        // Portable checkpoints already own their member bytes. The original
        // external inputs need not still exist to resume the owned phase.
        if options
            .collection_artifacts
            .iter()
            .all(|path| path.exists())
            && identity != input_identity(options, None)?
        {
            return Err(AppError::input("collection input changed since checkpoint"));
        }
        return Ok(identity);
    }
    let mut digests = options
        .collection_artifacts
        .iter()
        .map(|path| {
            let report = master_reduction_inspect(path)?;
            Ok(blake3::hash(&serde_json::to_vec(&report).map_err(io)?)
                .to_hex()
                .to_string())
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    digests.sort();
    Ok(json!(digests))
}

pub(super) fn complete(
    options: &MasterReductionOptions,
    primary: &TerminalRelationSession,
    report: &mut Value,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
    started: Instant,
) -> Result<(), AppError> {
    if report["collection"]["status"] == "completed" && report["collection_state"].is_object() {
        let _ = load_master_reduction(&options.directory)?;
        return Ok(());
    }
    // Save the full source-search cursor before the bounded indivisible
    // collection prepare. A cancellation never loses completed finite work.
    let finite_feedback = feedback_for_phase(options, Some(report))?;
    report["stage"] = json!("terminal_collection");
    report["collection"] = json!({"strategy":STRATEGY,"status":"preparing",
        "finite_feedback_stage":if finite_feedback { "pending" } else { "disabled" },
        "finite_feedback_enabled":finite_feedback,"finite_feedback_recipe":FEEDBACK_RECIPE});
    record_limits(report)?;
    save(options, primary, report, "running", started, observer)?;
    report["stage"] = json!("terminal_collection");
    write_json(&options.directory.join("latest.json"), report)?;
    observer(event(report, "master_reduction_progress", started));
    if cancel.load(Ordering::Relaxed) {
        return Ok(());
    }
    let members = load_members(&options.directory, report)?;
    let sessions = session_refs(primary, &members);
    let timer = Instant::now();
    let limits = limits(report)?;
    let plan = if report["inherited_collection"].is_object() {
        let capsule = &report["inherited_collection"];
        let prior = capsule["sessions"]
            .as_array()
            .ok_or_else(|| AppError::input("inherited collection sessions missing"))?
            .iter()
            .map(|member| load_member(&options.directory, member))
            .collect::<Result<Vec<_>, _>>()?;
        let prior_refs = prior.iter().collect::<Vec<_>>();
        let old = TerminalCollectionPlan::from_native_bytes(
            &read_state(&options.directory, &capsule["native_state"])?,
            &prior_refs,
            limits,
            BinaryIoLimits::default(),
        )
        .map_err(io)?;
        if finite_feedback {
            old.refine_with_finite_feedback_cancellable(&sessions, limits, cancel)
                .map_err(io)?
        } else if options.operation == MasterReductionOperation::Refine {
            old.refine(&sessions, limits).map_err(io)?
        } else {
            old.rebind(&sessions, limits).map_err(io)?
        }
    } else if finite_feedback {
        TerminalCollectionPlan::prepare_with_finite_feedback_cancellable(&sessions, limits, cancel)
            .map_err(io)?
    } else if options.operation == MasterReductionOperation::Refine {
        TerminalCollectionPlan::prepare(&sessions, limits).map_err(io)?
    } else {
        TerminalCollectionPlan::from_sessions(&sessions, limits).map_err(io)?
    };
    let elapsed = timer.elapsed().as_secs_f64();
    let bytes = plan
        .to_native_bytes(BinaryIoLimits::default())
        .map_err(io)?;
    report["collection_state"] = write_state(&options.directory, &bytes, "collection")?;
    let statistics = plan.statistics();
    report["collection"] = json!({"strategy":STRATEGY,"status":"completed","preparation_seconds":elapsed,
        "families":sessions.len(),"raw_terminals":plan.raw_terminals().len(),
        "normalized_terminals":statistics.normalized_terminals,
        "before_collection":statistics.precollection_remaining,
        "global_alias_classes":statistics.global_alias_classes,
        "terminal_equations":statistics.terminal_equations,
        "finite_feedback_enabled":finite_feedback,
        "finite_feedback_stage":if finite_feedback { "completed" } else { "disabled" },
        "finite_feedback_recipe":FEEDBACK_RECIPE,
        "finite_feedback_rows":statistics.finite_feedback_rows,
        "finite_feedback_columns":statistics.finite_feedback_columns,
        "finite_feedback_auxiliary_columns":statistics.finite_feedback_auxiliary_columns,
        "finite_feedback_aliases":statistics.finite_feedback_aliases,
        "finite_feedback_equations":statistics.finite_feedback_equations,
        "finite_feedback_nonzeros":statistics.finite_feedback_nonzeros,
        "finite_feedback_replay_operations":statistics.finite_feedback_replay_operations,
        "finite_feedback_count_meaning":"cumulative retained proof layers; disabled discovery preserves prior feedback maps",
        "passthrough_terminals":statistics.passthrough_terminals,
        "groups":statistics.collection_groups,
        "proof_layers":statistics.proof_layers,
        "alias_count_meaning":"sum over retained proof layers; not a current-basis census after multiple refinements",
        "remaining_terminals":plan.remaining_terminals().len(),
        "authority":"inherits all participating finite-session and equation-provider authority; only new diagonal sources receive independent regenerated-source replay",
        "coverage":"collection does not enlarge the primary published campaign scope",
        "native_bytes":bytes.len(),"minimality_claim":false});
    record_counts(report, &plan, primary)?;
    report
        .as_object_mut()
        .unwrap()
        .remove("inherited_collection");
    write_json(&options.directory.join("latest.json"), report)?;
    Ok(())
}

fn record_counts(
    report: &mut Value,
    plan: &TerminalCollectionPlan,
    primary: &TerminalRelationSession,
) -> Result<(), AppError> {
    let mut primary_outputs = std::collections::BTreeSet::new();
    for key in primary.raw_terminals() {
        primary_outputs.extend(
            plan.apply(primary.family_owner(), key)
                .map_err(io)?
                .terms()
                .keys()
                .cloned(),
        );
    }
    report["collection"]["primary_remaining_terminals"] = json!(primary_outputs.len());
    report["remaining_terminals"] = json!(primary_outputs.len());
    report["collection"]["primary_before_collection"] =
        json!(primary.statistics().remaining_terminals);
    Ok(())
}
