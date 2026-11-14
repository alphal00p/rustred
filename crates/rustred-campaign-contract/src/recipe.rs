//! Kind `solve-recipe` (coord-recipe-solve.md v1.1; lane adapters, interfaces/adapters.md 1.2 / 1.4): the records of a
//! sealed `out/results.jsonl`, their consistency with the job's `keys` input (part of S for this kind), and the TYPED
//! per-key outcomes: each SOLVED + proved record is acceptable on its own, every other key is an unsolved key routed by
//! its failure class.
//!
//! Everything here works on VERIFIED canonical lines (`seal::VerifiedHandle::canonical_lines`), never on a path.
//! Generic: a key is an integer vector of the length the keys input gives; nothing here knows an arity.
//!
//! Mounted at the crate root (`rustred_campaign_contract::recipe`, lane int5); `classify::recipe` re-exports it.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// `type` of a per-key record.
pub const RELATION_TYPE: &str = "relation";
/// `failure.schema` of an unsolved record.
pub const FAILURE_SCHEMA: &str = "rustred.recipe-failure.v1";

/// Verdicts (closed list v1.1, coord-recipe-solve.md 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Verdict {
    Solved,
    NoRelationInSeeds,
    NotInDomain,
    NotLower,
    Irregular,
    ProofFailed,
    DigestMismatch,
    BudgetWork,
    BudgetWall,
    Interrupted,
    Error,
    /// `--no-proof` diagnostics; never accepted, never in a job's sealed file.
    SolvedUnproved,
}

impl Verdict {
    pub const ALL: [Verdict; 12] = [
        Verdict::Solved,
        Verdict::NoRelationInSeeds,
        Verdict::NotInDomain,
        Verdict::NotLower,
        Verdict::Irregular,
        Verdict::ProofFailed,
        Verdict::DigestMismatch,
        Verdict::BudgetWork,
        Verdict::BudgetWall,
        Verdict::Interrupted,
        Verdict::Error,
        Verdict::SolvedUnproved,
    ];

    /// The verdict's name as the verb writes it.
    pub fn name(self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default()
    }

    pub fn from_name(s: &str) -> Option<Verdict> {
        serde_json::from_value(Value::String(s.to_string())).ok()
    }

    /// The failure class of a non-SOLVED verdict (= the verdict in lower case); `None` for SOLVED / SOLVED_UNPROVED.
    pub const fn class(self) -> Option<FailureClass> {
        Some(match self {
            Verdict::Solved | Verdict::SolvedUnproved => return None,
            Verdict::NoRelationInSeeds => FailureClass::NoRelationInSeeds,
            Verdict::NotInDomain => FailureClass::NotInDomain,
            Verdict::NotLower => FailureClass::NotLower,
            Verdict::Irregular => FailureClass::Irregular,
            Verdict::ProofFailed => FailureClass::ProofFailed,
            Verdict::DigestMismatch => FailureClass::DigestMismatch,
            Verdict::BudgetWork => FailureClass::BudgetWork,
            Verdict::BudgetWall => FailureClass::BudgetWall,
            Verdict::Interrupted => FailureClass::Interrupted,
            Verdict::Error => FailureClass::Error,
        })
    }
}

/// Failure classes (`failure.class`, the verdict in lower case).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    NoRelationInSeeds,
    NotInDomain,
    NotLower,
    Irregular,
    ProofFailed,
    DigestMismatch,
    BudgetWork,
    BudgetWall,
    Interrupted,
    Error,
}

impl FailureClass {
    pub fn name(self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default()
    }

    /// What the caller does with a key of this class (interfaces/adapters.md 1.4).
    pub const fn route(self) -> KeyRoute {
        match self {
            FailureClass::NoRelationInSeeds | FailureClass::NotInDomain => KeyRoute::NextRecipeRung,
            FailureClass::BudgetWork => KeyRoute::NextWorkClass,
            FailureClass::BudgetWall => KeyRoute::LadderDecides,
            FailureClass::Irregular => KeyRoute::SubCase,
            FailureClass::Interrupted => KeyRoute::Rerun,
            FailureClass::ProofFailed
            | FailureClass::NotLower
            | FailureClass::Error
            | FailureClass::DigestMismatch => KeyRoute::Attention,
        }
    }

    /// The class is decided by a clock (per-key wall budget).
    pub const fn time_decided(self) -> bool {
        matches!(self, FailureClass::BudgetWall)
    }
}

/// Route of an unsolved key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyRoute {
    /// Another recipe (the next recipe rung) for that key.
    NextRecipeRung,
    /// The same recipe with the next work class (a larger `--max-trace-rows`).
    NextWorkClass,
    /// Time-decided (per-key wall budget): the ladder decides.
    LadderDecides,
    /// The pole locus is a sub-case (not a rung).
    SubCase,
    /// Interrupted before it was solved: run again (never inside a sealed `end` file).
    Rerun,
    /// Never retried silently.
    Attention,
}

/// One line of a `keys` input: `[ints]` or `{"key":[ints], "tag": "..", ...}` (other members ignored).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyLine {
    pub key: Vec<i64>,
    #[serde(default)]
    pub tag: Option<String>,
}

/// Parse a `keys` input (JSONL; blank lines skipped). Every key must have the length of the first.
pub fn parse_keys_input(bytes: &[u8]) -> Result<Vec<KeyLine>, String> {
    let text = std::str::from_utf8(bytes).map_err(|e| format!("keys input: {e}"))?;
    let mut out: Vec<KeyLine> = Vec::new();
    for (i, l) in text.lines().enumerate() {
        if l.trim().is_empty() {
            continue;
        }
        let v: Value =
            serde_json::from_str(l).map_err(|e| format!("keys input line {}: {e}", i + 1))?;
        let (key, tag) = match &v {
            Value::Array(_) => (v.clone(), None),
            Value::Object(m) => (
                m.get("key").cloned().unwrap_or(Value::Null),
                m.get("tag").and_then(Value::as_str).map(str::to_string),
            ),
            _ => return Err(format!("keys input line {}: not a key", i + 1)),
        };
        let key: Vec<i64> = serde_json::from_value(key)
            .map_err(|e| format!("keys input line {}: key: {e}", i + 1))?;
        if key.is_empty() || out.first().is_some_and(|f| f.key.len() != key.len()) {
            return Err(format!("keys input line {}: key length differs", i + 1));
        }
        out.push(KeyLine { key, tag });
    }
    Ok(out)
}

/// An accepted relation: a SOLVED, exactly proved, descending record at the key of its input line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptedRelation {
    pub ordinal: u64,
    pub key: Vec<i64>,
    pub tag: Option<String>,
    pub relation_digest: String,
    /// Right-hand-side keys (from `rhs[*][0]`, `I(..)` texts), in record order.
    pub rhs_keys: Vec<Vec<i64>>,
    pub recipe_digest: String,
    pub determinism_id: String,
    /// The verified canonical line of the record.
    pub canonical: Vec<u8>,
}

/// An unsolved key: a typed per-key outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnsolvedKey {
    pub ordinal: u64,
    pub key: Vec<i64>,
    pub tag: Option<String>,
    pub verdict: Verdict,
    pub class: FailureClass,
    pub route: KeyRoute,
    /// The record's `failure` object, verbatim.
    pub failure: Value,
    pub recipe_digest: String,
    pub determinism_id: String,
}

/// The per-key outcome of one input line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum KeyOutcome {
    Accepted(AcceptedRelation),
    Unsolved(UnsolvedKey),
}

impl KeyOutcome {
    pub fn ordinal(&self) -> u64 {
        match self {
            KeyOutcome::Accepted(a) => a.ordinal,
            KeyOutcome::Unsolved(u) => u.ordinal,
        }
    }
}

/// What a sealed results file holds (part of `classify::VerifiedOutputs` for kind solve-recipe).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationTally {
    /// Relation records (= lines of the keys input).
    pub records: u64,
    /// Accepted records (SOLVED + proved + descending).
    pub accepted: u64,
    /// Unsolved records per failure class.
    pub unsolved: BTreeMap<FailureClass, u64>,
    pub recipe_digest: Option<String>,
    pub determinism_id: Option<String>,
}

impl RelationTally {
    pub fn unsolved_total(&self) -> u64 {
        self.unsolved.values().sum()
    }
    /// Keys whose outcome a clock decided (per-key wall budget).
    pub fn time_decided_keys(&self) -> u64 {
        self.unsolved
            .iter()
            .filter(|(c, _)| c.time_decided())
            .map(|(_, n)| n)
            .sum()
    }
}

fn is_hex64(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Parse an `I(a,b,..)` key text of length `arity`.
pub fn parse_key_text(t: &str, arity: usize) -> Option<Vec<i64>> {
    let inner = t.trim().strip_prefix("I(")?.strip_suffix(')')?;
    let v: Option<Vec<i64>> = inner.split(',').map(|x| x.trim().parse().ok()).collect();
    v.filter(|v| v.len() == arity)
}

fn same_or_set(slot: &mut Option<String>, v: Option<&str>, what: &str) -> Result<String, String> {
    let v = v.ok_or_else(|| format!("{what} missing"))?;
    match slot {
        None => {
            *slot = Some(v.to_string());
        }
        Some(old) if old != v => return Err(format!("two different {what} values in one file")),
        _ => {}
    }
    Ok(v.to_string())
}

/// The per-key outcomes of a VERIFIED results file (its canonical lines, end record last) against the job's keys
/// input, with the tally. `Err` = the file is not a consistent results file of this job (S false: contract
/// violation). Checks (interfaces/adapters.md 1.2): one relation record per input line, in input order, ordinal and key
/// equal; closed verdict list; SOLVED records proved, descending, with a 64-hex relation digest and rhs, no failure;
/// unsolved records with `failure.class` = the verdict in lower case and `failure.schema`; no INTERRUPTED /
/// SOLVED_UNPROVED record in an `end` file; one recipe digest and one determinism id in the whole file; the end
/// record's `records` and `counts` equal the records.
pub fn key_outcomes(
    lines: &[Vec<u8>],
    keys: &[KeyLine],
) -> Result<(Vec<KeyOutcome>, RelationTally), String> {
    let (end_line, recs) = lines.split_last().ok_or("results file without records")?;
    let end: Value = serde_json::from_slice(end_line).map_err(|e| format!("end record: {e}"))?;
    let end_type = end["type"].as_str().unwrap_or("");
    if !matches!(end_type, "end" | "partial") {
        return Err(format!(
            "last record of type {end_type:?} is not an end record"
        ));
    }
    let sealed_end = end_type == "end";
    if recs.len() != keys.len() {
        return Err(format!(
            "{} relation records for {} input keys",
            recs.len(),
            keys.len()
        ));
    }
    let arity = keys.first().map_or(0, |k| k.key.len());
    let mut t = RelationTally::default();
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut out = Vec::with_capacity(recs.len());
    for (i, (line, want)) in recs.iter().zip(keys).enumerate() {
        let r: Value = serde_json::from_slice(line).map_err(|e| format!("record {i}: {e}"))?;
        let at = |m: &str| format!("record {i}: {m}");
        if r["type"] != RELATION_TYPE {
            return Err(at("not a relation record"));
        }
        if r["ordinal"].as_u64() != Some(i as u64) {
            return Err(at("ordinal is not the input line number"));
        }
        let key: Vec<i64> =
            serde_json::from_value(r["key"].clone()).map_err(|e| at(&format!("key: {e}")))?;
        if key != want.key {
            return Err(at("key is not the input line's key"));
        }
        let verdict = r["verdict"]
            .as_str()
            .and_then(Verdict::from_name)
            .ok_or_else(|| at("verdict not in the closed list"))?;
        *counts.entry(verdict.name()).or_default() += 1;
        let recipe_digest = same_or_set(
            &mut t.recipe_digest,
            r["recipe_digest"].as_str(),
            "recipe_digest",
        )
        .map_err(|e| at(&e))?;
        let determinism_id = same_or_set(
            &mut t.determinism_id,
            r["determinism_id"].as_str(),
            "determinism_id",
        )
        .map_err(|e| at(&e))?;
        t.records += 1;
        match verdict {
            Verdict::Solved => {
                let digest = r["relation_digest"].as_str().unwrap_or("");
                if !is_hex64(digest) {
                    return Err(at("SOLVED without a 64-hex relation_digest"));
                }
                if r["proof"]["ok"] != true {
                    return Err(at("SOLVED without an exact proof (proof.ok)"));
                }
                if r["all_lower"] != true {
                    return Err(at("SOLVED with a right-hand side not all lower"));
                }
                if !r["failure"].is_null() {
                    return Err(at("SOLVED with a failure object"));
                }
                let rhs = r["rhs"]
                    .as_array()
                    .ok_or_else(|| at("SOLVED without rhs"))?;
                let mut rhs_keys = Vec::with_capacity(rhs.len());
                for x in rhs {
                    let txt = x[0].as_str().ok_or_else(|| at("rhs entry"))?;
                    rhs_keys.push(
                        parse_key_text(txt, arity)
                            .ok_or_else(|| at(&format!("rhs key {txt:?}")))?,
                    );
                }
                t.accepted += 1;
                out.push(KeyOutcome::Accepted(AcceptedRelation {
                    ordinal: i as u64,
                    key,
                    tag: want.tag.clone(),
                    relation_digest: digest.to_string(),
                    rhs_keys,
                    recipe_digest,
                    determinism_id,
                    canonical: line.clone(),
                }));
            }
            Verdict::SolvedUnproved => return Err(at("SOLVED_UNPROVED in a job's results")),
            v => {
                let class = v.class().expect("non-SOLVED verdict has a class");
                if class == FailureClass::Interrupted && sealed_end {
                    return Err(at("INTERRUPTED record in a sealed `end` file"));
                }
                let f = &r["failure"];
                if f["schema"] != FAILURE_SCHEMA || f["class"].as_str() != Some(&class.name()) {
                    return Err(at(
                        "failure object without the schema or with another class",
                    ));
                }
                if !r["relation_digest"].is_null() {
                    return Err(at("an unsolved record with a relation digest"));
                }
                *t.unsolved.entry(class).or_default() += 1;
                out.push(KeyOutcome::Unsolved(UnsolvedKey {
                    ordinal: i as u64,
                    key,
                    tag: want.tag.clone(),
                    verdict: v,
                    class,
                    route: class.route(),
                    failure: f.clone(),
                    recipe_digest,
                    determinism_id,
                }));
            }
        }
    }
    if end["records"].as_u64() != Some(recs.len() as u64) {
        return Err("end record `records` differs from the records".into());
    }
    let end_counts: BTreeMap<String, u64> =
        serde_json::from_value(end["counts"].clone()).map_err(|e| format!("end counts: {e}"))?;
    if end_counts != counts {
        return Err("end record `counts` differ from the records".into());
    }
    if !recs.is_empty() {
        for (field, want) in [
            ("recipe_digest", &t.recipe_digest),
            ("determinism_id", &t.determinism_id),
        ] {
            if end[field].as_str() != want.as_deref() {
                return Err(format!("end record {field} differs from the records'"));
            }
        }
    }
    Ok((out, t))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdict_names_and_classes() {
        for v in Verdict::ALL {
            assert_eq!(Verdict::from_name(&v.name()), Some(v));
            if let Some(c) = v.class() {
                assert_eq!(c.name(), v.name().to_lowercase());
            }
        }
        assert_eq!(Verdict::Solved.class(), None);
        assert_eq!(FailureClass::NotInDomain.route(), KeyRoute::NextRecipeRung);
        assert_eq!(FailureClass::BudgetWall.route(), KeyRoute::LadderDecides);
        assert_eq!(FailureClass::Irregular.route(), KeyRoute::SubCase);
        assert_eq!(FailureClass::DigestMismatch.route(), KeyRoute::Attention);
    }

    #[test]
    fn key_texts_are_generic_in_arity() {
        assert_eq!(parse_key_text("I(1,-2,0)", 3), Some(vec![1, -2, 0]));
        assert_eq!(parse_key_text("I(1,-2,0)", 4), None);
        assert_eq!(parse_key_text("J(1)", 1), None);
    }

    #[test]
    fn keys_input_forms() {
        let k = parse_keys_input(b"[1,2]\n\n{\"key\":[3,4],\"tag\":\"t\",\"x\":1}\n").unwrap();
        assert_eq!(k.len(), 2);
        assert_eq!(k[1].tag.as_deref(), Some("t"));
        assert!(parse_keys_input(b"[1,2]\n[1]\n").is_err());
    }
}
