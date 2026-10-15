use super::*;
use std::{
    fs::{self, File},
    io::{Read, Write},
    mem::size_of,
    path::Path,
};

pub(super) const MAX_FRAME: usize = 64 * 1024 * 1024;
pub(super) const CURRENT: &str = "CURRENT";
pub(super) const PAGE_MAGIC: &[u8] = b"RUSTRED-FINITE-WORK-1\0";
pub(super) const STATE_MAGIC: &[u8] = b"RUSTRED-FINITE-STATE-1\0";
pub(super) const MANIFEST_MAGIC: &[u8] = b"RUSTRED-FINITE-GENERATION-1\0";
pub(super) const TIP_MAGIC: &[u8] = b"RUSTRED-FINITE-CURRENT-1\0";
pub(super) const TIP_LIMIT: usize = 256;
const MAX_CURSOR: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub(super) struct ObjectRef {
    pub digest: Digest,
    pub bytes: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub(super) struct PageRef {
    pub object: ObjectRef,
    pub records: u64,
    pub first: StoredWork,
    pub last: StoredWork,
}
#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub(super) struct Manifest {
    pub execution: RawExecutionBinding,
    pub generation: u64,
    pub previous: Option<Digest>,
    pub state: ObjectRef,
    pub scope_digest: Digest,
    pub runs: Vec<Vec<PageRef>>,
}
#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub(super) struct Tip {
    pub generation: u64,
    pub manifest: ObjectRef,
}
#[derive(bincode::Encode, bincode::Decode)]
pub(super) struct Page {
    pub execution: RawExecutionBinding,
    pub records: Vec<StoredWork>,
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
    let mut bytes = capacity
        .checked_mul(size_of::<StoredWork>())
        .ok_or_else(|| resource("page capacity overflow"))?;
    for record in records {
        add(&mut bytes, record.heap_bytes()?)?;
    }
    Ok(bytes)
}
pub(super) fn validate_manifest(
    manifest: &Manifest,
    expected: &RawExecutionBinding,
    limits: StorageLimits,
) -> Result<(u64, usize)> {
    let arity = binding(expected, limits)?;
    if &manifest.execution != expected
        || manifest.generation == 0
        || (manifest.generation == 1) != manifest.previous.is_none()
    {
        return Err(corrupt("finite generation execution/ancestry binding"));
    }
    if manifest.runs.len() > limits.max_runs {
        return Err(resource("finite run directory"));
    }
    if manifest.state.bytes == 0 || manifest.state.bytes > limits.max_state_bytes as u64 {
        return Err(resource("finite state bytes"));
    }
    let mut pages = 0usize;
    let mut records = 0u64;
    let mut disk = manifest.state.bytes;
    let mut digests = std::collections::BTreeSet::new();
    let mut bytes = size_of::<Manifest>();
    add(
        &mut bytes,
        manifest
            .runs
            .capacity()
            .checked_mul(size_of::<Vec<PageRef>>())
            .ok_or_else(|| resource("run directory overflow"))?,
    )?;
    for run in &manifest.runs {
        if run.is_empty() {
            return Err(corrupt("empty finite run"));
        }
        add(
            &mut bytes,
            run.capacity()
                .checked_mul(size_of::<PageRef>())
                .ok_or_else(|| resource("page directory overflow"))?,
        )?;
        let mut previous = None;
        for page in run {
            page.first.validate(arity)?;
            page.last.validate(arity)?;
            if page.records > limits.max_page_records as u64
                || page.object.bytes > limits.max_page_bytes as u64
            {
                return Err(resource("finite page record/byte allowance"));
            }
            if page.records == 0
                || page.object.bytes == 0
                || page.first > page.last
                || previous.is_some_and(|old| old >= &page.first)
            {
                return Err(corrupt("invalid finite page range/count/frame"));
            }
            previous = Some(&page.last);
            pages = pages
                .checked_add(1)
                .ok_or_else(|| resource("finite page count overflow"))?;
            if pages > limits.max_pages {
                return Err(resource("finite page count allowance"));
            }
            if !digests.insert(page.object.digest) {
                return Err(corrupt("duplicate referenced finite page"));
            }
            records = records
                .checked_add(page.records)
                .ok_or_else(|| resource("finite Work count overflow"))?;
            disk = disk
                .checked_add(page.object.bytes)
                .ok_or_else(|| resource("referenced finite disk overflow"))?;
            add(&mut bytes, page.first.heap_bytes()?)?;
            add(&mut bytes, page.last.heap_bytes()?)?;
            // Includes the bounded validation-only digest set payload; tree
            // allocator overhead is not advertised as exact process RSS.
            add(&mut bytes, size_of::<Digest>())?;
            if bytes > limits.max_directory_bytes || disk > limits.max_disk_bytes {
                return Err(resource("finite directory/referenced disk allowance"));
            }
        }
    }
    if pages > limits.max_pages || bytes > limits.max_directory_bytes {
        return Err(resource("finite page/index allowance"));
    }
    Ok((records, bytes))
}
pub(super) fn validate_page(
    page: &Page,
    reference: &PageRef,
    execution: &RawExecutionBinding,
    limits: StorageLimits,
) -> Result<usize> {
    let arity = binding(execution, limits)?;
    if page.execution != *execution
        || page.records.len() as u64 != reference.records
        || page.records.first() != Some(&reference.first)
        || page.records.last() != Some(&reference.last)
        || page.records.is_empty()
        || page.records.len() > limits.max_page_records
        || page.records.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(corrupt(
            "finite page execution/range/sort/duplicate mismatch",
        ));
    }
    for work in &page.records {
        work.validate(arity)?;
    }
    work_payload(&page.records, page.records.capacity())
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
    let census_count = state
        .census
        .terminals
        .len()
        .checked_add(state.census.zeros.len())
        .and_then(|n| n.checked_add(state.census.selections.len()))
        .ok_or_else(|| resource("census record overflow"))?;
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
    for keys in [&state.census.terminals, &state.census.zeros] {
        if keys.iter().any(|key| key.len() != arity)
            || keys.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(corrupt("raw census key arity/sort/duplicate"));
        }
    }
    if state
        .census
        .terminals
        .len()
        .checked_add(state.census.zeros.len())
        .ok_or_else(|| resource("raw census size overflow"))? as u64
        > records
    {
        return Err(corrupt("raw terminal/zero count exceeds completed Work"));
    }
    if state
        .census
        .terminals
        .iter()
        .any(|key| state.census.zeros.binary_search(key).is_ok())
    {
        return Err(corrupt("raw terminal/zero authority overlap"));
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
fn state_payload(state: &GenerationState) -> Result<usize> {
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
    for keys in [&state.census.terminals, &state.census.zeros] {
        add(
            &mut bytes,
            keys.capacity()
                .checked_mul(size_of::<Vec<i64>>())
                .ok_or_else(|| resource("census capacity overflow"))?,
        )?;
        for key in keys {
            add(
                &mut bytes,
                key.capacity()
                    .checked_mul(size_of::<i64>())
                    .ok_or_else(|| resource("census key capacity overflow"))?,
            )?;
        }
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
    for (old, new) in [
        (&old.census.terminals, &new.census.terminals),
        (&old.census.zeros, &new.census.zeros),
    ] {
        if old.iter().any(|key| new.binary_search(key).is_err()) {
            return Err(corrupt("raw cumulative census lost a key"));
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

/// The same bounded native bincode framing pattern as existing cache indexes.
pub(super) fn encode(
    value_magic: &[u8],
    value: &impl bincode::Encode,
    limit: usize,
) -> Result<Vec<u8>> {
    if limit > MAX_FRAME || limit < value_magic.len() + 32 {
        return Err(resource("finite frame allowance"));
    }
    let mut writer = BoundedBytes {
        bytes: Vec::new(),
        limit: limit - 32,
    };
    writer.write_all(value_magic).map_err(io)?;
    bincode::encode_into_std_write(value, &mut writer, bincode::config::standard())
        .map_err(resource)?;
    let digest = blake3::hash(&writer.bytes);
    writer.bytes.extend_from_slice(digest.as_bytes());
    Ok(writer.bytes)
}
pub(super) fn decode<T: bincode::Decode<()>>(
    magic: &[u8],
    bytes: &[u8],
    limit: usize,
) -> Result<T> {
    if limit > MAX_FRAME
        || bytes.len() > limit
        || bytes.len() < magic.len() + 32
        || !bytes.starts_with(magic)
    {
        return Err(corrupt("finite frame magic/length"));
    }
    let end = bytes.len() - 32;
    if blake3::hash(&bytes[..end]).as_bytes() != &bytes[end..] {
        return Err(corrupt("finite frame digest"));
    }
    let (value, consumed) = bincode::decode_from_slice(
        &bytes[magic.len()..end],
        bincode::config::standard().with_limit::<MAX_FRAME>(),
    )
    .map_err(corrupt)?;
    if consumed != end - magic.len() {
        return Err(corrupt("trailing finite frame bytes"));
    }
    Ok(value)
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
struct BoundedBytes {
    bytes: Vec<u8>,
    limit: usize,
}
impl Write for BoundedBytes {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let length = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or_else(|| std::io::Error::other("finite frame overflow"))?;
        if length > self.limit {
            return Err(std::io::Error::other("finite frame allowance exceeded"));
        }
        if length > self.bytes.capacity() {
            let capacity = length
                .max(self.bytes.capacity().saturating_mul(2))
                .min(self.limit);
            self.bytes
                .try_reserve_exact(capacity - self.bytes.len())
                .map_err(std::io::Error::other)?;
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
