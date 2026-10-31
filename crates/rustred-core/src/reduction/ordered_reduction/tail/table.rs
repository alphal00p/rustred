//! The persistent tail table: for every tabulated identity of a tail, its
//! complete reduction onto the tail's single terminal (value), the guards
//! of that whole reduction (closure set) and its own checked step in compact
//! form (phase, children by table id with step coefficients, terminal
//! coefficient, own guards in step order). The steps let a later request
//! repeat the forward pass's zero decisions over the tail without expanding
//! it (see `tail`).
//!
//! Every part of an entry is a function of its identity and the program
//! alone. The table lives in the step cache's directory (one file,
//! [`TAIL_FILE`]) and is bound by versioning only: the file schema
//! ([`ORDERED_TAIL_SCHEMA`]), the hand-bumped semantics version
//! [`ORDERED_TAIL_SEMANTICS`] (to bump whenever a checked step or the meaning
//! of an entry changes) and cheap identity fields of the program (adapter,
//! family fingerprint, coefficient variables, unresolved policy: the step
//! cache's binding); no source or program digest. Like the step cache, a
//! table belongs to one program and data version: keep one directory per
//! program and data version (terminal conventions cannot change while a
//! step cache is attached). A file whose header
//! differs, or that is corrupt (payload digest), is rejected loudly (one
//! line on stderr and in the table statistics) and the run starts from an
//! empty table (cold); the next save replaces the file.
//!
//! File layout (deterministic for a given set of entries): magic, the
//! identity header (schema, semantics and identity fields, one text block),
//! payload length and BLAKE3 digest, then the payload: the terminal
//! identities, guard coefficients, values, step coefficients and guard sets,
//! each table sorted by its encoding, and the entries sorted by identity
//! (children refer to entry positions).
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{PoisonError, RwLock, RwLockReadGuard};

use super::Interner;
use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};
use crate::family::IntegralKey;
use crate::reduction::OrderedKeyStage;
use crate::reduction::ReductionError;
use crate::reduction::ordered_reduction::step_cache::codec::{
    FxBuild, NodeStage, Reader, Writer, check_tail_identity, decode_node, encode_node,
};
use crate::reduction::ordered_reduction::step_cache::failure;

/// File schema of tail tables.
pub const ORDERED_TAIL_SCHEMA: &str = "rustred.ordered-tail.v3";
/// Version of the tail-entry semantics (complete reduction onto one
/// terminal, DFS union of the guards of every step of the reduction, the
/// checked step itself). Bump it by hand whenever an adapter's checked step
/// or the meaning of a stored entry changes; tables of another version are
/// rejected.
pub const ORDERED_TAIL_SEMANTICS: &str = "rustred-ordered-tail-2026-10-31";

/// File name of the tail table inside a step-cache directory.
pub(crate) const TAIL_FILE: &str = "ordered-tail.rrtail";

const MAGIC: &[u8; 16] = b"RR-ORDERED-TAIL2";
const NONE: u32 = u32::MAX;

/// One tabulated identity. `terminal` is `None` exactly for identities whose
/// complete reduction is zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TailEntry {
    pub terminal: Option<u32>,
    pub value: u32,
    /// Closure guard set (all guards of the reduction).
    pub guards: u32,
    /// Scheduler phase of the identity.
    pub phase: u32,
    /// Range of `TailData::edges` (children in step order).
    pub edges: (u32, u32),
    /// Step coefficient of the terminal emission (`NONE`: none).
    pub terminal_coefficient: u32,
    /// Range of `TailData::own_guards` (step order).
    pub own_guards: (u32, u32),
}

impl TailEntry {
    pub(crate) fn has_terminal_emission(&self) -> bool {
        self.terminal_coefficient != NONE
    }
}

/// The interned contents of a table.
#[derive(Default)]
pub(crate) struct TailData {
    pub index: HashMap<Box<[u8]>, u32, FxBuild>,
    pub keys: Vec<Box<[u8]>>,
    pub entries: Vec<TailEntry>,
    /// `(child entry, step coefficient)`.
    pub edges: Vec<(u32, u32)>,
    pub own_guards: Vec<u32>,
    pub terminals: Interner<IntegralKey>,
    pub values: Interner<Coefficient>,
    pub coefficients: Interner<Coefficient>,
    pub guards: Interner<Coefficient>,
    /// Sorted guard ids.
    pub sets: Interner<Box<[u32]>>,
    /// Every entry, parents before children (phases ascending; within a
    /// phase, a topological order of its same-phase steps).
    pub order: Vec<u32>,
}

/// An entry to add (see [`TailData::fill`]): terminal, guard set,
/// coefficient and guard ids are this table's (`intern_ref`), children are
/// entry ids.
pub(crate) struct NewEntry<'a> {
    pub terminal: Option<u32>,
    pub value: &'a Coefficient,
    pub guards: u32,
    pub phase: u32,
    pub children: &'a [(u32, u32)],
    pub terminal_coefficient: Option<u32>,
    pub own_guards: &'a [u32],
}

impl TailData {
    pub(crate) fn id(&self, key: &[u8]) -> Option<u32> {
        self.index.get(key).copied()
    }

    pub(crate) fn entry(&self, id: u32) -> &TailEntry {
        &self.entries[id as usize]
    }

    pub(crate) fn children(&self, entry: &TailEntry) -> &[(u32, u32)] {
        &self.edges[entry.edges.0 as usize..entry.edges.1 as usize]
    }

    pub(crate) fn own_guards(&self, entry: &TailEntry) -> &[u32] {
        &self.own_guards[entry.own_guards.0 as usize..entry.own_guards.1 as usize]
    }

    /// Reserve an entry id for `key` (absent); its contents are set by
    /// `fill`.
    pub(crate) fn reserve(&mut self, key: Box<[u8]>) -> u32 {
        let id = u32::try_from(self.entries.len()).expect("fewer than 2^32 tail entries");
        self.index.insert(key.clone(), id);
        self.keys.push(key);
        self.entries.push(TailEntry {
            terminal: None,
            value: NONE,
            guards: NONE,
            phase: 0,
            edges: (0, 0),
            terminal_coefficient: NONE,
            own_guards: (0, 0),
        });
        id
    }

    /// Set the contents of a reserved entry.
    pub(crate) fn fill(&mut self, id: u32, entry: NewEntry<'_>) {
        let value = self.values.intern_ref(entry.value).0;
        let start = self.edges.len() as u32;
        self.edges.extend_from_slice(entry.children);
        let edges = (start, self.edges.len() as u32);
        let start = self.own_guards.len() as u32;
        self.own_guards.extend_from_slice(entry.own_guards);
        self.entries[id as usize] = TailEntry {
            terminal: entry.terminal,
            value,
            guards: entry.guards,
            phase: entry.phase,
            edges,
            terminal_coefficient: entry.terminal_coefficient.unwrap_or(NONE),
            own_guards: (start, self.own_guards.len() as u32),
        };
    }

    /// Recompute `order` (after entries were added).
    pub(crate) fn sort(&mut self) -> Result<(), String> {
        let count = self.entries.len();
        let mut by_phase: Vec<u32> = (0..count as u32).collect();
        by_phase.sort_by_key(|&id| self.entries[id as usize].phase);
        // In-degrees over same-phase edges.
        let mut indegree = vec![0u32; count];
        for entry in &self.entries {
            for &(child, _) in self.children(entry) {
                if self.entries[child as usize].phase == entry.phase {
                    indegree[child as usize] += 1;
                }
            }
        }
        let mut order = Vec::with_capacity(count);
        let mut start = 0;
        while start < count {
            let phase = self.entries[by_phase[start] as usize].phase;
            let mut end = start;
            while end < count && self.entries[by_phase[end] as usize].phase == phase {
                end += 1;
            }
            let mut ready: Vec<u32> = by_phase[start..end]
                .iter()
                .copied()
                .filter(|&id| indegree[id as usize] == 0)
                .rev()
                .collect();
            let first = order.len();
            while let Some(id) = ready.pop() {
                order.push(id);
                let entry = self.entries[id as usize];
                for &(child, _) in self.children(&entry) {
                    if self.entries[child as usize].phase == phase {
                        indegree[child as usize] -= 1;
                        if indegree[child as usize] == 0 {
                            ready.push(child);
                        }
                    }
                }
            }
            if order.len() - first != end - start {
                return Err("tail table steps of one phase form a cycle".into());
            }
            start = end;
        }
        self.order = order;
        Ok(())
    }
}

/// Counters of one table.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OrderedTailTableStatistics {
    pub entries: usize,
    pub edges: usize,
    pub values: usize,
    pub coefficients: usize,
    pub guards: usize,
    pub guard_sets: usize,
    pub terminals: usize,
    /// Entries read from the file when the table was opened.
    pub loaded_entries: usize,
    /// Bytes of the file read at opening or written by the last save.
    pub file_bytes: u64,
    pub load_seconds: f64,
    /// Entries were added since the table was read or saved.
    pub dirty: bool,
    /// Why the file found at opening was not used (the table started cold).
    pub rejected: Option<String>,
}

/// Complete reductions and steps of tail identities (see the module docs),
/// shared by every request of one program through its step cache.
pub struct OrderedTailTable {
    data: RwLock<TailData>,
    /// Schema, semantics and identity fields (see the module docs).
    identity: String,
    rejected: Option<String>,
    path: Option<PathBuf>,
    populate: bool,
    dirty: AtomicBool,
    loaded_entries: usize,
    file_bytes: AtomicU64,
    load_seconds: f64,
}

impl std::fmt::Debug for OrderedTailTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OrderedTailTable")
            .field("path", &self.path)
            .field("populate", &self.populate)
            .finish_non_exhaustive()
    }
}

impl OrderedTailTable {
    /// The table of `path` (read now when the file exists), or an empty
    /// in-memory table without a path. `identity` is the program's identity
    /// header (see [`tail_identity`]). A file with another header, or a
    /// corrupt one, is rejected loudly and the table starts empty.
    pub(crate) fn open(
        path: Option<PathBuf>,
        identity: String,
        populate: bool,
        context: &CoefficientContext,
    ) -> Result<Self, ReductionError> {
        let mut table = Self {
            data: RwLock::new(TailData::default()),
            identity,
            rejected: None,
            path,
            populate,
            dirty: AtomicBool::new(false),
            loaded_entries: 0,
            file_bytes: AtomicU64::new(0),
            load_seconds: 0.0,
        };
        let Some(path) = &table.path else {
            return Ok(table);
        };
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(table),
            Err(error) => return Err(failure(format!("{}: {error}", path.display()))),
        };
        let started = std::time::Instant::now();
        match decode(&bytes, &table.identity, context) {
            Ok(data) => {
                table.loaded_entries = data.entries.len();
                table.file_bytes = AtomicU64::new(bytes.len() as u64);
                table.load_seconds = started.elapsed().as_secs_f64();
                table.data = RwLock::new(data);
            }
            Err(reason) => {
                eprintln!(
                    "rustred: ordered tail table {} rejected ({reason}); starting from an \
                     empty table",
                    path.display()
                );
                table.rejected = Some(reason);
            }
        }
        Ok(table)
    }

    pub(crate) fn read(&self) -> RwLockReadGuard<'_, TailData> {
        self.data.read().unwrap_or_else(PoisonError::into_inner)
    }

    /// Whether requests add the entries they compute.
    pub(crate) fn populates(&self) -> bool {
        self.populate
    }

    /// Add entries under the write lock: `update` returns how many it added
    /// (only absent identities, through `reserve` and `fill`); the order is
    /// recomputed. Nothing is added to a read-only table.
    pub(crate) fn update(
        &self,
        update: impl FnOnce(&mut TailData) -> usize,
    ) -> Result<usize, ReductionError> {
        if !self.populate {
            return Ok(0);
        }
        let mut data = self.data.write().unwrap_or_else(PoisonError::into_inner);
        let added = update(&mut data);
        if added > 0 {
            data.sort().map_err(failure)?;
            self.dirty.store(true, Ordering::Relaxed);
        }
        Ok(added)
    }

    pub fn statistics(&self) -> OrderedTailTableStatistics {
        let data = self.read();
        OrderedTailTableStatistics {
            entries: data.entries.len(),
            edges: data.edges.len(),
            values: data.values.len(),
            coefficients: data.coefficients.len(),
            guards: data.guards.len(),
            guard_sets: data.sets.len(),
            terminals: data.terminals.len(),
            loaded_entries: self.loaded_entries,
            file_bytes: self.file_bytes.load(Ordering::Relaxed),
            load_seconds: self.load_seconds,
            dirty: self.dirty.load(Ordering::Relaxed),
            rejected: self.rejected.clone(),
        }
    }

    /// Write the table when entries were added since it was read or saved:
    /// `Some((entries, bytes))`, else `None`. The file is replaced
    /// atomically.
    pub(crate) fn save(
        &self,
        nvars: usize,
        write: impl FnOnce(&Path, &[u8]) -> Result<(), ReductionError>,
    ) -> Result<Option<(usize, u64)>, ReductionError> {
        let Some(path) = &self.path else {
            return Ok(None);
        };
        if !self.dirty.load(Ordering::Relaxed) {
            return Ok(None);
        }
        let data = self.read();
        let bytes = encode(&data, &self.identity, nvars);
        write(path, &bytes)?;
        self.dirty.store(false, Ordering::Relaxed);
        self.file_bytes.store(bytes.len() as u64, Ordering::Relaxed);
        Ok(Some((data.entries.len(), bytes.len() as u64)))
    }
}

fn coefficient_bytes(coefficient: &Coefficient) -> Vec<u8> {
    let mut writer = Writer::default();
    writer.polynomial(&coefficient.numerator);
    writer.polynomial(&coefficient.denominator);
    writer.bytes
}

/// Sort encodings; returns them sorted and the new id of every old id.
fn sorted(encodings: Vec<Vec<u8>>) -> (Vec<Vec<u8>>, Vec<u32>) {
    let mut order: Vec<u32> = (0..encodings.len() as u32).collect();
    order.sort_unstable_by(|&left, &right| encodings[left as usize].cmp(&encodings[right as usize]));
    let mut map = vec![0u32; encodings.len()];
    for (new, &old) in order.iter().enumerate() {
        map[old as usize] = new as u32;
    }
    let mut encodings = encodings;
    let sorted = order
        .iter()
        .map(|&old| std::mem::take(&mut encodings[old as usize]))
        .collect();
    (sorted, map)
}

fn optional(value: u32, map: &[u32]) -> u64 {
    if value == NONE {
        0
    } else {
        u64::from(map[value as usize]) + 1
    }
}

/// The identity header of a program's tail tables: schema, semantics and
/// the cheap identity fields of its step-cache binding (no digest of the
/// program or of the engine sources).
pub(crate) fn tail_identity(
    binding: &crate::reduction::ordered_reduction::step_cache::OrderedStepCacheBinding,
) -> String {
    let policy = match binding.unresolved_policy {
        None => "none",
        Some(crate::solver::UnresolvedPolicy::Reject) => "reject",
        Some(crate::solver::UnresolvedPolicy::Preserve) => "preserve",
    };
    format!(
        "schema {ORDERED_TAIL_SCHEMA}\nsemantics {ORDERED_TAIL_SEMANTICS}\nadapter {:?}\n\
         family {:?}\nvariables {:?}\nunresolved {policy}\n",
        binding.adapter, binding.family_fingerprint, binding.context_variables,
    )
}

fn encode(data: &TailData, identity: &str, nvars: usize) -> Vec<u8> {
    let terminals = sorted(
        data.terminals
            .values()
            .iter()
            .map(|key| {
                let mut bytes = Vec::new();
                let encoded = encode_node(OrderedKeyStage::Integral, key, &mut bytes);
                debug_assert!(encoded, "tail terminals are encodable");
                bytes
            })
            .collect(),
    );
    let guards = sorted(data.guards.values().iter().map(coefficient_bytes).collect());
    let values = sorted(data.values.values().iter().map(coefficient_bytes).collect());
    let coefficients = sorted(
        data.coefficients
            .values()
            .iter()
            .map(coefficient_bytes)
            .collect(),
    );
    let sets = sorted(
        data.sets
            .values()
            .iter()
            .map(|set| {
                let mut ids: Vec<u32> = set.iter().map(|&id| guards.1[id as usize]).collect();
                ids.sort_unstable();
                let mut writer = Writer::default();
                writer.len(ids.len());
                for id in ids {
                    writer.varint(u64::from(id));
                }
                writer.bytes
            })
            .collect(),
    );
    let mut entries: Vec<u32> = (0..data.entries.len() as u32).collect();
    entries.sort_unstable_by(|&left, &right| data.keys[left as usize].cmp(&data.keys[right as usize]));
    let mut position = vec![0u32; entries.len()];
    for (new, &old) in entries.iter().enumerate() {
        position[old as usize] = new as u32;
    }
    let mut payload = Writer::default();
    payload.len(nvars);
    payload.len(terminals.0.len());
    for bytes in &terminals.0 {
        payload.len(bytes.len());
        payload.raw(bytes);
    }
    for table in [&guards.0, &values.0, &coefficients.0, &sets.0] {
        payload.len(table.len());
        for bytes in table {
            payload.raw(bytes);
        }
    }
    payload.len(entries.len());
    for &id in &entries {
        let key = &data.keys[id as usize];
        let entry = &data.entries[id as usize];
        payload.len(key.len());
        payload.raw(key);
        payload.varint(entry.terminal.map_or(0, |id| u64::from(terminals.1[id as usize]) + 1));
        payload.varint(u64::from(values.1[entry.value as usize]));
        payload.varint(u64::from(sets.1[entry.guards as usize]));
        payload.varint(u64::from(entry.phase));
        let children = data.children(entry);
        payload.len(children.len());
        for &(child, coefficient) in children {
            payload.varint(u64::from(position[child as usize]));
            payload.varint(u64::from(coefficients.1[coefficient as usize]));
        }
        payload.varint(optional(entry.terminal_coefficient, &coefficients.1));
        let own = data.own_guards(entry);
        payload.len(own.len());
        for &guard in own {
            payload.varint(u64::from(guards.1[guard as usize]));
        }
    }
    let payload = payload.bytes;
    let mut out = Vec::with_capacity(16 + 8 + identity.len() + 40 + payload.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(identity.len() as u64).to_le_bytes());
    out.extend_from_slice(identity.as_bytes());
    out.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    out.extend_from_slice(blake3::hash(&payload).as_bytes());
    out.extend_from_slice(&payload);
    out
}

fn decode(
    bytes: &[u8],
    identity: &str,
    context: &CoefficientContext,
) -> Result<TailData, String> {
    if bytes.len() < 24 || &bytes[..16] != MAGIC {
        return Err("not an ordered tail table of this schema".into());
    }
    let length = u64::from_le_bytes(bytes[16..24].try_into().expect("eight bytes"));
    let header_end = usize::try_from(length)
        .ok()
        .and_then(|length| length.checked_add(24))
        .filter(|&end| end + 40 <= bytes.len())
        .ok_or("truncated tail table header")?;
    if &bytes[24..header_end] != identity.as_bytes() {
        return Err(format!(
            "version or identity differs: file {:?}, engine {identity:?}",
            String::from_utf8_lossy(&bytes[24..header_end])
        ));
    }
    let length = u64::from_le_bytes(
        bytes[header_end..header_end + 8]
            .try_into()
            .expect("eight bytes"),
    );
    let digest: [u8; 32] = bytes[header_end + 8..header_end + 40]
        .try_into()
        .expect("32 bytes");
    let payload = &bytes[header_end + 40..];
    if payload.len() as u64 != length {
        return Err("tail table payload length differs from its header".into());
    }
    if *blake3::hash(payload).as_bytes() != digest {
        return Err("tail table payload digest differs (corrupt file)".into());
    }
    let limits = ExactAlgebraLimits::default();
    let template = &context.template().numerator;
    let mut reader = Reader::new(payload);
    let bound = payload.len();
    if reader.len(bound)? != template.nvars() {
        return Err("tail table coefficient variables differ from the context".into());
    }
    let mut data = TailData::default();
    let count = reader.len(bound)?;
    for _ in 0..count {
        let length = reader.len(bound)?;
        let (stage, key) = decode_node(reader.take(length)?)?;
        if stage != NodeStage::Integral {
            return Err("tail table terminal is not a plain integral".into());
        }
        if !data.terminals.intern_ref(&key).1 {
            return Err("duplicate tail table terminal".into());
        }
    }
    let coefficient = |reader: &mut Reader<'_>| -> Result<Coefficient, String> {
        let numerator = reader.polynomial(template)?;
        let denominator = reader.polynomial(template)?;
        let value = Coefficient {
            numerator,
            denominator,
        };
        context
            .validate_with_limits(&value, limits)
            .map_err(|error| format!("invalid tail table coefficient: {error}"))?;
        Ok(value)
    };
    for interner in [&mut data.guards, &mut data.values, &mut data.coefficients] {
        let count = reader.len(bound)?;
        for _ in 0..count {
            let value = coefficient(&mut reader)?;
            if !interner.intern_ref(&value).1 {
                return Err("duplicate tail table coefficient".into());
            }
        }
    }
    let guard_count = data.guards.len();
    let count = reader.len(bound)?;
    for _ in 0..count {
        let length = reader.len(guard_count)?;
        let mut ids = Vec::with_capacity(length);
        for _ in 0..length {
            ids.push(reader.index(guard_count)?);
        }
        if ids.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err("tail table guard set is not sorted".into());
        }
        if !data.sets.intern_ref(&ids.into_boxed_slice()).1 {
            return Err("duplicate tail table guard set".into());
        }
    }
    let (terminals, values, coefficients, sets) = (
        data.terminals.len(),
        data.values.len(),
        data.coefficients.len(),
        data.sets.len(),
    );
    let optional = |reader: &mut Reader<'_>, len: usize| -> Result<u32, String> {
        match reader.varint()? {
            0 => Ok(NONE),
            id => u32::try_from(id - 1)
                .ok()
                .filter(|&id| (id as usize) < len)
                .ok_or_else(|| "tail table index out of range".to_owned()),
        }
    };
    let count = reader.len(bound)?;
    data.entries.reserve(count);
    data.keys.reserve(count);
    for _ in 0..count {
        let length = reader.len(bound)?;
        let key: Box<[u8]> = reader.take(length)?.into();
        check_tail_identity(&key)?;
        if data.keys.last().is_some_and(|last| *last >= key) {
            return Err("tail table entries are not sorted".into());
        }
        let terminal = optional(&mut reader, terminals)?;
        let value = reader.index(values)?;
        let guards = reader.index(sets)?;
        let phase = u32::try_from(reader.varint()?).map_err(|_| "tail table phase overflows")?;
        let children = reader.len(bound)?;
        let start = data.edges.len() as u32;
        for _ in 0..children {
            let child = reader.index(count)?;
            let coefficient = reader.index(coefficients)?;
            data.edges.push((child, coefficient));
        }
        let edges = (start, data.edges.len() as u32);
        let terminal_coefficient = optional(&mut reader, coefficients)?;
        let own = reader.len(bound)?;
        let start = data.own_guards.len() as u32;
        for _ in 0..own {
            data.own_guards.push(reader.index(guard_count)?);
        }
        if terminal == NONE && !data.values.value(value).is_zero() {
            return Err("tail table entry without terminal has a nonzero value".into());
        }
        let id = data.entries.len() as u32;
        data.index.insert(key.clone(), id);
        data.keys.push(key);
        data.entries.push(TailEntry {
            terminal: (terminal != NONE).then_some(terminal),
            value,
            guards,
            phase,
            edges,
            terminal_coefficient,
            own_guards: (start, data.own_guards.len() as u32),
        });
    }
    if !reader.is_empty() {
        return Err("trailing bytes after the tail table".into());
    }
    for entry in &data.entries {
        for &(child, _) in data.children(entry) {
            if data.entries[child as usize].phase < entry.phase {
                return Err("tail table child in an earlier phase".into());
            }
        }
    }
    data.sort()?;
    Ok(data)
}
