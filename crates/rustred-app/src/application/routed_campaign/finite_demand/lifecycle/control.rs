//! Source/control commitments adjacent to the authoritative native store.
use super::super::{
    storage::{self, RawExecutionBinding, ValidatedGeneration},
    windows::WindowLimits,
};
use super::FiniteDemandMode;
use super::{
    control_io::{self, ControlFiles},
    policy::{ControlLimits, FiniteDemandPolicy},
};
use crate::application::routed_campaign::{input::Selection, walking::finite_scope};
use crate::{AppError, OwnerDomainWalkRequest};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

pub(super) const ENGINE: &str = "rustred.finite-demand.v1";
pub(super) fn hex(value: [u8; 32]) -> String {
    blake3::Hash::from(value).to_hex().to_string()
}
fn check(cancel: &AtomicBool) -> Result<(), AppError> {
    if cancel.load(Ordering::Acquire) {
        Err(AppError::new(
            crate::AppErrorKind::Cancelled,
            "finite lifecycle cancelled",
        ))
    } else {
        Ok(())
    }
}
pub(super) fn json_bytes(value: &impl Serialize) -> Result<Vec<u8>, AppError> {
    serde_json::to_vec_pretty(value).map_err(|e| AppError::serialization(e.to_string()))
}
fn read_json<T: for<'de> Deserialize<'de>>(path: &Path, limit: usize) -> Result<T, AppError> {
    serde_json::from_slice(&control_io::read(path, limit)?)
        .map_err(|e| AppError::input(format!("invalid finite control {}: {e}", path.display())))
}
fn optional_json<T: for<'de> Deserialize<'de>>(
    path: &Path,
    limit: usize,
) -> Result<Option<T>, AppError> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => read_json(path, limit).map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(AppError::execution(error.to_string())),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Execution {
    pub(super) physical_arity: u32,
    pub(super) program_identity: [u8; 32],
}
impl From<&RawExecutionBinding> for Execution {
    fn from(value: &RawExecutionBinding) -> Self {
        Self {
            physical_arity: value.physical_arity,
            program_identity: value.program_identity,
        }
    }
}
impl Execution {
    pub(super) fn native(&self) -> RawExecutionBinding {
        RawExecutionBinding {
            physical_arity: self.physical_arity,
            program_identity: self.program_identity,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Launch {
    schema: String,
    engine: String,
    pub(super) request: String,
    owner_base: PathBuf,
    pub(super) windows: WindowLimits,
    pub(super) initial_policy: FiniteDemandPolicy,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Object {
    path: PathBuf,
    bytes: u64,
    blake3: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Amendment {
    pub(super) sequence: u64,
    pub(super) parent: String,
    pub(super) digest: String,
    pub(super) queries: usize,
    pub(super) first_input: usize,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SourceInfo {
    pub(super) request: String,
    pub(super) total_queries: usize,
    pub(super) total_rows: usize,
    pub(super) required_rows: usize,
    pub(super) auxiliary_rows: usize,
    pub(super) amendments: Vec<Amendment>,
    files: Vec<Object>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScopeJoin {
    execution: Execution,
    scope: String,
    cursor_policies: String,
}
impl ScopeJoin {
    fn captured(generation: &ValidatedGeneration) -> Self {
        let mut hash = blake3::Hasher::new();
        hash.update(b"finite-control-cursor-policies-v1\0");
        hash.update(&(generation.state().cursors.len() as u64).to_le_bytes());
        for cursor in &generation.state().cursors {
            hash.update(&cursor.source_policy);
        }
        Self {
            execution: generation.execution().into(),
            scope: hex(generation.scope_digest()),
            cursor_policies: hash.finalize().to_hex().to_string(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActiveSource {
    join: ScopeJoin,
    source: SourceInfo,
    policy: FiniteDemandPolicy,
}

pub(super) struct Control {
    files: ControlFiles,
    source: SourceInfo,
    policy: FiniteDemandPolicy,
}
impl Control {
    pub(super) fn prepare(
        directory: &Path,
        mode: FiniteDemandMode,
        source: &OwnerDomainWalkRequest,
        policy: &FiniteDemandPolicy,
        cancel: &AtomicBool,
    ) -> Result<Self, AppError> {
        check(cancel)?;
        let (selection, arity, _) = Selection::parse_with_byte_limit(
            &source.matching.selection_json,
            policy.max_source_bytes,
        )?;
        let parsed = finite_scope::parse(
            source,
            arity,
            policy.max_total_rows,
            policy.max_source_bytes,
        )?;
        let create = mode == FiniteDemandMode::Create;
        let mut files = ControlFiles::open(directory, create, policy.control)?;
        let parent = files
            .root
            .parent()
            .ok_or_else(|| AppError::input("finite namespace has no campaign parent"))?;
        let owner_base = source
            .matching
            .owner_base
            .canonicalize()
            .map_err(|e| AppError::execution(e.to_string()))?;
        let local_base = owner_base.strip_prefix(parent).map_err(|_| {
            AppError::input("stage owner resources inside the new campaign before finite execution")
        })?;
        for path in selection
            .owners
            .iter()
            .map(|r| &r.path)
            .chain(selection.domain_rule_overlays.iter().map(|r| &r.path))
            .chain(selection.preferred_owner_programs.iter().map(|r| &r.path))
        {
            check(cancel)?;
            if path.is_absolute()
                || !owner_base
                    .join(path)
                    .canonicalize()
                    .map_err(|e| AppError::execution(e.to_string()))?
                    .starts_with(parent)
            {
                return Err(AppError::input(
                    "finite campaign owner resource references must be portable and campaign-local",
                ));
            }
        }
        let launch = Launch {
            schema: "rustred.finite-demand-launch.v1".into(),
            engine: ENGINE.into(),
            request: parsed.request_binding.clone(),
            owner_base: PathBuf::from("..").join(local_base),
            windows: policy.windows.into(),
            initial_policy: policy.clone(),
        };
        if create {
            files.write(Path::new("launch.json"), &json_bytes(&launch)?, false)?;
        } else {
            let old: Launch = read_json(
                &files.root.join("launch.json"),
                document_limit(policy.control),
            )?;
            old.validate()?;
            if old.request != launch.request
                || old.owner_base != launch.owner_base
                || old.windows != launch.windows
            {
                return Err(AppError::input(
                    "finite engine/original source/partition differs from launch",
                ));
            }
        }
        if mode == FiniteDemandMode::RepairPreparation {
            if !preparing_without_store(&files.root)? {
                return Err(AppError::input(
                    "preparation repair requires no native store/execution marker; an existing or missing-CURRENT store is never reset",
                ));
            }
        } else if mode == FiniteDemandMode::Resume {
            if preparing_without_store(&files.root)? {
                return Err(AppError::input(
                    "initial preparation has no native store; explicitly use --repair-preparation (no stored Work is reused)",
                ));
            }
            control_io::directory(&files.root.join("store"))?;
            let current=std::fs::symlink_metadata(files.root.join("store/CURRENT"))
                .map_err(|e|AppError::input(format!("initialized finite store lacks readable CURRENT; refusing ambiguous recovery: {e}")))?;
            if current.file_type().is_symlink() || !current.is_file() {
                return Err(AppError::input("finite CURRENT is not a regular file"));
            }
        }
        let mut objects = Vec::new();
        let documents = std::iter::once(("selection", source.matching.selection_json.as_str()))
            .chain(std::iter::once((
                "queries",
                source.matching.queries_json.as_str(),
            )))
            .chain(
                source
                    .amendments
                    .iter()
                    .map(|a| ("amendment", a.text.as_str())),
            );
        for (kind, text) in documents {
            check(cancel)?;
            let digest = blake3::hash(text.as_bytes()).to_hex().to_string();
            let path = PathBuf::from(format!("sources/{kind}-{digest}.json"));
            files.write(&path, text.as_bytes(), false)?;
            objects.push(Object {
                path,
                bytes: text.len() as u64,
                blake3: digest,
            });
        }
        let info = source_info(&parsed, objects)?;
        files.write(
            Path::new("requested-source.json"),
            &json_bytes(&info)?,
            true,
        )?;
        Ok(Self {
            files,
            source: info,
            policy: policy.clone(),
        })
    }
    pub(super) fn store(&self) -> PathBuf {
        self.files.root.join("store")
    }
    pub(super) fn directory(&self) -> &Path {
        &self.files.root
    }
    pub(super) fn require_committed_source(&self) -> Result<(), AppError> {
        if self.source.auxiliary_rows != 0 {
            return Err(AppError::input(
                "finite publication requires required-only input scope; auxiliary rows are not discarded",
            ));
        }
        let execution: Execution = read_json(
            &self.files.root.join("execution.json"),
            document_limit(self.policy.control),
        )?;
        let generation = ValidatedGeneration::open_generated(
            &self.store(),
            &execution.native(),
            self.policy.storage.into(),
        )
        .map_err(super::super::backend::storage_error)?;
        if generation.state().scope.rows.len() != self.source.total_rows
            || generation.state().scope.original_source
                != super::super::backend::original_source_identity(&self.source.request)?
        {
            return Err(AppError::input(
                "publication cannot append requested scope; run/resume it first",
            ));
        }
        Ok(())
    }
    /// Called only with the generation returned by Session built from this
    /// exact request. A postcommit control IO error stops the public loop;
    /// native CURRENT is preserved, never rolled back or promoted from here.
    pub(super) fn adopt_source(
        &mut self,
        generation: &ValidatedGeneration,
    ) -> Result<(), AppError> {
        if generation.state().scope.rows.len() != self.source.total_rows
            || generation.state().scope.original_source
                != super::super::backend::original_source_identity(&self.source.request)?
        {
            return Err(AppError::internal_invariant(
                "committed finite scope differs from supplied source",
            ));
        }
        let execution = Execution::from(generation.execution());
        let execution_bytes = json_bytes(&execution)?;
        let active = ActiveSource {
            join: ScopeJoin::captured(generation),
            source: self.source.clone(),
            policy: self.policy.clone(),
        };
        let active_bytes = json_bytes(&active)?;
        self.files.write(Path::new("execution.json"), &execution_bytes, false)
            .and_then(|()| self.files.write(Path::new("active-source.json"), &active_bytes, true))
            .map_err(|e| AppError::new(e.kind(), format!("native generation {} is durable; active source metadata needs resupplied-source resume repair: {e}", hex(generation.digest()))))
    }
}
impl Launch {
    fn validate(&self) -> Result<(), AppError> {
        if self.schema != "rustred.finite-demand-launch.v1" || self.engine != ENGINE {
            Err(AppError::input(
                "not a supported finite-demand namespace; no legacy checkpoint import",
            ))
        } else {
            Ok(())
        }
    }
}

pub(super) struct Inspection {
    pub(super) generation: Option<ValidatedGeneration>,
    pub(super) source: Option<SourceInfo>,
    pub(super) source_status: &'static str,
    pub(super) requested: Option<SourceInfo>,
}
pub(super) fn inspect(
    directory: &Path,
    policy: Option<&FiniteDemandPolicy>,
) -> Result<Inspection, AppError> {
    control_io::directory(directory)?;
    control_io::directory(&directory.join("sources"))?;
    // Read grants come from the caller, never from serialized control policy.
    // Defaults retain independent byte bounds; all count limits are uncapped.
    let control = policy.map_or(
        ControlLimits {
            max_bytes: 256 << 20,
            max_files: usize::MAX,
        },
        |p| p.control,
    );
    control_io::validate_inventory(directory, control)?;
    let document_limit = document_limit(control);
    let launch: Launch = read_json(&directory.join("launch.json"), document_limit)?;
    launch.validate()?;
    if preparing_without_store(directory)? {
        return Ok(Inspection {
            generation: None,
            source: None,
            source_status: "unadmitted",
            requested: optional_json(&directory.join("requested-source.json"), document_limit)?,
        });
    }
    let execution: Execution = read_json(&directory.join("execution.json"), document_limit)?;
    let active: Option<ActiveSource> =
        optional_json(&directory.join("active-source.json"), document_limit)?;
    let generation = ValidatedGeneration::open_generated(
        &directory.join("store"),
        &execution.native(),
        inspection_limits(policy, execution.physical_arity)?,
    )
    .map_err(super::super::backend::storage_error)?;
    let source_status = match &active {
        Some(active) if active.join == ScopeJoin::captured(&generation) => "bound",
        Some(_) => "stale",
        None => "missing",
    };
    let source = if source_status == "bound" {
        let source = active.unwrap().source;
        Some(verify_source_files(
            directory,
            &source,
            control,
            policy.map_or(document_limit, |p| p.max_source_bytes),
            &generation,
        )?)
    } else {
        None
    };
    let requested = optional_json(&directory.join("requested-source.json"), document_limit)?;
    Ok(Inspection {
        generation: Some(generation),
        source,
        source_status,
        requested,
    })
}
fn document_limit(grant: ControlLimits) -> usize {
    usize::try_from(grant.max_bytes).unwrap_or(usize::MAX)
}
fn entry_exists(path: &Path) -> Result<bool, AppError> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(AppError::execution(e.to_string())),
    }
}
fn preparing_without_store(directory: &Path) -> Result<bool, AppError> {
    Ok(!entry_exists(&directory.join("store"))?
        && !entry_exists(&directory.join("execution.json"))?
        && !entry_exists(&directory.join("active-source.json"))?)
}
fn verify_source_files(
    directory: &Path,
    source: &SourceInfo,
    grant: ControlLimits,
    max_source_bytes: usize,
    generation: &ValidatedGeneration,
) -> Result<SourceInfo, AppError> {
    let maximum = grant.max_bytes;
    let total = source
        .files
        .iter()
        .try_fold(0u64, |n, f| n.checked_add(f.bytes));
    let query_bytes = source
        .files
        .iter()
        .skip(1)
        .try_fold(0u64, |n, f| n.checked_add(f.bytes));
    if source.files.len() > grant.max_files
        || total.is_none_or(|n| n > maximum)
        || source
            .files
            .first()
            .is_some_and(|f| f.bytes > max_source_bytes as u64)
        || query_bytes.is_none_or(|n| n > max_source_bytes as u64)
        || source.files.len() < 2
        || source.amendments.len().checked_add(2) != Some(source.files.len())
    {
        return Err(AppError::limit(
            "committed source inspection byte/file allowance",
        ));
    }
    let mut documents = Vec::new();
    for (index, object) in source.files.iter().enumerate() {
        let kind = match index {
            0 => "selection",
            1 => "queries",
            _ => "amendment",
        };
        if object.blake3.len() != 64
            || !object
                .blake3
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            || object.path != PathBuf::from(format!("sources/{kind}-{}.json", object.blake3))
        {
            return Err(AppError::input("invalid committed source object path"));
        }
        let bytes = control_io::read(
            &directory.join(&object.path),
            usize::try_from(object.bytes)
                .map_err(|_| AppError::limit("source object exceeds platform byte range"))?,
        )?;
        if bytes.len() as u64 != object.bytes
            || blake3::hash(&bytes).to_hex().as_str() != object.blake3
        {
            return Err(AppError::input("committed finite source object changed"));
        }
        documents.push(
            String::from_utf8(bytes)
                .map_err(|_| AppError::input("finite source object is not UTF-8"))?,
        );
    }
    let (_, arity, _) = Selection::parse_with_byte_limit(&documents[0], max_source_bytes)?;
    if arity != generation.execution().physical_arity as usize
        || generation.state().scope.original_source
            != super::super::backend::original_source_identity(&source.request)?
    {
        return Err(AppError::input(
            "finite source status request/arity differs from CURRENT",
        ));
    }
    let amendments = documents
        .iter()
        .skip(2)
        .zip(source.files.iter().skip(2))
        .map(|(text, file)| crate::OwnerDomainWalkAmendment {
            path: file.path.clone(),
            text: text.clone(),
        })
        .collect::<Vec<_>>();
    let parsed = finite_scope::parse_bound_sources(
        &documents[1],
        &amendments,
        source.request.clone(),
        arity,
        generation.state().scope.rows.len().max(1),
        max_source_bytes,
        generation.state().scope.rows.len(),
        max_source_bytes,
    )?;
    if parsed.rows.len() != generation.state().scope.rows.len() {
        return Err(AppError::input(
            "finite source status row count differs from CURRENT",
        ));
    }
    for (ordinal, (row, saved)) in parsed
        .rows
        .iter()
        .zip(&generation.state().scope.rows)
        .enumerate()
    {
        let origin = super::super::backend::origin_identity(
            row.source,
            parsed.sources[row.source]["blake3"]
                .as_str()
                .ok_or_else(|| AppError::internal_invariant("source digest absent"))?,
            ordinal,
            &row.query.id,
            row.query.auxiliary,
            row.query.role_declared,
        )?;
        let role = if row.query.auxiliary {
            storage::ScopeRole::Auxiliary
        } else {
            storage::ScopeRole::Required
        };
        if origin != saved.origin || role != saved.role {
            return Err(AppError::input(
                "finite source status row origin/role differs from CURRENT",
            ));
        }
    }
    let verified = source_info(&parsed, source.files.clone())?;
    if &verified != source {
        return Err(AppError::input(
            "finite source status chain/count metadata differs from native parsed inputs",
        ));
    }
    Ok(verified)
}

fn source_info(parsed: &finite_scope::Scope, objects: Vec<Object>) -> Result<SourceInfo, AppError> {
    let original_rows = parsed.sources[0]["rows"]
        .as_u64()
        .ok_or_else(|| AppError::internal_invariant("original source rows absent"))?
        as usize;
    let mut first = original_rows;
    let mut amendments = Vec::new();
    for item in parsed.sources.iter().skip(1) {
        let queries = item["rows"]
            .as_u64()
            .ok_or_else(|| AppError::internal_invariant("amendment rows absent"))?
            as usize;
        amendments.push(Amendment {
            sequence: item["sequence"]
                .as_u64()
                .ok_or_else(|| AppError::internal_invariant("amendment sequence absent"))?,
            parent: item["parent"]
                .as_str()
                .ok_or_else(|| AppError::internal_invariant("amendment parent absent"))?
                .into(),
            digest: item["blake3"]
                .as_str()
                .ok_or_else(|| AppError::internal_invariant("amendment digest absent"))?
                .into(),
            queries,
            first_input: first,
        });
        first = first
            .checked_add(queries)
            .ok_or_else(|| AppError::limit("source row offset overflow"))?;
    }
    let required_rows = parsed.rows.iter().filter(|r| !r.query.auxiliary).count();
    Ok(SourceInfo {
        request: parsed.request_binding.clone(),
        total_queries: original_rows,
        total_rows: parsed.rows.len(),
        required_rows,
        auxiliary_rows: parsed.rows.len() - required_rows,
        amendments,
        files: objects,
    })
}
fn inspection_limits(
    policy: Option<&FiniteDemandPolicy>,
    physical_arity: u32,
) -> Result<storage::StorageLimits, AppError> {
    if let Some(policy) = policy {
        let mut limits: storage::StorageLimits = policy.storage.into();
        limits.max_scope_rows = limits.max_scope_rows.min(policy.max_total_rows);
        return Ok(limits);
    }
    // Independent read defaults keep stale saved operational grants from
    // hiding a newer committed scope. They confer no execution authority.
    const FRAME: usize = rustred::persistence::finite_work::FRAME_LIMIT;
    let arity = usize::try_from(physical_arity)
        .map_err(|_| AppError::limit("inspection physical arity exceeds platform range"))?;
    if arity == 0 {
        return Err(AppError::limit("inspection physical arity allowance"));
    }
    // Metadata-only inspection does not read/reserve raw pages. Nevertheless,
    // its policy must pass the same decoded-capacity preflight as the writer.
    // This native-derived ceiling accepts every legal page at this arity and
    // cannot overflow as an unlimited count would.
    let max_raw_page_records = storage::RawCensusLimits::max_records_for_native_frame(arity)
        .map_err(super::super::backend::storage_error)?;
    Ok(storage::StorageLimits {
        raw: storage::RawCensusLimits {
            max_page_records: max_raw_page_records,
            max_page_bytes: FRAME,
            max_runs: usize::MAX,
            max_pages: usize::MAX,
            max_directory_bytes: 256 << 20,
            max_stored_references: u64::MAX,
            max_merge_runs: usize::MAX,
            max_merge_cached_pages: 0,
            max_merge_payload_bytes: 0,
            max_merge_read_bytes: 0,
            max_decode_transient_bytes: 2 * FRAME,
        },
        max_arity: usize::MAX,
        max_delta_records: usize::MAX,
        max_delta_payload_bytes: 0,
        max_page_records: usize::MAX, // Work payload pages are not decoded.
        max_page_bytes: FRAME,
        max_cached_pages: 0,
        max_cached_page_bytes: 0,
        max_directory_bytes: 256 << 20,
        max_manifest_bytes: FRAME,
        max_state_bytes: FRAME,
        max_scope_rows: usize::MAX,
        max_census_records: usize::MAX,
        max_runs: usize::MAX,
        max_pages: usize::MAX,
        max_physical_files: usize::MAX,
        max_disk_bytes: u64::MAX, // Referenced size only; no Work/raw payload IO.
        max_lookup_pages: 0,
        max_lookup_bytes: 0,
        max_transient_bytes: 2 * FRAME,
    })
}
