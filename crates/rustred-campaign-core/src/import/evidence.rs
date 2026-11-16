//! Evidence import (interfaces/import.md section 5): per-key facts, per-key proofs and solved recipe relations, each
//! bound EXACTLY to (record_sha256 | recipe source, key). Lines that name no imported record are counted `foreign`,
//! lines without a record digest `unbound`; neither is ever used. Conflicting answers are an error (fail closed).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::BufRead;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use super::error::{ImportError, io_err};
use super::lines::{
    parse_key_text, parse_named_key_text, parse_term_key_digest, term_key_digest, verify_file,
};
use super::load::ImportedSet;
use super::spec::FileRef;
use crate::closure::{CoefFact, FactSource, RegFact, RuleId};

/// Schema of a COMPACT fact verdict line (interfaces/import.md 5.5): one line per (record, key) with the verdict and
/// the term-key digests of the NON-ZERO terms only.
pub const FACT_VERDICT_SCHEMA: &str = "rustred.campaign.fact-verdict.v1";

/// Coefficient facts of the terms of one (rule, key).
#[derive(Clone, Debug, PartialEq, Eq)]
enum Terms {
    /// `rustred.proof-kernel.instantiate.v1`: per term index (hash of the instantiated term key, fact).
    Full(Vec<(u64, CoefFact)>),
    /// Compact verdict line: sorted term-key digests of the NON-ZERO terms, of the terms whose coefficient is unknown
    /// (denominator identically zero), and the number of terms of the record (checked against the closure's body).
    NonZero {
        nonzero: Box<[u64]>,
        unknown: Box<[u64]>,
        n_terms: Option<u32>,
    },
}

/// A fact read from an attached evidence store (lane t266run, interfaces/evidence-store.md): term INDICES (record rhs
/// order) whose coefficient is zero / unknown; every other index < n_terms is non-zero. The store checked the term list
/// against our instantiation when it was built.
fn store_coefficient(f: &super::store::StoreFact, term: usize) -> CoefFact {
    if term >= f.n_terms as usize {
        return CoefFact::Unknown;
    }
    let t = term as u32;
    if f.unknown.binary_search(&t).is_ok() {
        CoefFact::Unknown
    } else if f.zero.binary_search(&t).is_ok() {
        CoefFact::Zero
    } else {
        CoefFact::NonZero
    }
}

/// The fact of (rule, key): from memory, from a store, or both (lane t266run).
enum FactRef<'a> {
    Mem(&'a FactRec),
    Store(std::sync::Arc<super::store::StoreEntry>),
    /// Both exist: every answer is the common one, else Unknown (fail closed).
    Both(&'a FactRec, std::sync::Arc<super::store::StoreEntry>),
}

impl FactRef<'_> {
    fn sf(e: &super::store::StoreEntry) -> &super::store::StoreFact {
        e.fact.as_ref().expect("store fact present")
    }
    fn regular(&self) -> RegFact {
        match self {
            FactRef::Mem(f) => f.regular,
            FactRef::Store(e) => {
                let f = Self::sf(e);
                if f.usable {
                    f.regular
                } else {
                    RegFact::Unknown
                }
            }
            FactRef::Both(m, e) => {
                let f = Self::sf(e);
                let s = if f.usable {
                    f.regular
                } else {
                    RegFact::Unknown
                };
                if s == m.regular { s } else { RegFact::Unknown }
            }
        }
    }
    fn lift(&self) -> bool {
        match self {
            FactRef::Mem(f) => f.lift,
            FactRef::Store(e) => {
                let f = Self::sf(e);
                f.usable && f.regular == RegFact::Regular
            }
            FactRef::Both(m, e) => {
                let f = Self::sf(e);
                m.lift && f.usable && f.regular == RegFact::Regular
            }
        }
    }
    fn coefficient(&self, term: usize, term_key: &[i32]) -> CoefFact {
        match self {
            FactRef::Mem(f) => f.coefficient(term, term_key),
            FactRef::Store(e) => {
                let f = Self::sf(e);
                if f.usable {
                    store_coefficient(f, term)
                } else {
                    CoefFact::Unknown
                }
            }
            FactRef::Both(m, e) => {
                let f = Self::sf(e);
                let a = m.coefficient(term, term_key);
                let b = if f.usable {
                    store_coefficient(f, term)
                } else {
                    CoefFact::Unknown
                };
                if a == b { a } else { CoefFact::Unknown }
            }
        }
    }
}

/// Options of [`Evidence::from_spec_with`].
#[derive(Clone, Debug, Default)]
pub struct EvidenceOptions {
    /// Some: the spec's facts and proofs files go into ONE disk-backed store (interfaces/evidence-store.md).
    pub store: Option<super::store::StoreOptions>,
}

/// An attached store: the store, store record id by our rule index, store status id -> our interned status.
struct AttachedStore {
    store: std::sync::Arc<super::store::EvidenceStore>,
    rec_of_ri: Vec<Option<u32>>,
    status_map: Vec<u32>,
}

/// Counts of the attached stores (statement).
#[derive(Clone, Debug, Default, Serialize)]
pub struct StoreUse {
    pub dir: String,
    pub origin: String,
    pub entries: u64,
    pub data_sha256: String,
    pub records_bound: usize,
    pub counts: super::store::StoreCounts,
    pub inputs: Vec<(String, String)>,
}

/// One usable fact: (rule, key) -> regularity, lift and per-term coefficient facts.
#[derive(Clone, Debug, PartialEq, Eq)]
struct FactRec {
    regular: RegFact,
    /// in case, chart ok, no branch, no error, regular.
    lift: bool,
    terms: Terms,
    /// lane closeloop3: who produced the fact (the FACT BASIS of the keys it decides; not part of the fact itself).
    producer: &'static str,
}

/// lane closeloop3 (interfaces/closeloop.md 16): the fact basis word of a fact line: `rr_proof` (rr_proof instantiate
/// answers, or compact lines stating `producer: rr_proof`), `exact_eval` (compact lines of the exact evaluation of the
/// record's own coefficient text, `rr_coef_facts`), `compact` (a compact line that states no known producer).
fn fact_producer(v: &Value, compact: bool) -> &'static str {
    if !compact {
        return "rr_proof";
    }
    match v.get("producer").and_then(Value::as_str) {
        Some("exact_eval") => "exact_eval",
        Some("rr_proof") => "rr_proof",
        // lane closeloop4: rr_proof instantiate `--elided-loci facts` (coefficient facts of an elided record only)
        Some("rr_proof_elided") => "rr_proof_elided",
        _ => "compact",
    }
}

impl FactRec {
    /// The coefficient fact of term `term` (instantiated key `term_key`). A term whose coefficient is identically zero
    /// in d at the key is NOT an edge (the closure drops `Zero` terms in Tier C): in the compact form every term whose
    /// digest is not listed among the non-zero (or unknown) ones is `Zero`.
    fn coefficient(&self, term: usize, term_key: &[i32]) -> CoefFact {
        match &self.terms {
            Terms::Full(v) => match v.get(term) {
                Some(&(h, c)) if h == term_hash(term_key) => c,
                _ => CoefFact::Unknown,
            },
            Terms::NonZero {
                nonzero,
                unknown,
                n_terms,
            } => {
                if n_terms.is_some_and(|n| term >= n as usize) {
                    return CoefFact::Unknown;
                }
                let d = term_key_digest(term_key);
                if unknown.binary_search(&d).is_ok() {
                    CoefFact::Unknown
                } else if nonzero.binary_search(&d).is_ok() {
                    CoefFact::NonZero
                } else {
                    CoefFact::Zero
                }
            }
        }
    }
}

/// Per-key proof status of (rule, key) (interned: one value per distinct (status word, file)).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct ProofStatus {
    pub status: String,
    pub per_key: bool,
    pub file: String,
}

/// Schema of bank-conv's bound proof lines (coord-bank-tables.md v1.4): a compact proof verdict line with `binds`.
pub const BANK_CONV_PROOF_SCHEMA: &str = "rustred.bank-conv.proof.v1";

/// Status words that give the level `per-key` (rr_proof instance-span / instance-exact, per key).
pub fn is_per_key_status(word: &str) -> bool {
    word == "EXACT_IN_SPAN_AT_KEY" || word == "EXACT_PROVED_AT_KEY"
}

/// lane closeloop4 (audit-closeloop m1): a proof status word that DECIDES something about the key (not an error, a
/// time-out, a budget stop, an unsupported input or an undecided attempt).
pub fn is_decided_status(word: &str) -> bool {
    let w = word.to_ascii_uppercase();
    ![
        "ERROR",
        "TIMEOUT",
        "TIME_OUT",
        "UNDECIDED",
        "NOT_DECIDED",
        "BUDGET",
        "SKIP",
        "UNSUPPORTED",
        "REFUSED",
        "UNKNOWN",
    ]
    .iter()
    .any(|x| w.contains(x))
}

/// One solved relation for (recipe source, key).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SolvedRelation {
    pub source: String,
    pub key: Vec<i32>,
    pub verdict: String,
    pub recipe_sha256: Option<String>,
    pub recipe_digest: Option<String>,
    pub determinism_id: Option<String>,
    pub binary_sha256: Option<String>,
    pub relation_digest: Option<String>,
    pub rhs: Vec<Vec<i32>>,
    pub proof_ok: bool,
    pub regular: bool,
    pub file: String,
}

/// Counts of the evidence import (reported in statements).
#[derive(Clone, Debug, Default, Serialize)]
pub struct EvidenceCounts {
    pub fact_lines: usize,
    pub facts_bound: usize,
    pub facts_unusable: usize,
    pub facts_foreign: usize,
    pub facts_unbound: usize,
    pub fact_other_lines: usize,
    pub proof_entries: usize,
    pub proofs_bound: usize,
    pub proofs_per_key: usize,
    pub proofs_foreign: usize,
    pub proofs_unbound: usize,
    pub proof_refusals: usize,
    /// lane closeloop4 (audit-closeloop m1): (record, key) pairs with contradicting proof verdicts (no per-key level),
    /// in memory (lane t266run: the stores count their own, `stores[].counts.proof_conflicts`).
    pub proof_conflicts: usize,
    /// Compact proof verdict lines (`record_sha256`, `key`, `verdict`; no `schema`) read.
    pub proof_verdict_lines: usize,
    /// Compact fact verdict lines (`rustred.campaign.fact-verdict.v1`) read.
    pub fact_verdict_lines: usize,
    /// JSON objects in proof files that are not rr_proof records (e.g. a sample list): counted, not read.
    pub proof_other_objects: usize,
    pub solved_lines: usize,
    pub solved_bound: usize,
    pub solved_foreign: usize,
    pub files: Vec<(String, String)>,
    /// Up to [`FOREIGN_EXAMPLES`] foreign record digests (facts / proofs) for diagnosis.
    pub foreign_examples: Vec<String>,
    /// Attached evidence stores (lane t266run).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stores: Vec<StoreUse>,
}

/// The imported evidence of one set.
#[derive(Default)]
pub struct Evidence {
    rule_index: HashMap<(String, i64), u32>,
    /// lane t266run: the same index as table name / path -> label -> rule (looked up without allocating: the closure
    /// asks per term).
    rule_index2: HashMap<String, HashMap<i64, u32>>,
    sha_index: HashMap<String, u32>,
    facts: HashMap<(u32, Vec<i32>), FactRec>,
    /// (rule, key) -> index into `statuses` (compact: no per-entry strings).
    proofs: HashMap<(u32, Box<[i32]>), u32>,
    statuses: Vec<ProofStatus>,
    status_index: HashMap<ProofStatus, u32>,
    /// rule -> its `index` field (checked against an answer's `index` / `rule` echo).
    rule_record_index: Vec<Option<i64>>,
    pub solved: BTreeMap<String, BTreeMap<Vec<i32>, SolvedRelation>>,
    /// lane closeloop2 (closeloop.md 13.6): Some = a recipe-served key is `per-key` only when its (recipe source, key)
    /// is listed (an exact per-key proof is bound to the relation's point record), else `imported`; None = as before.
    pub recipe_proofs: Option<std::collections::HashSet<(String, Vec<i32>)>>,
    pub counts: EvidenceCounts,
    label: String,
    fact_files: Vec<String>,
    /// lane closeloop4 (closeloop.md 18): (rule, key) pairs whose coefficient facts are DISPUTED (the two independent
    /// routes disagree): every term is `Unknown` there (a term is an edge unless its zero is confirmed; the key's
    /// row is never certified).
    disputed: std::collections::HashSet<(u32, Vec<i32>)>,
    /// lane closeloop4 (audit-closeloop M1): set by [`Evidence::accept_ledger_verified_files`] AFTER the spec's evidence
    /// was loaded (pinned): later `load_*` calls may name files the caller verified against its ledger.
    ledger_verified: bool,
    /// lane closeloop4 (audit-closeloop m1): (rule, key) with contradicting proof verdicts.
    proof_conflicts: std::collections::HashSet<(u32, Box<[i32]>)>,
    /// lane t266run: disk-backed evidence stores consulted when the maps above miss.
    stores: Vec<AttachedStore>,
    /// rule index -> record_sha256.
    rule_sha: Vec<String>,
    /// lane t266run: the conflicted (rule, key) pairs a check ASKED for a per-key level (each denied it): the
    /// node's verdict names them (`contradicting_proofs`).
    conflicts_used: std::sync::Mutex<BTreeSet<(u32, Vec<i32>)>>,
}

/// How many foreign record digests are kept as examples (diagnosis only).
pub const FOREIGN_EXAMPLES: usize = 20;

fn term_hash(k: &[i32]) -> u64 {
    // FNV-1a over the i32 LE bytes (a consistency check of the term key; the term index pins the term).
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for x in k {
        for b in x.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

fn json_key(v: &Value, arity: usize) -> Option<Vec<i32>> {
    match v {
        Value::Array(_) => serde_json::from_value::<Vec<i32>>(v.clone())
            .ok()
            .filter(|k| k.len() == arity),
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

/// Schema of a JSON evidence manifest (lane closeloop4, audit-closeloop M1).
pub const EVIDENCE_MANIFEST_SCHEMA: &str = "rustred.campaign.evidence-manifest.v1";

/// lane closeloop4 (audit-closeloop M1): the PINNED files of an evidence reference, each verified against its pin
/// BEFORE use: a file entry needs its `sha256`; a directory entry needs a `manifest` (itself pinned by sha256) that
/// lists every `*.json` / `*.jsonl` file of the directory with its sha256 (and size, JSON form); an unlisted file, a
/// listed file that is missing, a size or digest mismatch refuses. Sorted by path. This is the ONLY way spec evidence
/// is read.
pub fn resolve_evidence(base: &Path, r: &FileRef) -> Result<Vec<(PathBuf, String)>, ImportError> {
    let p = super::spec::resolve(base, &r.path);
    // procfix7 round 5 (gate9 G9-B1 class): a stat error is an error, never "not a directory" / "not a file"
    if !super::store::is_dir_typed(&p)? {
        let Some(want) = r.sha256.as_deref() else {
            return Err(ImportError::Spec(format!(
                "evidence file {} is not pinned: give its sha256 (unpinned evidence is never read)",
                p.display()
            )));
        };
        let d = verify_file(&p, Some(want))?;
        return Ok(vec![(p, d)]);
    }
    if r.sha256.is_some() {
        return Err(ImportError::Spec(format!(
            "{}: a directory has no sha256: name its manifest (`manifest: {{path, sha256}}`)",
            p.display()
        )));
    }
    let Some(m) = r.manifest.as_deref() else {
        return Err(ImportError::Spec(format!(
            "evidence directory {} is not pinned: give its manifest (`manifest: {{path, sha256}}`, e.g. its SHA256SUMS)",
            p.display()
        )));
    };
    let mp = super::spec::resolve(base, &m.path);
    let Some(mwant) = m.sha256.as_deref() else {
        return Err(ImportError::Spec(format!(
            "manifest {} of evidence directory {} is not pinned: give its sha256",
            mp.display(),
            p.display()
        )));
    };
    verify_file(&mp, Some(mwant))?;
    // name -> (sha256, optional size)
    let mtext = std::fs::read_to_string(&mp).map_err(|e| io_err(&mp, e))?;
    let mut listed: BTreeMap<String, (String, Option<u64>)> = BTreeMap::new();
    if let Ok(v) = serde_json::from_str::<Value>(&mtext)
        && v.is_object()
    {
        if v["schema"] != EVIDENCE_MANIFEST_SCHEMA {
            return Err(ImportError::Spec(format!(
                "manifest {}: schema {} (expected {EVIDENCE_MANIFEST_SCHEMA})",
                mp.display(),
                v["schema"]
            )));
        }
        for f in v["files"].as_array().into_iter().flatten() {
            let (Some(name), Some(sha)) = (f["path"].as_str(), f["sha256"].as_str()) else {
                return Err(ImportError::Spec(format!(
                    "manifest {}: entry {f}",
                    mp.display()
                )));
            };
            listed.insert(name.to_string(), (sha.to_string(), f["bytes"].as_u64()));
        }
    } else {
        for (name, sha) in super::lines::read_sha256sums(&mp)? {
            listed.insert(name.trim_start_matches('*').to_string(), (sha, None));
        }
    }
    let mut v = vec![];
    for e in std::fs::read_dir(&p).map_err(|e| io_err(&p, e))? {
        let e = e.map_err(|er| io_err(&p, er))?;
        let q = e.path();
        // procfix7 round 5: the entry's type from the listing (a symlink is followed by a TYPED stat): an entry whose
        // type cannot be read fails the listing (it was skipped: an unlisted file escaped the manifest check)
        let ok = super::store::entry_is_file(&e, &q)?
            && q.extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x == "json" || x == "jsonl");
        if ok && q != mp {
            v.push(q);
        }
    }
    v.sort();
    let mut seen = std::collections::BTreeSet::new();
    let mut out = vec![];
    for q in v {
        let name = q
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let Some((sha, bytes)) = listed.get(&name) else {
            return Err(ImportError::Spec(format!(
                "evidence file {} is not listed in the manifest {} (an unlisted file is never read)",
                q.display(),
                mp.display()
            )));
        };
        if let Some(b) = bytes {
            let got = std::fs::metadata(&q).map_err(|e| io_err(&q, e))?.len();
            if got != *b {
                return Err(ImportError::Spec(format!(
                    "evidence file {}: {got} bytes, the manifest says {b}",
                    q.display()
                )));
            }
        }
        let d = verify_file(&q, Some(sha))?;
        seen.insert(name);
        out.push((q, d));
    }
    for name in listed.keys() {
        let is_ev = name.ends_with(".json") || name.ends_with(".jsonl");
        if is_ev && !seen.contains(name) && p.join(name) != mp {
            return Err(ImportError::Spec(format!(
                "evidence file {name} listed in {} is missing from {}",
                mp.display(),
                p.display()
            )));
        }
    }
    Ok(out)
}

/// The files of an evidence reference: spec evidence through [`resolve_evidence`] (pinned, the only mode); files the
/// CALLER verified against its own record (`ledger_verified`: the closing loop's answer files, each checked against
/// the BLAKE3 its ledger recorded right before) are read as given.
fn evidence_files(
    base: &Path,
    r: &FileRef,
    ledger_verified: bool,
) -> Result<Vec<(PathBuf, String)>, ImportError> {
    if ledger_verified && r.sha256.is_none() && r.manifest.is_none() {
        let p = super::spec::resolve(base, &r.path);
        if super::store::is_file_typed(&p)? {
            let d = verify_file(&p, None)?;
            return Ok(vec![(p, d)]);
        }
    }
    resolve_evidence(base, r)
}

/// Visit the objects of a file one at a time: JSONL (one object per line; streamed, bounded memory) or one JSON object
/// spread over several lines (pretty-printed; read whole).
fn each_object(
    path: &Path,
    f: &mut dyn FnMut(usize, Value) -> Result<(), ImportError>,
) -> Result<(), ImportError> {
    let bad = |line: usize, m: String| ImportError::Evidence {
        path: path.display().to_string(),
        line,
        msg: m,
    };
    let file = std::fs::File::open(path).map_err(|e| io_err(path, e))?;
    let mut rd = std::io::BufReader::with_capacity(1 << 20, file);
    let mut buf = String::new();
    let mut line = 0usize;
    let mut first = true;
    loop {
        buf.clear();
        let n = rd.read_line(&mut buf).map_err(|e| io_err(path, e))?;
        if n == 0 {
            return Ok(());
        }
        line += 1;
        let t = buf.trim();
        if t.is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(t) {
            Ok(v) => {
                first = false;
                f(line, v)?;
            }
            Err(_) if first && t.starts_with('{') => {
                // a pretty-printed single object: read the whole file
                let txt = std::fs::read_to_string(path).map_err(|e| io_err(path, e))?;
                let v: Value = serde_json::from_str(&txt).map_err(|e| bad(1, e.to_string()))?;
                return f(1, v);
            }
            Err(e) => return Err(bad(line, e.to_string())),
        }
    }
}

impl Evidence {
    /// Empty evidence for a set (rule index built from the set's records).
    pub fn new(set: &ImportedSet) -> Evidence {
        let mut ev = Evidence::default();
        let mut n = 0u32;
        for t in &set.rules {
            for r in t {
                ev.rule_index.insert((r.path.clone(), r.label), n);
                ev.rule_index.insert((r.table_name.clone(), r.label), n);
                ev.rule_index2
                    .entry(r.path.clone())
                    .or_default()
                    .insert(r.label, n);
                ev.rule_index2
                    .entry(r.table_name.clone())
                    .or_default()
                    .insert(r.label, n);
                ev.sha_index.insert(r.record_sha256.clone(), n);
                ev.rule_record_index.push(r.index);
                ev.rule_sha.push(r.record_sha256.clone());
                n += 1;
            }
        }
        ev.label = "import:none".into();
        ev
    }

    /// Load everything the spec names; with `opts.store` the facts and proofs files go into ONE disk-backed store
    /// (built or reused under the store directory) instead of memory (lane t266run).
    pub fn from_spec_with(
        set: &ImportedSet,
        base: &Path,
        opts: &EvidenceOptions,
    ) -> Result<Evidence, ImportError> {
        let Some(so) = &opts.store else {
            return Self::from_spec(set, base);
        };
        Self::check_families(set)?;
        let mut ev = Evidence::new(set);
        let sp = set.spec.evidence.clone();
        // lane closeloop4 (audit-closeloop M1): every store input is a PINNED file (manifests resolved, verified)
        let mut inputs: Vec<(PathBuf, Option<String>)> = vec![];
        for r in sp.facts.iter().chain(sp.proofs.iter()) {
            for (p, d) in resolve_evidence(base, r)? {
                inputs.push((p, Some(d)));
            }
        }
        if !inputs.is_empty() {
            let st = super::store::EvidenceStore::open_or_build(set, &inputs, so)?;
            ev.attach_store(st);
        }
        for r in &sp.solved {
            ev.load_solved(set, base, r)?;
        }
        Ok(ev)
    }

    /// Attach a disk-backed store: consulted for every (rule, key) the in-memory maps do not answer.
    pub fn attach_store(&mut self, st: std::sync::Arc<super::store::EvidenceStore>) {
        let m = st.meta();
        let rec_of_ri: Vec<Option<u32>> = self.rule_sha.iter().map(|s| st.rec_id(s)).collect();
        let status_map: Vec<u32> = m
            .statuses
            .iter()
            .map(|(w, pk, f)| {
                self.intern(ProofStatus {
                    status: w.clone(),
                    per_key: *pk,
                    file: f.clone(),
                })
            })
            .collect();
        self.counts.stores.push(StoreUse {
            dir: st.dir().display().to_string(),
            origin: st.origin.clone(),
            entries: m.entries,
            data_sha256: m.data_sha256.clone(),
            records_bound: rec_of_ri.iter().filter(|x| x.is_some()).count(),
            counts: m.counts.clone(),
            inputs: m
                .inputs
                .iter()
                .map(|i| (i.path.clone(), format!("sha256:{}", i.sha256)))
                .collect(),
        });
        if m.counts.facts_stored > 0 {
            self.fact_files.push(format!("store:{}", m.data_sha256));
            self.label = format!(
                "import:facts:{}",
                crate::closure::sha256::sha256_tag(self.fact_files.join(",").as_bytes())
            );
        }
        self.counts.proofs_per_key += m.counts.proofs_per_key as usize;
        self.stores.push(AttachedStore {
            store: st,
            rec_of_ri,
            status_map,
        });
    }

    /// The store entries of (rule index, key): the first store with a fact, the strongest proof over all stores.
    fn store_fact(&self, ri: u32, key: &[i32]) -> Option<std::sync::Arc<super::store::StoreEntry>> {
        for a in &self.stores {
            if let Some(rec) = a.rec_of_ri.get(ri as usize).copied().flatten()
                && let Some(e) = a.store.get_rec(rec, key)
                && e.fact.is_some()
            {
                return Some(e);
            }
        }
        None
    }

    /// Our interned status of the strongest store proof of (rule index, key).
    fn store_proof(&self, ri: u32, key: &[i32]) -> Option<u32> {
        let mut best: Option<u32> = None;
        for a in &self.stores {
            if let Some(rec) = a.rec_of_ri.get(ri as usize).copied().flatten()
                && let Some(e) = a.store.get_rec(rec, key)
                && let Some(s) = e.proof
            {
                let mine = a.status_map[s as usize];
                best = Some(match best {
                    Some(b)
                        if self.statuses[b as usize].per_key
                            || !self.statuses[mine as usize].per_key =>
                    {
                        b
                    }
                    _ => mine,
                });
            }
        }
        best
    }

    /// The fact of (rule index, key) from memory and / or the stores.
    fn fact_ref(&self, ri: u32, key: &[i32]) -> Option<FactRef<'_>> {
        // (no allocation when nothing is in memory: the store-backed case asks per term)
        let m = if self.facts.is_empty() {
            None
        } else {
            self.facts.get(&(ri, key.to_vec()))
        };
        let s = if self.stores.is_empty() {
            None
        } else {
            self.store_fact(ri, key)
        };
        match (m, s) {
            (Some(m), Some(s)) => Some(FactRef::Both(m, s)),
            (Some(m), None) => Some(FactRef::Mem(m)),
            (None, Some(s)) => Some(FactRef::Store(s)),
            (None, None) => None,
        }
    }

    /// The proof status index of (rule index, key): memory first (the strongest is kept there), else the stores; a
    /// per-key status wins.
    fn proof_index(&self, ri: u32, key: &[i32]) -> Option<u32> {
        let m = if self.proofs.is_empty() {
            None
        } else {
            self.proofs.get(&(ri, Box::<[i32]>::from(key))).copied()
        };
        if m.is_some_and(|i| self.statuses[i as usize].per_key) || self.stores.is_empty() {
            return m;
        }
        match (m, self.store_proof(ri, key)) {
            (Some(a), Some(b)) => Some(if self.statuses[b as usize].per_key {
                b
            } else {
                a
            }),
            (a, b) => a.or(b),
        }
    }

    /// lane closeloop4 (audit-closeloop m4): every spec evidence entry names the family it was produced for, and it is
    /// the set's.
    pub fn check_families(set: &ImportedSet) -> Result<(), ImportError> {
        let e = &set.spec.evidence;
        for r in e.facts.iter().chain(e.proofs.iter()).chain(e.solved.iter()) {
            match (&r.family, &set.family) {
                (Some(a), Some(b)) if a == b => {}
                (Some(a), b) => {
                    return Err(ImportError::Spec(format!(
                        "evidence {} was produced for family {a}, the set's tables are of family {}: refused",
                        r.path,
                        b.as_deref().unwrap_or("(none)")
                    )));
                }
                (None, _) => {
                    return Err(ImportError::Spec(format!(
                        "evidence {}: name the family it was produced for (`family`): evidence binds to a record of \
                         that family only",
                        r.path
                    )));
                }
            }
        }
        Ok(())
    }

    /// Load everything the spec names.
    pub fn from_spec(set: &ImportedSet, base: &Path) -> Result<Evidence, ImportError> {
        Self::check_families(set)?;
        let mut ev = Evidence::new(set);
        let sp = set.spec.evidence.clone();
        for r in &sp.facts {
            ev.load_facts(set, base, r)?;
        }
        for r in &sp.proofs {
            ev.load_proofs(set, base, r)?;
        }
        for r in &sp.solved {
            ev.load_solved(set, base, r)?;
        }
        Ok(ev)
    }

    fn foreign(&mut self, sha: &str) {
        if self.counts.foreign_examples.len() < FOREIGN_EXAMPLES
            && !self.counts.foreign_examples.iter().any(|s| s == sha)
        {
            self.counts.foreign_examples.push(sha.to_string());
        }
    }

    /// Facts: `rustred.proof-kernel.instantiate.v1` lines (every term with its coefficient word) and COMPACT
    /// `rustred.campaign.fact-verdict.v1` lines (the term-key digests of the NON-ZERO terms only), each with
    /// `record_sha256`; streamed one line at a time.
    pub fn load_facts(
        &mut self,
        set: &ImportedSet,
        base: &Path,
        r: &FileRef,
    ) -> Result<(), ImportError> {
        let arity = set.arity();
        for (path, digest) in evidence_files(base, r, self.ledger_verified)? {
            self.counts
                .files
                .push((path.display().to_string(), format!("sha256:{digest}")));
            self.fact_files.push(digest.clone());
            each_object(&path, &mut |line, v| {
                let bad = |m: String| ImportError::Evidence {
                    path: path.display().to_string(),
                    line,
                    msg: m,
                };
                let schema = v.get("schema").and_then(Value::as_str);
                let compact = schema == Some(FACT_VERDICT_SCHEMA);
                if schema != Some(crate::closure::facts::INSTANTIATE_SCHEMA) && !compact {
                    self.counts.fact_other_lines += 1;
                    return Ok(());
                }
                self.counts.fact_lines += 1;
                if compact {
                    self.counts.fact_verdict_lines += 1;
                }
                let Some(sha) = v.get("record_sha256").and_then(Value::as_str) else {
                    self.counts.facts_unbound += 1;
                    return Ok(());
                };
                let Some(&ri) = self.sha_index.get(sha) else {
                    self.counts.facts_foreign += 1;
                    let s = sha.to_string();
                    self.foreign(&s);
                    return Ok(());
                };
                self.check_index_echo(ri, &v).map_err(&bad)?;
                let mut key = v
                    .get("key")
                    .and_then(|k| json_key(k, arity))
                    .ok_or_else(|| bad("key".into()))?;
                self.complete_key(set, ri, &mut key).map_err(&bad)?;
                let (usable, regular, terms) = if compact {
                    let verdict = v.get("verdict").and_then(Value::as_str).unwrap_or("");
                    let (usable, regular) = match verdict {
                        "regular" => (true, RegFact::Regular),
                        "irregular" => (true, RegFact::Pole),
                        "hidden" => (true, RegFact::Unknown),
                        _ => (false, RegFact::Unknown),
                    };
                    let digests = |field: &str| -> Result<Box<[u64]>, ImportError> {
                        let mut d: Vec<u64> = match v.get(field) {
                            None | Some(Value::Null) => vec![],
                            Some(Value::Array(a)) => a
                                .iter()
                                .map(|x| {
                                    x.as_str()
                                        .and_then(parse_term_key_digest)
                                        .ok_or_else(|| bad(format!("{field}: term-key digest {x}")))
                                })
                                .collect::<Result<_, _>>()?,
                            Some(o) => return Err(bad(format!("{field}: {o}"))),
                        };
                        d.sort_unstable();
                        d.dedup();
                        Ok(d.into_boxed_slice())
                    };
                    let terms = Terms::NonZero {
                        nonzero: if usable {
                            digests("nonzero")?
                        } else {
                            Box::new([])
                        },
                        unknown: if usable {
                            digests("unknown")?
                        } else {
                            Box::new([])
                        },
                        n_terms: v.get("n_terms").and_then(Value::as_u64).map(|n| n as u32),
                    };
                    (
                        usable,
                        regular,
                        if usable { terms } else { Terms::Full(vec![]) },
                    )
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
                    let mut terms = vec![];
                    if usable {
                        for t in v.get("rhs").and_then(Value::as_array).into_iter().flatten() {
                            let ti = t
                                .get("term")
                                .and_then(Value::as_u64)
                                .ok_or_else(|| bad("rhs term".into()))?
                                as usize;
                            let tk = t
                                .get("key")
                                .and_then(|k| json_key(k, arity))
                                .ok_or_else(|| bad("rhs key".into()))?;
                            let c = match t.get("coef").and_then(Value::as_str) {
                                Some("zero") => CoefFact::Zero,
                                Some("nonzero") => CoefFact::NonZero,
                                _ => CoefFact::Unknown,
                            };
                            if terms.len() <= ti {
                                terms.resize(ti + 1, (0, CoefFact::Unknown));
                            }
                            terms[ti] = (term_hash(&tk), c);
                        }
                    }
                    (usable, regular, Terms::Full(terms))
                };
                if !usable {
                    self.counts.facts_unusable += 1;
                }
                let rec = FactRec {
                    regular,
                    lift: usable && regular == RegFact::Regular,
                    terms,
                    producer: fact_producer(&v, compact),
                };
                match self.facts.get(&(ri, key.clone())) {
                    // two producers agreeing on the same fact are no conflict (the first one stays the basis)
                    Some(old)
                        if (old.regular, old.lift, &old.terms)
                            != (rec.regular, rec.lift, &rec.terms) =>
                    {
                        return Err(bad(format!(
                            "conflicting facts for record {sha} at {key:?}"
                        )));
                    }
                    Some(_) => {}
                    None => {
                        self.facts.insert((ri, key), rec);
                        self.counts.facts_bound += 1;
                    }
                }
                Ok(())
            })?;
        }
        // lane closeloop4 (audit-closeloop m9): the label names the SET of fact files (sorted digests), never their load
        // order (= job completion order in the closing loop)
        let mut sorted = self.fact_files.clone();
        sorted.sort();
        sorted.dedup();
        self.label = format!(
            "import:facts:{}",
            crate::closure::sha256::sha256_tag(sorted.join(",").as_bytes())
        );
        Ok(())
    }

    /// An answer's `index` (or `rule`) echo must equal the record's `index` when both exist (fail closed).
    fn check_index_echo(&self, ri: u32, v: &Value) -> Result<(), String> {
        let echo = v
            .get("index")
            .and_then(Value::as_i64)
            .or_else(|| v.get("rule").and_then(Value::as_i64));
        match (
            echo,
            self.rule_record_index.get(ri as usize).copied().flatten(),
        ) {
            (Some(e), Some(i)) if e != i => Err(format!(
                "the answer names rule index {e}, the record with this record_sha256 has index {i}"
            )),
            _ => Ok(()),
        }
    }

    /// Complete the slots a key text did not name from the record's fixed slots (refused if a free slot is missing).
    fn complete_key(&self, set: &ImportedSet, ri: u32, key: &mut [i32]) -> Result<(), String> {
        if !key.contains(&i32::MIN) {
            return Ok(());
        }
        let (t, rr) = locate(set, ri);
        let fixed = &set.disp.tables()[t].domains[rr].fixed;
        for (i, x) in key.iter_mut().enumerate() {
            if *x == i32::MIN {
                *x = fixed[i].ok_or_else(|| format!("key misses the free slot n{i}"))?;
            }
        }
        Ok(())
    }

    fn intern(&mut self, ps: ProofStatus) -> u32 {
        if let Some(&i) = self.status_index.get(&ps) {
            return i;
        }
        let i = self.statuses.len() as u32;
        self.statuses.push(ps.clone());
        self.status_index.insert(ps, i);
        i
    }

    /// Record one per-key proof answer for (rule, key): the strongest is kept (a per-key proof wins over an undecided
    /// attempt; never weakened).
    fn put_proof(&mut self, ri: u32, key: Vec<i32>, word: String, file: &str) {
        let per_key = is_per_key_status(&word);
        let ps = ProofStatus {
            status: word,
            per_key,
            file: file.to_string(),
        };
        let si = self.intern(ps);
        self.counts.proofs_bound += 1;
        let k = (ri, key.into_boxed_slice());
        // lane closeloop4 (audit-closeloop m1): a per-key proof and a DECIDED contrary verdict for the same (record,
        // key) (e.g. IRREGULAR_KEY) are a CONFLICT: the key gets no per-key level (never "the positive one wins");
        // undecided attempts (errors, time-outs, refusals) stand beside a proof
        if let Some(&old) = self.proofs.get(&k) {
            let o = &self.statuses[old as usize];
            let n = &self.statuses[si as usize];
            if o.per_key != n.per_key
                && is_decided_status(if o.per_key { &n.status } else { &o.status })
            {
                self.counts.proof_conflicts += 1;
                self.proof_conflicts.insert(k.clone());
            }
        }
        match self.proofs.get(&k) {
            Some(&old) if self.statuses[old as usize].per_key && !per_key => {}
            _ => {
                self.proofs.insert(k, si);
            }
        }
    }

    /// Per-key proofs: rr_proof span / exact records (schema `rustred.proof-kernel.*`, `keys[]` entries), whole-file
    /// JSON or JSONL, and COMPACT proof verdict lines (one per (record, key): `record_sha256`, `key` (array, `I(..)` or
    /// rr_proof's `n0=..` text), `verdict` = the status word, optional `index` echo, optional `sha_ok`); a directory of
    /// them. Streamed one line at a time.
    pub fn load_proofs(
        &mut self,
        set: &ImportedSet,
        base: &Path,
        r: &FileRef,
    ) -> Result<(), ImportError> {
        let arity = set.arity();
        for (path, digest) in evidence_files(base, r, self.ledger_verified)? {
            self.counts
                .files
                .push((path.display().to_string(), format!("sha256:{digest}")));
            let file = path.display().to_string();
            each_object(&path, &mut |line, v| {
                let bad = |m: String| ImportError::Evidence {
                    path: file.clone(),
                    line,
                    msg: m,
                };
                let schema = v.get("schema").and_then(Value::as_str);
                let compact = (schema.is_none() || schema == Some(BANK_CONV_PROOF_SCHEMA))
                    && v.get("keys").is_none()
                    && v.get("record_sha256").is_some_and(Value::is_string)
                    && v.get("key").is_some()
                    && v.get("verdict").is_some_and(Value::is_string);
                if compact {
                    self.counts.proof_verdict_lines += 1;
                    self.counts.proof_entries += 1;
                    let sha = v["record_sha256"].as_str().unwrap_or_default();
                    let Some(&ri) = self.sha_index.get(sha) else {
                        self.counts.proofs_foreign += 1;
                        let s = sha.to_string();
                        self.foreign(&s);
                        return Ok(());
                    };
                    self.check_index_echo(ri, &v).map_err(&bad)?;
                    let mut key = json_key(&v["key"], arity).ok_or_else(|| bad("key".into()))?;
                    self.complete_key(set, ri, &mut key).map_err(&bad)?;
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
                        // the producer itself says the line it proved is not this record's byte copy
                        self.counts.proof_refusals += 1;
                        word = format!("SHA_NOT_OK:{word}");
                    }
                    self.put_proof(ri, key, word, &file);
                    return Ok(());
                }
                // only rr_proof records (schema rustred.proof-kernel.*): other JSON (sample lists, group files) is counted
                if !schema.is_some_and(|s| s.starts_with("rustred.proof-kernel.")) {
                    self.counts.proof_other_objects += 1;
                    return Ok(());
                }
                let Some(keys) = v.get("keys").and_then(Value::as_array) else {
                    if v.get("verdict").and_then(Value::as_str).is_some_and(|s| {
                        s == "ERROR" || s == "UNSUPPORTED" || s.starts_with("MISSING")
                    }) {
                        self.counts.proof_refusals += 1;
                    }
                    return Ok(());
                };
                let top_sha = v
                    .get("record_sha256")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                for k in keys {
                    self.counts.proof_entries += 1;
                    let sha = k
                        .get("record_sha256")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .or_else(|| top_sha.clone());
                    let Some(sha) = sha else {
                        self.counts.proofs_unbound += 1;
                        continue;
                    };
                    let Some(&ri) = self.sha_index.get(&sha) else {
                        self.counts.proofs_foreign += 1;
                        self.foreign(&sha);
                        continue;
                    };
                    let mut key = k
                        .get("key")
                        .and_then(|x| json_key(x, arity))
                        .ok_or_else(|| bad("proof entry key".into()))?;
                    self.complete_key(set, ri, &mut key)
                        .map_err(|m| bad(format!("proof {m}")))?;
                    let st = k.get("status").and_then(Value::as_str).unwrap_or("");
                    let word = st.split_whitespace().next().unwrap_or("").to_string();
                    self.put_proof(ri, key, word, &file);
                }
                Ok(())
            })?;
        }
        self.counts.proofs_per_key = self
            .proofs
            .values()
            .filter(|&&i| self.statuses[i as usize].per_key)
            .count()
            + self
                .stores
                .iter()
                .map(|a| a.store.meta().counts.proofs_per_key as usize)
                .sum::<usize>();
        Ok(())
    }

    /// Solved relations (`rustred.campaign.solved-relation.v1`, interfaces/import.md 6.2).
    pub fn load_solved(
        &mut self,
        set: &ImportedSet,
        base: &Path,
        r: &FileRef,
    ) -> Result<(), ImportError> {
        let arity = set.arity();
        for (path, digest) in evidence_files(base, r, self.ledger_verified)? {
            self.counts
                .files
                .push((path.display().to_string(), format!("sha256:{digest}")));
            each_object(&path, &mut |line, v| {
                let bad = |m: String| ImportError::Evidence {
                    path: path.display().to_string(),
                    line,
                    msg: m,
                };
                if v.get("schema").and_then(Value::as_str) != Some(super::SOLVED_SCHEMA) {
                    return Err(bad(format!("not a {} line", super::SOLVED_SCHEMA)));
                }
                self.counts.solved_lines += 1;
                let src = v
                    .get("source")
                    .and_then(Value::as_str)
                    .ok_or_else(|| bad("source".into()))?;
                if !set.recipes.iter().any(|rc| rc.id == src) {
                    self.counts.solved_foreign += 1;
                    return Ok(());
                }
                let key = v
                    .get("key")
                    .and_then(|k| json_key(k, arity))
                    .ok_or_else(|| bad("key".into()))?;
                let mut rhs = vec![];
                for t in v
                    .get("rhs")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default()
                {
                    rhs.push(
                        t.get("key")
                            .and_then(|k| json_key(k, arity))
                            .ok_or_else(|| bad("rhs key".into()))?,
                    );
                }
                let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
                let rel = SolvedRelation {
                    source: src.to_string(),
                    key: key.clone(),
                    verdict: s("verdict").unwrap_or_default(),
                    recipe_sha256: s("recipe_sha256"),
                    recipe_digest: s("recipe_digest"),
                    determinism_id: s("determinism_id"),
                    binary_sha256: s("binary_sha256"),
                    relation_digest: s("relation_digest"),
                    rhs,
                    proof_ok: v["proof"]["ok"] == true,
                    regular: v["regular"] == true,
                    file: path.display().to_string(),
                };
                let m = self.solved.entry(src.to_string()).or_default();
                match m.get(&key) {
                    // lane closeloop4 (audit-closeloop m5): two answers for (source, key) must be the SAME relation
                    // (verdict, rhs, digest), never "the first wins" when the digests are absent
                    Some(old)
                        if (&old.verdict, &old.rhs, &old.relation_digest)
                            != (&rel.verdict, &rel.rhs, &rel.relation_digest) =>
                    {
                        return Err(bad(format!(
                            "two different solved relations for ({src}, {key:?})"
                        )));
                    }
                    Some(_) => {}
                    None => {
                        m.insert(key, rel);
                        self.counts.solved_bound += 1;
                    }
                }
                Ok(())
            })?;
        }
        Ok(())
    }

    /// The per-key proof status of (rule, key), if any.
    pub fn proof(
        &self,
        set: &ImportedSet,
        table: usize,
        record: usize,
        key: &[i32],
    ) -> Option<&ProofStatus> {
        let sha = &set.rules[table][record].record_sha256;
        let ri = *self.sha_index.get(sha)?;
        self.proof_index(ri, key)
            .map(|i| &self.statuses[i as usize])
    }

    /// Whether a usable regularity fact exists for (rule, key).
    pub fn has_lift(&self, set: &ImportedSet, table: usize, record: usize, key: &[i32]) -> bool {
        let sha = &set.rules[table][record].record_sha256;
        self.sha_index
            .get(sha)
            .and_then(|ri| self.fact_ref(*ri, key))
            .is_some_and(|f| f.lift())
    }

    /// Whether any usable fact (memory or store) exists for (rule, key) (lane t266run: the closing loop asks a fact
    /// only for keys without one).
    pub fn has_fact(&self, set: &ImportedSet, table: usize, record: usize, key: &[i32]) -> bool {
        let sha = &set.rules[table][record].record_sha256;
        self.sha_index
            .get(sha)
            .and_then(|ri| self.fact_ref(*ri, key))
            // lane closeloop3: a USABLE fact line (an unusable answer is no fact)
            .is_some_and(|f| match f {
                FactRef::Mem(m) => !matches!(&m.terms, Terms::Full(t) if t.is_empty()) || m.lift,
                FactRef::Store(e) => FactRef::sf(&e).usable,
                FactRef::Both(m, e) => {
                    !matches!(&m.terms, Terms::Full(t) if t.is_empty())
                        || m.lift
                        || FactRef::sf(&e).usable
                }
            })
    }

    pub fn has_facts(&self) -> bool {
        !self.fact_files.is_empty()
    }

    /// lane closeloop4 (audit-closeloop M1): from now on `load_facts` / `load_proofs` / `load_solved` may name files
    /// WITHOUT a pin that the caller verified against its own ledger record (the closing loop's answer files); the
    /// spec's evidence is always loaded pinned ([`resolve_evidence`]).
    pub fn accept_ledger_verified_files(&mut self) {
        self.ledger_verified = true;
    }

    /// lane closeloop4 (closeloop.md 18): the coefficient facts of (record `sha`, `key`) are DISPUTED by a second
    /// route: from now on every term there is `Unknown` (an edge, the key uncertified). False = no such record.
    pub fn dispute(&mut self, sha: &str, key: &[i32]) -> bool {
        match self.sha_index.get(sha) {
            Some(&ri) => {
                self.disputed.insert((ri, key.to_vec()));
                true
            }
            None => false,
        }
    }

    /// lane closeloop4 (closeloop.md 18): the bound (in-memory) fact of (record `sha`, `key`) was CONFIRMED term by
    /// term by a second, independent route: its fact basis becomes `basis` (e.g. `rr_proof + exact_eval`). False = no
    /// in-memory fact.
    pub fn confirm_basis(&mut self, sha: &str, key: &[i32], basis: &'static str) -> bool {
        let Some(&ri) = self.sha_index.get(sha) else {
            return false;
        };
        match self.facts.get_mut(&(ri, key.to_vec())) {
            Some(f) => {
                f.producer = basis;
                true
            }
            None => false,
        }
    }

    /// lane closeloop3: the fact basis of (rule, key): the producer of the bound fact line (`rr_proof` | `exact_eval`
    /// | `compact`; `store` for a fact read from an attached evidence store only), `None` without a fact.
    pub fn fact_basis(&self, rule: &RuleId, key: &[i32]) -> Option<&'static str> {
        self.rec(rule, key).map(|f| match f {
            FactRef::Mem(m) | FactRef::Both(m, _) => m.producer,
            FactRef::Store(_) => "store",
        })
    }

    fn rec(&self, rule: &RuleId, key: &[i32]) -> Option<FactRef<'_>> {
        let ri = self.rule_index2.get(rule.table)?.get(&rule.label)?;
        self.fact_ref(*ri, key)
    }

    /// Whether an exact per-key proof is bound to the record named by `rule` at `key`.
    fn per_key(&self, rule: &RuleId, key: &[i32]) -> bool {
        let Some(&ri) = self
            .rule_index2
            .get(rule.table)
            .and_then(|m| m.get(&rule.label))
        else {
            return false;
        };
        let pk = self
            .proof_index(ri, key)
            .is_some_and(|i| self.statuses[i as usize].per_key);
        // closeloop4 m1 in every mode: a per-key proof contradicted by a decided verdict (in memory, inside one store,
        // or across memory / stores) gives no per-key level; the pair is recorded for the verdict's blocker
        pk && !self.conflicted(ri, key)
    }

    /// lane t266run: (rule, key) has a proof conflict ([`Self::proof_conflict`]); a conflicted pair is recorded in
    /// `conflicts_used` (the closing loop's blocker names it).
    fn conflicted(&self, ri: u32, key: &[i32]) -> bool {
        if self.proof_conflict(ri, key) {
            self.conflicts_used
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert((ri, key.to_vec()));
            return true;
        }
        false
    }

    /// [`Self::conflicted`] for a rule id of the closure.
    fn rule_conflicted(&self, rule: &RuleId, key: &[i32]) -> bool {
        self.rule_index2
            .get(rule.table)
            .and_then(|m| m.get(&rule.label))
            .is_some_and(|ri| self.conflicted(*ri, key))
    }

    /// Whether any proof conflict can exist: a conflict in memory or inside a store, or proofs from two sources or more.
    fn conflicts_possible(&self) -> bool {
        !self.proof_conflicts.is_empty()
            || self
                .stores
                .iter()
                .any(|a| a.store.meta().counts.proof_conflicts > 0)
            || usize::from(!self.proofs.is_empty())
                + self
                    .stores
                    .iter()
                    .filter(|a| a.store.meta().counts.proofs_stored > 0)
                    .count()
                >= 2
    }

    /// Whether (table, record) at `key` has an exact per-key proof that no decided verdict contradicts (lane t266run:
    /// the import's own row levels; a contradicted pair is recorded for the blocker).
    pub fn per_key_at(&self, set: &ImportedSet, table: usize, record: usize, key: &[i32]) -> bool {
        let sha = &set.rules[table][record].record_sha256;
        let Some(&ri) = self.sha_index.get(sha) else {
            return false;
        };
        self.proof_index(ri, key)
            .is_some_and(|i| self.statuses[i as usize].per_key)
            && !self.conflicted(ri, key)
    }

    /// lane t266run (closeloop4 m1 in store mode): (rule, key) has a per-key proof AND a decided contrary verdict:
    /// in memory, inside one store (its entry's conflict flag), or across memory and the stores.
    fn proof_conflict(&self, ri: u32, key: &[i32]) -> bool {
        if !self.conflicts_possible() {
            return false;
        }
        if !self.proof_conflicts.is_empty()
            && self
                .proof_conflicts
                .contains(&(ri, Box::<[i32]>::from(key)))
        {
            return true;
        }
        if self.stores.is_empty() {
            return false;
        }
        let (mut per, mut contra) = (false, false);
        let mut see = |si: u32| {
            let s = &self.statuses[si as usize];
            if s.per_key {
                per = true;
            } else if is_decided_status(&s.status) {
                contra = true;
            }
        };
        if !self.proofs.is_empty()
            && let Some(&m) = self.proofs.get(&(ri, Box::<[i32]>::from(key)))
        {
            see(m);
        }
        for a in &self.stores {
            if let Some(rec) = a.rec_of_ri.get(ri as usize).copied().flatten()
                && let Some(e) = a.store.get_rec(rec, key)
            {
                if e.proof_conflict {
                    return true;
                }
                if let Some(s) = e.proof {
                    see(a.status_map[s as usize]);
                }
            }
        }
        per && contra
    }

    /// lane t266run: the contradicted (record_sha256, key) pairs a check asked for a per-key level (each was denied
    /// it), sorted.
    pub fn conflicts_used(&self) -> Vec<(String, Vec<i32>)> {
        self.conflicts_used
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|(ri, k)| {
                (
                    self.rule_sha.get(*ri as usize).cloned().unwrap_or_default(),
                    k.clone(),
                )
            })
            .collect()
    }
}

fn locate(set: &ImportedSet, ri: u32) -> (usize, usize) {
    let mut n = ri as usize;
    for (t, v) in set.rules.iter().enumerate() {
        if n < v.len() {
            return (t, n);
        }
        n -= v.len();
    }
    (0, 0)
}

impl FactSource for Evidence {
    fn coefficient(&self, rule: &RuleId, key: &[i32], term: usize, term_key: &[i32]) -> CoefFact {
        if !self.disputed.is_empty()
            && self
                .rule_index2
                .get(rule.table)
                .and_then(|m| m.get(&rule.label))
                .is_some_and(|ri| self.disputed.contains(&(*ri, key.to_vec())))
        {
            return CoefFact::Unknown;
        }
        self.rec(rule, key)
            .map(|f| f.coefficient(term, term_key))
            .unwrap_or(CoefFact::Unknown)
    }
    fn regularity(&self, rule: &RuleId, key: &[i32]) -> RegFact {
        // a regularity fact first; else an exact per-key proof at the key implies the rule is regular there (the proof
        // evaluates the rule at the key; a pole is the different status IRREGULAR_KEY)
        let r = match self.rec(rule, key).map(|f| f.regular()) {
            Some(RegFact::Unknown) | None if self.per_key(rule, key) => RegFact::Regular,
            Some(f) => f,
            None => RegFact::Unknown,
        };
        // lane t266run (closeloop4 m1): a regularity fact does not make a pair regular whose proofs contradict each
        // other (a decided contrary verdict beside a per-key proof): unknown, recorded for the blocker
        if r == RegFact::Regular && self.rule_conflicted(rule, key) {
            return RegFact::Unknown;
        }
        r
    }
    fn per_key_proof(&self, rule: &RuleId, key: &[i32]) -> bool {
        // a Pole fact for the same (rule, key) contradicts the proof: fail closed (no lift)
        self.per_key(rule, key)
            && self
                .rec(rule, key)
                .is_none_or(|f| f.regular() != RegFact::Pole)
    }
    fn label(&self) -> String {
        self.label.clone()
    }
    fn witness_lift(&self, rule: &RuleId, key: &[i32]) -> RegFact {
        match self.rec(rule, key) {
            // lane t266run (closeloop4 m1): no lift through a fact for a pair whose proofs contradict each other
            Some(f) if f.lift() && self.rule_conflicted(rule, key) => RegFact::Unknown,
            Some(f) if f.lift() => RegFact::Regular,
            Some(f) if f.regular() == RegFact::Pole => RegFact::Pole,
            _ => RegFact::Unknown,
        }
    }
}
