use super::*;
use std::{
    fs::{self, File},
    io::Read,
    mem::size_of,
    path::Path,
};

pub(super) const MAX_FRAME: usize = rustred::persistence::finite_work::FRAME_LIMIT;
pub(super) use rustred::persistence::finite_work::metadata_decode_allowance;
pub(super) const CURRENT: &str = "CURRENT";
pub(super) const PAGE_MAGIC: &[u8] = rustred::persistence::finite_work::PAGE_MAGIC;
pub(super) const STATE_MAGIC: &[u8] = b"RUSTRED-FINITE-STATE-2\0";
pub(super) const MANIFEST_MAGIC: &[u8] = b"RUSTRED-FINITE-GENERATION-2\0";
pub(super) const TIP_MAGIC: &[u8] = b"RUSTRED-FINITE-CURRENT-2\0";
pub(super) const TIP_LIMIT: usize = 256;
const MAX_CURSOR: usize = 64 * 1024;

pub(super) use rustred::persistence::finite_work::{
    GeneratedObjectRef as ObjectRef, GeneratedWorkPageRecord as Page,
    GeneratedWorkPageRef as PageRef,
};
#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub(super) struct Manifest {
    pub execution: RawExecutionBinding,
    pub generation: u64,
    pub previous: Option<Digest>,
    pub state: ObjectRef,
    pub scope_digest: Digest,
    pub runs: Arc<Vec<Vec<PageRef>>>,
    pub raw: Arc<RawCensusDirectory>,
}
#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub(super) struct Tip {
    pub generation: u64,
    pub manifest: ObjectRef,
}

#[derive(bincode::Encode, bincode::Decode)]
pub(super) struct StateFile {
    pub execution: RawExecutionBinding,
    pub generation: u64,
    pub state: GenerationState,
}

pub(super) fn name(digest: &Digest, suffix: &str) -> String {
    format!("{}.{}", blake3::Hash::from_bytes(*digest).to_hex(), suffix)
}
pub(super) fn binding(binding: &RawExecutionBinding, limits: StorageLimits) -> Result<usize> {
    limits.validate()?;
    let arity = usize::try_from(binding.physical_arity).map_err(corrupt)?;
    if arity == 0 || arity > limits.max_arity {
        return Err(resource("finite-store physical arity"));
    }
    Ok(arity)
}
fn add(total: &mut usize, amount: usize) -> Result<()> {
    *total = total
        .checked_add(amount)
        .ok_or_else(|| resource("finite-store payload overflow"))?;
    Ok(())
}
pub(super) fn work_payload(records: &[StoredWork], capacity: usize) -> Result<usize> {
    rustred::persistence::finite_work::work_payload_bytes(records, capacity).map_err(Into::into)
}
pub(super) fn validate_manifest(
    manifest: &Manifest,
    expected: &RawExecutionBinding,
    limits: StorageLimits,
) -> Result<(u64, usize)> {
    binding(expected, limits)?;
    if &manifest.execution != expected
        || manifest.generation == 0
        || (manifest.generation == 1) != manifest.previous.is_none()
    {
        return Err(corrupt("finite generation execution/ancestry binding"));
    }
    if manifest.state.bytes == 0 || manifest.state.bytes > limits.max_state_bytes as u64 {
        return Err(resource("finite state bytes"));
    }
    let stats = rustred::persistence::finite_work::validate_directory(
        expected,
        &manifest.runs,
        limits.directory_limits(),
    )?;
    let (_, raw_bytes) =
        raw::validate_directory(&manifest.raw, expected.physical_arity as usize, limits.raw)?;
    if manifest
        .raw
        .stored_references(RawKeyKind::Terminal)?
        .checked_add(manifest.raw.stored_references(RawKeyKind::Zero)?)
        .is_none_or(|references| references > stats.records)
    {
        return Err(corrupt("raw key references exceed completed Work"));
    }
    if stats
        .referenced_bytes
        .checked_add(manifest.state.bytes)
        .and_then(|n| n.checked_add(raw_bytes))
        .is_none_or(|bytes| bytes > limits.max_disk_bytes)
    {
        return Err(resource("finite referenced state/page disk allowance"));
    }
    // Actual Work directory capacity, shared by the manifest and its readers.
    // The permission cap in StorageLimits is not a resident allocation.
    Ok((stats.records, stats.payload_bytes))
}
pub(super) fn validate_page(
    page: &Page,
    reference: &PageRef,
    execution: &RawExecutionBinding,
    limits: StorageLimits,
) -> Result<usize> {
    binding(execution, limits)?;
    rustred::persistence::finite_work::validate_page_record(
        page,
        reference,
        execution,
        limits.max_page_records,
    )
    .map_err(Into::into)
}
pub(super) fn scope_digest(scope: &ScopeBinding, limit: usize) -> Result<Digest> {
    Ok(*blake3::hash(&encode(b"RUSTRED-FINITE-SCOPE-1\0", scope, limit)?).as_bytes())
}
pub(super) fn validate_state(
    state: &GenerationState,
    execution: &RawExecutionBinding,
    records: u64,
    limits: StorageLimits,
) -> Result<()> {
    let arity = binding(execution, limits)?;
    if state.completed_work != records || state.scope.rows.len() != state.cursors.len() {
        return Err(corrupt("finite state membership/cursor count mismatch"));
    }
    let census_count = state.census.selections.len();
    if state.scope.rows.len() > limits.max_scope_rows
        || census_count > limits.max_census_records
        || state_payload(state)? > limits.max_state_bytes
    {
        return Err(resource("finite state resident payload/record allowance"));
    }
    let mut origins = std::collections::BTreeSet::new();
    let mut occurrences = 0u64;
    for (row, cursor) in state.scope.rows.iter().zip(&state.cursors) {
        if row.origin.is_empty()
            || row.origin.len() > 4096
            || !origins.insert(&row.origin)
            || cursor.encoded_cursor.is_empty()
            || cursor.encoded_cursor.len() > MAX_CURSOR
        {
            return Err(corrupt("finite scope origin/cursor framing"));
        }
        occurrences = occurrences
            .checked_add(cursor.input_occurrences)
            .ok_or_else(|| resource("input occurrence overflow"))?;
    }
    if occurrences != state.input_occurrences {
        return Err(corrupt("finite input occurrence total mismatch"));
    }
    let mut selected = 0u64;
    let mut previous = None;
    for selection in &state.census.selections {
        if selection.owner.len() != arity
            || selection.multiplicity == 0
            || previous.is_some_and(|old| old >= selection.identity())
        {
            return Err(corrupt("raw selected-rule census identity/order"));
        }
        previous = Some(selection.identity());
        selected = selected
            .checked_add(selection.multiplicity)
            .ok_or_else(|| resource("selection multiplicity overflow"))?;
    }
    if selected > records {
        return Err(corrupt("rule applications exceed completed Work"));
    }
    Ok(())
}
pub(super) fn state_payload(state: &GenerationState) -> Result<usize> {
    let mut bytes = size_of::<GenerationState>();
    add(
        &mut bytes,
        state
            .scope
            .rows
            .capacity()
            .checked_mul(size_of::<ScopeRow>())
            .ok_or_else(|| resource("scope capacity overflow"))?,
    )?;
    for row in &state.scope.rows {
        add(&mut bytes, row.origin.capacity())?;
    }
    add(
        &mut bytes,
        state
            .cursors
            .capacity()
            .checked_mul(size_of::<CursorRecord>())
            .ok_or_else(|| resource("cursor capacity overflow"))?,
    )?;
    for cursor in &state.cursors {
        add(&mut bytes, cursor.encoded_cursor.capacity())?;
    }
    add(
        &mut bytes,
        state
            .census
            .selections
            .capacity()
            .checked_mul(size_of::<SelectionRecord>())
            .ok_or_else(|| resource("selection capacity overflow"))?,
    )?;
    for selection in &state.census.selections {
        add(&mut bytes, selection.owner.capacity())?;
    }
    Ok(bytes)
}
pub(super) fn validate_extension(old: &GenerationState, new: &GenerationState) -> Result<()> {
    if old.scope.original_source != new.scope.original_source
        || !new.scope.rows.starts_with(&old.scope.rows)
        || new.cursors.len() < old.cursors.len()
        || new.completed_work < old.completed_work
        || new.input_occurrences < old.input_occurrences
    {
        return Err(StorageError::new(
            ErrorKind::Stale,
            "finite scope/history is not append-only",
        ));
    }
    for (old, new) in old.cursors.iter().zip(&new.cursors) {
        if old.source_policy != new.source_policy
            || new.completed_windows < old.completed_windows
            || new.input_occurrences < old.input_occurrences
            || (new.completed_windows == old.completed_windows && new != old)
        {
            return Err(StorageError::new(
                ErrorKind::Stale,
                "finite cursor moved backwards or changed policy",
            ));
        }
    }
    for selection in &old.census.selections {
        let found = new
            .census
            .selections
            .binary_search_by(|candidate| candidate.identity().cmp(&selection.identity()))
            .ok()
            .map(|index| &new.census.selections[index]);
        if found.is_none_or(|candidate| candidate.multiplicity < selection.multiplicity) {
            return Err(corrupt(
                "raw cumulative census lost a selected-rule occurrence",
            ));
        }
    }
    Ok(())
}

// The page and state framing share the core's bounded integer codec; only
// filesystem/generation metadata remain here.
pub(super) fn encode(magic: &[u8], value: &impl bincode::Encode, limit: usize) -> Result<Vec<u8>> {
    rustred::persistence::finite_work::encode_metadata_frame(magic, value, limit)
        .map_err(Into::into)
}
pub(super) fn decode<T: bincode::Decode<()>>(
    magic: &[u8],
    bytes: &[u8],
    limit: usize,
) -> Result<T> {
    rustred::persistence::finite_work::decode_metadata_frame(magic, bytes, limit)
        .map_err(Into::into)
}
pub(super) fn read(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path).map_err(io)?;
    if !metadata.is_file() || metadata.len() > limit as u64 {
        return Err(corrupt("finite file is not bounded/regular"));
    }
    let mut file = File::open(path).map_err(io)?;
    let len = usize::try_from(metadata.len()).map_err(resource)?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(len).map_err(resource)?;
    bytes.resize(len, 0);
    file.read_exact(&mut bytes).map_err(io)?;
    if file.read(&mut [0]).map_err(io)? != 0 {
        return Err(corrupt("finite file changed while reading"));
    }
    Ok(bytes)
}
pub(super) fn read_object(
    directory: &Path,
    subdir: &str,
    suffix: &str,
    reference: &ObjectRef,
    limit: usize,
) -> Result<Vec<u8>> {
    if reference.bytes > limit as u64 || reference.bytes == 0 {
        return Err(resource("finite object allowance"));
    }
    let bytes = read(
        &directory.join(subdir).join(name(&reference.digest, suffix)),
        limit,
    )?;
    if bytes.len() as u64 != reference.bytes || blake3::hash(&bytes).as_bytes() != &reference.digest
    {
        return Err(corrupt("finite object reference digest/length mismatch"));
    }
    Ok(bytes)
}
