//! Relations-only round boundary. Exact maps are inherited authority, not
//! newly replayed ordinary IBPs. No old auxiliary reducer or search cursor is
//! retained in this compact, content-addressed capsule.
use super::*;
use rustred::reduction::terminal_relations::{ProvedTerminalReduction, TerminalRelationRow};
use std::collections::BTreeMap;

pub(super) const RECIPE: &str = "relations-only-v1";

pub(super) fn record_authority(report: &mut Value) {
    if report["retained_relations"].is_object() {
        const RETAINED: &str = "inherits exact published terminal substitutions and guards under a content-bound source authority; retained maps are not newly regenerated ordinary-IBP proofs";
        let ordinary = report["relation_authority"].as_str().unwrap_or("");
        if !ordinary.ends_with(RETAINED) {
            report["relation_authority"] = json!(format!("{ordinary}; {RETAINED}"));
        }
    }
}

fn digest(value: &Value) -> bool {
    value
        .as_str()
        .is_some_and(|value| value.len() == 64 && value.bytes().all(|c| c.is_ascii_hexdigit()))
}

pub(super) fn authority(source: &Value) -> Result<String, AppError> {
    for field in [
        "manifest_blake3",
        "native_state_blake3",
        "collection_state_blake3",
        "program_binding",
    ] {
        if !digest(&source[field]) {
            return Err(AppError::input("invalid retained-relation source binding"));
        }
    }
    if !source["family_fingerprint"]
        .as_str()
        .is_some_and(|value| !value.is_empty())
    {
        return Err(AppError::input("retained-relation family identity missing"));
    }
    let mut hash = blake3::Hasher::new();
    hash.update(RECIPE.as_bytes());
    hash.update(&serde_json::to_vec(source).map_err(io)?);
    Ok(hash.finalize().to_hex().to_string())
}

fn state_bytes(directory: &Path, state: &Value) -> Result<Vec<u8>, AppError> {
    let name = state["file"]
        .as_str()
        .ok_or_else(|| AppError::input("retained-relation capsule file missing"))?;
    let bytes = storage::read_bounded(
        &storage::safe_child(directory, name)?,
        profile::native_io_limits(None).max_program_bytes,
    )?;
    if state["bytes"].as_u64() != Some(bytes.len() as u64)
        || state["blake3"].as_str() != Some(blake3::hash(&bytes).to_hex().as_str())
    {
        return Err(AppError::input(
            "retained-relation capsule digest/size mismatch",
        ));
    }
    Ok(bytes)
}

pub(super) fn write_state(directory: &Path, bytes: &[u8]) -> Result<Value, AppError> {
    let hash = blake3::hash(bytes).to_hex().to_string();
    let name = format!("retained-relations-{hash}.rrbin");
    write_file_atomically(&directory.join(&name), bytes, true).map_err(io)?;
    Ok(json!({"file":name,"bytes":bytes.len(),"blake3":hash}))
}

/// The complete collection must have been cold-loaded and authenticated first.
/// In particular its maps may contain equations absent from the local session.
pub(super) fn begin(
    source_report: &Value,
    source: &MasterReductionArtifact,
    options: &MasterReductionOptions,
    normalization: MasterNormalizationProfile,
    report: &mut Value,
) -> Result<TerminalRelationSession, AppError> {
    if !source.member_sessions().is_empty() || !options.collection_artifacts.is_empty() {
        return Err(AppError::input(
            "relations-only fresh search currently requires a single-family collection; cross-family relations cannot be discarded",
        ));
    }
    let primary = source.primary_session();
    let family = primary.family_owner();
    let mut reductions = Vec::with_capacity(primary.raw_terminals().len());
    let mut expected = BTreeMap::new();
    for target in primary.raw_terminals() {
        let exact = source.apply_terminal(family, target)?;
        let mut terms = TerminalRelationRow::new();
        for (key, value) in exact.terms() {
            if key.family_fingerprint() != family.fingerprint() {
                return Err(AppError::input(
                    "relations-only fresh search cannot yet import a cross-family collection output",
                ));
            }
            terms.insert(key.integral().clone(), value.clone());
        }
        expected.insert(target.clone(), terms.clone());
        reductions.push(ProvedTerminalReduction::new_with_shared_conditions(
            target.clone(),
            terms,
            exact.shared_nonzero_conditions(),
        ));
    }
    let source_binding = json!({
        "manifest_blake3":blake3::hash(&serde_json::to_vec(source_report).map_err(io)?).to_hex().to_string(),
        "native_state_blake3":source_report["native_state"]["blake3"],
        "collection_state_blake3":source_report["collection_state"]["blake3"],
        "family_fingerprint":family.fingerprint(),
        "program_binding":source_report["inputs"]["program_binding"],
        "scope_binding":source_report["scope_binding"],
        "relation_authority":source_report["relation_authority"],
        "normalization_profile":source_report["normalization_profile"],
        "meaning":"inherited exact published maps and guards; source digests are integrity/provenance, not new independent ordinary-IBP proof",
    });
    let binding = authority(&source_binding)?;
    let mut session = TerminalRelationSession::fresh_with_proved_reductions(
        family.clone(),
        primary.raw_terminals().clone(),
        0,
        binding.clone(),
        reductions,
        normalization.relation_limits(),
    )
    .map_err(io)?;
    if session.apply_all_terminals().map_err(io)? != expected {
        return Err(AppError::input(
            "relations-only import changed the published terminal convention",
        ));
    }
    // The provenance capsule always has depth zero: it must not duplicate a
    // potentially enormous new depth-three seed inventory. Extending the same
    // raw inventory below keeps the compact imported U and zero source cursor.
    let bytes = session
        .to_native_bytes(profile::native_io_limits(None))
        .map_err(io)?;
    report["retained_relations"] = json!({
        "recipe":RECIPE,"binding":binding,"source":source_binding,
        "native_state":write_state(&options.directory, &bytes)?,
        "raw_terminals":primary.raw_terminals().len(),
        "normalization_profile":normalization.name(),
        "ordinary_source_rows_included":0,"auxiliary_search_rows_included":0,
    });
    if options.seed_depth != 0 {
        session
            .extend(&Default::default(), options.seed_depth)
            .map_err(io)?;
    }
    report["fresh_search"] = json!(true);
    report["fresh_search_recipe"] = json!(RECIPE);
    report["collection_members"] = json!([]);
    report["collection_state"] = Value::Null;
    report["collection"] = json!({"strategy":collection::STRATEGY,"status":"pending"});
    for field in [
        "inherited_collection",
        "collection_limits",
        "inherited_equation_authority",
        "partial_refinement_source",
        "containing_sector_depth",
        "requested_containing_sector_depth",
        "source_selection",
        "circuit_equation_preparation",
        "numerator_symmetry_preparation",
        "numerator_symmetry_reduction",
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
        "selective_symmetry_feedback_enabled",
        "selective_symmetry_feedback",
        "local_dimensional_recurrence_preparation",
        "local_dimensional_recurrence_certificate",
        "local_dimensional_recurrence_assistance",
        "previous_stage_source",
        "reused_previous_stage",
    ] {
        report.as_object_mut().unwrap().remove(field);
    }
    Ok(session)
}

/// Validate the compact capsule on every cold load before application or any
/// future publication. It is self-contained and never opens the old source.
pub(super) fn validate(
    directory: &Path,
    report: &Value,
    session: &TerminalRelationSession,
) -> Result<(), AppError> {
    super::terminal_import::validate_archive(directory, report)?;
    let native_binding = session.retained_relations_binding();
    let record = &report["retained_relations"];
    if record.is_null() && native_binding.is_none() {
        return Ok(());
    }
    if record["recipe"] != RECIPE
        || (report["fresh_search"] == true && report["fresh_search_recipe"] != RECIPE)
        || record["binding"].as_str() != native_binding
        || record["binding"] != authority(&record["source"])?
        || record["source"]["family_fingerprint"] != session.family_owner().fingerprint()
        || record["source"]["program_binding"] != report["inputs"]["program_binding"]
        || record["ordinary_source_rows_included"] != 0
        || record["auxiliary_search_rows_included"] != 0
    {
        return Err(AppError::input(
            "retained-relation authority differs from native state or saved program",
        ));
    }
    let normalization = match record["normalization_profile"].as_str() {
        Some(name) if name == MasterNormalizationProfile::StandardV1.name() => {
            MasterNormalizationProfile::StandardV1
        }
        Some(name) if name == MasterNormalizationProfile::ConservativeV1.name() => {
            MasterNormalizationProfile::ConservativeV1
        }
        _ => {
            return Err(AppError::input(
                "invalid retained-relation normalization profile",
            ));
        }
    };
    let capsule = TerminalRelationSession::from_native_bytes(
        &state_bytes(directory, &record["native_state"])?,
        normalization.relation_limits(),
        profile::native_io_limits(None),
    )
    .map_err(io)?;
    let stats = capsule.statistics();
    if capsule.retained_relations_binding() != native_binding
        || capsule.family_owner().fingerprint() != session.family_owner().fingerprint()
        || !capsule.raw_terminals().is_subset(session.raw_terminals())
        || record["raw_terminals"].as_u64() != Some(capsule.raw_terminals().len() as u64)
        || stats.completed_source_rows != 0
        || stats.completed_seeds != 0
        || stats.pending_rebuild_rows != 0
        || stats.completed_rebuild_rows != 0
        || stats.auxiliary_columns != 0
        || stats.seed_depth != 0
        || capsule.assistance_binding().is_some()
    {
        return Err(AppError::input(
            "retained-relation capsule has incompatible maps, family or search state",
        ));
    }
    session
        .validate_retained_baseline_identity(&capsule)
        .map_err(io)?;
    // A rank extension may be checkpointed while its old row space is being
    // reindexed. Application is already unavailable during that state. Strict
    // containment is checked as soon as rebuilding ends, and again at every
    // subsequent save/cold application before any completed publication.
    if session.statistics().pending_rebuild_rows == 0 {
        session.validate_retained_baseline(&capsule).map_err(io)?;
    }
    Ok(())
}

pub(super) fn inherit(
    source: &Path,
    output: &Path,
    source_report: &Value,
    report: &mut Value,
) -> Result<(), AppError> {
    super::terminal_import::inherit_archive(source, output, source_report)?;
    if source_report["retained_relations"].is_null() {
        report.as_object_mut().unwrap().remove("retained_relations");
        return Ok(());
    }
    let mut record = source_report["retained_relations"].clone();
    record["native_state"] = write_state(output, &state_bytes(source, &record["native_state"])?)?;
    report["retained_relations"] = record;
    Ok(())
}
