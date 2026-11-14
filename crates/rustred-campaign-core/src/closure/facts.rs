//! Coefficient and regularity facts for Tier C (plan 2.4.8; engine contract EC-2 `rr_proof instantiate`).
//!
//! A fact is about one rule (stable identity [`RuleId`]: table name, record position, label) at one key (the key the
//! rule is applied at: the dispatched key, or the image g k for a symmetry credit) and, for coefficients, one term
//! (index into the rule's right-hand-side list). The closure asks through [`FactSource`]; missing facts come back as
//! `Unknown` and are collected as [`FactRequest`]s (batch protocol: write the requests, run EC-2, read the answers
//! with [`FileFacts`], re-run).

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io::{self, BufRead, Write};

use serde::{Deserialize, Serialize};

pub const FACT_SCHEMA: &str = "rustred.campaign.fact.v1";
pub const FACT_REQUEST_SCHEMA: &str = "rustred.campaign.fact-request.v1";

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

/// Stable rule identity (borrowed; no allocation per query).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RuleId<'a> {
    pub table: &'a str,
    pub record: usize,
    pub label: i64,
}

/// The source of facts. Implementations must be deterministic and thread-safe.
pub trait FactSource: Sync {
    fn coefficient(&self, rule: &RuleId, key: &[i32], term: usize) -> CoefFact;
    fn regularity(&self, rule: &RuleId, key: &[i32]) -> RegFact;
    /// False if some facts are provisional (e.g. modular at two primes): a closure using them is never `closed`.
    fn exact(&self) -> bool {
        true
    }
    /// Short label for reports ("none", "file:sha256:..").
    fn label(&self) -> String;
}

/// No facts (Tier K): `Unknown` everywhere.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoFacts;

impl FactSource for NoFacts {
    fn coefficient(&self, _: &RuleId, _: &[i32], _: usize) -> CoefFact {
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
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub rhs: Option<Vec<i32>>,
    pub reason: RequestReason,
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
    Line { line: usize, msg: String },
    /// Two answers for one (rule, key, term) disagree (fail closed).
    Conflict { line: usize },
}
impl std::fmt::Display for FactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for FactError {}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FactKey {
    table: String,
    record: usize,
    label: i64,
    key: Vec<i32>,
    term: u32,
}

fn fact_hash(table: &str, record: usize, label: i64, key: &[i32], term: u32) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    table.hash(&mut h);
    record.hash(&mut h);
    label.hash(&mut h);
    key.hash(&mut h);
    term.hash(&mut h);
    h.finish()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Val {
    C(CoefFact),
    R(RegFact),
}

/// Facts read from a JSONL answer file.
#[derive(Clone, Debug, Default)]
pub struct FileFacts {
    map: HashMap<u64, Vec<(FactKey, Val)>>,
    exact: bool,
    label: String,
    pub n_facts: usize,
    pub n_provisional_read_as_unknown: usize,
    /// Lines of other schemas (rr_proof's instantiate records).
    pub n_other_lines: usize,
    /// Refusal records of the producer (`ERROR`, `UNSUPPORTED: ...`): the affected facts stay Unknown.
    pub refusals: Vec<String>,
    /// Unknown answers with the producer's evidence text (typed Unknown(reason)).
    pub unknown_reasons: Vec<String>,
}

const REG_TERM: u32 = u32::MAX;

impl FileFacts {
    /// Read answers. Methods `exact*` are exact; any other method is provisional: read as `Unknown` unless
    /// `accept_provisional` (then the source reports `exact() == false`).
    pub fn read(r: impl BufRead, accept_provisional: bool) -> Result<FileFacts, FactError> {
        let mut ff = FileFacts { exact: true, ..Default::default() };
        let mut sha = super::sha256::Sha256::new();
        for (ln, line) in r.lines().enumerate() {
            let line = line.map_err(FactError::Io)?;
            sha.update(line.as_bytes());
            sha.update(b"\n");
            if line.trim().is_empty() {
                continue;
            }
            let raw: serde_json::Value =
                serde_json::from_str(&line).map_err(|e| FactError::Line { line: ln, msg: e.to_string() })?;
            if raw["schema"] != FACT_SCHEMA {
                // rr_proof instantiate writes its per-(rule, key) lines (`rustred.proof-kernel.instantiate.v1`) into
                // the same file, and refusal records {"verdict": "ERROR"|"UNSUPPORTED"|.., "error": ..}: kept as
                // refusals; the facts they would have given stay Unknown (never zero).
                if let Some(v) = raw["verdict"].as_str() {
                    ff.refusals.push(format!("{v}: {}", raw["error"].as_str().unwrap_or("")));
                } else {
                    ff.n_other_lines += 1;
                }
                continue;
            }
            let f: FactLine =
                serde_json::from_value(raw).map_err(|e| FactError::Line { line: ln, msg: e.to_string() })?;
            let exact = f.method.starts_with("exact");
            let (term, val) = match f.kind {
                FactKind::Coefficient => {
                    let t = f.term.ok_or(FactError::Line { line: ln, msg: "coefficient without term".into() })?;
                    let v = match f.value.as_str() {
                        "zero" => CoefFact::Zero,
                        "nonzero" => CoefFact::NonZero,
                        "unknown" => {
                            ff.unknown_reasons.push(f.evidence.as_ref().map(|e| e.to_string()).unwrap_or_default());
                            CoefFact::Unknown
                        }
                        o => return Err(FactError::Line { line: ln, msg: format!("value {o}") }),
                    };
                    (t as u32, Val::C(v))
                }
                FactKind::Regularity => {
                    let v = match f.value.as_str() {
                        "regular" => RegFact::Regular,
                        "pole" => RegFact::Pole,
                        "unknown" => RegFact::Unknown,
                        o => return Err(FactError::Line { line: ln, msg: format!("value {o}") }),
                    };
                    (REG_TERM, Val::R(v))
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
            let k = FactKey { table: f.table, record: f.record, label: f.label, key: f.key, term };
            let h = fact_hash(&k.table, k.record, k.label, &k.key, term);
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

    fn get(&self, rule: &RuleId, key: &[i32], term: u32) -> Option<Val> {
        let h = fact_hash(rule.table, rule.record, rule.label, key, term);
        self.map.get(&h)?.iter().find_map(|(k, v)| {
            (k.table == rule.table && k.record == rule.record && k.label == rule.label && k.key == key && k.term == term)
                .then_some(*v)
        })
    }
}

impl FactSource for FileFacts {
    fn coefficient(&self, rule: &RuleId, key: &[i32], term: usize) -> CoefFact {
        match self.get(rule, key, term as u32) {
            Some(Val::C(c)) => c,
            _ => CoefFact::Unknown,
        }
    }
    fn regularity(&self, rule: &RuleId, key: &[i32]) -> RegFact {
        match self.get(rule, key, REG_TERM) {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn read_and_conflict() {
        let txt = r#"{"schema":"rustred.campaign.fact.v1","table":"t","record":0,"label":5,"key":[1,0],"kind":"coefficient","term":2,"value":"zero","method":"exact"}
{"schema":"rustred.campaign.fact.v1","table":"t","record":0,"label":5,"key":[1,0],"kind":"regularity","value":"pole","method":"modular-2prime"}
"#;
        let f = FileFacts::read(txt.as_bytes(), false).unwrap();
        let r = RuleId { table: "t", record: 0, label: 5 };
        assert_eq!(f.coefficient(&r, &[1, 0], 2), CoefFact::Zero);
        assert_eq!(f.coefficient(&r, &[1, 0], 1), CoefFact::Unknown);
        assert_eq!(f.regularity(&r, &[1, 0]), RegFact::Unknown);
        assert!(f.exact());
        let f = FileFacts::read(txt.as_bytes(), true).unwrap();
        assert_eq!(f.regularity(&r, &[1, 0]), RegFact::Pole);
        assert!(!f.exact());
        let bad = format!("{txt}{}", txt.lines().next().unwrap().replace("\"zero\"", "\"nonzero\""));
        assert!(matches!(FileFacts::read(bad.as_bytes(), false), Err(FactError::Conflict { .. })));
    }
}
