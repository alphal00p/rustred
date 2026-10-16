//! Required raw-only finite source provenance. No Work page can be restored
//! from this portable archive, and no metadata hash proves closure by itself.
use super::*;
use crate::application::routed_campaign::finite_demand::{
    backend::publication::CompletedFiniteSource,
    storage::{
        RawCensusLimits, RawExecutionBinding, RawKeyKind, RawProvenance, StorageError,
        StorageLimits,
    },
};
use rustred::family::IntegralKey;
use std::collections::BTreeSet;

const KIND: &str = "finite-demand-generated-v1";
const SOURCE_SCHEMA: &str = "rustred.finite-publication-source.v1";
#[derive(Clone, Copy)]
struct Limits {
    storage: StorageLimits,
    source_bytes: usize,
    sources: usize,
    staging_attempts: usize,
}

/// Caller grants are never read from the publication itself. Default count
/// grants impose no extra ceiling on the separately bounded byte payloads.
fn limits(policy: Option<&crate::FiniteDemandPolicy>) -> Result<Limits, AppError> {
    if let Some(policy) = policy {
        return Ok(Limits {
            storage: policy.storage.into(),
            source_bytes: policy.control.max_bytes.try_into().map_err(|_| {
                AppError::limit("finite publication metadata allowance exceeds platform size")
            })?,
            sources: policy.control.max_files,
            staging_attempts: policy.control.max_files,
        });
    }
    Ok(Limits {
        storage: default_storage_limits(),
        source_bytes: 64 << 20,
        sources: usize::MAX,
        staging_attempts: usize::MAX,
    })
}
fn default_storage_limits() -> StorageLimits {
    StorageLimits {
        raw: RawCensusLimits {
            max_page_records: 8192,
            max_page_bytes: 8 << 20,
            max_runs: usize::MAX,
            max_pages: usize::MAX,
            max_directory_bytes: 256 << 20,
            max_stored_references: u64::MAX,
            max_merge_runs: usize::MAX,
            // Preserve the native default fan-in cache (R1 already has ~99
            // runs). Actual retained payload stays under the SAME256MiB cap;
            // the page-count grant does not preallocate page contents.
            max_merge_cached_pages: 1024,
            max_merge_payload_bytes: 256 << 20,
            max_merge_read_bytes: 128 << 30,
            max_decode_transient_bytes: 128 << 20,
        },
        max_arity: usize::MAX,
        max_delta_records: usize::MAX,
        max_delta_payload_bytes: (6u64 << 30).min(usize::MAX as u64) as usize,
        max_page_records: 8192,
        max_page_bytes: 8 << 20,
        max_cached_pages: 0,
        max_cached_page_bytes: 0,
        max_directory_bytes: 256 << 20,
        max_manifest_bytes: 64 << 20,
        max_state_bytes: 64 << 20,
        max_scope_rows: usize::MAX,
        max_census_records: usize::MAX,
        max_runs: usize::MAX,
        max_pages: usize::MAX,
        max_physical_files: usize::MAX,
        max_disk_bytes: 128 << 30,
        max_lookup_pages: 0,
        max_lookup_bytes: 0,
        max_transient_bytes: 128 << 20,
    }
}
fn fail(error: StorageError) -> AppError {
    use crate::application::routed_campaign::finite_demand::storage::ErrorKind;
    AppError::new(
        match error.kind {
            ErrorKind::Resource => crate::AppErrorKind::Limit,
            ErrorKind::Cancelled => crate::AppErrorKind::Cancelled,
            ErrorKind::Corrupt => crate::AppErrorKind::Serialization,
            _ => crate::AppErrorKind::Execution,
        },
        error.to_string(),
    )
}
fn check(cancel: &AtomicBool) -> Result<(), AppError> {
    if cancel.load(Ordering::Acquire) {
        Err(AppError::new(
            crate::AppErrorKind::Cancelled,
            "finite provenance cancelled",
        ))
    } else {
        Ok(())
    }
}
fn hex(bytes: [u8; 32]) -> String {
    blake3::Hash::from(bytes).to_hex().to_string()
}
fn digest(value: &Value) -> Result<[u8; 32], AppError> {
    Ok(*blake3::Hash::from_hex(
        value
            .as_str()
            .ok_or_else(|| AppError::input("finite provenance digest missing"))?,
    )
    .map_err(|_| AppError::input("finite provenance digest malformed"))?
    .as_bytes())
}
fn source_path(directory: &Path, record: &Value) -> Result<PathBuf, AppError> {
    storage::safe_child(
        directory,
        record["capsule"]
            .as_str()
            .ok_or_else(|| AppError::input("finite capsule path absent"))?,
    )
}
pub(super) fn validate_marker(report: &Value) -> Result<(), AppError> {
    if report["schema"] == FINITE_SCHEMA {
        if report["finite_source"]["kind"] != KIND
            || report["finite_source"]["required"] != true
            || report["finite_source"]["work_bytes_included"] != false
        {
            return Err(AppError::input(
                "v3 publication requires finite raw-only source provenance",
            ));
        }
    } else if report.get("finite_source").is_some()
        || report.get("inherited_finite_sources").is_some()
    {
        return Err(AppError::input(
            "required finite provenance cannot be downgraded to master schema v1/v2",
        ));
    }
    Ok(())
}

fn open(
    directory: &Path,
    record: &Value,
    limits: Limits,
) -> Result<(RawProvenance, Value), AppError> {
    if record["kind"] != KIND
        || record["required"] != true
        || record["work_bytes_included"] != false
    {
        return Err(AppError::input("unsupported required finite source"));
    }
    let capsule = source_path(directory, record)?;
    let bytes = storage::read_bounded(
        &storage::safe_child(&capsule, "source.json")?,
        limits.source_bytes,
    )?;
    if bytes.len() as u64 != record["source_bytes"].as_u64().unwrap_or(u64::MAX)
        || blake3::hash(&bytes).as_bytes() != &digest(&record["source_blake3"])?
    {
        return Err(AppError::input(
            "finite source provenance bytes/digest differ",
        ));
    }
    let source: Value = serde_json::from_slice(&bytes).map_err(io)?;
    if source["schema"] != SOURCE_SCHEMA
        || source["complete_required_scope"] != true
        || source["all_rows_exhausted"] != true
        || source["auxiliary_rows"] != 0
        || source["independently_verified"] != false
    {
        return Err(AppError::input(
            "finite source has no supported complete required-scope provenance",
        ));
    }
    let execution = RawExecutionBinding {
        physical_arity: source["execution"]["physical_arity"]
            .as_u64()
            .and_then(|n| n.try_into().ok())
            .ok_or_else(|| AppError::input("finite physical arity missing"))?,
        program_identity: digest(&source["execution"]["program_identity"])?,
    };
    let archive = RawProvenance::open_capsule(
        &capsule,
        &execution,
        digest(&record["generation_blake3"])?,
        limits.storage,
    )
    .map_err(fail)?;
    let payloads: Vec<String> =
        serde_json::from_value(source["owner_payload_digests"].clone()).map_err(io)?;
    let original = source["original_selection"]
        .as_str()
        .ok_or_else(|| AppError::input("finite original selection absent"))?;
    crate::application::routed_campaign::finite_demand::prepared::validate_portable_execution(
        original,
        &payloads,
        &source["execution_identity"],
        archive.execution(),
        limits.source_bytes,
    )?;
    crate::application::routed_campaign::finite_demand::backend::publication::validate_portable_scope(
        &archive,&source,limits.source_bytes,&AtomicBool::new(false))?;
    if record["generation"].as_u64() != Some(archive.generation())
        || source["generation_blake3"] != record["generation_blake3"]
        || source["input_occurrences"].as_u64() != Some(archive.state().input_occurrences)
        || source["completed_work"].as_u64() != Some(archive.state().completed_work)
        || source["scope_original_source"] != hex(archive.state().scope.original_source)
    {
        return Err(AppError::input(
            "finite source generation/state binding differs",
        ));
    }
    let rows = source["completed_rows"]
        .as_array()
        .ok_or_else(|| AppError::input("completed finite rows missing"))?;
    if rows.len() != archive.state().scope.rows.len() || rows.len() != archive.state().cursors.len()
    {
        return Err(AppError::input("finite source scope/cursor length differs"));
    }
    for ((row, cursor), recorded) in archive
        .state()
        .scope
        .rows
        .iter()
        .zip(&archive.state().cursors)
        .zip(rows)
    {
        if row.role
            != crate::application::routed_campaign::finite_demand::storage::ScopeRole::Required
            || recorded["origin"] != row.origin
            || recorded["exact_domain"] != hex(row.exact_domain)
            || recorded["source_policy"] != hex(cursor.source_policy)
            || recorded["cursor_blake3"]
                != blake3::hash(&cursor.encoded_cursor).to_hex().to_string()
            || recorded["input_occurrences"].as_u64() != Some(cursor.input_occurrences)
            || recorded["completed_windows"].as_u64() != Some(cursor.completed_windows)
        {
            return Err(AppError::input("finite completed-row commitment differs"));
        }
    }
    Ok((archive, source))
}

fn fresh_path(
    directory: &Path,
    generation: [u8; 32],
    limits: Limits,
) -> Result<(String, PathBuf), AppError> {
    for ordinal in 0..limits.staging_attempts {
        let name = format!("finite-source-{}-{ordinal:04}", hex(generation));
        let path = directory.join(&name);
        if !path.exists() {
            return Ok((name, path));
        }
    }
    Err(AppError::limit("finite source staging namespace exhausted"))
}
fn copied_record(
    directory: &Path,
    archive: &RawProvenance,
    source: &Value,
    cancel: &AtomicBool,
    limits: Limits,
) -> Result<Value, AppError> {
    check(cancel)?;
    let bytes = serde_json::to_vec(source).map_err(io)?;
    if bytes.len() > limits.source_bytes {
        return Err(AppError::limit("finite source metadata byte allowance"));
    }
    let (name, path) = fresh_path(directory, archive.digest(), limits)?;
    let copied = archive
        .copy_capsule(&path, limits.storage, cancel)
        .map_err(fail)?;
    check(cancel)?;
    write_file_atomically(&path.join("source.json"), &bytes, false).map_err(io)?;
    let record = json!({"kind":KIND,"required":true,"capsule":name,"generation":copied.generation(),
        "generation_blake3":hex(copied.digest()),"source_bytes":bytes.len(),
        "source_blake3":blake3::hash(&bytes).to_hex().to_string(),"work_bytes_included":false,
        "assurance":"trusted_generated; integrity is not independent dependency replay"});
    open(directory, &record, limits)?;
    check(cancel)?;
    Ok(record)
}

pub(super) fn capture(
    captured: &CompletedFiniteSource,
    directory: &Path,
    report: &mut Value,
    cancel: &AtomicBool,
    policy: Option<&crate::FiniteDemandPolicy>,
) -> Result<(), AppError> {
    check(cancel)?;
    if report["finite_source"].is_object() {
        if report["finite_source"]["generation_blake3"] != hex(captured.source_identity()) {
            return Err(AppError::input(
                "publication resume captured a different finite generation",
            ));
        }
        return validate_required_with_cancel(directory, report, cancel, limits(policy)?);
    }
    let archive = captured.raw_provenance();
    let state = archive.state();
    let request = captured.request();
    let w = captured.window_policy();
    let source = json!({"schema":SOURCE_SCHEMA,"generation_blake3":hex(captured.source_identity()),
        "execution":{"physical_arity":archive.execution().physical_arity,"program_identity":hex(archive.execution().program_identity)},
        "execution_identity":captured.execution_identity(),
        "scope_original_source":hex(state.scope.original_source),"request_binding":request.checkpoint_binding(),
        "original_selection":request.matching.selection_json,"original_queries":request.matching.queries_json,
        "amendments":request.amendments.iter().map(|a|a.text.as_str()).collect::<Vec<_>>(),
        "owner_payload_digests":captured.owner_payload_digests(),
        "window_policy":{"max_seed_points":w.max_seed_points,"max_seed_bytes":w.max_seed_bytes,
            "max_positive_layers":w.max_positive_layers,"max_partition_checks":w.max_partition_checks},
        "complete_required_scope":true,"all_rows_exhausted":true,"auxiliary_rows":0,"independently_verified":false,
        "completed_rows":state.scope.rows.iter().zip(&state.cursors).map(|(r,c)|json!({"origin":r.origin,
            "exact_domain":hex(r.exact_domain),"source_policy":hex(c.source_policy),
            "input_occurrences":c.input_occurrences,"completed_windows":c.completed_windows,
            "cursor_blake3":blake3::hash(&c.encoded_cursor).to_hex().to_string()})).collect::<Vec<_>>(),
        "input_occurrences":state.input_occurrences,"completed_work":state.completed_work,
        "inventory":captured.summary()});
    // All required bytes exist and validate before the caller exposes latest.
    let limits = limits(policy)?;
    let record = copied_record(directory, archive, &source, cancel, limits)?;
    let inputs = storage::package_inputs(request, directory, captured.owner_payload_digests())?;
    check(cancel)?;
    report["finite_source"] = record;
    report["inventory"] = captured.summary().clone();
    report["inputs"] = inputs;
    report["independently_verified"] = json!(false);
    report["publication_assurance"] = json!("trusted_generated");
    validate_required_with_cancel(directory, report, cancel, limits)
}

fn records(report: &Value, limits: Limits) -> Result<Vec<&Value>, AppError> {
    validate_marker(report)?;
    if report["schema"] != FINITE_SCHEMA {
        return Ok(Vec::new());
    }
    let inherited = report
        .get("inherited_finite_sources")
        .map(|value| {
            value
                .as_array()
                .ok_or_else(|| AppError::input("inherited finite sources malformed"))
        })
        .transpose()?;
    if inherited
        .map_or(0, Vec::len)
        .checked_add(1)
        .is_none_or(|n| n > limits.sources)
    {
        return Err(AppError::limit("inherited finite source count"));
    }
    let mut rows = vec![&report["finite_source"]];
    if let Some(inherited) = inherited {
        rows.extend(inherited);
    }
    Ok(rows)
}
fn validate_inputs(
    directory: &Path,
    report: &Value,
    source: &Value,
    archive: &RawProvenance,
    limits: Limits,
) -> Result<(), AppError> {
    for (file, field) in [
        ("inputs/original-selection.json", "original_selection"),
        ("inputs/queries.json", "original_queries"),
    ] {
        let expected = source[field]
            .as_str()
            .ok_or_else(|| AppError::input("finite original input missing"))?;
        if storage::digest_file(&storage::safe_child(directory, file)?)?
            != blake3::hash(expected.as_bytes()).to_hex().to_string()
        {
            return Err(AppError::input(
                "finite original input/source commitment differs",
            ));
        }
    }
    let expected = source["owner_payload_digests"]
        .as_array()
        .ok_or_else(|| AppError::input("finite owner vector missing"))?;
    let actual = report["inputs"]["payloads"]
        .as_array()
        .ok_or_else(|| AppError::input("published owner vector missing"))?;
    if actual.len() != expected.len() || actual.iter().zip(expected).any(|(a, e)| &a["blake3"] != e)
    {
        return Err(AppError::input("published finite owner vector differs"));
    }
    let payloads: Vec<String> =
        serde_json::from_value(source["owner_payload_digests"].clone()).map_err(io)?;
    let selection_path = storage::safe_child(directory, "inputs/selection.json")?;
    let portable = storage::read_bounded(&selection_path, limits.source_bytes)?;
    if storage::digest_file(&selection_path)? != report["inputs"]["program_binding"] {
        return Err(AppError::input(
            "portable selection program binding differs",
        ));
    }
    crate::application::routed_campaign::finite_demand::prepared::validate_portable_execution(
        std::str::from_utf8(&portable).map_err(io)?,
        &payloads,
        &source["execution_identity"],
        archive.execution(),
        limits.source_bytes,
    )?;
    let amendments = storage::read_json_bounded(
        &storage::safe_child(directory, "inputs/amendments.json")?,
        limits.source_bytes,
    )?;
    let expected = source["amendments"]
        .as_array()
        .ok_or_else(|| AppError::input("finite amendments missing"))?;
    let actual = amendments
        .as_array()
        .ok_or_else(|| AppError::input("published amendments malformed"))?;
    if actual.len() != expected.len() {
        return Err(AppError::input("finite amendment count differs"));
    }
    for (a, e) in actual.iter().zip(expected) {
        let bytes = e
            .as_str()
            .ok_or_else(|| AppError::input("finite amendment bytes missing"))?
            .as_bytes();
        let path = storage::safe_child(
            &directory.join("inputs"),
            a["path"]
                .as_str()
                .ok_or_else(|| AppError::input("amendment path missing"))?,
        )?;
        if storage::digest_file(&path)? != blake3::hash(bytes).to_hex().to_string() {
            return Err(AppError::input("finite amendment source bytes differ"));
        }
    }
    Ok(())
}
pub(super) fn validate_required(
    directory: &Path,
    report: &Value,
    policy: Option<&crate::FiniteDemandPolicy>,
) -> Result<(), AppError> {
    validate_required_with_cancel(directory, report, &AtomicBool::new(false), limits(policy)?)
}
fn validate_required_with_cancel(
    directory: &Path,
    report: &Value,
    cancel: &AtomicBool,
    limits: Limits,
) -> Result<(), AppError> {
    for (index, record) in records(report, limits)?.into_iter().enumerate() {
        check(cancel)?;
        let (archive, source) = open(directory, record, limits)?;
        let summary = archive
            .visit_raw_census(limits.storage.raw, cancel, |_, _| Ok(()), |_, _| Ok(()))
            .map_err(fail)?;
        if source["inventory"]["encountered"]["terminals"].as_u64() != Some(summary.terminals)
            || source["inventory"]["encountered"]["visited_zeros"].as_u64() != Some(summary.zeros)
            || source["inventory"]["encountered"]["composite_rule_identities"].as_u64()
                != Some(archive.state().census.selections.len() as u64)
        {
            return Err(AppError::input("finite required raw census differs"));
        }
        let multiplicity = archive
            .state()
            .census
            .selections
            .iter()
            .try_fold(0u64, |n, r| n.checked_add(r.multiplicity))
            .ok_or_else(|| AppError::limit("finite selected-rule multiplicity overflow"))?;
        if source["inventory"]["encountered"]["rule_applications"].as_u64() != Some(multiplicity) {
            return Err(AppError::input(
                "finite required selection multiplicity differs",
            ));
        }
        if index == 0 {
            validate_inputs(directory, report, &source, &archive, limits)?;
            if report["inventory"] != source["inventory"] {
                return Err(AppError::input(
                    "publication replaced its exact raw inventory",
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_session(
    directory: &Path,
    report: &Value,
    session: &TerminalRelationSession,
    policy: Option<&crate::FiniteDemandPolicy>,
) -> Result<(), AppError> {
    let limits = limits(policy)?;
    for record in records(report, limits)? {
        let (archive, _) = open(directory, record, limits)?;
        archive
            .visit_raw_census(
                limits.storage.raw,
                &AtomicBool::new(false),
                |_, _| Ok(()),
                |kind, key| {
                    if kind == RawKeyKind::Terminal
                        && !session.raw_terminals().contains(
                            &IntegralKey::try_new(key.iter().copied()).map_err(|e| {
                                rustred::persistence::finite_work::GeneratedWorkError::new(
                    rustred::persistence::finite_work::GeneratedWorkErrorKind::Corrupt,e)
                            })?,
                        )
                    {
                        return Err(rustred::persistence::finite_work::GeneratedWorkError::new(
                            rustred::persistence::finite_work::GeneratedWorkErrorKind::Corrupt,
                            "current or inherited raw source key missing from terminal session",
                        )
                        .into());
                    }
                    Ok(())
                },
            )
            .map_err(fail)?;
    }
    Ok(())
}

pub(super) fn inherit(
    source_directory: &Path,
    directory: &Path,
    report: &mut Value,
    cancel: &AtomicBool,
    policy: Option<&crate::FiniteDemandPolicy>,
) -> Result<(), AppError> {
    let limits = limits(policy)?;
    validate_required_with_cancel(source_directory, report, cancel, limits)?;
    let copied = records(report, limits)?
        .into_iter()
        .map(|r| {
            let (archive, source) = open(source_directory, r, limits)?;
            copied_record(directory, &archive, &source, cancel, limits)
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    report["finite_source"] = copied[0].clone();
    if copied.len() > 1 {
        report["inherited_finite_sources"] = json!(&copied[1..]);
    }
    validate_required_with_cancel(directory, report, cancel, limits)
}
pub(super) fn inherit_previous(
    source: &Path,
    directory: &Path,
    previous: &Value,
    report: &mut Value,
    cancel: &AtomicBool,
    policy: Option<&crate::FiniteDemandPolicy>,
) -> Result<(), AppError> {
    if previous["schema"] != FINITE_SCHEMA {
        return Ok(());
    }
    if report["schema"] != FINITE_SCHEMA {
        return Err(AppError::input(
            "a walk publication cannot discard inherited finite source provenance",
        ));
    }
    let source_directory = if source.is_dir() {
        source
    } else {
        source.parent().unwrap_or(Path::new("."))
    };
    let limits = limits(policy)?;
    validate_required_with_cancel(source_directory, previous, cancel, limits)?;
    let mut seen = records(report, limits)?
        .into_iter()
        .map(|r| digest(&r["generation_blake3"]))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let mut inherited = report["inherited_finite_sources"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for record in records(previous, limits)? {
        if !seen.insert(digest(&record["generation_blake3"])?) {
            continue;
        }
        let (archive, source) = open(source_directory, record, limits)?;
        inherited.push(copied_record(directory, &archive, &source, cancel, limits)?);
    }
    if !inherited.is_empty() {
        report["inherited_finite_sources"] = json!(inherited);
    }
    Ok(())
}
