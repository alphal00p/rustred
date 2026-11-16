//! Disk-backed EVIDENCE STORE (interfaces/evidence-store.md; lane t266run): the per-key facts and proofs of big inputs
//! in ONE sorted binary file per evidence set, answered by point lookups through a small in-memory block index.
//!
//! `Evidence` (evidence.rs) keeps every fact and proof in memory; a node with ~10^7 served keys cannot hold that next to
//! its closure under the per-process memory rule. The store keeps the same information per (record_sha256, key):
//! the strongest per-key proof status, the regularity, the usable flag, `n_terms` and the sorted term INDICES (record rhs
//! order, fixed by the record text) whose coefficient is identically zero at the key / whose denominator vanishes.
//! Every term-key statement of an input is checked against OUR instantiation of the record at the key while building
//! (instantiate lines: per term key; compact digest lines: digests -> indices; index-form lines: `terms_fnv64`).
//!
//! A LIBRARY: no process, no clock in any identifying field; nothing node-specific (L8). Results never depend on the
//! sort budget or the thread count.

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap, HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering as AtOrd};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::error::{ImportError, io_err};
use super::evidence::{BANK_CONV_PROOF_SCHEMA, FACT_VERDICT_SCHEMA, is_per_key_status};
use super::index::sample_digest;
use super::lines::{parse_key_text, parse_named_key_text, parse_term_key_digest, term_key_digest};
use super::load::ImportedSet;
use crate::closure::RegFact;
use crate::closure::sha256::Sha256;

/// Schema of the store's `meta.json`.
pub const STORE_SCHEMA: &str = "rustred.campaign.evidence-store.v1";
/// Format version (part of the cache name and of the reuse check).
/// v2 (lane t266run, closeloop4 open item m1 in store mode): entries carry a proof-CONFLICT flag.
pub const STORE_VERSION: u32 = 2;
/// Entries per block of `data.bin`.
pub const BLOCK_ENTRIES: usize = 256;
/// Default sort-buffer budget (bytes).
pub const DEFAULT_BUDGET: usize = 256 << 20;

const F_PROOF: u8 = 1;
const F_FACT: u8 = 2;
const F_USABLE: u8 = 4;
/// The (record, key) has a per-key proof AND a decided contrary verdict (bits 8 / 16 hold the regularity).
const F_CONFLICT: u8 = 32;

/// How a store is built / where it is cached.
#[derive(Clone, Debug)]
pub struct StoreOptions {
    /// Cache directory (the store lives in `dir/<name>.evstore/`).
    pub dir: PathBuf,
    /// Sort-buffer budget in bytes (results never depend on it).
    pub budget_bytes: usize,
    /// Parse / instantiate threads (results never depend on it).
    pub threads: usize,
}

impl StoreOptions {
    pub fn new(dir: impl Into<PathBuf>) -> StoreOptions {
        StoreOptions {
            dir: dir.into(),
            budget_bytes: DEFAULT_BUDGET,
            threads: 4,
        }
    }
}

/// One input file as recorded in `meta.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreInput {
    pub path: String,
    pub size: u64,
    pub mtime_ns: u128,
    pub sample_sha256: String,
    pub sha256: String,
}

/// Counts of a store build (deterministic sums).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreCounts {
    pub lines: u64,
    pub fact_lines: u64,
    pub proof_entries: u64,
    pub other_objects: u64,
    pub foreign: u64,
    pub unbound: u64,
    pub facts_stored: u64,
    pub facts_unusable: u64,
    /// Fact lines whose term statement disagrees with our instantiation (refused, not stored).
    pub terms_mismatch: u64,
    /// Fact lines whose record cannot be instantiated at the key (no rule point / no body): not stored.
    pub not_instantiable: u64,
    /// Instantiate-line terms whose key differs from ours (stored as unknown).
    pub term_key_differs: u64,
    pub proofs_stored: u64,
    pub proofs_per_key: u64,
    pub duplicates_merged: u64,
    /// Entries with a per-key proof AND a decided contrary verdict (STORE_VERSION 2).
    #[serde(default)]
    pub proof_conflicts: u64,
}

/// `meta.json`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoreMeta {
    pub schema: String,
    pub version: u32,
    pub arity: usize,
    pub inputs: Vec<StoreInput>,
    /// record id -> record_sha256 (sorted).
    pub records: Vec<String>,
    /// status id -> (word, per_key, file).
    pub statuses: Vec<(String, bool, String)>,
    pub counts: StoreCounts,
    pub entries: u64,
    pub blocks: u64,
    pub data_sha256: String,
}

/// The fact part of an entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoreFact {
    pub regular: RegFact,
    pub usable: bool,
    pub n_terms: u32,
    pub zero: Box<[u32]>,
    pub unknown: Box<[u32]>,
}

/// One stored (record, key).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct StoreEntry {
    /// Status id (into `meta.statuses`): the strongest status seen (a per-key one wins over any other).
    pub proof: Option<u32>,
    pub fact: Option<StoreFact>,
    /// A per-key proof and a DECIDED contrary verdict were both given for this (record, key) (closeloop4 m1): the key
    /// gets no per-key level from this store (never "the positive one wins").
    pub proof_conflict: bool,
}

static STORE_SEQ: AtomicU64 = AtomicU64::new(1);

struct BlockIdx {
    rec: u32,
    key: Box<[i32]>,
    off: u64,
    len: u32,
}

/// An opened store.
pub struct EvidenceStore {
    id: u64,
    meta: StoreMeta,
    dir: PathBuf,
    data: File,
    index: Vec<BlockIdx>,
    rec_of_sha: HashMap<String, u32>,
    /// `built` | `reused`.
    pub origin: String,
}

/// One cached lookup per store and thread: (store id, record id, key, entry).
type CacheSlot = (u64, u32, Vec<i32>, Option<Arc<StoreEntry>>);
/// Stores with their own slot per thread (an evidence set holds a few stores; more evict round-robin).
const CACHE_SLOTS: usize = 16;
thread_local! {
    static CACHE: RefCell<Vec<CacheSlot>> = const { RefCell::new(Vec::new()) };
}

// ---------------------------------------------------------------- encoding

fn put_var(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return;
        }
        out.push(b | 0x80);
    }
}

fn get_var(b: &[u8], p: &mut usize) -> Option<u64> {
    let mut v = 0u64;
    let mut s = 0u32;
    loop {
        let x = *b.get(*p)?;
        *p += 1;
        v |= ((x & 0x7f) as u64) << s;
        if x & 0x80 == 0 {
            return Some(v);
        }
        s += 7;
        if s > 63 {
            return None;
        }
    }
}

fn zz(x: i32) -> u64 {
    ((x << 1) ^ (x >> 31)) as u32 as u64
}

fn unzz(v: u64) -> i32 {
    let v = v as u32;
    ((v >> 1) as i32) ^ -((v & 1) as i32)
}

fn reg_bits(r: RegFact) -> u8 {
    match r {
        RegFact::Unknown => 0,
        RegFact::Regular => 1,
        RegFact::Pole => 2,
    }
}

fn bits_reg(b: u8) -> RegFact {
    match b {
        1 => RegFact::Regular,
        2 => RegFact::Pole,
        _ => RegFact::Unknown,
    }
}

fn put_list(out: &mut Vec<u8>, v: &[u32]) {
    put_var(out, v.len() as u64);
    let mut prev = 0u32;
    for (i, &x) in v.iter().enumerate() {
        put_var(out, if i == 0 { x as u64 } else { (x - prev) as u64 });
        prev = x;
    }
}

fn get_list(b: &[u8], p: &mut usize) -> Option<Box<[u32]>> {
    let n = get_var(b, p)? as usize;
    let mut v = Vec::with_capacity(n.min(1 << 20));
    let mut prev = 0u32;
    for i in 0..n {
        let d = u32::try_from(get_var(b, p)?).ok()?;
        let x = if i == 0 { d } else { prev.checked_add(d)? };
        v.push(x);
        prev = x;
    }
    Some(v.into_boxed_slice())
}

fn encode_payload(e: &StoreEntry, out: &mut Vec<u8>) {
    let mut flags = 0u8;
    if e.proof.is_some() {
        flags |= F_PROOF;
    }
    if let Some(f) = &e.fact {
        flags |= F_FACT | (reg_bits(f.regular) << 3);
        if f.usable {
            flags |= F_USABLE;
        }
    }
    if e.proof_conflict {
        flags |= F_CONFLICT;
    }
    out.push(flags);
    if let Some(s) = e.proof {
        put_var(out, s as u64);
    }
    if let Some(f) = &e.fact {
        put_var(out, f.n_terms as u64);
        put_list(out, &f.zero);
        put_list(out, &f.unknown);
    }
}

fn decode_payload(b: &[u8], p: &mut usize) -> Option<StoreEntry> {
    let flags = *b.get(*p)?;
    *p += 1;
    let proof = if flags & F_PROOF != 0 {
        Some(u32::try_from(get_var(b, p)?).ok()?)
    } else {
        None
    };
    let fact = if flags & F_FACT != 0 {
        let n_terms = u32::try_from(get_var(b, p)?).ok()?;
        let zero = get_list(b, p)?;
        let unknown = get_list(b, p)?;
        Some(StoreFact {
            regular: bits_reg((flags >> 3) & 3),
            usable: flags & F_USABLE != 0,
            n_terms,
            zero,
            unknown,
        })
    } else {
        None
    };
    Some(StoreEntry {
        proof,
        fact,
        proof_conflict: flags & F_CONFLICT != 0,
    })
}

fn encode_entry(rec: u32, key: &[i32], payload: &[u8], out: &mut Vec<u8>) {
    put_var(out, rec as u64);
    for &x in key {
        put_var(out, zz(x));
    }
    out.extend_from_slice(payload);
}

/// Decode (rec, key) of the next entry and the payload span; `p` moves past the entry.
fn decode_head(b: &[u8], p: &mut usize, arity: usize, key: &mut Vec<i32>) -> Option<u32> {
    let rec = u32::try_from(get_var(b, p)?).ok()?;
    key.clear();
    for _ in 0..arity {
        key.push(unzz(get_var(b, p)?));
    }
    Some(rec)
}

fn skip_payload(b: &[u8], p: &mut usize) -> Option<()> {
    decode_payload(b, p).map(|_| ())
}

fn cmp_rk(a: (u32, &[i32]), b: (u32, &[i32])) -> Ordering {
    a.0.cmp(&b.0).then_with(|| a.1.cmp(b.1))
}

// ---------------------------------------------------------------- merging of two answers for one (record, key)

/// Merge `b` into `a` (deterministic, order-free): the proof with the stronger status (per-key first, then the smaller
/// (word, file)); facts must be equal (else an error naming the pair).
fn merge(
    a: &mut StoreEntry,
    b: StoreEntry,
    statuses: &[(String, bool, String)],
) -> Result<(), String> {
    a.proof_conflict |= b.proof_conflict;
    if let Some(sb) = b.proof {
        a.proof = Some(match a.proof {
            None => sb,
            Some(sa) => {
                let (wa, pa, fa) = &statuses[sa as usize];
                let (wb, pb, fb) = &statuses[sb as usize];
                // closeloop4 m1: a per-key proof beside a DECIDED contrary verdict is a conflict (undecided attempts
                // such as errors / time-outs stand beside a proof)
                if pa != pb && super::evidence::is_decided_status(if *pa { wb } else { wa }) {
                    a.proof_conflict = true;
                }
                let ka = (!pa, wa, fa);
                let kb = (!pb, wb, fb);
                if kb < ka { sb } else { sa }
            }
        });
    }
    if let Some(fb) = b.fact {
        match &a.fact {
            None => a.fact = Some(fb),
            Some(fa) if *fa == fb => {}
            Some(fa) => {
                return Err(format!(
                    "conflicting facts: {fa:?} vs {fb:?} (n_terms / zero / unknown / regularity differ)"
                ));
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- the builder

/// Buffered entries of one sort run: (rec, key offset into `keys`, payload offset, payload length).
#[derive(Default)]
struct Buffer {
    idx: Vec<(u32, u32, u32, u32)>,
    keys: Vec<i32>,
    payload: Vec<u8>,
}

impl Buffer {
    fn bytes(&self) -> usize {
        self.idx.len() * 16 + self.keys.len() * 4 + self.payload.len()
    }
    fn push(&mut self, rec: u32, key: &[i32], e: &StoreEntry) {
        let ko = self.keys.len() as u32;
        self.keys.extend_from_slice(key);
        let po = self.payload.len() as u32;
        encode_payload(e, &mut self.payload);
        let pl = self.payload.len() as u32 - po;
        self.idx.push((rec, ko, po, pl));
    }
}

/// A parsed answer: (record id, key, entry). Produced by the workers, consumed in input order by the builder.
type Parsed = Vec<(u32, Vec<i32>, StoreEntry)>;

/// Status interning shared by the workers: the distinct (status, flag, reason) values and their ids.
type StatusIntern = std::sync::Mutex<(
    Vec<(String, bool, String)>,
    HashMap<(String, bool, String), u32>,
)>;

struct Ctx<'a> {
    set: &'a ImportedSet,
    arity: usize,
    rec_of_sha: &'a HashMap<String, u32>,
    /// record id -> (table, record) in `set`.
    loc: &'a [(usize, usize)],
    /// status interning shared by the workers (under a lock: few distinct values).
    statuses: &'a StatusIntern,
}

fn fnv64(vals: &[i32]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for v in vals {
        for b in v.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

fn json_key(v: &Value, arity: usize) -> Option<Vec<i32>> {
    match v {
        Value::Array(a) => {
            if a.len() != arity {
                return None;
            }
            a.iter()
                .map(|x| x.as_i64().and_then(|y| i32::try_from(y).ok()))
                .collect()
        }
        Value::String(s) if s.contains('=') => parse_named_key_text(s, arity),
        Value::String(s) => parse_key_text(s, arity),
        Value::Object(o) => {
            if let Some(k) = o.get("key") {
                return json_key(k, arity);
            }
            let mut k = vec![i32::MIN; arity];
            for (name, x) in o {
                let i: usize = name.strip_prefix('n')?.parse().ok()?;
                if i >= arity {
                    return None;
                }
                k[i] = i32::try_from(x.as_i64()?).ok()?;
            }
            Some(k)
        }
        _ => None,
    }
}

impl Ctx<'_> {
    fn status(&self, word: String, file: &str) -> u32 {
        let per_key = is_per_key_status(&word);
        let k = (word, per_key, file.to_string());
        let mut g = self.statuses.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(&i) = g.1.get(&k) {
            return i;
        }
        let i = g.0.len() as u32;
        g.0.push(k.clone());
        g.1.insert(k, i);
        i
    }

    fn complete(&self, rec: u32, key: &mut [i32]) -> Result<(), String> {
        if !key.contains(&i32::MIN) {
            return Ok(());
        }
        let (t, r) = self.loc[rec as usize];
        let fixed = &self.set.disp.tables()[t].domains[r].fixed;
        for (i, x) in key.iter_mut().enumerate() {
            if *x == i32::MIN {
                *x = fixed[i].ok_or_else(|| format!("key misses the free slot n{i}"))?;
            }
        }
        Ok(())
    }

    fn echo_ok(&self, rec: u32, v: &Value) -> Result<(), String> {
        let echo = v
            .get("index")
            .and_then(Value::as_i64)
            .or_else(|| v.get("rule").and_then(Value::as_i64));
        let (t, r) = self.loc[rec as usize];
        match (echo, self.set.rules[t][r].index) {
            (Some(e), Some(i)) if e != i => Err(format!(
                "the answer names rule index {e}, the record with this record_sha256 has index {i}"
            )),
            _ => Ok(()),
        }
    }

    /// Our instantiated term keys of (record, key), flat (`n_terms * arity`), or None.
    fn terms(&self, rec: u32, key: &[i32]) -> Option<(usize, Vec<i32>)> {
        let (t, r) = self.loc[rec as usize];
        let mut ib = vec![];
        let mut cm = vec![];
        match self.set.bodies.instantiate(
            &self.set.disp,
            self.set.disp.rule_ref(t, r),
            key,
            &mut ib,
            &mut cm,
        ) {
            Ok(Some(n)) => Some((n, ib)),
            _ => None,
        }
    }

    /// One JSON object of an input file -> zero or more entries.
    fn object(
        &self,
        v: &Value,
        file: &str,
        out: &mut Parsed,
        c: &mut StoreCounts,
    ) -> Result<(), String> {
        let schema = v.get("schema").and_then(Value::as_str);
        if schema == Some(FACT_VERDICT_SCHEMA)
            || schema == Some(crate::closure::facts::INSTANTIATE_SCHEMA)
        {
            return self.fact(v, schema == Some(FACT_VERDICT_SCHEMA), out, c);
        }
        let compact_proof = (schema.is_none() || schema == Some(BANK_CONV_PROOF_SCHEMA))
            && v.get("keys").is_none()
            && v.get("record_sha256").is_some_and(Value::is_string)
            && v.get("key").is_some()
            && v.get("verdict").is_some_and(Value::is_string);
        if compact_proof {
            c.proof_entries += 1;
            let sha = v["record_sha256"].as_str().unwrap_or_default();
            let Some(&rec) = self.rec_of_sha.get(sha) else {
                c.foreign += 1;
                return Ok(());
            };
            self.echo_ok(rec, v)?;
            let mut key = json_key(&v["key"], self.arity).ok_or("key")?;
            self.complete(rec, &mut key)?;
            let mut word = v["verdict"]
                .as_str()
                .unwrap_or("")
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string();
            if v.get("sha_ok").is_some_and(|x| x != true)
                || v.get("binds").is_some_and(|x| x != true)
            {
                word = format!("SHA_NOT_OK:{word}");
            }
            let s = self.status(word, file);
            out.push((
                rec,
                key,
                StoreEntry {
                    proof_conflict: false,
                    proof: Some(s),
                    fact: None,
                },
            ));
            return Ok(());
        }
        if !schema.is_some_and(|s| s.starts_with("rustred.proof-kernel.")) {
            c.other_objects += 1;
            return Ok(());
        }
        let Some(keys) = v.get("keys").and_then(Value::as_array) else {
            c.other_objects += 1;
            return Ok(());
        };
        let top = v.get("record_sha256").and_then(Value::as_str);
        for k in keys {
            c.proof_entries += 1;
            let Some(sha) = k.get("record_sha256").and_then(Value::as_str).or(top) else {
                c.unbound += 1;
                continue;
            };
            let Some(&rec) = self.rec_of_sha.get(sha) else {
                c.foreign += 1;
                continue;
            };
            let mut key = k
                .get("key")
                .and_then(|x| json_key(x, self.arity))
                .ok_or("proof entry key")?;
            self.complete(rec, &mut key)?;
            let st = k.get("status").and_then(Value::as_str).unwrap_or("");
            let word = st.split_whitespace().next().unwrap_or("").to_string();
            let s = self.status(word, file);
            out.push((
                rec,
                key,
                StoreEntry {
                    proof_conflict: false,
                    proof: Some(s),
                    fact: None,
                },
            ));
        }
        Ok(())
    }

    fn fact(
        &self,
        v: &Value,
        compact: bool,
        out: &mut Parsed,
        c: &mut StoreCounts,
    ) -> Result<(), String> {
        c.fact_lines += 1;
        let Some(sha) = v.get("record_sha256").and_then(Value::as_str) else {
            c.unbound += 1;
            return Ok(());
        };
        let Some(&rec) = self.rec_of_sha.get(sha) else {
            c.foreign += 1;
            return Ok(());
        };
        self.echo_ok(rec, v)?;
        let mut key = v
            .get("key")
            .and_then(|k| json_key(k, self.arity))
            .ok_or("key")?;
        self.complete(rec, &mut key)?;
        let n = self.arity;
        let (usable, regular) = if compact {
            match v.get("verdict").and_then(Value::as_str).unwrap_or("") {
                "regular" => (true, RegFact::Regular),
                "irregular" => (true, RegFact::Pole),
                "hidden" => (true, RegFact::Unknown),
                _ => (false, RegFact::Unknown),
            }
        } else {
            let usable = v["in_case"] == true
                && v["chart_ok"] == true
                && v.get("branch").is_none_or(Value::is_null)
                && v.get("error").is_none_or(Value::is_null);
            let regular = match (usable, v.get("regular").and_then(Value::as_str)) {
                (true, Some("regular")) => RegFact::Regular,
                (true, Some("irregular")) => RegFact::Pole,
                _ => RegFact::Unknown,
            };
            (usable, regular)
        };
        if !usable {
            c.facts_unusable += 1;
            out.push((
                rec,
                key,
                StoreEntry {
                    proof_conflict: false,
                    proof: None,
                    fact: Some(StoreFact {
                        regular: RegFact::Unknown,
                        usable: false,
                        n_terms: 0,
                        zero: Box::new([]),
                        unknown: Box::new([]),
                    }),
                },
            ));
            return Ok(());
        }
        let Some((nt, ib)) = self.terms(rec, &key) else {
            c.not_instantiable += 1;
            return Ok(());
        };
        let stated_n = v.get("n_terms").and_then(Value::as_u64);
        let mut zero: Vec<u32> = vec![];
        let mut unknown: Vec<u32> = vec![];
        let idx_list = |field: &str| -> Result<Option<Vec<u32>>, String> {
            match v.get(field) {
                None | Some(Value::Null) => Ok(None),
                Some(Value::Array(a)) => a
                    .iter()
                    .map(|x| {
                        x.as_u64()
                            .and_then(|y| u32::try_from(y).ok())
                            .ok_or_else(|| format!("{field}: {x}"))
                    })
                    .collect::<Result<Vec<u32>, String>>()
                    .map(Some),
                Some(o) => Err(format!("{field}: {o}")),
            }
        };
        if compact && v.get("zero_idx").is_some() {
            // index form: the producer's term list must be ours
            if v.get("nonzero").is_some() {
                return Err("index form with a nonzero list".into());
            }
            let fnv = v
                .get("terms_fnv64")
                .and_then(Value::as_str)
                .and_then(|s| u64::from_str_radix(s, 16).ok());
            if stated_n != Some(nt as u64) || fnv != Some(fnv64(&ib)) {
                c.terms_mismatch += 1;
                return Ok(());
            }
            zero = idx_list("zero_idx")?.unwrap_or_default();
            unknown = idx_list("unknown_idx")?.unwrap_or_default();
            zero.sort_unstable();
            zero.dedup();
            unknown.sort_unstable();
            unknown.dedup();
            if zero.last().is_some_and(|&x| x as usize >= nt)
                || unknown.last().is_some_and(|&x| x as usize >= nt)
            {
                return Err("a term index >= n_terms".into());
            }
        } else if compact {
            if stated_n.is_some_and(|s| s != nt as u64) {
                c.terms_mismatch += 1;
                return Ok(());
            }
            let digests = |field: &str| -> Result<HashSet<u64>, String> {
                match v.get(field) {
                    None | Some(Value::Null) => Ok(HashSet::new()),
                    Some(Value::Array(a)) => a
                        .iter()
                        .map(|x| {
                            x.as_str()
                                .and_then(parse_term_key_digest)
                                .ok_or_else(|| format!("{field}: term-key digest {x}"))
                        })
                        .collect(),
                    Some(o) => Err(format!("{field}: {o}")),
                }
            };
            let nz = digests("nonzero")?;
            let un = digests("unknown")?;
            for j in 0..nt {
                let d = term_key_digest(&ib[j * n..(j + 1) * n]);
                if un.contains(&d) {
                    unknown.push(j as u32);
                } else if !nz.contains(&d) {
                    zero.push(j as u32);
                }
            }
        } else {
            // instantiate.v1: per term (index, key, coef); a term key that differs from ours -> unknown
            let mut seen = vec![false; nt];
            for t in v.get("rhs").and_then(Value::as_array).into_iter().flatten() {
                let ti = t.get("term").and_then(Value::as_u64).ok_or("rhs term")? as usize;
                let tk = t.get("key").and_then(|k| json_key(k, n)).ok_or("rhs key")?;
                if ti >= nt {
                    c.terms_mismatch += 1;
                    return Ok(());
                }
                seen[ti] = true;
                if tk.as_slice() != &ib[ti * n..(ti + 1) * n] {
                    c.term_key_differs += 1;
                    unknown.push(ti as u32);
                    continue;
                }
                match t.get("coef").and_then(Value::as_str) {
                    Some("zero") => zero.push(ti as u32),
                    Some("nonzero") => {}
                    _ => unknown.push(ti as u32),
                }
            }
            for (j, s) in seen.iter().enumerate() {
                if !s {
                    unknown.push(j as u32);
                }
            }
            zero.sort_unstable();
            zero.dedup();
            unknown.sort_unstable();
            unknown.dedup();
        }
        c.facts_stored += 1;
        out.push((
            rec,
            key,
            StoreEntry {
                proof_conflict: false,
                proof: None,
                fact: Some(StoreFact {
                    regular,
                    usable: true,
                    n_terms: nt as u32,
                    zero: zero.into_boxed_slice(),
                    unknown: unknown.into_boxed_slice(),
                }),
            },
        ));
        Ok(())
    }
}

fn mtime_ns(md: &std::fs::Metadata) -> u128 {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos())
}

fn canonical(p: &Path) -> Result<PathBuf, ImportError> {
    std::fs::canonicalize(p).map_err(|e| io_err(p, e))
}

/// The cache directory of an input list (order-free).
fn store_dir_for(dir: &Path, inputs: &[PathBuf]) -> Result<PathBuf, ImportError> {
    let mut v: Vec<String> = inputs
        .iter()
        .map(|p| canonical(p).map(|c| c.display().to_string()))
        .collect::<Result<_, _>>()?;
    v.sort();
    let mut h = Sha256::new();
    h.update(format!("{STORE_SCHEMA} v{STORE_VERSION}\n").as_bytes());
    for s in &v {
        h.update(s.as_bytes());
        h.update(b"\n");
    }
    Ok(dir.join(format!("{}.evstore", &h.finish_hex()[..16])))
}

/// procfix7 round 5 (gate9 G9-B1 class): is `p` a directory? Ok(false) when it is PROVEN absent or another kind; a stat
/// error is an error (never "not a directory").
pub(crate) fn is_dir_typed(p: &Path) -> Result<bool, ImportError> {
    match std::fs::metadata(p) {
        Ok(m) => Ok(m.is_dir()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(io_err(p, e)),
    }
}

/// procfix7 round 5: is `p` a regular file (symlinks followed)? As [`is_dir_typed`].
pub(crate) fn is_file_typed(p: &Path) -> Result<bool, ImportError> {
    match std::fs::metadata(p) {
        Ok(m) => Ok(m.is_file()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(io_err(p, e)),
    }
}

/// procfix7 round 5: a listed entry is a regular file: the listing's own type, a symlink followed by a typed stat; an
/// entry whose type cannot be read is an error (never skipped).
pub(crate) fn entry_is_file(e: &std::fs::DirEntry, q: &Path) -> Result<bool, ImportError> {
    let t = e.file_type().map_err(|er| io_err(q, er))?;
    if t.is_symlink() {
        return is_file_typed(q);
    }
    Ok(t.is_file())
}

/// Files of an input path: a file, or the `*.json` / `*.jsonl` files of a directory (sorted, not recursive).
pub fn input_files(p: &Path) -> Result<Vec<PathBuf>, ImportError> {
    if !is_dir_typed(p)? {
        return Ok(vec![p.to_path_buf()]);
    }
    let mut v = vec![];
    for e in std::fs::read_dir(p).map_err(|e| io_err(p, e))? {
        let e = e.map_err(|er| io_err(p, er))?;
        let q = e.path();
        if entry_is_file(&e, &q)?
            && q.extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x == "json" || x == "jsonl")
        {
            v.push(q);
        }
    }
    v.sort();
    Ok(v)
}

struct RunReader {
    rd: BufReader<File>,
    buf: Vec<u8>,
    arity: usize,
}

impl RunReader {
    /// Next (rec, key, entry) of a run file (length-prefixed entries).
    fn next(&mut self) -> Result<Option<(u32, Vec<i32>, StoreEntry)>, String> {
        let mut len = [0u8; 4];
        match self.rd.read_exact(&mut len) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e.to_string()),
        }
        let l = u32::from_le_bytes(len) as usize;
        self.buf.resize(l, 0);
        self.rd
            .read_exact(&mut self.buf)
            .map_err(|e| e.to_string())?;
        let mut p = 0usize;
        let mut key = vec![];
        let rec = decode_head(&self.buf, &mut p, self.arity, &mut key).ok_or("run entry head")?;
        let e = decode_payload(&self.buf, &mut p).ok_or("run entry payload")?;
        Ok(Some((rec, key, e)))
    }
}

struct HeapItem {
    rec: u32,
    key: Vec<i32>,
    run: usize,
    entry: StoreEntry,
}
impl PartialEq for HeapItem {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl Eq for HeapItem {}
impl PartialOrd for HeapItem {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for HeapItem {
    fn cmp(&self, o: &Self) -> Ordering {
        // min-heap on (rec, key, run)
        cmp_rk((o.rec, &o.key), (self.rec, &self.key)).then_with(|| o.run.cmp(&self.run))
    }
}

impl EvidenceStore {
    /// Reuse the store of `inputs` under `opts.dir` when it is complete and describes the inputs as they are now; else
    /// build it there.
    pub fn open_or_build(
        set: &ImportedSet,
        inputs: &[(PathBuf, Option<String>)],
        opts: &StoreOptions,
    ) -> Result<Arc<EvidenceStore>, ImportError> {
        let mut files: Vec<(PathBuf, Option<String>)> = vec![];
        for (p, sha) in inputs {
            let fs = input_files(p)?;
            if is_dir_typed(p)? && sha.is_some() {
                return Err(ImportError::Spec(format!(
                    "{}: a directory has no sha256",
                    p.display()
                )));
            }
            for f in fs {
                files.push((f, sha.clone()));
            }
        }
        let paths: Vec<PathBuf> = files.iter().map(|x| x.0.clone()).collect();
        let dir = store_dir_for(&opts.dir, &paths)?;
        if let Some(s) = Self::validate(&dir, &files, set.arity(), opts.threads)? {
            return Ok(Arc::new(s));
        }
        Self::build(set, &files, opts, &dir)?;
        let mut s = Self::open(&dir)?;
        s.origin = "built".into();
        Ok(Arc::new(s))
    }

    fn validate(
        dir: &Path,
        files: &[(PathBuf, Option<String>)],
        arity: usize,
        threads: usize,
    ) -> Result<Option<EvidenceStore>, ImportError> {
        let Ok(t) = std::fs::read_to_string(dir.join("meta.json")) else {
            return Ok(None);
        };
        let Ok(m) = serde_json::from_str::<StoreMeta>(&t) else {
            return Ok(None);
        };
        if m.schema != STORE_SCHEMA || m.version != STORE_VERSION || m.arity != arity {
            return Ok(None);
        }
        if m.inputs.len() != files.len() {
            return Ok(None);
        }
        let by_path: BTreeMap<&str, &StoreInput> =
            m.inputs.iter().map(|i| (i.path.as_str(), i)).collect();
        for (p, sha) in files {
            let c = canonical(p)?.display().to_string();
            let Some(i) = by_path.get(c.as_str()) else {
                return Ok(None);
            };
            let md = std::fs::metadata(p).map_err(|e| io_err(p, e))?;
            if i.size != md.len() || i.mtime_ns != mtime_ns(&md) {
                return Ok(None);
            }
            if i.sample_sha256 != sample_digest(p)? {
                return Ok(None);
            }
            if let Some(e) = sha {
                let e = e.trim().trim_start_matches("sha256:");
                if !e.eq_ignore_ascii_case(&i.sha256) {
                    return Err(ImportError::Digest {
                        path: p.display().to_string(),
                        expected: e.to_string(),
                        got: i.sha256.clone(),
                    });
                }
            }
        }
        // inputs PINNED by an expected sha256 are re-hashed in full on every reuse (size / mtime / sampled digest prove
        // nothing against an edited line; audit-closeloop M1): parallel over the files
        let pinned: Vec<&(PathBuf, Option<String>)> =
            files.iter().filter(|(_, e)| e.is_some()).collect();
        let next = std::sync::atomic::AtomicUsize::new(0);
        let errs: std::sync::Mutex<Vec<ImportError>> = std::sync::Mutex::new(vec![]);
        std::thread::scope(|sc| {
            for _ in 0..threads.max(1).min(pinned.len().max(1)) {
                sc.spawn(|| {
                    loop {
                        let i = next.fetch_add(1, AtOrd::Relaxed);
                        let Some((p, e)) = pinned.get(i) else {
                            break;
                        };
                        if let Err(er) = super::lines::verify_file(p, e.as_deref()) {
                            errs.lock().unwrap_or_else(|x| x.into_inner()).push(er);
                        }
                    }
                });
            }
        });
        if let Some(e) = errs
            .into_inner()
            .unwrap_or_else(|x| x.into_inner())
            .into_iter()
            .next()
        {
            return Err(e);
        }
        let mut s = Self::open(dir)?;
        s.origin = if pinned.is_empty() {
            "reused".into()
        } else {
            "reused_reverified".into()
        };
        Ok(Some(s))
    }

    /// Open a built store directory.
    pub fn open(dir: &Path) -> Result<EvidenceStore, ImportError> {
        let mp = dir.join("meta.json");
        let t = std::fs::read_to_string(&mp).map_err(|e| io_err(&mp, e))?;
        let meta: StoreMeta = serde_json::from_str(&t).map_err(|e| ImportError::Evidence {
            path: mp.display().to_string(),
            line: 1,
            msg: e.to_string(),
        })?;
        let ip = dir.join("index.bin");
        let ib = std::fs::read(&ip).map_err(|e| io_err(&ip, e))?;
        let a = meta.arity;
        let rec_len = 4 + 4 * a + 8 + 4;
        if ib.len() != rec_len * meta.blocks as usize {
            return Err(ImportError::Evidence {
                path: ip.display().to_string(),
                line: 0,
                msg: "index length".into(),
            });
        }
        let mut index = Vec::with_capacity(meta.blocks as usize);
        for c in ib.chunks_exact(rec_len) {
            let u32at = |o: usize| u32::from_le_bytes(c[o..o + 4].try_into().unwrap_or([0; 4]));
            let rec = u32at(0);
            let key: Box<[i32]> = (0..a).map(|i| u32at(4 + 4 * i) as i32).collect();
            let off = u64::from_le_bytes(c[4 + 4 * a..12 + 4 * a].try_into().unwrap_or([0; 8]));
            let len = u32at(12 + 4 * a);
            index.push(BlockIdx { rec, key, off, len });
        }
        let dp = dir.join("data.bin");
        let data = File::open(&dp).map_err(|e| io_err(&dp, e))?;
        let rec_of_sha = meta
            .records
            .iter()
            .enumerate()
            .map(|(i, s)| (s.clone(), i as u32))
            .collect();
        Ok(EvidenceStore {
            id: STORE_SEQ.fetch_add(1, AtOrd::Relaxed),
            meta,
            dir: dir.to_path_buf(),
            data,
            index,
            rec_of_sha,
            origin: "opened".into(),
        })
    }

    pub fn meta(&self) -> &StoreMeta {
        &self.meta
    }
    /// The store's record id of a record_sha256.
    pub fn rec_id(&self, sha: &str) -> Option<u32> {
        self.rec_of_sha.get(sha).copied()
    }
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The entry of (record_sha256, key), if stored.
    pub fn get(&self, rec_sha: &str, key: &[i32]) -> Option<Arc<StoreEntry>> {
        let rec = *self.rec_of_sha.get(rec_sha)?;
        self.get_rec(rec, key)
    }

    /// The entry of (record id, key) (per-thread one-entry cache).
    pub fn get_rec(&self, rec: u32, key: &[i32]) -> Option<Arc<StoreEntry>> {
        let hit = CACHE.with(|c| {
            c.borrow()
                .iter()
                .find(|(id, r, k, _)| *id == self.id && *r == rec && k.as_slice() == key)
                .map(|(_, _, _, e)| e.clone())
        });
        if let Some(e) = hit {
            return e;
        }
        let e = self.lookup(rec, key).map(Arc::new);
        CACHE.with(|c| {
            let mut c = c.borrow_mut();
            let slot = (self.id, rec, key.to_vec(), e.clone());
            if let Some(i) = c.iter().position(|(id, ..)| *id == self.id) {
                c[i] = slot;
            } else if c.len() < CACHE_SLOTS {
                c.push(slot);
            } else {
                let i = (self.id as usize) % CACHE_SLOTS;
                c[i] = slot;
            }
        });
        e
    }

    fn lookup(&self, rec: u32, key: &[i32]) -> Option<StoreEntry> {
        // last block whose first entry is <= (rec, key)
        let pos = self
            .index
            .partition_point(|b| cmp_rk((b.rec, &b.key), (rec, key)) != Ordering::Greater);
        if pos == 0 {
            return None;
        }
        let b = &self.index[pos - 1];
        let mut buf = vec![0u8; b.len as usize];
        self.data.read_exact_at(&mut buf, b.off).ok()?;
        let mut p = 0usize;
        let mut k = Vec::with_capacity(self.meta.arity);
        while p < buf.len() {
            let r = decode_head(&buf, &mut p, self.meta.arity, &mut k)?;
            match cmp_rk((r, &k), (rec, key)) {
                Ordering::Less => skip_payload(&buf, &mut p)?,
                Ordering::Equal => return decode_payload(&buf, &mut p),
                Ordering::Greater => return None,
            }
        }
        None
    }

    /// Every entry in store order (tests, audits): (record_sha256, key, entry).
    pub fn for_each(
        &self,
        f: &mut dyn FnMut(&str, &[i32], &StoreEntry),
    ) -> Result<(), ImportError> {
        let dp = self.dir.join("data.bin");
        for b in &self.index {
            let mut buf = vec![0u8; b.len as usize];
            self.data
                .read_exact_at(&mut buf, b.off)
                .map_err(|e| io_err(&dp, e))?;
            let mut p = 0usize;
            let mut k = vec![];
            while p < buf.len() {
                let bad = || ImportError::Evidence {
                    path: dp.display().to_string(),
                    line: 0,
                    msg: "corrupt block".into(),
                };
                let r = decode_head(&buf, &mut p, self.meta.arity, &mut k).ok_or_else(bad)?;
                let e = decode_payload(&buf, &mut p).ok_or_else(bad)?;
                f(&self.meta.records[r as usize], &k, &e);
            }
        }
        Ok(())
    }

    fn build(
        set: &ImportedSet,
        files: &[(PathBuf, Option<String>)],
        opts: &StoreOptions,
        dir: &Path,
    ) -> Result<(), ImportError> {
        let arity = set.arity();
        // record ids: the set's records sorted by sha256 (set-independent naming through the sha)
        let mut recs: Vec<(String, (usize, usize))> = set
            .rules
            .iter()
            .enumerate()
            .flat_map(|(t, v)| {
                v.iter()
                    .enumerate()
                    .map(move |(r, i)| (i.record_sha256.clone(), (t, r)))
            })
            .collect();
        recs.sort();
        recs.dedup_by(|a, b| a.0 == b.0);
        let rec_of_sha: HashMap<String, u32> = recs
            .iter()
            .enumerate()
            .map(|(i, (s, _))| (s.clone(), i as u32))
            .collect();
        let loc: Vec<(usize, usize)> = recs.iter().map(|x| x.1).collect();
        let statuses = std::sync::Mutex::new((Vec::new(), HashMap::new()));
        let ctx = Ctx {
            set,
            arity,
            rec_of_sha: &rec_of_sha,
            loc: &loc,
            statuses: &statuses,
        };
        let tmp = dir.with_extension("evstore.tmp");
        // procfix7 round 5: the temporary of an earlier build is removed only when PROVEN present; a stat error is an
        // error (never "absent": a build into a stale temporary would mix its files into the new store)
        match std::fs::symlink_metadata(&tmp) {
            Ok(m) if m.is_dir() => {
                std::fs::remove_dir_all(&tmp).map_err(|e| io_err(&tmp, e))?;
            }
            Ok(_) => {
                return Err(ImportError::Core(format!(
                    "{} exists and is not a directory: refused (nothing removed)",
                    tmp.display()
                )));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(io_err(&tmp, e)),
        }
        std::fs::create_dir_all(&tmp).map_err(|e| io_err(&tmp, e))?;
        let mut counts = StoreCounts::default();
        let mut inputs_meta = vec![];
        let mut runs: Vec<PathBuf> = vec![];
        let mut buf = Buffer::default();
        // pass over the inputs: workers parse batches of lines, the builder consumes them in input order
        for (path, expected) in files {
            let md = std::fs::metadata(path).map_err(|e| io_err(path, e))?;
            let sample = sample_digest(path)?;
            let mut hasher = Sha256::new();
            let file = path.display().to_string();
            let threads = opts.threads.max(1);
            if pretty_single_object(path)? {
                // a pretty-printed single object (an rr_proof span record): read the whole file
                let w = std::fs::read(path).map_err(|e| io_err(path, e))?;
                hasher.update(&w);
                let v: Value = serde_json::from_slice(&w).map_err(|e| ImportError::Evidence {
                    path: file.clone(),
                    line: 1,
                    msg: e.to_string(),
                })?;
                let mut out: Parsed = vec![];
                let mut c = StoreCounts {
                    lines: 1,
                    ..Default::default()
                };
                ctx.object(&v, &file, &mut out, &mut c)
                    .map_err(|m| ImportError::Evidence {
                        path: file.clone(),
                        line: 1,
                        msg: m,
                    })?;
                add_counts(&mut counts, &c);
                for (rec, key, e) in out {
                    buf.push(rec, &key, &e);
                }
            } else {
                let f = File::open(path).map_err(|e| io_err(path, e))?;
                let mut rd = BufReader::with_capacity(1 << 22, f);
                let mut line_no = 0usize;
                loop {
                    // up to `threads` batches of ~8 MiB of whole lines
                    let mut batches: Vec<(usize, Vec<u8>)> = vec![];
                    for _ in 0..threads {
                        let mut b: Vec<u8> = Vec::with_capacity(8 << 20);
                        let start = line_no;
                        while b.len() < (8 << 20) {
                            let n = rd.read_until(b'\n', &mut b).map_err(|e| io_err(path, e))?;
                            if n == 0 {
                                break;
                            }
                            line_no += 1;
                        }
                        if b.is_empty() {
                            break;
                        }
                        hasher.update(&b);
                        batches.push((start, b));
                    }
                    if batches.is_empty() {
                        break;
                    }
                    let results = parse_batches(&ctx, &file, &batches);
                    for r in results {
                        let (parsed, c) = r?;
                        add_counts(&mut counts, &c);
                        for (rec, key, e) in parsed {
                            buf.push(rec, &key, &e);
                        }
                        if buf.bytes() >= opts.budget_bytes.max(1 << 16) {
                            runs.push(write_run(&mut buf, &tmp, runs.len(), arity, &statuses)?);
                        }
                    }
                }
            }
            let sha = hasher.finish_hex();
            if let Some(e) = expected {
                let e = e.trim().trim_start_matches("sha256:");
                if !e.eq_ignore_ascii_case(&sha) {
                    return Err(ImportError::Digest {
                        path: file,
                        expected: e.to_string(),
                        got: sha,
                    });
                }
            }
            inputs_meta.push(StoreInput {
                path: canonical(path)?.display().to_string(),
                size: md.len(),
                mtime_ns: mtime_ns(&md),
                sample_sha256: sample,
                sha256: sha,
            });
        }
        if !buf.idx.is_empty() || runs.is_empty() {
            runs.push(write_run(&mut buf, &tmp, runs.len(), arity, &statuses)?);
        }
        // the statuses in a canonical order (sorted) so that ids do not depend on the parse schedule
        let (st_list, _) = std::mem::take(&mut *statuses.lock().unwrap_or_else(|e| e.into_inner()));
        let mut order: Vec<u32> = (0..st_list.len() as u32).collect();
        order.sort_by(|&a, &b| st_list[a as usize].cmp(&st_list[b as usize]));
        let mut remap = vec![0u32; st_list.len()];
        for (new, &old) in order.iter().enumerate() {
            remap[old as usize] = new as u32;
        }
        let st_sorted: Vec<(String, bool, String)> =
            order.iter().map(|&o| st_list[o as usize].clone()).collect();
        // k-way merge of the runs into data.bin + index.bin
        let dp = tmp.join("data.bin");
        let ip = tmp.join("index.bin");
        let mut dw =
            BufWriter::with_capacity(1 << 22, File::create(&dp).map_err(|e| io_err(&dp, e))?);
        let mut iw =
            BufWriter::with_capacity(1 << 20, File::create(&ip).map_err(|e| io_err(&ip, e))?);
        let mut dh = Sha256::new();
        let mut readers: Vec<RunReader> = vec![];
        for r in &runs {
            let f = File::open(r).map_err(|e| io_err(r, e))?;
            readers.push(RunReader {
                rd: BufReader::with_capacity(1 << 20, f),
                buf: vec![],
                arity,
            });
        }
        let mut heap = BinaryHeap::new();
        for (i, rr) in readers.iter_mut().enumerate() {
            if let Some((rec, key, entry)) = rr.next().map_err(ImportError::Core)? {
                heap.push(HeapItem {
                    rec,
                    key,
                    run: i,
                    entry,
                });
            }
        }
        let mut block: Vec<u8> = vec![];
        let mut block_n = 0usize;
        let mut block_first: Option<(u32, Vec<i32>)> = None;
        let mut off = 0u64;
        let mut entries = 0u64;
        let mut blocks = 0u64;
        let mut proofs_stored = 0u64;
        let mut proofs_per_key = 0u64;
        let mut proof_conflicts = 0u64;
        let mut cur: Option<(u32, Vec<i32>, StoreEntry)> = None;
        let mut payload = vec![];
        let mut flush_entry = |cur: (u32, Vec<i32>, StoreEntry),
                               block: &mut Vec<u8>,
                               block_n: &mut usize,
                               block_first: &mut Option<(u32, Vec<i32>)>|
         -> Result<(), ImportError> {
            let (rec, key, mut e) = cur;
            if e.proof_conflict {
                proof_conflicts += 1;
            }
            if let Some(s) = e.proof {
                let s2 = remap[s as usize];
                e.proof = Some(s2);
                proofs_stored += 1;
                if st_sorted[s2 as usize].1 {
                    proofs_per_key += 1;
                }
            }
            payload.clear();
            encode_payload(&e, &mut payload);
            if block_first.is_none() {
                *block_first = Some((rec, key.clone()));
            }
            encode_entry(rec, &key, &payload, block);
            *block_n += 1;
            entries += 1;
            if *block_n >= BLOCK_ENTRIES {
                write_block(
                    &mut dw,
                    &mut iw,
                    &mut dh,
                    block,
                    block_first,
                    &mut off,
                    arity,
                )
                .map_err(|e| io_err(&dp, e))?;
                blocks += 1;
                *block_n = 0;
            }
            Ok(())
        };
        while let Some(it) = heap.pop() {
            if let Some((rec, key, entry)) = readers[it.run].next().map_err(ImportError::Core)? {
                heap.push(HeapItem {
                    rec,
                    key,
                    run: it.run,
                    entry,
                });
            }
            match cur.as_mut() {
                Some((r, k, e)) if *r == it.rec && *k == it.key => {
                    counts.duplicates_merged += 1;
                    merge(e, it.entry, &st_list).map_err(|m| ImportError::Evidence {
                        path: dir.display().to_string(),
                        line: 0,
                        msg: format!("record {} key {:?}: {m}", recs[it.rec as usize].0, it.key),
                    })?;
                }
                _ => {
                    if let Some(c) = cur.take() {
                        flush_entry(c, &mut block, &mut block_n, &mut block_first)?;
                    }
                    cur = Some((it.rec, it.key, it.entry));
                }
            }
        }
        if let Some(c) = cur.take() {
            flush_entry(c, &mut block, &mut block_n, &mut block_first)?;
        }
        if block_n > 0 {
            write_block(
                &mut dw,
                &mut iw,
                &mut dh,
                &mut block,
                &mut block_first,
                &mut off,
                arity,
            )
            .map_err(|e| io_err(&dp, e))?;
            blocks += 1;
        }
        dw.flush().map_err(|e| io_err(&dp, e))?;
        iw.flush().map_err(|e| io_err(&ip, e))?;
        drop(dw);
        drop(iw);
        drop(readers);
        for r in &runs {
            let _ = std::fs::remove_file(r);
        }
        counts.proofs_stored = proofs_stored;
        counts.proofs_per_key = proofs_per_key;
        counts.proof_conflicts = proof_conflicts;
        let meta = StoreMeta {
            schema: STORE_SCHEMA.into(),
            version: STORE_VERSION,
            arity,
            inputs: inputs_meta,
            records: recs.into_iter().map(|x| x.0).collect(),
            statuses: st_sorted,
            counts,
            entries,
            blocks,
            data_sha256: dh.finish_hex(),
        };
        let mp = tmp.join("meta.json");
        std::fs::write(
            &mp,
            serde_json::to_string_pretty(&meta).map_err(|e| ImportError::Core(e.to_string()))?,
        )
        .map_err(|e| io_err(&mp, e))?;
        // procfix7 (destructive action, positive proof): an existing store directory is replaced only when, read back
        // now, it is a directory whose meta.json names this store schema; a stat error or anything else is refused
        match std::fs::symlink_metadata(dir) {
            Ok(m) if m.is_dir() => {
                let meta = dir.join("meta.json");
                let ours = std::fs::read(&meta)
                    .map_err(|e| io_err(&meta, e))
                    .map(|b| {
                        serde_json::from_slice::<serde_json::Value>(&b)
                            .is_ok_and(|v| v["schema"] == STORE_SCHEMA)
                    })?;
                if !ours {
                    return Err(ImportError::Core(format!(
                        "{} is not an evidence store of this schema: refused (nothing removed)",
                        dir.display()
                    )));
                }
                std::fs::remove_dir_all(dir).map_err(|e| io_err(dir, e))?;
            }
            Ok(_) => {
                return Err(ImportError::Core(format!(
                    "{} exists and is not a directory: refused (nothing removed)",
                    dir.display()
                )));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(io_err(dir, e)),
        }
        std::fs::rename(&tmp, dir).map_err(|e| io_err(dir, e))?;
        Ok(())
    }
}

/// True when the file's first non-empty line is not a JSON value but starts an object (a pretty-printed object).
fn pretty_single_object(path: &Path) -> Result<bool, ImportError> {
    let f = File::open(path).map_err(|e| io_err(path, e))?;
    let mut rd = BufReader::with_capacity(1 << 20, f);
    let mut l: Vec<u8> = vec![];
    loop {
        l.clear();
        if rd.read_until(b'\n', &mut l).map_err(|e| io_err(path, e))? == 0 {
            return Ok(false);
        }
        let t = l.trim_ascii();
        if t.is_empty() {
            continue;
        }
        return Ok(t.starts_with(b"{") && serde_json::from_slice::<Value>(t).is_err());
    }
}

/// Parse batches of JSONL text in parallel (one scoped thread per batch); results in batch order.
fn parse_batches(
    ctx: &Ctx<'_>,
    file: &str,
    batches: &[(usize, Vec<u8>)],
) -> Vec<Result<(Parsed, StoreCounts), ImportError>> {
    std::thread::scope(|s| {
        let hs: Vec<_> = batches
            .iter()
            .map(|(start, b)| {
                s.spawn(move || {
                    let mut out: Parsed = vec![];
                    let mut c = StoreCounts::default();
                    for (i, l) in b.split(|&x| x == b'\n').enumerate() {
                        let t = l.trim_ascii();
                        if t.is_empty() {
                            continue;
                        }
                        c.lines += 1;
                        let line = start + i + 1;
                        let v: Value =
                            serde_json::from_slice(t).map_err(|e| ImportError::Evidence {
                                path: file.to_string(),
                                line,
                                msg: format!("json: {e}"),
                            })?;
                        ctx.object(&v, file, &mut out, &mut c).map_err(|m| {
                            ImportError::Evidence {
                                path: file.to_string(),
                                line,
                                msg: m,
                            }
                        })?;
                    }
                    Ok((out, c))
                })
            })
            .collect();
        hs.into_iter()
            .map(|h| {
                h.join()
                    .unwrap_or_else(|_| Err(ImportError::Core("store worker panicked".into())))
            })
            .collect()
    })
}

fn add_counts(a: &mut StoreCounts, b: &StoreCounts) {
    a.lines += b.lines;
    a.fact_lines += b.fact_lines;
    a.proof_entries += b.proof_entries;
    a.other_objects += b.other_objects;
    a.foreign += b.foreign;
    a.unbound += b.unbound;
    a.facts_stored += b.facts_stored;
    a.facts_unusable += b.facts_unusable;
    a.terms_mismatch += b.terms_mismatch;
    a.not_instantiable += b.not_instantiable;
    a.term_key_differs += b.term_key_differs;
}

fn write_block(
    dw: &mut BufWriter<File>,
    iw: &mut BufWriter<File>,
    dh: &mut Sha256,
    block: &mut Vec<u8>,
    first: &mut Option<(u32, Vec<i32>)>,
    off: &mut u64,
    arity: usize,
) -> std::io::Result<()> {
    let (rec, key) = first.take().unwrap_or((0, vec![0; arity]));
    dw.write_all(block)?;
    dh.update(block);
    iw.write_all(&rec.to_le_bytes())?;
    for x in &key {
        iw.write_all(&x.to_le_bytes())?;
    }
    iw.write_all(&off.to_le_bytes())?;
    iw.write_all(&(block.len() as u32).to_le_bytes())?;
    *off += block.len() as u64;
    block.clear();
    Ok(())
}

/// Sort the buffer (merging equal (rec, key) entries) and write it as a run file of length-prefixed entries.
fn write_run(
    buf: &mut Buffer,
    tmp: &Path,
    n: usize,
    arity: usize,
    statuses: &StatusIntern,
) -> Result<PathBuf, ImportError> {
    let p = tmp.join(format!("run-{n:05}.bin"));
    let keys = &buf.keys;
    let a = arity;
    buf.idx.sort_by(|x, y| {
        cmp_rk(
            (x.0, &keys[x.1 as usize..x.1 as usize + a]),
            (y.0, &keys[y.1 as usize..y.1 as usize + a]),
        )
        .then_with(|| {
            buf.payload[x.2 as usize..(x.2 + x.3) as usize]
                .cmp(&buf.payload[y.2 as usize..(y.2 + y.3) as usize])
        })
    });
    let st = statuses.lock().unwrap_or_else(|e| e.into_inner()).0.clone();
    let mut w = BufWriter::with_capacity(1 << 20, File::create(&p).map_err(|e| io_err(&p, e))?);
    let mut i = 0usize;
    let mut out = vec![];
    let mut pl = vec![];
    while i < buf.idx.len() {
        let (rec, ko, po, ln) = buf.idx[i];
        let key = &keys[ko as usize..ko as usize + a];
        let mut q = po as usize;
        let mut e = decode_payload(&buf.payload, &mut q)
            .ok_or_else(|| ImportError::Core("buffer payload".into()))?;
        let _ = ln;
        let mut j = i + 1;
        while j < buf.idx.len() {
            let (r2, k2, p2, _) = buf.idx[j];
            if r2 != rec || &keys[k2 as usize..k2 as usize + a] != key {
                break;
            }
            let mut q2 = p2 as usize;
            let e2 = decode_payload(&buf.payload, &mut q2)
                .ok_or_else(|| ImportError::Core("buffer payload".into()))?;
            merge(&mut e, e2, &st).map_err(|m| ImportError::Evidence {
                path: p.display().to_string(),
                line: 0,
                msg: format!("record id {rec} key {key:?}: {m}"),
            })?;
            j += 1;
        }
        pl.clear();
        encode_payload(&e, &mut pl);
        out.clear();
        encode_entry(rec, key, &pl, &mut out);
        w.write_all(&(out.len() as u32).to_le_bytes())
            .and_then(|_| w.write_all(&out))
            .map_err(|e| io_err(&p, e))?;
        i = j;
    }
    w.flush().map_err(|e| io_err(&p, e))?;
    *buf = Buffer::default();
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_and_zigzag_round_trip() {
        let mut b = vec![];
        for v in [0u64, 1, 127, 128, 300, u32::MAX as u64, u64::MAX >> 1] {
            put_var(&mut b, v);
        }
        let mut p = 0;
        for v in [0u64, 1, 127, 128, 300, u32::MAX as u64, u64::MAX >> 1] {
            assert_eq!(get_var(&b, &mut p), Some(v));
        }
        for x in [0, 1, -1, 31, -32, i32::MAX, i32::MIN] {
            assert_eq!(unzz(zz(x)), x);
        }
        let e = StoreEntry {
            proof_conflict: false,
            proof: Some(3),
            fact: Some(StoreFact {
                regular: RegFact::Pole,
                usable: true,
                n_terms: 1000,
                zero: vec![1, 5, 999].into_boxed_slice(),
                unknown: vec![7].into_boxed_slice(),
            }),
        };
        let mut o = vec![];
        encode_payload(&e, &mut o);
        let mut p = 0;
        assert_eq!(decode_payload(&o, &mut p), Some(e));
        assert_eq!(p, o.len());
    }
}
