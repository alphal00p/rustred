//! Coefficient and regularity facts for Tier C (plan 2.4.8; engine contract EC-2 `rr_proof instantiate`).
//!
//! A fact is about one rule CONTENT ([`RuleId::digest`]: the rule's content digest, see `closure.md` section 11: the
//! table's coefficient-file digest, the record's position and label, its case domain and its right-hand-side key
//! patterns) at one key (the key the rule is applied at: the dispatched key, or the image g k for a symmetry credit)
//! and, for coefficients, one term: the term's index in the right-hand-side list AND the term's own instantiated
//! right-hand-side key. The table path, record and label are kept for the producer's request format only; a fact is
//! looked up by (rule digest, key, term index, term key) (audit loop-A M4). The closure asks through [`FactSource`];
//! missing facts come back as `Unknown` and are collected as [`FactRequest`]s (batch protocol: write the requests, run
//! EC-2, read the answers with [`FileFacts::read_bound`] against THOSE requests, re-run).

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io::{self, BufRead, Write};

use serde::{Deserialize, Serialize};

pub const FACT_SCHEMA: &str = "rustred.campaign.fact.v1";
pub const FACT_REQUEST_SCHEMA: &str = "rustred.campaign.fact-request.v1";
/// The producer's per-(rule, key) line, which carries the instantiated right-hand-side key of every term.
pub const INSTANTIATE_SCHEMA: &str = "rustred.proof-kernel.instantiate.v1";

/// Whether the coefficient of a term vanishes identically in d at the key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoefFact {
    Zero,
    NonZero,
    Unknown,
}

/// Whether a rule is regular at the key (no coefficient / weight denominator or witness locus vanishes as a
/// polynomial in d).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegFact {
    Regular,
    Pole,
    Unknown,
}

/// Rule identity of a fact query (borrowed; no allocation per query). `digest` is THE identity (rule content digest);
/// `table` (the producer's table path), `record` and `label` address the rule for the producer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RuleId<'a> {
    pub table: &'a str,
    pub record: usize,
    pub label: i64,
    pub digest: &'a str,
}

/// The source of facts. Implementations must be deterministic and thread-safe. A source must answer `Zero` /
/// `Regular` only for the rule CONTENT named by `rule.digest` and, for coefficients, only if the term with index
/// `term` of that rule at `key` has the instantiated key `term_key`.
pub trait FactSource: Sync {
    fn coefficient(&self, rule: &RuleId, key: &[i32], term: usize, term_key: &[i32]) -> CoefFact;
    fn regularity(&self, rule: &RuleId, key: &[i32]) -> RegFact;
    /// False if some facts are provisional (e.g. modular at two primes): a closure using them is never `closed`.
    fn exact(&self) -> bool {
        true
    }
    /// Short label for reports ("none", "file:sha256:..").
    fn label(&self) -> String;
    /// The configuration identity (without the facts component) of the run whose requests these facts answer
    /// (provenance; reported). `None` for sources not read against a request file.
    fn requested_by(&self) -> Option<String> {
        None
    }
    /// The per-key EXACT fact that lifts a witness-absent block (coordinator ruling 2026-11-14): `Regular` only if the
    /// producer reported, for exactly (rule content, key): in case, on no exception branch, and regular.
    fn witness_lift(&self, _rule: &RuleId, _key: &[i32]) -> RegFact {
        RegFact::Unknown
    }
    /// The evidence oracle of the per-key lift (`ClosureConfig::per_key_lift`, closure.md section 16): `true` only if
    /// an EXACT per-key proof of the rule's instantiated relation at the key (rr_proof `EXACT_IN_SPAN_AT_KEY` /
    /// `EXACT_PROVED_AT_KEY`) is bound to exactly (this record, key). Consulted ONLY for records whose witness loci are
    /// elided or absent, and only under `PerKeyLift::Proof`. The proof evaluates the rule at the key (a pole there is
    /// the different status `IRREGULAR_KEY`), so a key served this way needs no separate regularity fact.
    fn per_key_proof(&self, _rule: &RuleId, _key: &[i32]) -> bool {
        false
    }
    /// audit r2-planners G1: a STRUCTURAL zero proof (lane zeros2's zeros route) covers term `term` of `rule` applied
    /// at `key` (numerator identically zero on a proved locus holding the key's rule point, denominator not vanishing
    /// there). A SECOND route of a first-route zero only: consumers drop a term on a first-route zero confirmed by it
    /// (`import::zero_proofs::zero_route_of`), never on the proof alone. Default: no proof.
    fn zero_proof(&self, _rule: &RuleId, _key: &[i32], _term: usize) -> bool {
        false
    }
}

/// No facts (Tier K): `Unknown` everywhere.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoFacts;

impl FactSource for NoFacts {
    fn coefficient(&self, _: &RuleId, _: &[i32], _: usize, _: &[i32]) -> CoefFact {
        CoefFact::Unknown
    }
    fn regularity(&self, _: &RuleId, _: &[i32]) -> RegFact {
        RegFact::Unknown
    }
    fn label(&self) -> String {
        "none".into()
    }
}

/// Kind of a requested or answered fact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactKind {
    Coefficient,
    Regularity,
}

/// Why a fact is requested.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestReason {
    /// A term with a positive numerator index: must vanish.
    PositiveNumerator,
    /// A top-sector term that is not lower than the key: must vanish.
    NotLower,
    /// Regularity of the winning rule at the key.
    Regularity,
    /// Regularity + in case + no exception branch at a key where the selected record carries no witness loci (the
    /// fact that lifts the witness-absent block).
    WitnessLift,
}

/// One fact request (one JSONL line of the request file).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FactRequest {
    pub schema: String,
    pub table: String,
    pub record: usize,
    pub label: i64,
    pub key: Vec<i32>,
    pub kind: FactKind,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub term: Option<usize>,
    /// The instantiated right-hand-side key of term `term` (coefficient requests).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub rhs: Option<Vec<i32>>,
    pub reason: RequestReason,
    /// The rule's content digest (the fact identity; extra field, ignored by the producer).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub rule_digest: Option<String>,
    /// The configuration identity (without facts) of the requesting run (extra field, ignored by the producer).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub config: Option<String>,
}

/// Write requests as JSONL, sorted and de-duplicated (deterministic).
pub fn write_fact_requests(reqs: &[FactRequest], mut w: impl Write) -> io::Result<()> {
    let mut v: Vec<&FactRequest> = reqs.iter().collect();
    v.sort();
    v.dedup();
    for r in v {
        writeln!(w, "{}", serde_json::to_string(r).map_err(io::Error::other)?)?;
    }
    w.flush()
}

/// One answered fact (one JSONL line of the answer file).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactLine {
    pub schema: String,
    pub table: String,
    pub record: usize,
    pub label: i64,
    pub key: Vec<i32>,
    pub kind: FactKind,
    #[serde(default)]
    pub term: Option<usize>,
    pub value: String,
    pub method: String,
    #[serde(default)]
    pub evidence: Option<serde_json::Value>,
}

/// Errors reading facts.
#[derive(Debug)]
pub enum FactError {
    Io(io::Error),
    Line {
        line: usize,
        msg: String,
    },
    /// Two answers for one (rule, key, term) disagree (fail closed).
    Conflict {
        line: usize,
    },
    /// The request set given to [`FileFacts::read_bound`] is unusable (no rule digest, no term key, mixed
    /// configuration identities).
    Requests(String),
}
impl std::fmt::Display for FactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for FactError {}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FactKey {
    digest: String,
    key: Vec<i32>,
    term: u32,
    term_key: Vec<i32>,
}

fn fact_hash(digest: &str, key: &[i32], term: u32, term_key: &[i32]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    digest.hash(&mut h);
    key.hash(&mut h);
    term.hash(&mut h);
    term_key.hash(&mut h);
    h.finish()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Val {
    C(CoefFact),
    R(RegFact),
}

/// Facts read from a JSONL answer file, bound to the request set they answer.
#[derive(Clone, Debug, Default)]
pub struct FileFacts {
    map: HashMap<u64, Vec<(FactKey, Val)>>,
    exact: bool,
    label: String,
    requested_by: Option<String>,
    /// False for the deprecated unbound reader: every lookup is `Unknown`.
    bound: bool,
    pub n_facts: usize,
    pub n_provisional_read_as_unknown: usize,
    /// Lines of other schemas (rr_proof's instantiate records; used for the term-key check).
    pub n_other_lines: usize,
    /// Refusal records of the producer (`ERROR`, `UNSUPPORTED: ...`): the affected facts stay Unknown.
    pub refusals: Vec<String>,
    /// Unknown answers with the producer's evidence text (typed Unknown(reason)).
    pub unknown_reasons: Vec<String>,
    /// Answers that match no request (table, record, label, key, kind, term): rejected.
    pub n_rejected_unrequested: usize,
    /// Coefficient answers whose term key could not be verified (no instantiate line for the (rule, key), or the
    /// producer's term key differs from the request's): rejected.
    pub n_rejected_term_key: usize,
    /// Answers read by the deprecated unbound reader (all rejected).
    pub n_rejected_unbound: usize,
    /// Up to 100 rejection examples.
    pub rejections: Vec<String>,
}

const REG_TERM: u32 = u32::MAX;
const LIFT_TERM: u32 = u32::MAX - 1;

/// (producer rule index, key) of an instantiate line.
type InstKey = (i64, Vec<i32>);

type ReqIndex = HashMap<
    (String, usize, i64, Vec<i32>, FactKind, Option<usize>),
    Option<(String, Option<Vec<i32>>)>,
>;

impl FileFacts {
    /// DEPRECATED (audit loop-A M4): answers read without the request set cannot be bound to a rule content or a
    /// term key; the result answers `Unknown` everywhere (strict) and counts the lines in `n_rejected_unbound`.
    #[deprecated(
        note = "use FileFacts::read_bound with the request set (facts are keyed by rule digest and term key)"
    )]
    pub fn read(r: impl BufRead, accept_provisional: bool) -> Result<FileFacts, FactError> {
        let mut ff = Self::read_bound(r, &[], accept_provisional)?;
        ff.n_rejected_unbound = ff.n_rejected_unrequested;
        ff.n_rejected_unrequested = 0;
        ff.bound = false;
        ff.map.clear();
        ff.n_facts = 0;
        Ok(ff)
    }

    /// Read answers against the requests they answer. An answer is used only if it matches a request on (table,
    /// record, label, key, kind, term); a coefficient answer additionally needs the producer's term key (the
    /// `rhs[term].key` of its instantiate line for that (rule index, key)) to equal the request's `rhs`. The fact is
    /// stored under the REQUEST's rule digest and term key. Every other answer is rejected and counted. Methods
    /// `exact*` are exact; any other method is provisional: read as `Unknown` unless `accept_provisional` (then the
    /// source reports `exact() == false`). All requests must carry a rule digest (and coefficient requests a term
    /// key) and one common configuration identity, which the facts record as `requested_by`.
    pub fn read_bound(
        r: impl BufRead,
        requests: &[FactRequest],
        accept_provisional: bool,
    ) -> Result<FileFacts, FactError> {
        let mut ff = FileFacts {
            exact: true,
            bound: true,
            ..Default::default()
        };
        // ---- requests
        let mut idx: ReqIndex = HashMap::new();
        let mut config: Option<Option<String>> = None;
        for q in requests {
            let Some(d) = &q.rule_digest else {
                return Err(FactError::Requests(format!(
                    "request without rule digest: {} record {} key {:?}",
                    q.table, q.record, q.key
                )));
            };
            if q.kind == FactKind::Coefficient && (q.rhs.is_none() || q.term.is_none()) {
                return Err(FactError::Requests(format!(
                    "coefficient request without term / term key: {} record {} key {:?}",
                    q.table, q.record, q.key
                )));
            }
            match &config {
                None => config = Some(q.config.clone()),
                Some(c) if *c != q.config => {
                    return Err(FactError::Requests(
                        "requests of different configuration identities".into(),
                    ));
                }
                _ => {}
            }
            let k = (
                q.table.clone(),
                q.record,
                q.label,
                q.key.clone(),
                q.kind,
                q.term,
            );
            let v = (d.clone(), q.rhs.clone());
            match idx.get(&k) {
                None => {
                    idx.insert(k, Some(v));
                }
                Some(Some(old)) if *old == v => {}
                Some(_) => {
                    // two requests with one producer address but different rule contents / term keys: ambiguous
                    idx.insert(k, None);
                }
            }
        }
        ff.requested_by = config.flatten();
        // ---- answers
        let mut sha = super::sha256::Sha256::new();
        // (rule index, key) -> per-term keys (None: two instantiate lines disagree)
        let mut inst: HashMap<InstKey, Option<Vec<Vec<i32>>>> = HashMap::new();
        // (rule index, key) -> in case, chart ok, on no exception branch (false if two lines disagree)
        let mut inst_ok: HashMap<InstKey, bool> = HashMap::new();
        let mut lines: Vec<(usize, FactLine)> = vec![];
        for (ln, line) in r.lines().enumerate() {
            let line = line.map_err(FactError::Io)?;
            sha.update(line.as_bytes());
            sha.update(b"\n");
            if line.trim().is_empty() {
                continue;
            }
            let raw: serde_json::Value =
                serde_json::from_str(&line).map_err(|e| FactError::Line {
                    line: ln,
                    msg: e.to_string(),
                })?;
            if raw["schema"] != FACT_SCHEMA {
                if raw["schema"] == INSTANTIATE_SCHEMA {
                    ff.n_other_lines += 1;
                    let rule = raw["rule"].as_i64();
                    let key: Option<Vec<i32>> = serde_json::from_value(raw["key"].clone()).ok();
                    let terms: Option<Vec<Vec<i32>>> = raw["rhs"].as_array().and_then(|a| {
                        a.iter()
                            .enumerate()
                            .map(|(j, t)| {
                                (t["term"].as_u64() == Some(j as u64))
                                    .then(|| serde_json::from_value(t["key"].clone()).ok())
                                    .flatten()
                            })
                            .collect()
                    });
                    let ok = raw["in_case"] == true
                        && raw["chart_ok"] == true
                        && raw["branch"].is_null()
                        && raw["error"].is_null();
                    if let (Some(rule), Some(key)) = (rule, key) {
                        let o = inst_ok.entry((rule, key.clone())).or_insert(ok);
                        *o = *o && ok;
                        let e = inst.entry((rule, key)).or_insert_with(|| terms.clone());
                        if *e != terms {
                            *e = None;
                        }
                    }
                } else if let Some(v) = raw["verdict"].as_str() {
                    // refusal records {"verdict": "ERROR"|"UNSUPPORTED"|.., "error": ..}: the facts they would have
                    // given stay Unknown (never zero)
                    ff.refusals
                        .push(format!("{v}: {}", raw["error"].as_str().unwrap_or("")));
                } else {
                    ff.n_other_lines += 1;
                }
                continue;
            }
            let f: FactLine = serde_json::from_value(raw).map_err(|e| FactError::Line {
                line: ln,
                msg: e.to_string(),
            })?;
            lines.push((ln, f));
        }
        let reject = |ff: &mut FileFacts, what: &str, f: &FactLine| {
            if ff.rejections.len() < 100 {
                ff.rejections.push(format!(
                    "{what}: {} record {} label {} key {:?} {:?} term {:?}",
                    f.table, f.record, f.label, f.key, f.kind, f.term
                ));
            }
        };
        for (ln, f) in lines {
            let exact = f.method.starts_with("exact");
            let rk = (
                f.table.clone(),
                f.record,
                f.label,
                f.key.clone(),
                f.kind,
                f.term,
            );
            let Some(Some((digest, rhs))) = idx.get(&rk) else {
                ff.n_rejected_unrequested += 1;
                reject(&mut ff, "unrequested", &f);
                continue;
            };
            let (term, term_key, val) = match f.kind {
                FactKind::Coefficient => {
                    let t = f.term.ok_or(FactError::Line {
                        line: ln,
                        msg: "coefficient without term".into(),
                    })?;
                    let produced = inst
                        .get(&(f.label, f.key.clone()))
                        .and_then(|x| x.as_ref())
                        .and_then(|v| v.get(t));
                    let want = rhs.as_ref().expect("checked above");
                    if produced != Some(want) {
                        ff.n_rejected_term_key += 1;
                        reject(&mut ff, "term key not verified", &f);
                        continue;
                    }
                    let v = match f.value.as_str() {
                        "zero" => CoefFact::Zero,
                        "nonzero" => CoefFact::NonZero,
                        "unknown" => {
                            ff.unknown_reasons.push(
                                f.evidence
                                    .as_ref()
                                    .map(|e| e.to_string())
                                    .unwrap_or_default(),
                            );
                            CoefFact::Unknown
                        }
                        o => {
                            return Err(FactError::Line {
                                line: ln,
                                msg: format!("value {o}"),
                            });
                        }
                    };
                    (t as u32, want.clone(), Val::C(v))
                }
                FactKind::Regularity => {
                    let v = match f.value.as_str() {
                        "regular" => RegFact::Regular,
                        "pole" => RegFact::Pole,
                        "unknown" => RegFact::Unknown,
                        o => {
                            return Err(FactError::Line {
                                line: ln,
                                msg: format!("value {o}"),
                            });
                        }
                    };
                    (REG_TERM, vec![], Val::R(v))
                }
            };
            let val = if exact || accept_provisional {
                if !exact {
                    ff.exact = false;
                }
                val
            } else {
                ff.n_provisional_read_as_unknown += 1;
                match val {
                    Val::C(_) => Val::C(CoefFact::Unknown),
                    Val::R(_) => Val::R(RegFact::Unknown),
                }
            };
            if val == Val::R(RegFact::Regular)
                && inst_ok.get(&(f.label, f.key.clone())) == Some(&true)
            {
                let lk = FactKey {
                    digest: digest.clone(),
                    key: f.key.clone(),
                    term: LIFT_TERM,
                    term_key: vec![],
                };
                let h = fact_hash(&lk.digest, &lk.key, LIFT_TERM, &[]);
                let b = ff.map.entry(h).or_default();
                if !b.iter().any(|(kk, _)| *kk == lk) {
                    b.push((lk, val));
                }
            }
            let k = FactKey {
                digest: digest.clone(),
                key: f.key,
                term,
                term_key,
            };
            let h = fact_hash(&k.digest, &k.key, term, &k.term_key);
            let bucket = ff.map.entry(h).or_default();
            if let Some((_, old)) = bucket.iter().find(|(kk, _)| *kk == k) {
                if *old != val {
                    return Err(FactError::Conflict { line: ln });
                }
                continue;
            }
            bucket.push((k, val));
            ff.n_facts += 1;
        }
        ff.label = format!("file:sha256:{}", sha.finish_hex());
        Ok(ff)
    }

    /// Number of rejected answers (unrequested, unverified term key, unbound reader).
    pub fn n_rejected(&self) -> usize {
        self.n_rejected_unrequested + self.n_rejected_term_key + self.n_rejected_unbound
    }

    fn get(&self, digest: &str, key: &[i32], term: u32, term_key: &[i32]) -> Option<Val> {
        if !self.bound {
            return None;
        }
        let h = fact_hash(digest, key, term, term_key);
        self.map.get(&h)?.iter().find_map(|(k, v)| {
            (k.digest == digest && k.key == key && k.term == term && k.term_key == term_key)
                .then_some(*v)
        })
    }
}

impl FactSource for FileFacts {
    fn coefficient(&self, rule: &RuleId, key: &[i32], term: usize, term_key: &[i32]) -> CoefFact {
        match self.get(rule.digest, key, term as u32, term_key) {
            Some(Val::C(c)) => c,
            _ => CoefFact::Unknown,
        }
    }
    fn regularity(&self, rule: &RuleId, key: &[i32]) -> RegFact {
        match self.get(rule.digest, key, REG_TERM, &[]) {
            Some(Val::R(r)) => r,
            _ => RegFact::Unknown,
        }
    }
    fn exact(&self) -> bool {
        self.exact
    }
    fn label(&self) -> String {
        self.label.clone()
    }
    fn requested_by(&self) -> Option<String> {
        self.requested_by.clone()
    }
    fn witness_lift(&self, rule: &RuleId, key: &[i32]) -> RegFact {
        match self.get(rule.digest, key, LIFT_TERM, &[]) {
            Some(Val::R(r)) => r,
            _ => RegFact::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn req(kind: FactKind, term: Option<usize>, rhs: Option<Vec<i32>>) -> FactRequest {
        FactRequest {
            schema: FACT_REQUEST_SCHEMA.into(),
            table: "t".into(),
            record: 0,
            label: 5,
            key: vec![1, 0],
            kind,
            term,
            rhs,
            reason: RequestReason::PositiveNumerator,
            rule_digest: Some("D".into()),
            config: Some("C".into()),
        }
    }
    const INST: &str = r#"{"schema":"rustred.proof-kernel.instantiate.v1","rule":5,"key":[1,0],"rhs":[{"term":0,"key":[0,0],"coef":"nonzero"},{"term":1,"key":[1,-1],"coef":"nonzero"},{"term":2,"key":[0,1],"coef":"zero"}]}"#;
    #[test]
    fn read_bound_conflict_and_rejections() {
        let txt = format!(
            r#"{INST}
{{"schema":"rustred.campaign.fact.v1","table":"t","record":0,"label":5,"key":[1,0],"kind":"coefficient","term":2,"value":"zero","method":"exact"}}
{{"schema":"rustred.campaign.fact.v1","table":"t","record":0,"label":5,"key":[1,0],"kind":"regularity","value":"pole","method":"modular-2prime"}}
"#
        );
        let reqs = vec![
            req(FactKind::Coefficient, Some(2), Some(vec![0, 1])),
            req(FactKind::Regularity, None, None),
        ];
        let f = FileFacts::read_bound(txt.as_bytes(), &reqs, false).unwrap();
        let r = RuleId {
            table: "t",
            record: 0,
            label: 5,
            digest: "D",
        };
        assert_eq!(f.coefficient(&r, &[1, 0], 2, &[0, 1]), CoefFact::Zero);
        // another term key, another digest, another term: Unknown
        assert_eq!(f.coefficient(&r, &[1, 0], 2, &[0, 2]), CoefFact::Unknown);
        assert_eq!(
            f.coefficient(&RuleId { digest: "E", ..r }, &[1, 0], 2, &[0, 1]),
            CoefFact::Unknown
        );
        assert_eq!(f.coefficient(&r, &[1, 0], 1, &[1, -1]), CoefFact::Unknown);
        assert_eq!(f.regularity(&r, &[1, 0]), RegFact::Unknown);
        assert!(f.exact());
        assert_eq!(f.requested_by().as_deref(), Some("C"));
        let f = FileFacts::read_bound(txt.as_bytes(), &reqs, true).unwrap();
        assert_eq!(f.regularity(&r, &[1, 0]), RegFact::Pole);
        assert!(!f.exact());
        // a request with another term key: the answer is rejected
        let reqs2 = vec![req(FactKind::Coefficient, Some(2), Some(vec![9, 9]))];
        let f = FileFacts::read_bound(txt.as_bytes(), &reqs2, false).unwrap();
        assert_eq!(f.n_rejected_term_key, 1);
        assert_eq!(f.n_rejected_unrequested, 1);
        assert_eq!(f.coefficient(&r, &[1, 0], 2, &[9, 9]), CoefFact::Unknown);
        let bad = format!(
            "{txt}{}",
            txt.lines()
                .nth(1)
                .unwrap()
                .replace("\"zero\"", "\"nonzero\"")
        );
        assert!(matches!(
            FileFacts::read_bound(bad.as_bytes(), &reqs, false),
            Err(FactError::Conflict { .. })
        ));
        #[allow(deprecated)]
        let u = FileFacts::read(txt.as_bytes(), false).unwrap();
        assert_eq!(u.coefficient(&r, &[1, 0], 2, &[0, 1]), CoefFact::Unknown);
        assert_eq!(u.n_rejected_unbound, 2);
    }
}
