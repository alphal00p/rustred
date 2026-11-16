//! vk5l WP2: relation entries `rustred.relation-entry.v1` and the persistent
//! relation cache (interface: plan/vk5l/interfaces/relation-entry.md).
//!
//! One record format for shipped relations, run-time cache entries and
//! campaign results: the engine's own `relation` record of `solve-recipe`
//! (canonical JSON, volatile keys dropped) as `body`, inside an envelope
//! (`recipe`, `bound_to`, `evidence`, `diagnostics`). The entry's identity is
//! `entry_digest` = SHA-256 of the canonical body; the engine's
//! `relation_digest` stays inside the body. Lookup key = (node identity, key
//! in the TABLE frame). Validity: version constants and the node identity
//! (family text + node order + arity), never the binary (decision Q2).
//!
//! A cache ROOT holds one directory per node identity; every writing process
//! appends to its own segment file (one entry per line, fsync after each), so
//! writers need no lock, a torn last line after a crash is ignored, and two
//! caches merge by copying segment files. Read-only roots (shipped / pre-filled
//! relations) come first, the writable run-time cache after them.
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use serde_json::{Value, json};

use crate::node::recipe_job::canonical;

/// Schema of one entry.
pub const RELATION_ENTRY_SCHEMA: &str = "rustred.relation-entry.v1";
/// Schema of a cache root's stamp file.
pub const RELATION_CACHE_SCHEMA: &str = "rustred.relation-cache.v1";
/// The frame of every entry key: the engine's table (recipe, presented) frame.
pub const KEY_FRAME: &str = "table";
/// The determinism-id major line an entry must have.
pub const DETERMINISM_MAJOR: &str = "rustred.recipe-solve.v2/elim=rustred.recipe-elim.v1/";
/// The determinism line of an IMPORTED relation that is not a recipe solve
/// (a symmetry image or a symmetry relation at the key, exact, from a
/// statement): `rustred.relation-import.v1/kind=<kind>/statement=<id>`.
pub const IMPORT_MAJOR: &str = "rustred.relation-import.v1/";
/// Optional cap of the writable cache (bytes of its node directories).
pub const MAX_BYTES_ENV: &str = "RUSTRED_P_RELATION_CACHE_MAX_BYTES";
/// `0` / `off` / `no`: entries of evidence level `none` are not used.
pub const ALLOW_FIELD_ENV: &str = "RUSTRED_P_RELATIONS_ALLOW_FIELD";
/// Keys of the engine record that depend on the batch or the clock.
const VOLATILE: [&str; 4] = ["seconds", "vmhwm_kb", "ordinal", "tag"];

// ----- digests and identities -----

/// SHA-256 of `bytes`, lower-case hex (FIPS 180-4; no extra dependency).
pub fn sha256_hex(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut message = bytes.to_vec();
    let bits = (bytes.len() as u64).wrapping_mul(8);
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bits.to_be_bytes());
    for block in message.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for (i, word) in block.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (state, value) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *state = state.wrapping_add(value);
        }
    }
    h.iter().map(|word| format!("{word:08x}")).collect()
}

/// The node identity: BLAKE3 of the canonical JSON
/// `{"arity", "family_blake3", "order"}` (family text as the engine reads it).
pub fn node_identity(family_text: &str, order: &Value, arity: usize) -> String {
    let family = blake3::hash(family_text.as_bytes()).to_hex().to_string();
    let text = canonical(&json!({"arity": arity, "family_blake3": family, "order": order}));
    blake3::hash(text.as_bytes()).to_hex().to_string()
}

/// Symbolica's save-format version (read from its own export header).
pub fn symbolica_save_format() -> Option<u16> {
    static FORMAT: OnceLock<Option<u16>> = OnceLock::new();
    *FORMAT.get_or_init(|| {
        let mut bytes = Vec::new();
        symbolica::state::State::export(&mut bytes).ok()?;
        Some(u16::from_le_bytes([*bytes.get(4)?, *bytes.get(5)?]))
    })
}

/// The entry body of an engine relation record: the record without its
/// volatile keys; `key` filled in when the record lacks it.
pub fn entry_body(record: &Value, key_table: &[i64]) -> Value {
    let mut body = record.clone();
    if let Value::Object(map) = &mut body {
        for key in VOLATILE {
            map.remove(key);
        }
        map.entry("key").or_insert_with(|| json!(key_table));
    }
    body
}

/// `entry_digest` of a body.
pub fn entry_digest(body: &Value) -> String {
    sha256_hex(canonical(body).as_bytes())
}

/// Evidence levels, best first.
pub fn evidence_rank(level: &str) -> u8 {
    match level {
        "per-key" => 2,
        "imported" => 1,
        _ => 0,
    }
}

/// What a producer knows about one entry beyond the engine record.
#[derive(Clone, Debug)]
pub struct EntryMeta {
    pub node: String,
    pub node_identity: String,
    pub package_revision: Option<String>,
    pub recipe_id: String,
    /// The recipe object as solved (its canonical JSON is hashed).
    pub recipe: Value,
    pub recipe_digest: String,
    pub relation_rule: String,
    pub evidence_level: &'static str,
    pub pole_order_at_4: Option<u32>,
    pub regular_alternative: &'static str,
    pub outside_certified_set: bool,
    pub written_by: String,
}

/// One complete entry.
pub fn build_entry(meta: &EntryMeta, key_table: &[i64], body: Value) -> Value {
    let digest = entry_digest(&body);
    let binary = crate::node::recipe::binary_identity();
    let determinism = body
        .get("determinism_id")
        .and_then(Value::as_str)
        .unwrap_or(crate::node::recipe::DETERMINISM_ID)
        .to_owned();
    let verdict = body.get("verdict").cloned().unwrap_or(Value::Null);
    json!({
        "schema": RELATION_ENTRY_SCHEMA,
        "node": meta.node,
        "node_identity": meta.node_identity,
        "key_frame": KEY_FRAME,
        "key": key_table,
        "entry_digest": digest,
        "body": body,
        "recipe": {"id": meta.recipe_id, "file_sha256": sha256_hex(canonical(&meta.recipe).as_bytes()),
                   "engine_digest": meta.recipe_digest},
        "bound_to": {"engine": {"git_commit": binary["git_commit"], "binary_blake3": binary["blake3"]},
                     "relation_rule": meta.relation_rule, "determinism_id": determinism,
                     "node_identity": meta.node_identity, "package_revision": meta.package_revision,
                     "symbolica_save_format": symbolica_save_format()},
        "evidence": {"level": meta.evidence_level,
                     "proof": {"tool": "rustred.recipe-solve/exact-weights", "tool_digest": binary["blake3"],
                               "record_sha256": digest, "verdict": verdict},
                     "fact": Value::Null},
        "diagnostics": {"pole_order_at": {"4": meta.pole_order_at_4},
                        "regular_alternative": meta.regular_alternative,
                        "recipe": meta.recipe_id, "outside_certified_set": meta.outside_certified_set},
        "written": {"utc": utc_now(), "by": meta.written_by},
    })
}

fn utc_now() -> String {
    // seconds since the epoch, rendered as an ISO date-time (UTC)
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs()) as i64;
    let (days, rest) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    // civil-from-days (H. Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest / 60 % 60,
        rest % 60
    )
}

/// Why an entry is not used (counted).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryRefusal {
    /// Another node identity (or schema / determinism line): ignored.
    Foreign,
    /// Unreadable, torn, or its digest does not match its body.
    Corrupt,
    /// Not a proved SOLVED relation of its key.
    Unproved,
    /// Evidence `none` while field entries are not allowed.
    Field,
}

/// The field-entry setting of [`ALLOW_FIELD_ENV`] (default: allowed).
pub fn allow_field_from_env() -> bool {
    !matches!(
        std::env::var(ALLOW_FIELD_ENV).as_deref().map(str::trim),
        Ok("0" | "off" | "no" | "false")
    )
}

/// The size cap of [`MAX_BYTES_ENV`], if set.
pub fn max_bytes_from_env() -> Option<u64> {
    std::env::var(MAX_BYTES_ENV)
        .ok()
        .and_then(|value| value.trim().parse().ok())
}

/// The settings of one store (programmatic; `None` fields fall back to the
/// environment).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StoreOptions {
    /// Cap of the writable root's node directory (bytes); inserts above it
    /// are skipped and counted, nothing is evicted.
    pub max_bytes: Option<u64>,
    /// Whether entries of evidence level `none` are used.
    pub allow_field: Option<bool>,
}

/// The total size of the relation entries (`*.jsonl` of every node
/// directory) under a cache root; 0 when the root does not exist.
pub fn bytes_on_disk(root: &Path) -> u64 {
    let Ok(nodes) = std::fs::read_dir(root) else {
        return 0;
    };
    nodes
        .filter_map(Result::ok)
        .filter(|node| node.path().is_dir())
        .filter_map(|node| std::fs::read_dir(node.path()).ok())
        .flatten()
        .filter_map(Result::ok)
        .filter(|file| file.path().extension().is_some_and(|ext| ext == "jsonl"))
        .filter_map(|file| file.metadata().ok())
        .map(|meta| meta.len())
        .sum()
}

/// The load check of one entry (interface section 3, items 1-6), field
/// entries as [`ALLOW_FIELD_ENV`] says.
pub fn check_entry(entry: &Value, node_identity: &str) -> Result<Vec<i64>, EntryRefusal> {
    check_entry_with(entry, node_identity, allow_field_from_env())
}

/// [`check_entry`] with an explicit field-entry setting.
pub fn check_entry_with(
    entry: &Value,
    node_identity: &str,
    allow_field: bool,
) -> Result<Vec<i64>, EntryRefusal> {
    if entry["schema"] != RELATION_ENTRY_SCHEMA || entry["node_identity"] != node_identity {
        return Err(EntryRefusal::Foreign);
    }
    let determinism = entry["bound_to"]["determinism_id"].as_str().unwrap_or("");
    if !determinism.starts_with(DETERMINISM_MAJOR) && !determinism.starts_with(IMPORT_MAJOR) {
        return Err(EntryRefusal::Foreign);
    }
    if entry["key_frame"] != KEY_FRAME {
        return Err(EntryRefusal::Foreign);
    }
    let key: Vec<i64> =
        serde_json::from_value(entry["key"].clone()).map_err(|_| EntryRefusal::Corrupt)?;
    let body = &entry["body"];
    if entry["entry_digest"].as_str() != Some(entry_digest(body).as_str()) {
        return Err(EntryRefusal::Corrupt);
    }
    if body["verdict"] != "SOLVED"
        || body["all_lower"] != Value::Bool(true)
        || body["proof"]["ok"] != Value::Bool(true)
        || body["key"] != entry["key"]
    {
        return Err(EntryRefusal::Unproved);
    }
    if evidence_rank(entry["evidence"]["level"].as_str().unwrap_or("none")) == 0 && !allow_field {
        return Err(EntryRefusal::Field);
    }
    Ok(key)
}

/// The rank of an entry: (evidence, -(pole order at d = 4), -(rhs terms),
/// digest), larger is better except for the digest (smaller wins).
fn rank(entry: &Value) -> (u8, i64, i64) {
    let evidence = evidence_rank(entry["evidence"]["level"].as_str().unwrap_or("none"));
    let pole = entry["diagnostics"]["pole_order_at"]["4"]
        .as_i64()
        .unwrap_or(i64::MAX / 2);
    let terms = entry["body"]["rhs"]
        .as_array()
        .map_or(i64::MAX / 2, |rhs| rhs.len() as i64);
    (evidence, -pole, -terms)
}

// ----- counters -----

static SHIPPED_HITS: AtomicU64 = AtomicU64::new(0);
static CACHE_HITS: AtomicU64 = AtomicU64::new(0);
static MISSES: AtomicU64 = AtomicU64::new(0);
static INSERTS: AtomicU64 = AtomicU64::new(0);
static SKIPPED_CAP: AtomicU64 = AtomicU64::new(0);
static FOREIGN: AtomicU64 = AtomicU64::new(0);
static CORRUPT: AtomicU64 = AtomicU64::new(0);
static BYTES_WRITTEN: AtomicU64 = AtomicU64::new(0);

/// Process-wide counters of the relation caches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RelationCacheStatistics {
    pub shipped_hits: u64,
    pub cache_hits: u64,
    pub misses: u64,
    pub inserts: u64,
    pub inserts_skipped_cap: u64,
    pub foreign: u64,
    pub corrupt: u64,
    pub bytes_written: u64,
}

impl RelationCacheStatistics {
    /// `self - before` (the counters of one request, approximate when
    /// several requests run at once in the process).
    pub fn since(&self, before: &Self) -> Self {
        Self {
            shipped_hits: self.shipped_hits - before.shipped_hits,
            cache_hits: self.cache_hits - before.cache_hits,
            misses: self.misses - before.misses,
            inserts: self.inserts - before.inserts,
            inserts_skipped_cap: self.inserts_skipped_cap - before.inserts_skipped_cap,
            foreign: self.foreign - before.foreign,
            corrupt: self.corrupt - before.corrupt,
            bytes_written: self.bytes_written - before.bytes_written,
        }
    }
}

pub fn relation_cache_statistics() -> RelationCacheStatistics {
    RelationCacheStatistics {
        shipped_hits: SHIPPED_HITS.load(Ordering::Relaxed),
        cache_hits: CACHE_HITS.load(Ordering::Relaxed),
        misses: MISSES.load(Ordering::Relaxed),
        inserts: INSERTS.load(Ordering::Relaxed),
        inserts_skipped_cap: SKIPPED_CAP.load(Ordering::Relaxed),
        foreign: FOREIGN.load(Ordering::Relaxed),
        corrupt: CORRUPT.load(Ordering::Relaxed),
        bytes_written: BYTES_WRITTEN.load(Ordering::Relaxed),
    }
}

// ----- the store -----

/// Where a found entry came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoreSource {
    Shipped,
    Cache,
}

#[derive(Clone, Debug)]
struct Located {
    path: PathBuf,
    offset: u64,
    len: usize,
    rank: (u8, i64, i64),
    digest: String,
    writable: bool,
}

#[derive(Debug)]
struct Layer {
    directory: PathBuf,
    writable: bool,
}

/// The relation entries of one node over its cache roots.
#[derive(Debug)]
pub struct RelationStore {
    node: String,
    node_identity: String,
    layers: Vec<Layer>,
    index: OnceLock<Mutex<HashMap<Vec<i64>, Located>>>,
    writer: Mutex<Option<(PathBuf, File)>>,
    disk_bytes: AtomicU64,
    max_bytes: Option<u64>,
    allow_field: bool,
}

/// The directory of a node under a cache root.
pub fn node_directory(root: &Path, node: &str, node_identity: &str) -> PathBuf {
    let safe: String = node
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_') {
                c
            } else {
                '-'
            }
        })
        .collect();
    root.join(format!(
        "{safe}-{}",
        &node_identity[..16.min(node_identity.len())]
    ))
}

impl RelationStore {
    /// The store of `node` over read-only `shipped` roots and an optional
    /// writable `cache` root. Nothing is read before the first lookup.
    pub fn new(node: &str, node_identity: &str, shipped: &[PathBuf], cache: Option<&Path>) -> Self {
        Self::with_options(node, node_identity, shipped, cache, StoreOptions::default())
    }

    /// [`Self::new`] with explicit settings (`None` fields: the environment).
    pub fn with_options(
        node: &str,
        node_identity: &str,
        shipped: &[PathBuf],
        cache: Option<&Path>,
        options: StoreOptions,
    ) -> Self {
        let mut layers: Vec<Layer> = shipped
            .iter()
            .map(|root| Layer {
                directory: node_directory(root, node, node_identity),
                writable: false,
            })
            .collect();
        if let Some(root) = cache {
            layers.push(Layer {
                directory: node_directory(root, node, node_identity),
                writable: true,
            });
        }
        let max_bytes = options.max_bytes.or_else(max_bytes_from_env);
        let allow_field = options.allow_field.unwrap_or_else(allow_field_from_env);
        Self {
            node: node.to_owned(),
            node_identity: node_identity.to_owned(),
            layers,
            index: OnceLock::new(),
            writer: Mutex::new(None),
            disk_bytes: AtomicU64::new(0),
            max_bytes,
            allow_field,
        }
    }

    /// The layer directories, with whether each is writable.
    pub fn layers(&self) -> Vec<(PathBuf, bool)> {
        self.layers
            .iter()
            .map(|layer| (layer.directory.clone(), layer.writable))
            .collect()
    }

    /// Bytes of the node directories on disk (at load, plus what this
    /// process wrote).
    pub fn bytes_on_disk(&self) -> u64 {
        self.index();
        self.disk_bytes.load(Ordering::Relaxed)
    }

    fn index(&self) -> &Mutex<HashMap<Vec<i64>, Located>> {
        self.index.get_or_init(|| Mutex::new(self.load()))
    }

    /// Scan every layer: the best valid entry per key; a later (more
    /// writable) layer replaces an earlier one only with strictly better
    /// evidence (the package's choice stands).
    fn load(&self) -> HashMap<Vec<i64>, Located> {
        let mut index: HashMap<Vec<i64>, Located> = HashMap::new();
        let mut bytes = 0u64;
        for layer in &self.layers {
            let Ok(listing) = std::fs::read_dir(&layer.directory) else {
                continue;
            };
            let mut files: Vec<PathBuf> = listing
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
                .collect();
            // merged files first, then segments in name (time) order
            files.sort_by_key(|path| {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                (!name.starts_with("merged-"), name)
            });
            let mut own: HashMap<Vec<i64>, Located> = HashMap::new();
            for path in files {
                let Ok(file) = File::open(&path) else {
                    continue;
                };
                bytes += file.metadata().map_or(0, |m| m.len());
                let mut reader = BufReader::new(file);
                let mut offset = 0u64;
                let mut line = Vec::new();
                loop {
                    line.clear();
                    let Ok(read) = reader.read_until(b'\n', &mut line) else {
                        break;
                    };
                    if read == 0 {
                        break;
                    }
                    let start = offset;
                    offset += read as u64;
                    if line.last() != Some(&b'\n') {
                        CORRUPT.fetch_add(1, Ordering::Relaxed); // torn last line
                        break;
                    }
                    let Ok(entry) = serde_json::from_slice::<Value>(&line[..line.len() - 1]) else {
                        CORRUPT.fetch_add(1, Ordering::Relaxed);
                        continue;
                    };
                    let key = match check_entry_with(&entry, &self.node_identity, self.allow_field)
                    {
                        Ok(key) => key,
                        Err(EntryRefusal::Foreign) => {
                            FOREIGN.fetch_add(1, Ordering::Relaxed);
                            continue;
                        }
                        Err(_) => {
                            CORRUPT.fetch_add(1, Ordering::Relaxed);
                            continue;
                        }
                    };
                    let located = Located {
                        path: path.clone(),
                        offset: start,
                        len: read - 1,
                        rank: rank(&entry),
                        digest: entry["entry_digest"].as_str().unwrap_or("").to_owned(),
                        writable: layer.writable,
                    };
                    let better = own.get(&key).is_none_or(|old| {
                        located.rank > old.rank
                            || (located.rank == old.rank && located.digest < old.digest)
                    });
                    if better {
                        own.insert(key, located);
                    }
                }
            }
            for (key, located) in own {
                let replace = index
                    .get(&key)
                    .is_none_or(|old| located.rank.0 > old.rank.0);
                if replace {
                    index.insert(key, located);
                }
            }
        }
        self.disk_bytes.store(bytes, Ordering::Relaxed);
        index
    }

    /// The entry of `key_table` (TABLE frame), checked again on read.
    pub fn get(&self, key_table: &[i64]) -> Option<(Value, StoreSource)> {
        let located = self
            .index()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(key_table)
            .cloned();
        let Some(located) = located else {
            MISSES.fetch_add(1, Ordering::Relaxed);
            return None;
        };
        let entry = read_line(&located.path, located.offset, located.len)
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .filter(|entry| {
                check_entry_with(entry, &self.node_identity, self.allow_field).as_deref()
                    == Ok(key_table)
            });
        match entry {
            Some(entry) => {
                let source = if located.writable {
                    CACHE_HITS.fetch_add(1, Ordering::Relaxed);
                    StoreSource::Cache
                } else {
                    SHIPPED_HITS.fetch_add(1, Ordering::Relaxed);
                    StoreSource::Shipped
                };
                Some((entry, source))
            }
            None => {
                CORRUPT.fetch_add(1, Ordering::Relaxed);
                MISSES.fetch_add(1, Ordering::Relaxed);
                None
            }
        }
    }

    /// Append `entry` to this process's segment of the writable layer
    /// (a no-op without one, or above the size cap). One line, one write,
    /// then fsync.
    pub fn put(&self, entry: &Value) -> Result<(), String> {
        let Some(layer) = self.layers.iter().find(|layer| layer.writable) else {
            return Ok(());
        };
        let key: Vec<i64> =
            serde_json::from_value(entry["key"].clone()).map_err(|e| e.to_string())?;
        let mut line = serde_json::to_vec(entry).map_err(|e| e.to_string())?;
        line.push(b'\n');
        let current = self.bytes_on_disk();
        if self
            .max_bytes
            .is_some_and(|cap| current + line.len() as u64 > cap)
        {
            SKIPPED_CAP.fetch_add(1, Ordering::Relaxed);
            return Ok(());
        }
        let mut writer = self
            .writer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if writer.is_none() {
            *writer = Some(self.open_segment(&layer.directory)?);
        }
        let (path, file) = writer.as_mut().expect("opened");
        let offset = file.seek(SeekFrom::End(0)).map_err(|e| e.to_string())?;
        file.write_all(&line).map_err(|e| e.to_string())?;
        file.sync_data().map_err(|e| e.to_string())?;
        INSERTS.fetch_add(1, Ordering::Relaxed);
        BYTES_WRITTEN.fetch_add(line.len() as u64, Ordering::Relaxed);
        self.disk_bytes
            .fetch_add(line.len() as u64, Ordering::Relaxed);
        let located = Located {
            path: path.clone(),
            offset,
            len: line.len() - 1,
            rank: rank(entry),
            digest: entry["entry_digest"].as_str().unwrap_or("").to_owned(),
            writable: true,
        };
        let mut index = self
            .index()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let replace = index.get(&key).is_none_or(|old| {
            if old.writable {
                located.rank > old.rank
            } else {
                located.rank.0 > old.rank.0
            }
        });
        if replace {
            index.insert(key, located);
        }
        Ok(())
    }

    fn open_segment(&self, directory: &Path) -> Result<(PathBuf, File), String> {
        std::fs::create_dir_all(directory).map_err(|e| format!("{}: {e}", directory.display()))?;
        if let Some(root) = directory.parent() {
            write_new(
                &root.join("RELATION_CACHE.json"),
                &json!({"schema": RELATION_CACHE_SCHEMA, "created": utc_now()}),
            );
        }
        write_new(
            &directory.join("NODE.json"),
            &json!({"node": self.node, "node_identity": self.node_identity}),
        );
        let host = std::fs::read_to_string("/proc/sys/kernel/hostname")
            .map(|text| text.trim().to_owned())
            .unwrap_or_else(|_| "host".to_owned());
        let stamp = utc_now().replace([':', '-'], "");
        for _ in 0..16 {
            let tag: u32 = rand::random();
            let path = directory.join(format!(
                "seg-{stamp}-{host}-{}-{:06x}.jsonl",
                std::process::id(),
                tag & 0xff_ffff
            ));
            match OpenOptions::new().create_new(true).append(true).open(&path) {
                Ok(file) => return Ok((path, file)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(format!("{}: {error}", path.display())),
            }
        }
        Err("no free segment name".to_owned())
    }
}

/// Write `value` to `path` only if it does not exist (tmp + rename).
fn write_new(path: &Path, value: &Value) {
    if path.exists() {
        return;
    }
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    if std::fs::write(&tmp, format!("{value}\n")).is_ok() && !path.exists() {
        let _ = std::fs::rename(&tmp, path);
    }
    let _ = std::fs::remove_file(&tmp);
}

fn read_line(path: &Path, offset: u64, len: usize) -> Option<Vec<u8>> {
    let mut file = File::open(path).ok()?;
    file.seek(SeekFrom::Start(offset)).ok()?;
    let mut bytes = vec![0u8; len];
    file.read_exact(&mut bytes).ok()?;
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_the_standard_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    fn body(key: &[i64], coefficient: &str) -> Value {
        json!({"type": "relation", "key": key, "verdict": "SOLVED", "all_lower": true,
               "proof": {"ok": true, "kind": "exact-weights"}, "determinism_id": crate::node::recipe::DETERMINISM_ID,
               "rhs": [["I(1,1)", coefficient, "top"]], "relation_digest": "x"})
    }

    fn meta(identity: &str, pole: Option<u32>) -> EntryMeta {
        EntryMeta {
            node: "N".into(),
            node_identity: identity.into(),
            package_revision: None,
            recipe_id: "r".into(),
            recipe: json!({"id": "r"}),
            recipe_digest: "d".into(),
            relation_rule: "first_admissible".into(),
            evidence_level: "per-key",
            pole_order_at_4: pole,
            regular_alternative: "not_tried",
            outside_certified_set: false,
            written_by: "test".into(),
        }
    }

    /// Segments: written entries are found by a new store (restart), a torn
    /// last line and a foreign node identity are ignored, the better entry
    /// (regular at d = 4) wins, the size cap stops inserts.
    #[test]
    fn segments_reload_across_stores_and_ignore_torn_and_foreign_lines() {
        let root = std::env::temp_dir().join(format!("rr-relentry-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let id = node_identity("I(x)", &json!({"name": "o"}), 2);
        let store = RelationStore::new("N", &id, &[], Some(&root));
        assert!(store.get(&[2, 1]).is_none());
        let pole = build_entry(&meta(&id, Some(1)), &[2, 1], body(&[2, 1], "1/(d-4)"));
        let regular = build_entry(&meta(&id, Some(0)), &[2, 1], body(&[2, 1], "1/(d-3)"));
        store.put(&pole).unwrap();
        store.put(&regular).unwrap();
        assert_eq!(
            store.get(&[2, 1]).unwrap().0["entry_digest"],
            regular["entry_digest"]
        );
        // restart: a new store reads the segment
        let again = RelationStore::new("N", &id, &[], Some(&root));
        let (found, source) = again.get(&[2, 1]).unwrap();
        assert_eq!(source, StoreSource::Cache);
        assert_eq!(found["entry_digest"], regular["entry_digest"]);
        // a torn line and a foreign entry in another segment
        let directory = node_directory(&root, "N", &id);
        let other = node_identity("I(y)", &json!({"name": "o"}), 2);
        let foreign = build_entry(&meta(&other, Some(0)), &[3, 1], body(&[3, 1], "1"));
        std::fs::write(
            directory.join("seg-zz-torn.jsonl"),
            format!("{foreign}\n{{\"schema\": \"rustred.relation-entry.v1\", \"tor"),
        )
        .unwrap();
        let before = relation_cache_statistics();
        let third = RelationStore::new("N", &id, &[], Some(&root));
        assert!(third.get(&[3, 1]).is_none());
        assert!(third.get(&[2, 1]).is_some());
        let after = relation_cache_statistics().since(&before);
        assert!(after.foreign >= 1 && after.corrupt >= 1, "{after:?}");
        // a tampered body is refused
        let mut tampered = regular.clone();
        tampered["body"]["rhs"][0][1] = json!("2");
        assert_eq!(check_entry(&tampered, &id), Err(EntryRefusal::Corrupt));
        // the programmatic cap: no insert above it; field entries refused when not allowed
        let capped = RelationStore::with_options(
            "N",
            &id,
            &[],
            Some(&root),
            StoreOptions {
                max_bytes: Some(1),
                allow_field: Some(false),
            },
        );
        let skipped = relation_cache_statistics().inserts_skipped_cap;
        let disk = bytes_on_disk(&root);
        assert!(disk > 0);
        capped.put(&pole).unwrap();
        assert_eq!(relation_cache_statistics().inserts_skipped_cap, skipped + 1);
        assert_eq!(bytes_on_disk(&root), disk, "nothing written above the cap");
        let mut field = regular.clone();
        field["evidence"]["level"] = json!("none");
        assert_eq!(
            check_entry_with(&field, &id, false),
            Err(EntryRefusal::Field)
        );
        assert!(check_entry_with(&field, &id, true).is_ok());
        // shipped (read-only) first; the cache does not replace it at equal evidence
        let shipped = RelationStore::new("N", &id, std::slice::from_ref(&root), None);
        assert_eq!(shipped.get(&[2, 1]).unwrap().1, StoreSource::Shipped);
        let _ = std::fs::remove_dir_all(&root);
    }
}
