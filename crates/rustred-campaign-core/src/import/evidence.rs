//! Evidence import (interfaces/import.md section 5): per-key facts, per-key proofs and solved recipe relations, each
//! bound EXACTLY to (record_sha256 | recipe source, key). Lines that name no imported record are counted `foreign`,
//! lines without a record digest `unbound`; neither is ever used. Conflicting answers are an error (fail closed).

use std::collections::{BTreeMap, HashMap};
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

/// One usable fact: (rule, key) -> regularity, lift and per-term coefficient facts.
#[derive(Clone, Debug, PartialEq, Eq)]
struct FactRec {
    regular: RegFact,
    /// in case, chart ok, no branch, no error, regular.
    lift: bool,
    terms: Terms,
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
}

/// The imported evidence of one set.
#[derive(Default)]
pub struct Evidence {
    rule_index: HashMap<(String, i64), u32>,
    sha_index: HashMap<String, u32>,
    facts: HashMap<(u32, Vec<i32>), FactRec>,
    /// (rule, key) -> index into `statuses` (compact: no per-entry strings).
    proofs: HashMap<(u32, Box<[i32]>), u32>,
    statuses: Vec<ProofStatus>,
    status_index: HashMap<ProofStatus, u32>,
    /// rule -> its `index` field (checked against an answer's `index` / `rule` echo).
    rule_record_index: Vec<Option<i64>>,
    pub solved: BTreeMap<String, BTreeMap<Vec<i32>, SolvedRelation>>,
    pub counts: EvidenceCounts,
    label: String,
    fact_files: Vec<String>,
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

/// Files of an evidence reference: a file, or every `*.json` / `*.jsonl` file of a directory (sorted, not recursive).
fn evidence_files(base: &Path, r: &FileRef) -> Result<Vec<(PathBuf, String)>, ImportError> {
    let p = super::spec::resolve(base, &r.path);
    if p.is_dir() {
        if r.sha256.is_some() {
            return Err(ImportError::Spec(format!(
                "{}: a directory has no sha256",
                p.display()
            )));
        }
        let mut v = vec![];
        for e in std::fs::read_dir(&p).map_err(|e| io_err(&p, e))? {
            let e = e.map_err(|er| io_err(&p, er))?;
            let q = e.path();
            let ok = q.is_file()
                && q.extension()
                    .and_then(|x| x.to_str())
                    .is_some_and(|x| x == "json" || x == "jsonl");
            if ok {
                v.push(q);
            }
        }
        v.sort();
        v.into_iter()
            .map(|q| {
                let d = verify_file(&q, None)?;
                Ok((q, d))
            })
            .collect()
    } else {
        let d = verify_file(&p, r.sha256.as_deref())?;
        Ok(vec![(p, d)])
    }
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
                ev.sha_index.insert(r.record_sha256.clone(), n);
                ev.rule_record_index.push(r.index);
                n += 1;
            }
        }
        ev.label = "import:none".into();
        ev
    }

    /// Load everything the spec names.
    pub fn from_spec(set: &ImportedSet, base: &Path) -> Result<Evidence, ImportError> {
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
        for (path, digest) in evidence_files(base, r)? {
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
                };
                match self.facts.get(&(ri, key.clone())) {
                    Some(old) if *old != rec => {
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
        self.label = format!(
            "import:facts:{}",
            crate::closure::sha256::sha256_tag(self.fact_files.join(",").as_bytes())
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
        for (path, digest) in evidence_files(base, r)? {
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
            .count();
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
        for (path, digest) in evidence_files(base, r)? {
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
                    Some(old) if *old != rel && old.relation_digest != rel.relation_digest => {
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
        self.proofs
            .get(&(ri, Box::<[i32]>::from(key)))
            .map(|&i| &self.statuses[i as usize])
    }

    /// Whether a usable regularity fact exists for (rule, key).
    pub fn has_lift(&self, set: &ImportedSet, table: usize, record: usize, key: &[i32]) -> bool {
        let sha = &set.rules[table][record].record_sha256;
        self.sha_index
            .get(sha)
            .and_then(|ri| self.facts.get(&(*ri, key.to_vec())))
            .is_some_and(|f| f.lift)
    }

    pub fn has_facts(&self) -> bool {
        !self.fact_files.is_empty()
    }

    fn rec(&self, rule: &RuleId, key: &[i32]) -> Option<&FactRec> {
        let ri = self.rule_index.get(&(rule.table.to_string(), rule.label))?;
        self.facts.get(&(*ri, key.to_vec()))
    }

    /// Whether an exact per-key proof is bound to the record named by `rule` at `key`.
    fn per_key(&self, rule: &RuleId, key: &[i32]) -> bool {
        self.rule_index
            .get(&(rule.table.to_string(), rule.label))
            .and_then(|ri| self.proofs.get(&(*ri, Box::<[i32]>::from(key))))
            .is_some_and(|&i| self.statuses[i as usize].per_key)
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
        self.rec(rule, key)
            .map(|f| f.coefficient(term, term_key))
            .unwrap_or(CoefFact::Unknown)
    }
    fn regularity(&self, rule: &RuleId, key: &[i32]) -> RegFact {
        // a regularity fact first; else an exact per-key proof at the key implies the rule is regular there (the proof
        // evaluates the rule at the key; a pole is the different status IRREGULAR_KEY)
        match self.rec(rule, key).map(|f| f.regular) {
            Some(RegFact::Unknown) | None if self.per_key(rule, key) => RegFact::Regular,
            Some(f) => f,
            None => RegFact::Unknown,
        }
    }
    fn per_key_proof(&self, rule: &RuleId, key: &[i32]) -> bool {
        // a Pole fact for the same (rule, key) contradicts the proof: fail closed (no lift)
        self.per_key(rule, key)
            && self
                .rec(rule, key)
                .is_none_or(|f| f.regular != RegFact::Pole)
    }
    fn label(&self) -> String {
        self.label.clone()
    }
    fn witness_lift(&self, rule: &RuleId, key: &[i32]) -> RegFact {
        match self.rec(rule, key) {
            Some(f) if f.lift => RegFact::Regular,
            Some(f) if f.regular == RegFact::Pole => RegFact::Pole,
            _ => RegFact::Unknown,
        }
    }
}
