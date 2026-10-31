//! Persistent, weight-independent one-step records of the Ordered scheduler.
//!
//! An Ordered adapter's checked step is a deterministic function of the popped
//! identity and the immutable program: the selected route or rule, the ordered
//! children with their exact step coefficients, emitted terminal step
//! coefficients and the step's nonzero conditions in first-appearance order.
//! [`OrderedStepCache`] records exactly that output, one record per expanded
//! identity, and replays it on a later expansion of the same identity. A hit
//! returns the same children in the same order with the same coefficients and
//! the same guards in the same order as the checked step, so the scheduler
//! performs the same merges, cancellations and guard interning as an uncached
//! run, in every weight arithmetic and in every scheduler pass.
//!
//! The cache never stores frontier or terminal weights, zero-erasure
//! decisions or final answers. Steps whose output depends on earlier steps of
//! the same request (closed-artifact product factorization, whose memoized
//! lower-factor reductions retain guards only at first use) are marked opaque
//! and always recomputed.
//!
//! Records are sharded by [`OrderedStepShard`]: the operational stage, the
//! support size and the topology class (the owner sector of saved
//! applications). A cache opened on a directory loads a shard lazily at the
//! first lookup into it, populates it on misses, and [`OrderedStepCache::save`]
//! writes each changed shard as one deterministic file (sorted identities,
//! coefficients and records; the same records produce byte-identical files for
//! any thread count) plus a manifest. Every file carries the cache stamp,
//! a BLAKE3 digest of the [`OrderedStepCacheBinding`] (program digest, family,
//! coefficient variables, adapter, unresolved policy, candidate terminal
//! convention) and of the engine that recorded it (schema, step semantics
//! version, a build-time digest of the RustRed sources and the Symbolica
//! build); a mismatched or corrupt file is rejected with
//! [`ReductionError::OrderedStepCache`], never used. Step coefficients are
//! stored with their denominators factored over a basis of irreducible
//! factors, so modular arithmetics can map them cheaply.
//!
//! Memory: resident records are counted per shard. With
//! [`OrderedStepCacheOptions::max_resident_bytes`] or an active Symbolica
//! RSS budget, records or shards that do not fit are skipped (counted in the
//! statistics); a skipped record only costs a recomputation.
pub(crate) mod codec;
mod factor;
pub(crate) mod kernel;
#[cfg(test)]
pub(crate) mod tests;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError, RwLock};

use symbolica::domains::integer::Integer;

use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};
use crate::family::IntegralKey;
use crate::reduction::ReductionError;
use crate::reduction::ordered_reduction::tail::table::{TAIL_FILE, tail_identity};
use crate::reduction::ordered_reduction::tail::{OrderedTailTable, OrderedTailTableStatistics};
use crate::solver::UnresolvedPolicy;
use codec::{FxBuild, NODE_HEADER, Reader, Writer, hex, parse_hex32};
use factor::{FactorBasis, FactoredDenominator, Polynomial};

/// File-format and manifest schema.
pub const ORDERED_STEP_CACHE_SCHEMA: &str = "rustred.ordered-step-cache.v2";
/// Version of the checked Ordered step semantics. It changes whenever an
/// adapter can return a different step for the same identity and program
/// (e.g. dropping zero-sector transport endpoints), which invalidates every
/// existing cache through the stamp. A golden digest of fixture records
/// (`step_cache::tests`) fails until it is bumped after such a change.
pub const ORDERED_STEP_SEMANTICS: &str = "rustred-ordered-steps-2026-10-31";
/// BLAKE3 digest (hex) of every Rust source of `rustred` and `rustred-order`,
/// computed at build time. It is part of every stamp, so records are only
/// replayed by an engine built from exactly the sources that recorded them,
/// even when a change of a checked step forgot to bump
/// [`ORDERED_STEP_SEMANTICS`].
pub const ORDERED_STEP_SOURCES: &str = env!("RUSTRED_ORDERED_STEP_SOURCES");

const SHARD_MAGIC: &[u8; 16] = b"RR-ORDERED-STEP1";
const MANIFEST_FILE: &str = "ordered-step-cache.txt";
const SHARD_DIRECTORY: &str = "shards";
const SHARD_HEADER: usize = 16 + 32 + 1 + 2 + 8 + 8 + 32;

pub(crate) fn failure(detail: impl Into<String>) -> ReductionError {
    ReductionError::OrderedStepCache {
        detail: detail.into(),
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The Ordered adapter whose checked steps a cache records.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OrderedStepCacheAdapter {
    /// Closed lower-loop artifacts (`Reducer`).
    ClosedArtifact,
    /// Candidate programs (`CandidateReducer`).
    CandidateProgram,
    /// Saved Route/Apply applications (`RoutedCandidateApplication`).
    SavedApplication,
}

impl OrderedStepCacheAdapter {
    fn as_str(self) -> &'static str {
        match self {
            Self::ClosedArtifact => "closed-artifact",
            Self::CandidateProgram => "candidate-program",
            Self::SavedApplication => "saved-application",
        }
    }
}

/// Everything a checked step depends on besides its identity and the engine
/// (whose sources and Symbolica build every stamp includes, see
/// [`ordered_step_engine`]).
///
/// `program_digest` digests every program file the steps read (rule tables,
/// owners, routes). Loaders compute it from the bytes they read where they
/// can: `PreparedSavedApplication::ordered_step_cache_binding` in
/// rustred-app, and the digest a candidate bundle loader declares on its
/// `CandidateReducer` (see `CandidateReducer::ordered_step_cache_binding`),
/// which every attachment and request then checks. Otherwise it is the
/// caller's statement. The remaining fields are checked against the program
/// at attachment and at every request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderedStepCacheBinding {
    /// Adapter whose checked steps the cache records.
    pub adapter: OrderedStepCacheAdapter,
    /// Digest of the program files the steps read (see above).
    pub program_digest: [u8; 32],
    /// Fingerprint of the integral family of the program.
    pub family_fingerprint: String,
    /// Coefficient-context parameter names, in variable order.
    pub context_variables: Vec<String>,
    /// Residual policy of candidate programs and saved applications: under
    /// `Preserve` an uncovered identity is a terminal step, under `Reject`
    /// an error. `None` for closed artifacts.
    pub unresolved_policy: Option<UnresolvedPolicy>,
    /// Digest of a candidate program's installed terminal convention (raw,
    /// terminal aliases or a weighted normalization): the terminal step row
    /// of every raw terminal, computed by RustRed
    /// (`CandidateReducer::ordered_step_cache_binding`). `None` for closed
    /// artifacts and saved applications, whose terminal steps are fixed by
    /// the program.
    pub terminal_convention: Option<[u8; 32]>,
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '%' => out.push_str("%25"),
            ' ' => out.push_str("%20"),
            '\n' => out.push_str("%0A"),
            '\r' => out.push_str("%0D"),
            '\t' => out.push_str("%09"),
            other => out.push(other),
        }
    }
    out
}

/// Symbolica as identified by its build: `git describe` output or the crate
/// version. Describe output is reduced to its seven-digit commit prefix, so a
/// checkout with tags (`v3.0.1-110-g9cd57678`) and a Cargo git checkout
/// without them (`9cd5767`) name the same sources alike.
fn symbolica_identity(version: &str) -> String {
    let described = version.strip_prefix("symbolica-").unwrap_or(version);
    let commit = described
        .rsplit_once("-g")
        .map_or(described, |(_, commit)| commit);
    if commit.len() >= 7 && commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        format!("git-{}", commit[..7].to_ascii_lowercase())
    } else {
        escape(described)
    }
}

/// The engine part of every stamp: RustRed version, the build-time digest
/// of its sources ([`ORDERED_STEP_SOURCES`]) and the Symbolica build.
pub fn ordered_step_engine() -> &'static str {
    static ENGINE: OnceLock<String> = OnceLock::new();
    ENGINE.get_or_init(|| {
        format!(
            "rustred {} sources {ORDERED_STEP_SOURCES} symbolica {}",
            env!("CARGO_PKG_VERSION"),
            symbolica_identity(symbolica::license::LicenseManager::get_version())
        )
    })
}

impl OrderedStepCacheBinding {
    /// A binding without a terminal convention, for closed artifacts and
    /// saved applications. Candidate programs take theirs from
    /// `CandidateReducer::ordered_step_cache_binding`.
    pub fn new(
        adapter: OrderedStepCacheAdapter,
        program_digest: [u8; 32],
        family_fingerprint: impl Into<String>,
        context: &CoefficientContext,
        unresolved_policy: Option<UnresolvedPolicy>,
    ) -> Self {
        Self {
            adapter,
            program_digest,
            family_fingerprint: family_fingerprint.into(),
            context_variables: context.parameter_names().to_vec(),
            unresolved_policy,
            terminal_convention: None,
        }
    }

    /// Canonical description, including the engine; its BLAKE3 digest is
    /// the stamp.
    pub fn description(&self) -> String {
        self.describe(ordered_step_engine())
    }

    fn describe(&self, engine: &str) -> String {
        let policy = match self.unresolved_policy {
            None => "none",
            Some(UnresolvedPolicy::Reject) => "reject",
            Some(UnresolvedPolicy::Preserve) => "preserve",
        };
        let terminals = self
            .terminal_convention
            .map_or_else(|| "none".to_owned(), |digest| hex(&digest));
        let variables: Vec<_> = self.context_variables.iter().map(|v| escape(v)).collect();
        format!(
            "schema {ORDERED_STEP_CACHE_SCHEMA}\nsemantics {ORDERED_STEP_SEMANTICS}\n\
             engine {engine}\nadapter {}\nfamily {}\nprogram {}\nvariables {} {}\n\
             unresolved {policy}\nterminals {terminals}\n",
            self.adapter.as_str(),
            escape(&self.family_fingerprint),
            hex(&self.program_digest),
            variables.len(),
            variables.join(" "),
        )
    }

    /// Validity stamp written into every cache file.
    pub fn stamp(&self) -> [u8; 32] {
        *blake3::hash(self.description().as_bytes()).as_bytes()
    }
}

/// Digest of a terminal convention: each raw terminal (in order) with its
/// terminal step row, the exact terms a checked step emits for it.
pub(crate) fn terminal_convention_digest<'a, E>(
    rows: impl ExactSizeIterator<
        Item = Result<(&'a IntegralKey, BTreeMap<IntegralKey, Coefficient>), E>,
    >,
) -> Result<[u8; 32], E> {
    fn key(writer: &mut Writer, key: &IntegralKey) {
        writer.len(key.powers().len());
        for &power in key.powers() {
            writer.zigzag(power);
        }
    }
    let mut writer = Writer::default();
    writer.raw(b"rustred.ordered-step-terminal-convention.v1\n");
    writer.len(rows.len());
    for row in rows {
        let (terminal, terms) = row?;
        key(&mut writer, terminal);
        writer.len(terms.len());
        for (output, coefficient) in &terms {
            key(&mut writer, output);
            writer.polynomial(&coefficient.numerator);
            writer.polynomial(&coefficient.denominator);
        }
    }
    Ok(*blake3::hash(&writer.bytes).as_bytes())
}

/// Resource behavior of one cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OrderedStepCacheOptions {
    /// Record the steps of misses (live population). Off: read-only.
    pub populate: bool,
    /// Ceiling on the estimated resident bytes of loaded and inserted
    /// records; beyond it new records and shards are skipped.
    pub max_resident_bytes: Option<usize>,
    /// Resident growth covered by one Symbolica RSS budget check
    /// (`check_allocation`). A refused check stops all further growth.
    pub budget_grant_bytes: usize,
    /// Largest shard file accepted on load.
    pub max_shard_file_bytes: u64,
}

impl Default for OrderedStepCacheOptions {
    fn default() -> Self {
        Self {
            populate: true,
            max_resident_bytes: None,
            budget_grant_bytes: 64 << 20,
            max_shard_file_bytes: 64 << 30,
        }
    }
}

/// Operational stage of a recorded identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OrderedStepStage {
    /// Closed and candidate programs.
    Integral,
    /// Saved-application transport towards an owner.
    Route,
    /// Saved-application owner rules.
    Apply,
}

impl OrderedStepStage {
    fn as_str(self) -> &'static str {
        match self {
            Self::Integral => "integral",
            Self::Route => "route",
            Self::Apply => "apply",
        }
    }
    fn tag(self) -> u8 {
        match self {
            Self::Integral => 0,
            Self::Route => 1,
            Self::Apply => 2,
        }
    }
    fn from_tag(tag: u8) -> Option<Self> {
        Some(match tag {
            0 => Self::Integral,
            1 => Self::Route,
            2 => Self::Apply,
            _ => return None,
        })
    }
    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "integral" => Self::Integral,
            "route" => Self::Route,
            "apply" => Self::Apply,
            _ => return None,
        })
    }
}

/// One shard: stage, support size and topology class. The class is the owner
/// sector (bit `i` = position `i`) for saved applications: the Apply owner,
/// or the owner a Route identity is transported to. It is zero for closed
/// and candidate programs, whose shards are their support levels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OrderedStepShard {
    pub stage: OrderedStepStage,
    pub support: u16,
    pub class: u64,
}

impl OrderedStepShard {
    fn file_stem(&self) -> String {
        format!(
            "{}-s{:02}-{:016x}",
            self.stage.as_str(),
            self.support,
            self.class
        )
    }
}

impl fmt::Display for OrderedStepShard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.file_stem())
    }
}

/// Residency of one shard.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OrderedStepShardState {
    /// Created in this process; not on disk.
    #[default]
    New,
    /// Listed in the manifest and not read yet.
    OnDisk,
    /// Resident (read from disk and/or populated).
    Resident,
    /// Not loaded or populated because of the memory budget.
    Skipped,
    /// Its file failed validation; it is never used.
    Rejected,
}

/// Counters of one shard. Hits, misses and inserts are cumulative over every
/// request that used the cache.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrderedStepCacheShardStatistics {
    pub shard: OrderedStepShard,
    pub state: OrderedStepShardState,
    /// Resident records (loaded plus inserted).
    pub records: usize,
    /// Records read from the shard file.
    pub loaded_records: usize,
    pub hits: u64,
    /// Lookups without a record (including lookups into a skipped shard).
    pub misses: u64,
    pub inserts: u64,
    /// Records not stored because of the budget, a skipped shard or a
    /// non-compact identity.
    pub skipped_inserts: u64,
    /// Hits on opaque records, whose steps are recomputed.
    pub opaque_hits: u64,
    /// Estimated resident bytes (identities, coefficients, records).
    pub resident_bytes: usize,
    /// Bytes of the shard file in the manifest, when there is one.
    pub file_bytes: u64,
    /// Wall seconds spent reading, checking and decoding the shard file.
    pub load_seconds: f64,
    /// Resident records not yet saved.
    pub dirty: bool,
}

/// Snapshot of a cache; `shards` is sorted by shard.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OrderedStepCacheStatistics {
    pub shards: Vec<OrderedStepCacheShardStatistics>,
    pub records: usize,
    pub hits: u64,
    pub misses: u64,
    pub inserts: u64,
    pub skipped_inserts: u64,
    pub resident_bytes: usize,
    pub file_bytes: u64,
    pub load_seconds: f64,
    pub loaded_shards: usize,
    pub skipped_shards: usize,
    pub rejected_shards: usize,
    /// Interned irreducible denominator factors.
    pub factor_basis: usize,
    /// A refused budget check stopped all further growth.
    pub budget_exhausted: bool,
}

/// Result of [`OrderedStepCache::save`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OrderedStepCacheSaveReport {
    pub directory: PathBuf,
    pub shards_written: usize,
    pub records_written: usize,
    pub bytes_written: u64,
    /// Every shard in the manifest after the save.
    pub shards: usize,
    pub records: usize,
    pub bytes: u64,
    /// Entries and bytes of the tail table file when it was written by this
    /// save (see `OrderedStepCache::tail_statistics`), else zero.
    pub tail_entries: usize,
    pub tail_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ManifestEntry {
    file: String,
    records: usize,
    bytes: u64,
    digest: [u8; 32],
}

/// One step: children and terminals refer to identity and coefficient ids
/// of the shard's tables, in the checked step's order.
struct Record {
    terms: Box<[(u32, u32)]>,
    terminals: Box<[(u32, u32)]>,
    guards: Box<[u32]>,
    opaque: bool,
}

#[derive(Default)]
struct ShardData {
    nodes: Vec<Arc<[u8]>>,
    node_index: HashMap<Arc<[u8]>, u32, FxBuild>,
    coefficients: Vec<Arc<Coefficient>>,
    coefficient_index: HashMap<Arc<Coefficient>, u32, FxBuild>,
    /// Denominator factorizations read from disk or computed when saving.
    factored: Vec<Option<FactoredDenominator>>,
    records: HashMap<u32, Record, FxBuild>,
    bytes: usize,
    loaded_records: usize,
    load_seconds: f64,
}

const NODE_OVERHEAD: usize = 64;
const COEFFICIENT_OVERHEAD: usize = 2 * std::mem::size_of::<Polynomial>() + 96;
const RECORD_OVERHEAD: usize = std::mem::size_of::<Record>() + 48;

fn polynomial_bytes(polynomial: &Polynomial) -> usize {
    let large: usize = polynomial
        .coefficients
        .iter()
        .map(|coefficient| match coefficient {
            Integer::Large(_) => coefficient.significant_bits().div_ceil(8) as usize + 16,
            _ => 0,
        })
        .sum();
    polynomial.coefficients.len() * std::mem::size_of::<Integer>()
        + polynomial.exponents.len() * 2
        + large
}

fn coefficient_bytes(coefficient: &Coefficient) -> usize {
    COEFFICIENT_OVERHEAD
        + polynomial_bytes(&coefficient.numerator)
        + polynomial_bytes(&coefficient.denominator)
}

impl ShardData {
    fn node_id(&mut self, bytes: &[u8]) -> Result<u32, ReductionError> {
        if let Some(&id) = self.node_index.get(bytes) {
            return Ok(id);
        }
        let id = u32::try_from(self.nodes.len())
            .map_err(|_| failure("ordered step cache shard exceeds u32 identities"))?;
        let node: Arc<[u8]> = Arc::from(bytes);
        self.nodes.push(Arc::clone(&node));
        self.node_index.insert(node, id);
        self.bytes += bytes.len() + NODE_OVERHEAD;
        Ok(id)
    }

    fn coefficient_id(&mut self, coefficient: &Coefficient) -> Result<u32, ReductionError> {
        if let Some(&id) = self.coefficient_index.get(coefficient) {
            return Ok(id);
        }
        let id = u32::try_from(self.coefficients.len())
            .map_err(|_| failure("ordered step cache shard exceeds u32 coefficients"))?;
        let value = Arc::new(coefficient.clone());
        self.coefficients.push(Arc::clone(&value));
        self.coefficient_index.insert(value, id);
        self.factored.push(None);
        self.bytes += coefficient_bytes(coefficient);
        Ok(id)
    }
}

/// Upper bound of the bytes `record` can add (every identity and coefficient
/// new), computed without table lookups.
fn record_bytes_bound(record: &StepRecordInput<'_>) -> usize {
    let nodes = 1 + record.terms.len() + record.terminals.len();
    RECORD_OVERHEAD
        + 8 * (record.terms.len() + record.terminals.len())
        + 4 * record.guards.len()
        + nodes * (record.key.len() + NODE_OVERHEAD)
        + record
            .terms
            .iter()
            .chain(record.terminals.iter())
            .map(|(_, coefficient)| coefficient_bytes(coefficient))
            .chain(record.guards.iter().map(coefficient_bytes))
            .sum::<usize>()
}

enum ShardState {
    OnDisk(ManifestEntry),
    Resident {
        file: Option<ManifestEntry>,
        dirty: bool,
    },
    Skipped,
    Rejected {
        error: String,
    },
}

struct ShardInner {
    state: ShardState,
    data: ShardData,
}

struct Shard {
    id: OrderedStepShard,
    inner: RwLock<ShardInner>,
    hits: AtomicU64,
    misses: AtomicU64,
    inserts: AtomicU64,
    skipped_inserts: AtomicU64,
    opaque_hits: AtomicU64,
}

/// One step to record. Identities are encoded by
/// [`codec::encode_node`]; coefficients are exact step coefficients.
pub(crate) struct StepRecordInput<'a> {
    pub key: &'a [u8],
    pub terms: Vec<(&'a [u8], &'a Coefficient)>,
    pub terminals: Vec<(&'a [u8], &'a Coefficient)>,
    pub guards: &'a [Coefficient],
    pub opaque: bool,
}

/// Borrowed record of a hit; valid while the shard is read-locked.
pub(crate) struct RecordView<'a> {
    data: &'a ShardData,
    record: &'a Record,
}

impl<'a> RecordView<'a> {
    /// Children in the checked step's order: encoded identity and exact step
    /// coefficient.
    pub(crate) fn terms(&self) -> impl ExactSizeIterator<Item = (&'a [u8], &'a Coefficient)> + 'a {
        let data = self.data;
        self.record.terms.iter().map(move |&(node, coefficient)| {
            (
                &*data.nodes[node as usize],
                &*data.coefficients[coefficient as usize],
            )
        })
    }

    pub(crate) fn terminals(
        &self,
    ) -> impl ExactSizeIterator<Item = (&'a [u8], &'a Coefficient)> + 'a {
        let data = self.data;
        self.record
            .terminals
            .iter()
            .map(move |&(node, coefficient)| {
                (
                    &*data.nodes[node as usize],
                    &*data.coefficients[coefficient as usize],
                )
            })
    }

    /// Nonzero conditions in the checked step's first-appearance order.
    pub(crate) fn guards(&self) -> impl ExactSizeIterator<Item = &'a Coefficient> + 'a {
        let data = self.data;
        self.record
            .guards
            .iter()
            .map(move |&coefficient| &*data.coefficients[coefficient as usize])
    }
}

/// Outcome of one lookup.
pub(crate) enum Lookup<T> {
    Hit(T),
    /// The identity's step depends on request history; compute it.
    Opaque,
    Miss,
    /// The shard is skipped (budget); compute and do not record.
    Unavailable,
}

/// The shard counter a lookup increments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LookupCount {
    Hit,
    Opaque,
    /// No record, or a skipped shard.
    Miss,
}

impl<T> Lookup<T> {
    pub(crate) fn count(&self) -> LookupCount {
        match self {
            Self::Hit(_) => LookupCount::Hit,
            Self::Opaque => LookupCount::Opaque,
            Self::Miss | Self::Unavailable => LookupCount::Miss,
        }
    }
}

/// Thread-safe store of one-step records, shared by every request, pass and
/// weight arithmetic of one program through an `Arc`. See the module docs.
pub struct OrderedStepCache {
    binding: OrderedStepCacheBinding,
    stamp: [u8; 32],
    options: OrderedStepCacheOptions,
    directory: Option<PathBuf>,
    manifest: Mutex<BTreeMap<OrderedStepShard, ManifestEntry>>,
    shards: RwLock<BTreeMap<OrderedStepShard, Arc<Shard>>>,
    basis: Mutex<FactorBasis>,
    resident: AtomicUsize,
    granted: AtomicUsize,
    exhausted: AtomicBool,
    save_lock: Mutex<()>,
    /// The tail table, read at its first use.
    tail: Mutex<Option<Arc<OrderedTailTable>>>,
}

impl fmt::Debug for OrderedStepCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OrderedStepCache")
            .field("binding", &self.binding)
            .field("directory", &self.directory)
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}

impl OrderedStepCache {
    /// A cache without a directory; it is populated by requests and dropped
    /// with its last handle.
    pub fn in_memory(
        binding: OrderedStepCacheBinding,
        options: OrderedStepCacheOptions,
    ) -> Arc<Self> {
        Arc::new(Self::build(binding, options, None, BTreeMap::new()))
    }

    /// Open (or prepare to create) the cache in `directory`. An existing
    /// manifest must carry this binding's stamp; its shards are read lazily.
    /// Nothing is written before [`Self::save`].
    pub fn open(
        directory: impl Into<PathBuf>,
        binding: OrderedStepCacheBinding,
        options: OrderedStepCacheOptions,
    ) -> Result<Arc<Self>, ReductionError> {
        let directory = directory.into();
        let path = directory.join(MANIFEST_FILE);
        let manifest = match std::fs::read(&path) {
            Ok(bytes) => parse_manifest(&bytes, &binding)
                .map_err(|error| failure(format!("{}: {error}", path.display())))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(error) => return Err(failure(format!("{}: {error}", path.display()))),
        };
        Ok(Arc::new(Self::build(
            binding,
            options,
            Some(directory),
            manifest,
        )))
    }

    fn build(
        binding: OrderedStepCacheBinding,
        options: OrderedStepCacheOptions,
        directory: Option<PathBuf>,
        manifest: BTreeMap<OrderedStepShard, ManifestEntry>,
    ) -> Self {
        Self {
            stamp: binding.stamp(),
            binding,
            options,
            directory,
            manifest: Mutex::new(manifest),
            shards: RwLock::new(BTreeMap::new()),
            basis: Mutex::new(FactorBasis::default()),
            resident: AtomicUsize::new(0),
            granted: AtomicUsize::new(0),
            exhausted: AtomicBool::new(false),
            save_lock: Mutex::new(()),
            tail: Mutex::new(None),
        }
    }

    /// The tail table kept with this cache (complete reductions of tail
    /// identities, see `ordered_reduction::tail`), read from the directory
    /// at its first use and populated like the step records (unless the
    /// cache is read-only). It is bound by versioning only (schema, semantics
    /// version and the binding's identity fields, see `tail::table`); a file
    /// of another version or a corrupt one is rejected loudly and the table
    /// starts cold. Closed-artifact caches have no tail table: their steps
    /// may depend on earlier steps of a request.
    pub(crate) fn tail_table(
        &self,
        context: &CoefficientContext,
    ) -> Result<Option<Arc<OrderedTailTable>>, ReductionError> {
        if self.binding.adapter == OrderedStepCacheAdapter::ClosedArtifact {
            return Ok(None);
        }
        let mut slot = lock(&self.tail);
        if let Some(table) = &*slot {
            return Ok(Some(Arc::clone(table)));
        }
        let table = Arc::new(OrderedTailTable::open(
            self.directory.as_ref().map(|directory| directory.join(TAIL_FILE)),
            tail_identity(&self.binding),
            self.options.populate,
            context,
        )?);
        *slot = Some(Arc::clone(&table));
        Ok(Some(table))
    }

    /// Counters of the tail table, once a request has used it.
    pub fn tail_statistics(&self) -> Option<OrderedTailTableStatistics> {
        lock(&self.tail).as_ref().map(|table| table.statistics())
    }

    pub fn binding(&self) -> &OrderedStepCacheBinding {
        &self.binding
    }

    pub fn stamp(&self) -> [u8; 32] {
        self.stamp
    }

    pub fn directory(&self) -> Option<&Path> {
        self.directory.as_deref()
    }

    pub fn options(&self) -> OrderedStepCacheOptions {
        self.options
    }

    /// Check a program against this cache before a request uses it.
    /// `program_digest` is the digest RustRed knows for the program (declared
    /// by its loader), `None` when only the caller can vouch for it.
    pub(crate) fn check_binding(
        &self,
        adapter: OrderedStepCacheAdapter,
        family_fingerprint: &str,
        context: &CoefficientContext,
        unresolved_policy: Option<UnresolvedPolicy>,
        terminal_convention: Option<[u8; 32]>,
        program_digest: Option<[u8; 32]>,
    ) -> Result<(), ReductionError> {
        let binding = &self.binding;
        if binding.adapter != adapter {
            return Err(failure(format!(
                "cache records {} steps, not {} steps",
                binding.adapter.as_str(),
                adapter.as_str()
            )));
        }
        if binding.family_fingerprint != family_fingerprint {
            return Err(failure("cache family differs from the program family"));
        }
        if binding.context_variables != context.parameter_names() {
            return Err(failure(
                "cache coefficient variables differ from the program context",
            ));
        }
        if binding.unresolved_policy != unresolved_policy {
            return Err(failure(
                "cache unresolved policy differs from the program policy",
            ));
        }
        if binding.terminal_convention != terminal_convention {
            return Err(failure(
                "cache terminal convention differs from the installed one (bind candidate \
                 caches with CandidateReducer::ordered_step_cache_binding)",
            ));
        }
        if program_digest.is_some_and(|digest| digest != binding.program_digest) {
            return Err(failure(
                "cache program digest differs from the digest of the loaded program",
            ));
        }
        Ok(())
    }

    fn shard(&self, id: OrderedStepShard) -> Arc<Shard> {
        if let Some(shard) = self
            .shards
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&id)
        {
            return Arc::clone(shard);
        }
        let mut shards = self.shards.write().unwrap_or_else(PoisonError::into_inner);
        Arc::clone(shards.entry(id).or_insert_with(|| {
            let state = match lock(&self.manifest).get(&id) {
                Some(entry) => ShardState::OnDisk(entry.clone()),
                None => ShardState::Resident {
                    file: None,
                    dirty: false,
                },
            };
            Arc::new(Shard {
                id,
                inner: RwLock::new(ShardInner {
                    state,
                    data: ShardData::default(),
                }),
                hits: AtomicU64::new(0),
                misses: AtomicU64::new(0),
                inserts: AtomicU64::new(0),
                skipped_inserts: AtomicU64::new(0),
                opaque_hits: AtomicU64::new(0),
            })
        }))
    }

    /// Account `bytes` of resident growth against the configured ceiling and
    /// the active Symbolica RSS budgets. `false` means: do not grow.
    fn reserve(&self, bytes: usize) -> bool {
        if self.exhausted.load(Ordering::Relaxed) {
            return false;
        }
        let total = self.resident.fetch_add(bytes, Ordering::Relaxed) + bytes;
        if self
            .options
            .max_resident_bytes
            .is_some_and(|limit| total > limit)
        {
            self.resident.fetch_sub(bytes, Ordering::Relaxed);
            return false;
        }
        while total > self.granted.load(Ordering::Relaxed) {
            let grant = self.options.budget_grant_bytes.max(bytes);
            match symbolica::utils::resources::check_allocation(grant as u64) {
                Ok(()) => {
                    self.granted.fetch_add(grant, Ordering::Relaxed);
                }
                Err(error) => {
                    if matches!(
                        error,
                        symbolica::utils::resources::ResourceLimitError::Memory { .. }
                    ) {
                        self.exhausted.store(true, Ordering::Relaxed);
                    }
                    self.resident.fetch_sub(bytes, Ordering::Relaxed);
                    return false;
                }
            }
        }
        true
    }

    fn ensure_loaded(
        &self,
        shard: &Shard,
        context: &CoefficientContext,
    ) -> Result<(), ReductionError> {
        if !matches!(
            shard
                .inner
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .state,
            ShardState::OnDisk(_)
        ) {
            return Ok(());
        }
        let mut inner = shard.inner.write().unwrap_or_else(PoisonError::into_inner);
        let ShardState::OnDisk(entry) = &inner.state else {
            return Ok(());
        };
        let entry = entry.clone();
        // Decoded size is bounded by a small multiple of the file size.
        let estimate = usize::try_from(entry.bytes.saturating_mul(3)).unwrap_or(usize::MAX);
        if !self.reserve(estimate) {
            inner.state = ShardState::Skipped;
            return Ok(());
        }
        let started = std::time::Instant::now();
        match self.read_shard(shard.id, &entry, context) {
            Ok(mut data) => {
                data.load_seconds = started.elapsed().as_secs_f64();
                if data.bytes > estimate {
                    self.resident
                        .fetch_add(data.bytes - estimate, Ordering::Relaxed);
                } else {
                    self.resident
                        .fetch_sub(estimate - data.bytes, Ordering::Relaxed);
                }
                inner.data = data;
                inner.state = ShardState::Resident {
                    file: Some(entry),
                    dirty: false,
                };
                Ok(())
            }
            Err(error) => {
                self.resident.fetch_sub(estimate, Ordering::Relaxed);
                let error = format!("shard {} ({}): {error}", shard.id, entry.file);
                inner.state = ShardState::Rejected {
                    error: error.clone(),
                };
                Err(failure(error))
            }
        }
    }

    /// Look up the record of `key` (encoded identity) in `shard`, loading
    /// the shard on first use. `build` runs on a hit, under the shard's read
    /// lock.
    pub(crate) fn lookup<T>(
        &self,
        id: OrderedStepShard,
        key: &[u8],
        context: &CoefficientContext,
        build: impl FnOnce(RecordView<'_>) -> T,
    ) -> Result<Lookup<T>, ReductionError> {
        let shard = self.shard(id);
        let found = self.find(&shard, key, context, build)?;
        Self::count(&shard, found.count());
        Ok(found)
    }

    /// [`Self::lookup`] for a step computed ahead of its turn by a worker of
    /// the concurrent scheduler, whose key may still cancel. It loads the
    /// shard like a lookup but counts nothing and creates no shard that is
    /// neither present nor listed in the manifest (such a shard holds no
    /// record). The caller counts the lookup with [`Self::count_lookup`] at
    /// the key's sequential turn, so the statistics are those of a
    /// sequential run.
    pub(crate) fn probe<T>(
        &self,
        id: OrderedStepShard,
        key: &[u8],
        context: &CoefficientContext,
        build: impl FnOnce(RecordView<'_>) -> T,
    ) -> Result<Lookup<T>, ReductionError> {
        let present = self
            .shards
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&id)
            .cloned();
        let shard = match present {
            Some(shard) => shard,
            None => {
                let listed = lock(&self.manifest).contains_key(&id);
                if !listed {
                    return Ok(Lookup::Miss);
                }
                self.shard(id)
            }
        };
        self.find(&shard, key, context, build)
    }

    /// Count one lookup of `id` that [`Self::probe`] answered.
    pub(crate) fn count_lookup(&self, id: OrderedStepShard, count: LookupCount) {
        Self::count(&self.shard(id), count);
    }

    fn count(shard: &Shard, count: LookupCount) {
        let counter = match count {
            LookupCount::Hit => &shard.hits,
            LookupCount::Opaque => &shard.opaque_hits,
            LookupCount::Miss => &shard.misses,
        };
        counter.fetch_add(1, Ordering::Relaxed);
    }

    /// The uncounted lookup of `key` in `shard`, loading it on first use.
    fn find<T>(
        &self,
        shard: &Shard,
        key: &[u8],
        context: &CoefficientContext,
        build: impl FnOnce(RecordView<'_>) -> T,
    ) -> Result<Lookup<T>, ReductionError> {
        self.ensure_loaded(shard, context)?;
        let inner = shard.inner.read().unwrap_or_else(PoisonError::into_inner);
        match &inner.state {
            ShardState::Resident { .. } => {}
            ShardState::Skipped => return Ok(Lookup::Unavailable),
            ShardState::Rejected { error, .. } => return Err(failure(error.clone())),
            ShardState::OnDisk(_) => {
                return Err(failure("ordered step cache shard was not loaded"));
            }
        }
        let record = inner
            .data
            .node_index
            .get(key)
            .and_then(|id| inner.data.records.get(id));
        let Some(record) = record else {
            return Ok(Lookup::Miss);
        };
        if record.opaque {
            return Ok(Lookup::Opaque);
        }
        Ok(Lookup::Hit(build(RecordView {
            data: &inner.data,
            record,
        })))
    }

    /// Record one checked step. Inserting an identity that already has a
    /// record is a no-op (records are deterministic functions of the key).
    /// Returns whether a record was stored.
    pub(crate) fn insert(
        &self,
        id: OrderedStepShard,
        record: StepRecordInput<'_>,
    ) -> Result<bool, ReductionError> {
        if !self.options.populate {
            return Ok(false);
        }
        let shard = self.shard(id);
        let mut inner = shard.inner.write().unwrap_or_else(PoisonError::into_inner);
        let ShardInner { state, data } = &mut *inner;
        let ShardState::Resident { dirty, .. } = state else {
            shard.skipped_inserts.fetch_add(1, Ordering::Relaxed);
            return Ok(false);
        };
        if data
            .node_index
            .get(record.key)
            .is_some_and(|key| data.records.contains_key(key))
        {
            return Ok(false);
        }
        // Reserve the lookup-free upper bound, then release what interning
        // shared with earlier records.
        let added = record_bytes_bound(&record);
        if !self.reserve(added) {
            shard.skipped_inserts.fetch_add(1, Ordering::Relaxed);
            return Ok(false);
        }
        let before = data.bytes;
        let key = data.node_id(record.key)?;
        let mut terms = Vec::with_capacity(record.terms.len());
        for (node, coefficient) in &record.terms {
            terms.push((data.node_id(node)?, data.coefficient_id(coefficient)?));
        }
        let mut terminals = Vec::with_capacity(record.terminals.len());
        for (node, coefficient) in &record.terminals {
            terminals.push((data.node_id(node)?, data.coefficient_id(coefficient)?));
        }
        let mut guards = Vec::with_capacity(record.guards.len());
        for guard in record.guards {
            guards.push(data.coefficient_id(guard)?);
        }
        data.bytes += RECORD_OVERHEAD + 8 * (terms.len() + terminals.len()) + 4 * guards.len();
        data.records.insert(
            key,
            Record {
                terms: terms.into(),
                terminals: terminals.into(),
                guards: guards.into(),
                opaque: record.opaque,
            },
        );
        // Keep the global census equal to the per-shard estimates.
        let actual = data.bytes - before;
        if actual > added {
            self.resident.fetch_add(actual - added, Ordering::Relaxed);
        } else {
            self.resident.fetch_sub(added - actual, Ordering::Relaxed);
        }
        *dirty = true;
        shard.inserts.fetch_add(1, Ordering::Relaxed);
        Ok(true)
    }

    /// Count a record that could not be offered (non-compact identity).
    pub(crate) fn skip_insert(&self, id: OrderedStepShard) {
        self.shard(id)
            .skipped_inserts
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn statistics(&self) -> OrderedStepCacheStatistics {
        let mut statistics = OrderedStepCacheStatistics {
            factor_basis: lock(&self.basis).len(),
            budget_exhausted: self.exhausted.load(Ordering::Relaxed),
            ..Default::default()
        };
        let shards = self.shards.read().unwrap_or_else(PoisonError::into_inner);
        let manifest = lock(&self.manifest);
        let mut ids: BTreeSet<OrderedStepShard> = shards.keys().copied().collect();
        ids.extend(manifest.keys().copied());
        for id in ids {
            let mut shard_statistics = OrderedStepCacheShardStatistics {
                shard: id,
                state: OrderedStepShardState::OnDisk,
                records: 0,
                loaded_records: 0,
                hits: 0,
                misses: 0,
                inserts: 0,
                skipped_inserts: 0,
                opaque_hits: 0,
                resident_bytes: 0,
                file_bytes: manifest.get(&id).map_or(0, |entry| entry.bytes),
                load_seconds: 0.0,
                dirty: false,
            };
            if let Some(shard) = shards.get(&id) {
                let inner = shard.inner.read().unwrap_or_else(PoisonError::into_inner);
                shard_statistics.state = match &inner.state {
                    ShardState::OnDisk(_) => OrderedStepShardState::OnDisk,
                    ShardState::Resident { file: None, .. } => OrderedStepShardState::New,
                    ShardState::Resident { .. } => OrderedStepShardState::Resident,
                    ShardState::Skipped => OrderedStepShardState::Skipped,
                    ShardState::Rejected { .. } => OrderedStepShardState::Rejected,
                };
                if let ShardState::Resident { file, dirty } = &inner.state {
                    shard_statistics.dirty = *dirty;
                    if let Some(file) = file {
                        shard_statistics.file_bytes = file.bytes;
                    }
                }
                shard_statistics.records = inner.data.records.len();
                shard_statistics.loaded_records = inner.data.loaded_records;
                shard_statistics.resident_bytes = inner.data.bytes;
                shard_statistics.load_seconds = inner.data.load_seconds;
                shard_statistics.hits = shard.hits.load(Ordering::Relaxed);
                shard_statistics.misses = shard.misses.load(Ordering::Relaxed);
                shard_statistics.inserts = shard.inserts.load(Ordering::Relaxed);
                shard_statistics.skipped_inserts = shard.skipped_inserts.load(Ordering::Relaxed);
                shard_statistics.opaque_hits = shard.opaque_hits.load(Ordering::Relaxed);
            }
            match shard_statistics.state {
                OrderedStepShardState::Resident => statistics.loaded_shards += 1,
                OrderedStepShardState::Skipped => statistics.skipped_shards += 1,
                OrderedStepShardState::Rejected => statistics.rejected_shards += 1,
                _ => {}
            }
            statistics.records += shard_statistics.records;
            statistics.hits += shard_statistics.hits;
            statistics.misses += shard_statistics.misses;
            statistics.inserts += shard_statistics.inserts;
            statistics.skipped_inserts += shard_statistics.skipped_inserts;
            statistics.resident_bytes += shard_statistics.resident_bytes;
            statistics.file_bytes += shard_statistics.file_bytes;
            statistics.load_seconds += shard_statistics.load_seconds;
            statistics.shards.push(shard_statistics);
        }
        statistics
    }

    /// Write every changed resident shard and the manifest into the cache
    /// directory. Shard files are content-addressed and written before the
    /// manifest is replaced atomically, so an interrupted save leaves the
    /// previous cache intact. Shards that were never loaded keep their files.
    ///
    /// A directory has one writer at a time. A replaced shard file is removed
    /// after the new manifest is in place, so another process still holding
    /// the previous manifest fails loudly when it loads that shard; it never
    /// reads records of another file.
    pub fn save(&self) -> Result<OrderedStepCacheSaveReport, ReductionError> {
        let directory = self
            .directory
            .clone()
            .ok_or_else(|| failure("an in-memory ordered step cache has no directory"))?;
        let _saving = lock(&self.save_lock);
        let io =
            |path: &Path, error: std::io::Error| failure(format!("{}: {error}", path.display()));
        let shard_directory = directory.join(SHARD_DIRECTORY);
        std::fs::create_dir_all(&shard_directory).map_err(|error| io(&shard_directory, error))?;
        let mut manifest = lock(&self.manifest).clone();
        let mut report = OrderedStepCacheSaveReport {
            directory: directory.clone(),
            ..Default::default()
        };
        let mut replaced = Vec::new();
        let shards: Vec<_> = self
            .shards
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
            .cloned()
            .collect();
        for shard in shards {
            let mut inner = shard.inner.write().unwrap_or_else(PoisonError::into_inner);
            let ShardInner { state, data } = &mut *inner;
            let ShardState::Resident { file, dirty } = state else {
                continue;
            };
            if !*dirty || data.records.is_empty() {
                continue;
            }
            let (bytes, records) = self.encode_shard(shard.id, data)?;
            let digest = *blake3::hash(&bytes[SHARD_HEADER..]).as_bytes();
            let name = format!(
                "{SHARD_DIRECTORY}/{}.{}.rrstep",
                shard.id.file_stem(),
                hex(&digest[..8])
            );
            write_atomically(&directory.join(&name), &bytes)?;
            let entry = ManifestEntry {
                file: name,
                records,
                bytes: bytes.len() as u64,
                digest,
            };
            report.shards_written += 1;
            report.records_written += records;
            report.bytes_written += entry.bytes;
            if let Some(old) = file.replace(entry.clone())
                && old.file != entry.file
            {
                replaced.push(old.file);
            }
            *dirty = false;
            manifest.insert(shard.id, entry);
        }
        let mut text = self.binding.description();
        text.push_str(&format!("stamp {}\n", hex(&self.stamp)));
        for (id, entry) in &manifest {
            text.push_str(&format!(
                "shard {} {} {:016x} {} {} {} {}\n",
                id.stage.as_str(),
                id.support,
                id.class,
                entry.file,
                entry.records,
                entry.bytes,
                hex(&entry.digest)
            ));
            report.shards += 1;
            report.records += entry.records;
            report.bytes += entry.bytes;
        }
        text.push_str("end\n");
        write_atomically(&directory.join(MANIFEST_FILE), text.as_bytes())?;
        *lock(&self.manifest) = manifest;
        let tail = lock(&self.tail).clone();
        if let Some(table) = tail
            && let Some((entries, bytes)) =
                table.save(self.binding.context_variables.len(), write_atomically)?
        {
            report.tail_entries = entries;
            report.tail_bytes = bytes;
        }
        for file in replaced {
            // The new manifest no longer refers to it; a leftover is harmless.
            let _ = std::fs::remove_file(directory.join(file));
        }
        Ok(report)
    }

    /// Deterministic shard file: identities, factors, coefficients and
    /// records sorted by their encodings, so the bytes depend only on the
    /// set of records.
    fn encode_shard(
        &self,
        id: OrderedStepShard,
        data: &mut ShardData,
    ) -> Result<(Vec<u8>, usize), ReductionError> {
        let mut records: Vec<(&[u8], &Record)> = data
            .records
            .iter()
            .map(|(key, record)| (&*data.nodes[*key as usize], record))
            .collect();
        records.sort_unstable_by(|left, right| left.0.cmp(right.0));
        // Referenced identities and coefficients.
        let mut node_ids = BTreeSet::new();
        let mut coefficient_ids = BTreeSet::new();
        for (key, record) in &records {
            node_ids.insert(data.node_index[*key]);
            for &(node, coefficient) in record.terms.iter().chain(record.terminals.iter()) {
                node_ids.insert(node);
                coefficient_ids.insert(coefficient);
            }
            coefficient_ids.extend(record.guards.iter().copied());
        }
        let node_bytes = records.first().map_or(NODE_HEADER, |(key, _)| key.len());
        let mut nodes: Vec<u32> = node_ids.into_iter().collect();
        nodes.sort_unstable_by(|&left, &right| {
            data.nodes[left as usize].cmp(&data.nodes[right as usize])
        });
        let mut node_map = HashMap::with_capacity(nodes.len());
        for (local, &node) in nodes.iter().enumerate() {
            if data.nodes[node as usize].len() != node_bytes {
                return Err(failure("ordered step cache shard mixes identity widths"));
            }
            node_map.insert(node, local as u32);
        }
        // Factor every referenced denominator over the global basis.
        let mut basis = lock(&self.basis);
        let mut factor_ids = BTreeSet::new();
        for &coefficient in &coefficient_ids {
            let slot = coefficient as usize;
            if data.factored[slot].is_none() {
                let denominator = &data.coefficients[slot].denominator;
                data.factored[slot] = Some(basis.factor(denominator).map_err(failure)?);
            }
            for &(factor, _) in data.factored[slot]
                .as_ref()
                .expect("factored")
                .factors
                .iter()
            {
                factor_ids.insert(factor);
            }
        }
        let mut factors: Vec<(Vec<u8>, u32)> = factor_ids
            .into_iter()
            .map(|factor| {
                let mut writer = Writer::default();
                writer.polynomial(basis.get(factor).expect("interned factor"));
                (writer.bytes, factor)
            })
            .collect();
        drop(basis);
        factors.sort_unstable();
        let factor_map: HashMap<u32, u32> = factors
            .iter()
            .enumerate()
            .map(|(local, (_, factor))| (*factor, local as u32))
            .collect();
        let mut coefficients: Vec<(Vec<u8>, u32)> = Vec::with_capacity(coefficient_ids.len());
        for &coefficient in &coefficient_ids {
            let value = &data.coefficients[coefficient as usize];
            let factored = data.factored[coefficient as usize]
                .as_ref()
                .expect("factored");
            let mut local: Vec<(u32, u32)> = factored
                .factors
                .iter()
                .map(|&(factor, power)| (factor_map[&factor], power))
                .collect();
            local.sort_unstable();
            let mut writer = Writer::default();
            writer.polynomial(&value.numerator);
            writer.polynomial(&value.denominator);
            writer.integer(&factored.content);
            writer.len(local.len());
            for (factor, power) in local {
                writer.varint(u64::from(factor));
                writer.varint(u64::from(power));
            }
            coefficients.push((writer.bytes, coefficient));
        }
        coefficients.sort_unstable();
        let coefficient_map: HashMap<u32, u32> = coefficients
            .iter()
            .enumerate()
            .map(|(local, (_, coefficient))| (*coefficient, local as u32))
            .collect();
        let nvars = self.binding.context_variables.len();
        let mut payload = Writer::default();
        payload.len(node_bytes);
        payload.len(nvars);
        payload.len(factors.len());
        for (bytes, _) in &factors {
            payload.raw(bytes);
        }
        payload.len(coefficients.len());
        for (bytes, _) in &coefficients {
            payload.raw(bytes);
        }
        payload.len(nodes.len());
        for &node in &nodes {
            payload.raw(&data.nodes[node as usize]);
        }
        payload.len(records.len());
        for (key, record) in &records {
            payload.varint(u64::from(node_map[&data.node_index[*key]]));
            payload.u8(u8::from(record.opaque));
            for list in [&record.terms, &record.terminals] {
                payload.len(list.len());
                for &(node, coefficient) in list.iter() {
                    payload.varint(u64::from(node_map[&node]));
                    payload.varint(u64::from(coefficient_map[&coefficient]));
                }
            }
            payload.len(record.guards.len());
            for guard in record.guards.iter() {
                payload.varint(u64::from(coefficient_map[guard]));
            }
        }
        let payload = payload.bytes;
        let mut out = Vec::with_capacity(SHARD_HEADER + payload.len());
        out.extend_from_slice(SHARD_MAGIC);
        out.extend_from_slice(&self.stamp);
        out.push(id.stage.tag());
        out.extend_from_slice(&id.support.to_le_bytes());
        out.extend_from_slice(&id.class.to_le_bytes());
        out.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        out.extend_from_slice(blake3::hash(&payload).as_bytes());
        out.extend_from_slice(&payload);
        Ok((out, records.len()))
    }

    fn read_shard(
        &self,
        id: OrderedStepShard,
        entry: &ManifestEntry,
        context: &CoefficientContext,
    ) -> Result<ShardData, String> {
        let directory = self.directory.as_ref().ok_or("cache has no directory")?;
        let path = directory.join(&entry.file);
        let length = std::fs::metadata(&path)
            .map_err(|error| error.to_string())?
            .len();
        if length != entry.bytes || length > self.options.max_shard_file_bytes {
            return Err(format!(
                "file has {length} bytes, the manifest declares {}",
                entry.bytes
            ));
        }
        let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
        if bytes.len() < SHARD_HEADER || &bytes[..16] != SHARD_MAGIC {
            return Err("not an ordered step cache shard".into());
        }
        if bytes[16..48] != self.stamp {
            return Err("shard stamp differs from the cache binding".into());
        }
        let stage = OrderedStepStage::from_tag(bytes[48]).ok_or("invalid shard stage")?;
        let support = u16::from_le_bytes(bytes[49..51].try_into().expect("two bytes"));
        let class = u64::from_le_bytes(bytes[51..59].try_into().expect("eight bytes"));
        if (OrderedStepShard {
            stage,
            support,
            class,
        }) != id
        {
            return Err("shard file belongs to another shard".into());
        }
        let payload_length = u64::from_le_bytes(bytes[59..67].try_into().expect("eight bytes"));
        let payload = &bytes[SHARD_HEADER..];
        if payload_length != payload.len() as u64 {
            return Err("shard payload length differs from its header".into());
        }
        let digest = blake3::hash(payload);
        if digest.as_bytes()[..] != bytes[67..99] || *digest.as_bytes() != entry.digest {
            return Err("shard payload digest differs".into());
        }
        decode_payload(payload, context, &mut lock(&self.basis))
    }
}

fn decode_payload(
    payload: &[u8],
    context: &CoefficientContext,
    basis: &mut FactorBasis,
) -> Result<ShardData, String> {
    let limits = ExactAlgebraLimits::default();
    let template = &context.template().numerator;
    let one = template.one();
    let mut reader = Reader::new(payload);
    let bound = payload.len();
    let node_bytes = reader.len(bound)?;
    if node_bytes < NODE_HEADER {
        return Err("invalid identity width".into());
    }
    if reader.len(bound)? != template.nvars() {
        return Err("shard coefficient variables differ from the context".into());
    }
    let factor_count = reader.len(bound)?;
    let mut factors = Vec::with_capacity(factor_count);
    for _ in 0..factor_count {
        let factor = reader.polynomial(template)?;
        if factor.is_constant() || factor.lcoeff().is_negative() {
            return Err("shard denominator factor is not normalized".into());
        }
        context
            .validate_with_limits(
                &Coefficient {
                    numerator: factor.clone(),
                    denominator: one.clone(),
                },
                limits,
            )
            .map_err(|error| format!("invalid shard factor: {error}"))?;
        factors.push(basis.intern(factor)?);
    }
    let mut data = ShardData::default();
    let coefficient_count = reader.len(bound)?;
    for _ in 0..coefficient_count {
        let numerator = reader.polynomial(template)?;
        let denominator = reader.polynomial(template)?;
        let coefficient = Coefficient {
            numerator,
            denominator,
        };
        context
            .validate_with_limits(&coefficient, limits)
            .map_err(|error| format!("invalid shard coefficient: {error}"))?;
        let content = reader.integer()?;
        if content.is_zero() {
            return Err("shard denominator content is zero".into());
        }
        let count = reader.len(factors.len())?;
        let mut pairs = Vec::with_capacity(count);
        for _ in 0..count {
            let factor = factors[reader.index(factors.len())? as usize];
            let power = u32::try_from(reader.varint()?).map_err(|_| "factor power exceeds u32")?;
            pairs.push((factor, power));
        }
        pairs.sort_unstable();
        let factored = FactoredDenominator {
            content,
            factors: pairs.into(),
        };
        debug_assert_eq!(
            basis.expand(&factored, template).as_ref(),
            Ok(&coefficient.denominator)
        );
        let id = data.coefficients.len() as u32;
        if data.coefficient_index.contains_key(&coefficient) {
            return Err("shard repeats a coefficient".into());
        }
        data.bytes += coefficient_bytes(&coefficient);
        let value = Arc::new(coefficient);
        data.coefficients.push(Arc::clone(&value));
        data.coefficient_index.insert(value, id);
        data.factored.push(Some(factored));
    }
    let node_count = reader.len(bound)?;
    for _ in 0..node_count {
        let node = reader.take(node_bytes)?;
        codec::decode_node(node)?;
        let id = data.nodes.len() as u32;
        let node: Arc<[u8]> = Arc::from(node);
        if data.node_index.insert(Arc::clone(&node), id).is_some() {
            return Err("shard repeats an identity".into());
        }
        data.nodes.push(node);
        data.bytes += node_bytes + NODE_OVERHEAD;
    }
    let record_count = reader.len(bound)?;
    let nodes = data.nodes.len();
    let coefficients = data.coefficients.len();
    for _ in 0..record_count {
        let key = reader.index(nodes)?;
        let opaque = match reader.u8()? {
            0 => false,
            1 => true,
            _ => return Err("invalid record flag".into()),
        };
        let mut lists = [Vec::new(), Vec::new()];
        for list in &mut lists {
            let count = reader.len(bound)?;
            list.reserve(count);
            for _ in 0..count {
                list.push((reader.index(nodes)?, reader.index(coefficients)?));
            }
        }
        let count = reader.len(bound)?;
        let mut guards = Vec::with_capacity(count);
        for _ in 0..count {
            guards.push(reader.index(coefficients)?);
        }
        let [terms, terminals] = lists;
        data.bytes += RECORD_OVERHEAD + 8 * (terms.len() + terminals.len()) + 4 * guards.len();
        let record = Record {
            terms: terms.into(),
            terminals: terminals.into(),
            guards: guards.into(),
            opaque,
        };
        if data.records.insert(key, record).is_some() {
            return Err("shard repeats a record".into());
        }
    }
    if !reader.is_empty() {
        return Err("trailing bytes after the shard records".into());
    }
    data.loaded_records = data.records.len();
    Ok(data)
}

fn parse_manifest(
    bytes: &[u8],
    binding: &OrderedStepCacheBinding,
) -> Result<BTreeMap<OrderedStepShard, ManifestEntry>, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "manifest is not UTF-8")?;
    let description = binding.description();
    let expected = format!("{description}stamp {}\n", hex(&binding.stamp()));
    let Some(rest) = text.strip_prefix(&expected) else {
        let stored: String = text
            .lines()
            .take_while(|line| !line.starts_with("shard ") && *line != "end")
            .collect::<Vec<_>>()
            .join("; ");
        return Err(format!(
            "cache binding differs (stored: {stored}; expected: {})",
            expected.lines().collect::<Vec<_>>().join("; ")
        ));
    };
    let mut shards = BTreeMap::new();
    let mut ended = false;
    for line in rest.lines() {
        if ended {
            return Err("manifest has lines after its end".into());
        }
        if line == "end" {
            ended = true;
            continue;
        }
        let fields: Vec<&str> = line.split(' ').collect();
        let ["shard", stage, support, class, file, records, size, digest] = fields.as_slice()
        else {
            return Err(format!("invalid manifest line {line:?}"));
        };
        let id = OrderedStepShard {
            stage: OrderedStepStage::parse(stage).ok_or("invalid manifest stage")?,
            support: support.parse().map_err(|_| "invalid manifest support")?,
            class: u64::from_str_radix(class, 16).map_err(|_| "invalid manifest class")?,
        };
        let valid_file = file
            .strip_prefix(&format!("{SHARD_DIRECTORY}/"))
            .is_some_and(|name| {
                !name.is_empty()
                    && name
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
                    && !name.starts_with('.')
            });
        if !valid_file {
            return Err(format!("invalid manifest shard file {file:?}"));
        }
        let entry = ManifestEntry {
            file: (*file).to_owned(),
            records: records
                .parse()
                .map_err(|_| "invalid manifest record count")?,
            bytes: size.parse().map_err(|_| "invalid manifest byte count")?,
            digest: parse_hex32(digest)?,
        };
        if shards.insert(id, entry).is_some() {
            return Err("manifest repeats a shard".into());
        }
    }
    if !ended {
        return Err("manifest is truncated".into());
    }
    Ok(shards)
}

fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), ReductionError> {
    let io = |error: std::io::Error| failure(format!("{}: {error}", path.display()));
    let file_name = path
        .file_name()
        .ok_or_else(|| failure("cache path has no file name"))?
        .to_string_lossy()
        .into_owned();
    let temporary = path.with_file_name(format!(".{file_name}.tmp-{}", std::process::id()));
    {
        let mut file = std::fs::File::create(&temporary).map_err(io)?;
        file.write_all(bytes).map_err(io)?;
        file.sync_all().map_err(io)?;
    }
    std::fs::rename(&temporary, path).map_err(io)
}
