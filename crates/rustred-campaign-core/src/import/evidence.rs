//! Evidence import (interfaces/import.md section 5): per-key facts, per-key proofs and solved recipe relations, each
//! bound EXACTLY to (record_sha256 | recipe source, key). Lines that name no imported record are counted `foreign`,
//! lines without a record digest `unbound`; neither is ever used. Conflicting answers are an error (fail closed).

use std::collections::{BTreeMap, HashMap};
use std::io::BufRead;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use super::error::{ImportError, io_err};
use super::lines::{parse_key_text, verify_file};
use super::load::ImportedSet;
use super::spec::FileRef;
use crate::closure::{CoefFact, FactSource, RegFact, RuleId};

/// One usable fact: (rule, key) -> regularity, lift and per-term coefficient facts.
#[derive(Clone, Debug, PartialEq, Eq)]
struct FactRec {
    regular: RegFact,
    /// in case, chart ok, no branch, no error, regular.
    lift: bool,
    /// per term index: (hash of the instantiated term key, fact).
    terms: Vec<(u64, CoefFact)>,
}

/// Per-key proof status of (rule, key).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProofStatus {
    pub status: String,
    pub per_key: bool,
    pub file: String,
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
    proofs: HashMap<(u32, Vec<i32>), ProofStatus>,
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

/// Objects of a file: one JSON object (pretty-printed) or one per line.
fn objects(path: &Path) -> Result<Vec<(usize, Value)>, ImportError> {
    let txt = std::fs::read_to_string(path).map_err(|e| io_err(path, e))?;
    let t = txt.trim_start();
    if t.starts_with('{')
        && let Ok(v) = serde_json::from_str::<Value>(&txt)
    {
        return Ok(vec![(1, v)]);
    }
    let mut out = vec![];
    for (i, l) in txt.lines().enumerate() {
        if l.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(l).map_err(|e| ImportError::Evidence {
            path: path.display().to_string(),
            line: i + 1,
            msg: e.to_string(),
        })?;
        out.push((i + 1, v));
    }
    Ok(out)
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

    /// Facts: `rustred.proof-kernel.instantiate.v1` lines with `record_sha256`.
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
            let f = std::fs::File::open(&path).map_err(|e| io_err(&path, e))?;
            for (i, l) in std::io::BufReader::with_capacity(1 << 20, f)
                .lines()
                .enumerate()
            {
                let l = l.map_err(|e| io_err(&path, e))?;
                if l.trim().is_empty() {
                    continue;
                }
                let bad = |m: String| ImportError::Evidence {
                    path: path.display().to_string(),
                    line: i + 1,
                    msg: m,
                };
                let v: Value = serde_json::from_str(&l).map_err(|e| bad(e.to_string()))?;
                if v.get("schema").and_then(Value::as_str)
                    != Some(crate::closure::facts::INSTANTIATE_SCHEMA)
                {
                    self.counts.fact_other_lines += 1;
                    continue;
                }
                self.counts.fact_lines += 1;
                let Some(sha) = v.get("record_sha256").and_then(Value::as_str) else {
                    self.counts.facts_unbound += 1;
                    continue;
                };
                let Some(&ri) = self.sha_index.get(sha) else {
                    self.counts.facts_foreign += 1;
                    let s = sha.to_string();
                    self.foreign(&s);
                    continue;
                };
                let key = v
                    .get("key")
                    .and_then(|k| json_key(k, arity))
                    .ok_or_else(|| bad("key".into()))?;
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
                    for t in v
                        .get("rhs")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default()
                    {
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
                } else {
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
            }
        }
        self.label = format!(
            "import:facts:{}",
            crate::closure::sha256::sha256_tag(self.fact_files.join(",").as_bytes())
        );
        Ok(())
    }

    /// Per-key proofs: rr_proof span / exact records (whole-file JSON or JSONL; a directory of them).
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
            for (line, v) in objects(&path)? {
                // only rr_proof records (schema rustred.proof-kernel.*): other JSON (sample lists, group files) is counted
                if !v
                    .get("schema")
                    .and_then(Value::as_str)
                    .is_some_and(|s| s.starts_with("rustred.proof-kernel."))
                {
                    self.counts.proof_other_objects += 1;
                    continue;
                }
                let Some(keys) = v.get("keys").and_then(Value::as_array) else {
                    if v.get("verdict").and_then(Value::as_str).is_some_and(|s| {
                        s == "ERROR" || s == "UNSUPPORTED" || s.starts_with("MISSING")
                    }) {
                        self.counts.proof_refusals += 1;
                    }
                    continue;
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
                    let (t, rr) = locate(set, ri);
                    let fixed = &set.disp.tables()[t].domains[rr].fixed;
                    let mut key =
                        k.get("key")
                            .and_then(|x| json_key(x, arity))
                            .ok_or_else(|| ImportError::Evidence {
                                path: path.display().to_string(),
                                line,
                                msg: "proof entry key".into(),
                            })?;
                    for (i, x) in key.iter_mut().enumerate() {
                        if *x == i32::MIN {
                            *x = fixed[i].ok_or_else(|| ImportError::Evidence {
                                path: path.display().to_string(),
                                line,
                                msg: format!("proof key misses the free slot n{i}"),
                            })?;
                        }
                    }
                    let st = k
                        .get("status")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let word = st.split_whitespace().next().unwrap_or("").to_string();
                    let per_key = word == "EXACT_IN_SPAN_AT_KEY" || word == "EXACT_PROVED_AT_KEY";
                    let ps = ProofStatus {
                        status: word,
                        per_key,
                        file: path.display().to_string(),
                    };
                    self.counts.proofs_bound += 1;
                    match self.proofs.get(&(ri, key.clone())) {
                        // keep the strongest (a per-key proof wins over an undecided attempt); never weaken
                        Some(old) if old.per_key && !per_key => {}
                        _ => {
                            self.proofs.insert((ri, key), ps);
                        }
                    }
                }
            }
        }
        self.counts.proofs_per_key = self.proofs.values().filter(|p| p.per_key).count();
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
            for (line, v) in objects(&path)? {
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
                    continue;
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
            }
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
        self.proofs.get(&(ri, key.to_vec()))
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
        match self.rec(rule, key).and_then(|f| f.terms.get(term)) {
            Some(&(h, c)) if h == term_hash(term_key) => c,
            _ => CoefFact::Unknown,
        }
    }
    fn regularity(&self, rule: &RuleId, key: &[i32]) -> RegFact {
        self.rec(rule, key)
            .map(|f| f.regular)
            .unwrap_or(RegFact::Unknown)
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
