//! Portable publication boundary; no traversal ledger is opened here.
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::*;
use crate::application::atomic_file::write_file_atomically;

const MAX_METADATA: usize = 16 * 1024 * 1024;
/// Top-level `application.json` keys understood by this loader.
const KNOWN_KEYS: [&str; 8] = [
    "schema",
    "metadata",
    "selection",
    "queries",
    "terminal_maps",
    "terminal_layer",
    "payloads",
    "load_boundary",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Payload {
    pub(super) path: String,
    pub(super) bytes: u64,
    pub(super) blake3: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) encoding: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) chunks: Vec<transport::Chunk>,
}

impl ApplicationInputs {
    pub(super) fn read_payload(&self, path: &Path, bytes: u64) -> Result<Vec<u8>, AppError> {
        let relative = path
            .strip_prefix(&self.owner_base)
            .map_err(|_| AppError::input("owner read is outside runtime package"))?;
        let payload = self
            .payloads
            .iter()
            .find(|payload| Path::new(&payload.path) == relative)
            .ok_or_else(|| AppError::input("owner read is absent from payload inventory"))?;
        if payload.bytes != bytes {
            return Err(AppError::input("owner decoded-size declaration differs"));
        }
        transport::decode(&self.owner_base, payload)
    }
}

pub(super) struct ApplicationInputs {
    pub(super) selection: String,
    pub(super) queries: String,
    pub(super) owner_base: PathBuf,
    pub(super) payloads: Vec<Payload>,
}

/// Inspect the authenticated selection's physical arity without native import.
pub fn saved_application_arity(path: &Path) -> Result<usize, AppError> {
    let (selection, _) = domain_inputs(path, None)?;
    Ok(Selection::parse(&selection)?.1)
}

pub fn saved_application_arity_with_policy(
    path: &Path,
    policy: &crate::FiniteDemandPolicy,
) -> Result<usize, AppError> {
    let (selection, _) = domain_inputs(path, Some(policy))?;
    Ok(Selection::parse_with_byte_limit(&selection, metadata_limit(Some(policy))?)?.1)
}

pub(super) fn metadata_limit(
    policy: Option<&crate::FiniteDemandPolicy>,
) -> Result<usize, AppError> {
    policy.map_or(Ok(MAX_METADATA), |p| {
        usize::try_from(p.control.max_bytes).map_err(|_| {
            AppError::limit("finite application metadata allowance exceeds platform size")
        })
    })
}

fn domain_inputs(
    path: &Path,
    policy: Option<&crate::FiniteDemandPolicy>,
) -> Result<(String, String), AppError> {
    let maximum = metadata_limit(policy)?;
    let manifest = if path.is_dir() && path.join("application.json").is_file() {
        path.join("application.json")
    } else {
        publication_manifest(path)
    };
    let report: Value =
        serde_json::from_slice(&read_bounded(&manifest, maximum)?).map_err(failure)?;
    if report["schema"] == SAVED_APPLICATION_SCHEMA {
        let directory = manifest.parent().unwrap_or(Path::new("."));
        Ok((
            read_text_descriptor(directory, &report["selection"], maximum)?,
            read_text_descriptor(directory, &report["queries"], maximum)?,
        ))
    } else {
        let inputs = publication_inputs(&manifest, &report, policy)?;
        Ok((inputs.selection, inputs.queries))
    }
}

/// Read and authenticate the frozen entry union without importing owner blobs,
/// native terminal matrices, or any campaign ledger. Suitable for inventories.
pub fn saved_application_domains<const N: usize>(
    path: &Path,
) -> Result<Vec<SavedApplicationDomain<N>>, AppError> {
    domains_with_policy(path, None)
}

pub fn saved_application_domains_with_policy<const N: usize>(
    path: &Path,
    policy: &crate::FiniteDemandPolicy,
) -> Result<Vec<SavedApplicationDomain<N>>, AppError> {
    domains_with_policy(path, Some(policy))
}

fn domains_with_policy<const N: usize>(
    path: &Path,
    policy: Option<&crate::FiniteDemandPolicy>,
) -> Result<Vec<SavedApplicationDomain<N>>, AppError> {
    let (selection, queries) = domain_inputs(path, policy)?;
    let (_, arity, _) = Selection::parse_with_byte_limit(&selection, metadata_limit(policy)?)?;
    if arity > N {
        return Err(AppError::input("saved domain arity exceeds storage arity"));
    }
    match policy {
        Some(policy) => scope::recorded_domains_with_limits::<N>(
            &queries,
            arity,
            policy.max_total_rows,
            metadata_limit(Some(policy))?,
        ),
        None => scope::recorded_domains::<N>(&queries, arity),
    }
}

/// Prepare a fixed published artifact. A directory means `artifact.json`, never
/// its moving `latest.json` pointer. The native source matrices are checked once
/// and discarded before preparing shared owner programs.
pub fn prepare_saved_application<const N: usize>(
    publication: &Path,
    options: SavedApplicationOptions,
) -> Result<PreparedSavedApplication<N>, AppError> {
    prepare_with_policy(publication, options, None)
}

/// Cold preparation and export admission using explicit finite source grants.
pub fn prepare_saved_application_with_policy<const N: usize>(
    publication: &Path,
    options: SavedApplicationOptions,
    policy: &crate::FiniteDemandPolicy,
) -> Result<PreparedSavedApplication<N>, AppError> {
    prepare_with_policy(publication, options, Some(policy))
}

fn prepare_with_policy<const N: usize>(
    publication: &Path,
    options: SavedApplicationOptions,
    policy: Option<&crate::FiniteDemandPolicy>,
) -> Result<PreparedSavedApplication<N>, AppError> {
    let started = std::time::Instant::now();
    options.validate_preparation()?;
    let manifest = publication_manifest(publication);
    let maximum = metadata_limit(policy)?;
    let bytes = read_bounded(&manifest, maximum)?;
    let report: Value = serde_json::from_slice(&bytes).map_err(failure)?;
    let inputs = publication_inputs(&manifest, &report, policy)?;
    let (selection, arity, _) = Selection::parse_with_byte_limit(&inputs.selection, maximum)?;
    if arity > N {
        return Err(AppError::input(
            "saved publication exceeds requested storage arity",
        ));
    }
    if let Some(policy) = policy {
        scope::execution_domain_with_limits::<N>(
            &inputs.queries,
            arity,
            policy.max_total_rows,
            maximum,
        )?;
    } else {
        scope::execution_domain::<N>(&inputs.queries, arity)?;
    }
    let metadata_time = started.elapsed();
    let terminal_started = std::time::Instant::now();
    let artifact = match policy {
        Some(policy) => crate::load_master_reduction_with_policy(&manifest, policy)?,
        None => crate::load_master_reduction(&manifest)?,
    };
    if read_bounded(&manifest, maximum)? != bytes {
        return Err(AppError::input(
            "publication manifest changed during application preparation",
        ));
    }
    let terminals = TerminalMaps::from_artifact(&artifact)?;
    if terminals.rows.len() as u64 != report["raw_terminals"].as_u64().unwrap_or(u64::MAX)
        || terminals.remaining.len() as u64
            != report["remaining_terminals"].as_u64().unwrap_or(u64::MAX)
    {
        return Err(AppError::input(
            "publication terminal counts differ from its native maps",
        ));
    }
    // This full source-key comparison is the offline export check, not a load
    // of the retained refinement/search matrix on every runtime invocation.
    let compact = codec::encode(&terminals, options)?;
    codec::verify_roundtrip(&terminals, &compact, options)?;
    drop(compact);
    drop(artifact);
    let terminal_time = terminal_started.elapsed();
    let metadata = json!({
        "family_fingerprint":selection.family_fingerprint,
        "physical_arity":arity,
        "source_manifest_blake3":blake3::hash(&bytes).to_hex().to_string(),
        "source_scope_binding":report["scope_binding"],
        "source_generation":if report["schema"]=="rustred.master-reduction.v3" {
            &report["finite_source"]["generation"]
        } else {&report["inventory"]["verification"]["checkpoint"]["generation"]},
        "source_kind":if report["schema"]=="rustred.master-reduction.v3" {"finite-demand-generated-v1"} else {"saved-walk"},
        "publication_assurance":report["publication_assurance"],
        "publication_verification_mode":report["publication_verification_mode"],
        "relation_authority":report["relation_authority"],
        "inherited_equation_authority":report["inherited_equation_authority"],
        "normalization_profile":report["normalization_profile"],
        "coefficient_convention":"stored unit mass and common formal loop measure; external mass and measure factors remain caller-owned",
        "source_scope":report["scope"],
        "source_query_count":serde_json::from_str::<Value>(&inputs.queries).map_err(failure)?["queries"].as_array().map(Vec::len),
        "raw_terminals":terminals.rows.len(),
        "remaining_terminals":terminals.remaining.len(),
        "source_keys_checked":terminals.rows.len(),
        "native_generated_data":true,
        "minimality_claim":false,
        "family_closure_claim":false,
        "global_termination_claim":false,
        "numerical_master_values":false,
        "authority":"inherits the saved owners and published terminal substitutions; no certification upgrade",
    });
    let mut prepared = PreparedSavedApplication::from_parts_with_mode(
        inputs, terminals, metadata, options, false, policy,
    )?;
    prepared.preparation_statistics.metadata = metadata_time;
    prepared.preparation_statistics.terminal_maps = terminal_time;
    prepared.preparation_statistics.finish(started.elapsed());
    Ok(prepared)
}

/// Export from the explicit immutable publication using the same prepared
/// application API. No new identities or search phases are generated.
pub fn export_saved_application<const N: usize>(
    publication: &Path,
    destination: &Path,
    options: SavedApplicationOptions,
) -> Result<Value, AppError> {
    prepare_saved_application::<N>(publication, options)?.export_runtime(destination)
}

/// Export with explicit finite provenance and source metadata grants.
pub fn export_saved_application_with_policy<const N: usize>(
    publication: &Path,
    destination: &Path,
    options: SavedApplicationOptions,
    policy: &crate::FiniteDemandPolicy,
) -> Result<Value, AppError> {
    prepare_saved_application_with_policy::<N>(publication, options, policy)?
        .export_runtime(destination)
}

/// Authenticate application-only data once and retain frozen native frames.
/// Owner presence, geometry/context, zero certificates and routes are eager;
/// each owner's formulas and overlays are admitted/prepared once on first use.
pub fn load_saved_application<const N: usize>(
    directory: &Path,
    options: SavedApplicationOptions,
) -> Result<PreparedSavedApplication<N>, AppError> {
    load_with_policy(directory, options, None)
}

/// Authenticate an application-only runtime with explicit caller grants for
/// its metadata and exact entry scope. The saved package supplies no grants.
pub fn load_saved_application_with_policy<const N: usize>(
    directory: &Path,
    options: SavedApplicationOptions,
    policy: &crate::FiniteDemandPolicy,
) -> Result<PreparedSavedApplication<N>, AppError> {
    load_with_policy(directory, options, Some(policy))
}

fn load_with_policy<const N: usize>(
    directory: &Path,
    options: SavedApplicationOptions,
    policy: Option<&crate::FiniteDemandPolicy>,
) -> Result<PreparedSavedApplication<N>, AppError> {
    let started = std::time::Instant::now();
    options.validate_preparation()?;
    let manifest = if directory.is_dir() {
        directory.join("application.json")
    } else {
        directory.to_path_buf()
    };
    let directory = manifest.parent().unwrap_or(Path::new("."));
    let maximum = metadata_limit(policy)?;
    let report: Value =
        serde_json::from_slice(&read_bounded(&manifest, maximum)?).map_err(failure)?;
    if report["schema"] != SAVED_APPLICATION_SCHEMA {
        return Err(AppError::input("unsupported saved application schema"));
    }
    // A package feature this loader does not know must never be ignored.
    if let Some(object) = report.as_object() {
        if let Some(unknown) = object.keys().find(|key| !KNOWN_KEYS.contains(&key.as_str())) {
            return Err(AppError::input(format!(
                "saved application key {unknown:?} is not supported by this loader"
            )));
        }
    }
    let selection = read_text_descriptor(directory, &report["selection"], maximum)?;
    let queries = read_text_descriptor(directory, &report["queries"], maximum)?;
    let payloads: Vec<Payload> =
        serde_json::from_value(report["payloads"].clone()).map_err(failure)?;
    validate_selection_payloads(&selection, &payloads)?;
    let (_, arity, _) = Selection::parse_with_byte_limit(&selection, maximum)?;
    if arity > N {
        return Err(AppError::input(
            "saved runtime exceeds requested storage arity",
        ));
    }
    if let Some(policy) = policy {
        scope::execution_domain_with_limits::<N>(&queries, arity, policy.max_total_rows, maximum)?;
    } else {
        scope::execution_domain::<N>(&queries, arity)?;
    }
    let metadata_time = started.elapsed();
    let terminal_started = std::time::Instant::now();
    let maps = read_descriptor(
        directory,
        &report["terminal_maps"],
        options.binary_limits.max_program_bytes,
    )?;
    let mut terminals = codec::decode(&maps, options)?;
    if let Some(descriptor) = report.get("terminal_layer") {
        // RR-11 layered package: base rows stay byte-identical; the layer is
        // a final exact substitution of base terminals by layer masters.
        let bytes = read_descriptor(directory, descriptor, options.binary_limits.max_program_bytes)?;
        let layer = codec::decode(&bytes, options)?;
        if layer.families.keys().ne(terminals.families.keys()) {
            return Err(AppError::input("terminal layer families differ from the base maps"));
        }
        if report["metadata"]["refinement_layer"]["base_remaining_terminals"].as_u64()
            != Some(terminals.remaining.len() as u64)
        {
            return Err(AppError::input(
                "terminal layer base count differs from the base terminal maps",
            ));
        }
        let base_remaining = std::mem::take(&mut terminals.remaining);
        terminals.remaining = layer.remaining;
        terminals.layer = Some(super::TerminalLayer {
            rows: layer.rows,
            base_remaining,
        });
        terminals.validate(options)?;
    }
    if report["metadata"]["raw_terminals"].as_u64() != Some(terminals.rows.len() as u64)
        || report["metadata"]["remaining_terminals"].as_u64()
            != Some(terminals.remaining.len() as u64)
        || report["metadata"]["source_keys_checked"].as_u64() != Some(terminals.rows.len() as u64)
    {
        return Err(AppError::input("saved runtime terminal counts disagree"));
    }
    drop(maps);
    let terminal_time = terminal_started.elapsed();
    let mut prepared = PreparedSavedApplication::from_parts_deferred(
        ApplicationInputs {
            selection,
            queries,
            owner_base: directory.to_path_buf(),
            payloads,
        },
        terminals,
        report["metadata"].clone(),
        options,
        policy,
    )?;
    prepared.preparation_statistics.metadata = metadata_time;
    prepared.preparation_statistics.terminal_maps = terminal_time;
    prepared.preparation_statistics.finish(started.elapsed());
    Ok(prepared)
}

pub(super) fn export<const N: usize>(
    prepared: &PreparedSavedApplication<N>,
    destination: &Path,
) -> Result<Value, AppError> {
    let compact = codec::encode(&prepared.terminals, prepared.options)?;
    // The prepared source loader already checked every map against the source.
    // A runtime load has checked this compact codec at its import boundary.
    fs::create_dir(destination)
        .map_err(|e| AppError::input(format!("{}: {e}", destination.display())))?;
    fs::create_dir(destination.join("objects")).map_err(failure)?;
    let mut payloads = Vec::new();
    let mut exported = BTreeMap::<String, Payload>::new();
    for payload in &prepared.inputs.payloads {
        let mut packed = if let Some(previous) = exported.get(&payload.blake3) {
            previous.clone()
        } else {
            let packed = transport::export(&prepared.inputs.owner_base, destination, payload)?;
            exported.insert(payload.blake3.clone(), packed.clone());
            packed
        };
        packed.path = payload.path.clone();
        payloads.push(packed);
    }
    let selection = write_descriptor(
        destination,
        "selection.json",
        prepared.inputs.selection.as_bytes(),
    )?;
    let queries = write_descriptor(
        destination,
        "queries.json",
        prepared.inputs.queries.as_bytes(),
    )?;
    let terminal_maps = transport::export_bytes(&compact, destination, "terminal-maps.rrbin")?;
    drop(compact);
    let mut report = json!({
        "schema":SAVED_APPLICATION_SCHEMA,
        "metadata":prepared.metadata,
        "selection":selection, "queries":queries,
        "terminal_maps":terminal_maps, "payloads":payloads,
        "load_boundary":"trusted native generated Symbolica data; integrity and structural checks do not certify source rules",
    });
    if let Some(layer) = codec::encode_layer(&prepared.terminals, prepared.options)? {
        report["terminal_layer"] = serde_json::to_value(transport::export_bytes(
            &layer,
            destination,
            "terminal-layer.rrbin",
        )?)
        .map_err(failure)?;
    }
    write_file_atomically(
        &destination.join("application.json"),
        &serde_json::to_vec_pretty(&report).map_err(failure)?,
        false,
    )
    .map_err(failure)?;
    Ok(report)
}

fn publication_manifest(path: &Path) -> PathBuf {
    if path.is_dir() {
        path.join("artifact.json")
    } else {
        path.to_path_buf()
    }
}

fn publication_inputs(
    manifest: &Path,
    report: &Value,
    policy: Option<&crate::FiniteDemandPolicy>,
) -> Result<ApplicationInputs, AppError> {
    if report["schema"] != "rustred.master-reduction.v2"
        && report["schema"] != "rustred.master-reduction.v3"
    {
        return Err(AppError::input(
            "saved application requires a published master-reduction v2 or v3 artifact",
        ));
    }
    let directory = manifest.parent().unwrap_or(Path::new("."));
    crate::application::master_reduction::validate_required_source_with_policy(
        directory, report, policy,
    )?;
    let maximum = metadata_limit(policy)?;
    let inputs = &report["inputs"];
    let selection_path = safe_child(directory, required_string(inputs, "selection")?)?;
    let selection_bytes = read_bounded(&selection_path, maximum)?;
    authenticate(
        &selection_bytes,
        required_string(inputs, "program_binding")?,
    )?;
    let selection = String::from_utf8(selection_bytes).map_err(failure)?;
    let owner_base = selection_path
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();
    let mut payloads: Vec<Payload> =
        serde_json::from_value(inputs["payloads"].clone()).map_err(failure)?;
    for payload in &mut payloads {
        let full = safe_child(directory, &payload.path)?;
        payload.path = full
            .strip_prefix(&owner_base)
            .map_err(|_| AppError::input("owner payload is outside selection directory"))?
            .to_str()
            .ok_or_else(|| AppError::input("owner path is not UTF-8"))?
            .to_owned();
    }
    validate_selection_payloads(&selection, &payloads)?;
    let queries = read_bounded(
        &safe_child(directory, required_string(inputs, "queries")?)?,
        maximum,
    )?;
    authenticate(&queries, required_string(inputs, "queries_blake3")?)?;
    let base: Value = serde_json::from_slice(&queries).map_err(failure)?;
    let mut documents = vec![base];
    let amendments_path = safe_child(directory, required_string(inputs, "amendments")?)?;
    let amendments = read_bounded(&amendments_path, maximum)?;
    authenticate(&amendments, required_string(inputs, "amendments_blake3")?)?;
    let amendments: Vec<Value> = serde_json::from_slice(&amendments).map_err(failure)?;
    for amendment in amendments {
        let path = safe_child(
            amendments_path.parent().unwrap_or(directory),
            required_string(&amendment, "path")?,
        )?;
        let bytes = read_bounded(&path, maximum)?;
        authenticate(&bytes, required_string(&amendment, "blake3")?)?;
        documents.push(serde_json::from_slice(&bytes).map_err(failure)?);
    }
    let queries = combine_queries(&documents)?;
    let count = serde_json::from_str::<Value>(&queries).map_err(failure)?["queries"]
        .as_array()
        .map(Vec::len);
    if count.map(|n| n as u64) != report["scope"]["starting_queries"].as_u64() {
        return Err(AppError::input(
            "published query count differs from the authenticated query union",
        ));
    }
    Ok(ApplicationInputs {
        selection,
        queries,
        owner_base,
        payloads,
    })
}

fn combine_queries(documents: &[Value]) -> Result<String, AppError> {
    let mut queries = Vec::new();
    let mut required = Vec::new();
    let mut auxiliary = Vec::new();
    let mut ids = BTreeSet::new();
    for (ordinal, document) in documents.iter().enumerate() {
        let schema = required_string(document, "schema")?;
        if (ordinal == 0 && schema != "rustred.owner-domain-queries.json.v2")
            || (ordinal > 0
                && schema != crate::OWNER_DOMAIN_WALK_AMENDMENT_SCHEMA
                && schema != crate::OWNER_DOMAIN_WALK_SCOPE_EXTENSION_SCHEMA)
        {
            return Err(AppError::input(
                "unsupported saved scope query/amendment schema",
            ));
        }
        let rows = document["queries"]
            .as_array()
            .ok_or_else(|| AppError::input("saved scope has no query list"))?;
        let roles = document.get("query_roles");
        let local_ids = rows
            .iter()
            .map(|row| required_string(row, "id"))
            .collect::<Result<BTreeSet<_>, _>>()?;
        if local_ids.len() != rows.len() {
            return Err(AppError::input("saved scope has duplicate query IDs"));
        }
        if let Some(roles) = roles {
            if roles
                .as_object()
                .is_none_or(|object| object.keys().any(|k| k != "required" && k != "auxiliary"))
            {
                return Err(AppError::input("invalid saved query role declaration"));
            }
            let mut declared = BTreeSet::new();
            for role in ["required", "auxiliary"] {
                for id in roles[role]
                    .as_array()
                    .ok_or_else(|| AppError::input("incomplete saved query role declaration"))?
                {
                    let id = id
                        .as_str()
                        .ok_or_else(|| AppError::input("invalid saved query role ID"))?;
                    if !local_ids.contains(id) || !declared.insert(id) {
                        return Err(AppError::input("unknown or duplicate saved query role ID"));
                    }
                }
            }
            if declared != local_ids {
                return Err(AppError::input(
                    "saved query roles must declare every query",
                ));
            }
        }
        for row in rows {
            let id = required_string(row, "id")?;
            if !ids.insert(id.to_owned()) {
                return Err(AppError::input("saved scope has duplicate query IDs"));
            }
            if roles
                .and_then(|r| r["auxiliary"].as_array())
                .is_some_and(|r| r.iter().any(|v| v.as_str() == Some(id)))
            {
                auxiliary.push(json!(id));
            } else {
                required.push(json!(id));
            }
            queries.push(row.clone());
        }
    }
    // Both kinds are retained explicitly. Caller admission below only receives
    // the primary published scope, so do not silently enlarge it with helpers.
    if !auxiliary.is_empty() {
        return Err(AppError::input(
            "saved application exports with auxiliary query roots are not supported",
        ));
    }
    serde_json::to_string(&json!({"schema":"rustred.owner-domain-queries.json.v2",
        "queries":queries,"query_roles":{"required":required,"auxiliary":auxiliary}}))
    .map_err(failure)
}

fn validate_selection_payloads(selection: &str, payloads: &[Payload]) -> Result<(), AppError> {
    let selection: Value = serde_json::from_str(selection).map_err(failure)?;
    let mut index = 0;
    for group in ["owners", "domain_rule_overlays", "preferred_owner_programs"] {
        for owner in selection[group].as_array().into_iter().flatten() {
            let payload = payloads
                .get(index)
                .ok_or_else(|| AppError::input("missing saved owner digest"))?;
            if owner["path"].as_str() != Some(payload.path.as_str())
                || owner["bytes"].as_u64() != Some(payload.bytes)
            {
                return Err(AppError::input(
                    "saved owner selection differs from its payload inventory",
                ));
            }
            safe_child(Path::new("."), &payload.path)?;
            transport::validate(payload)?;
            if payload.blake3.len() != 64 || !payload.blake3.bytes().all(|c| c.is_ascii_hexdigit())
            {
                return Err(AppError::input("invalid saved owner digest"));
            }
            index += 1;
        }
    }
    if index != payloads.len() {
        return Err(AppError::input("saved payload inventory has extra entries"));
    }
    Ok(())
}

fn required_string<'a>(value: &'a Value, key: &str) -> Result<&'a str, AppError> {
    value[key]
        .as_str()
        .ok_or_else(|| AppError::input(format!("saved application missing {key}")))
}

pub(super) fn safe_child(directory: &Path, relative: &str) -> Result<PathBuf, AppError> {
    if relative.is_empty()
        || Path::new(relative)
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(AppError::input(
            "saved application path must be a relative child",
        ));
    }
    Ok(directory.join(relative))
}

pub(super) fn read_bounded(path: &Path, maximum: usize) -> Result<Vec<u8>, AppError> {
    let file = File::open(path).map_err(|e| AppError::input(format!("{}: {e}", path.display())))?;
    if file.metadata().map_err(failure)?.len() > maximum as u64 {
        return Err(AppError::limit(
            "saved application file exceeds input allowance",
        ));
    }
    let mut bytes = Vec::new();
    file.take((maximum as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(failure)?;
    if bytes.len() > maximum {
        return Err(AppError::limit("saved application input grew during read"));
    }
    Ok(bytes)
}

pub(super) fn authenticate(bytes: &[u8], expected: &str) -> Result<(), AppError> {
    if blake3::hash(bytes).to_hex().as_str() != expected {
        return Err(AppError::input("saved application input digest mismatch"));
    }
    Ok(())
}

fn read_descriptor(
    directory: &Path,
    descriptor: &Value,
    maximum: usize,
) -> Result<Vec<u8>, AppError> {
    let payload: Payload = serde_json::from_value(descriptor.clone()).map_err(failure)?;
    // Reject the declared decoded size before any chunk read or decompression.
    // A compressed transport must not bypass the caller's native byte grant.
    if payload.bytes > maximum as u64 {
        return Err(AppError::limit(
            "saved application decoded payload exceeds input allowance",
        ));
    }
    let bytes = transport::decode(directory, &payload)?;
    authenticate(&bytes, &payload.blake3)?;
    Ok(bytes)
}
fn read_text_descriptor(
    directory: &Path,
    descriptor: &Value,
    maximum: usize,
) -> Result<String, AppError> {
    String::from_utf8(read_descriptor(directory, descriptor, maximum)?).map_err(failure)
}
fn write_descriptor(directory: &Path, path: &str, bytes: &[u8]) -> Result<Value, AppError> {
    write_file_atomically(&safe_child(directory, path)?, bytes, false).map_err(failure)?;
    Ok(json!({"path":path,"bytes":bytes.len(),"blake3":blake3::hash(bytes).to_hex().to_string()}))
}

#[cfg(test)]
mod descriptor_tests {
    use super::*;

    #[test]
    fn raw_and_compressed_descriptors_preserve_native_bytes_and_check_whole_digest() {
        let scratch = super::super::tests::Scratch::new();
        fs::create_dir(scratch.0.join("objects")).unwrap();
        let bytes = b"native payload";
        let raw = write_descriptor(&scratch.0, "legacy.rrbin", bytes).unwrap();
        let packed = serde_json::to_value(
            transport::export_bytes(bytes, &scratch.0, "terminal-maps.rrbin").unwrap(),
        )
        .unwrap();
        for descriptor in [raw, packed] {
            assert_eq!(
                read_descriptor(&scratch.0, &descriptor, bytes.len()).unwrap(),
                bytes
            );
            let mut wrong_digest = descriptor.clone();
            wrong_digest["blake3"] = json!("0".repeat(64));
            assert!(read_descriptor(&scratch.0, &wrong_digest, bytes.len()).is_err());
        }
    }

    #[test]
    fn decoded_allowance_is_checked_before_accessing_raw_or_compressed_files() {
        let scratch = super::super::tests::Scratch::new();
        let raw = json!({"path":"missing.rrbin","bytes":101,"blake3":"0".repeat(64)});
        let mut packed = raw.clone();
        packed["encoding"] = json!("gzip-chunks-v1");
        packed["chunks"] = json!([{
            "path":"also-missing.gz.part", "bytes":1, "blake3":"0".repeat(64)
        }]);
        for descriptor in [raw, packed] {
            let error = read_descriptor(&scratch.0, &descriptor, 100).unwrap_err();
            assert!(
                error
                    .message()
                    .contains("decoded payload exceeds input allowance")
            );
        }
    }
}
