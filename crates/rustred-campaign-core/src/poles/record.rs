//! D1 per relation: the record `rustred.campaign.pole-relation.v1` (pole-diagnostics.md section 2 / 12; TRICKS 9, 12).
//!
//! One record per (relation, key, d0) for a relation at a key (`scope: "key"`), or per (rule, d0) for a parametric rule
//! with its per-key verdicts (`scope: "rule"`). The orders are computed by the algebra crate (exact, Symbolica); this
//! module holds the data, the summary (pole order = `max(0, max_j -v(c_j))` over the non-zero normalised coefficients,
//! top and all), the verdict ([`RegularAt`]) and the digests.
//!
//! REPRESENTATIONS (TRICKS 12(a)): a relation `c_0 I(k) = sum_j c_j I(k_j)`. The engine's solve-recipe results file
//! carries ONLY the NORMALISED relation (`c_0 = 1`: crates/rustred-core/src/node/recipe.rs, rhs coefficient = -P_k of
//! the relation normalised to target coefficient 1); a zero of the unnormalised target coefficient at d0 then shows as a
//! pole of the rhs coefficients. When a source gives the unnormalised form (`target_coefficient`), the verdict is made
//! on the NORMALISED orders `v(c_j) - v(c_0)` and the target's own order is recorded beside it (`target_order`): both
//! representations give the same verdict by construction.
//!
//! DIGESTS: the record names the relation's identity (`relation.relation_digest` = the engine's BLAKE3 of
//! `{relation_rule, rhs, target}`, `relation.entry_digest` = sha256 of the canonical body as `rustred.relation-entry.v1`
//! defines it ([`entry_digest`]), or a table record's sha256) and carries its own `record_digest` = sha256 of its
//! canonical JSON without that member. Deterministic: terms in the relation's order, `worst` sorted.

use serde_json::{Map, Value, json};

/// Schema name of the D1 record.
pub const POLE_RELATION_SCHEMA: &str = "rustred.campaign.pole-relation.v1";

/// Members of an engine relation record that `rustred.relation-entry.v1` drops from the body (volatile).
pub const VOLATILE_RELATION_MEMBERS: [&str; 4] = ["seconds", "vmhwm_kb", "ordinal", "tag"];

/// The verdict of one relation at one key and d0 (the call `is_regular_at` of the algebra crate returns it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegularAt {
    /// Every non-zero normalised coefficient is regular at d0.
    Regular,
    /// The largest pole order over the normalised coefficients (> 0).
    Singular { order: u64 },
    /// No verdict: the record is not a solved relation, a coefficient does not parse, the target coefficient vanishes
    /// at the key, the key does not match, ... (never "regular").
    Undecided { reason: String },
}

impl RegularAt {
    /// `regular` / `pole` / `undecided`: a statement about THIS RELATION at this key and d0 (TRICKS 15 / 16: never
    /// about the key, the system of rows or the existence of a regular relation; inherent / spurious are issued only by
    /// the composition onto a regular terminal set (lane epsdepth) or the t-adic elimination (lane tadic)).
    pub fn word(&self) -> &'static str {
        match self {
            RegularAt::Regular => "regular",
            RegularAt::Singular { .. } => "pole",
            RegularAt::Undecided { .. } => "undecided",
        }
    }

    /// `Some(0)` regular, `Some(p)` singular, `None` undecided (the selection's `pole_order`).
    pub fn pole_order(&self) -> Option<u64> {
        match self {
            RegularAt::Regular => Some(0),
            RegularAt::Singular { order } => Some(*order),
            RegularAt::Undecided { .. } => None,
        }
    }

    pub fn to_json(&self) -> Value {
        let of = "this relation at this key and d0";
        match self {
            RegularAt::Regular => json!({"relation": "regular", "pole_order": 0, "of": of}),
            RegularAt::Singular { order } => {
                json!({"relation": "pole", "pole_order": order, "of": of})
            }
            RegularAt::Undecided { reason } => {
                json!({"relation": "undecided", "pole_order": null, "reason": reason, "of": of})
            }
        }
    }
}

/// One right-hand-side term with its NORMALISED order at d0 (`None` = the coefficient is identically zero there).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TermPole {
    /// The key text as the source gives it (`I(..)`, a pattern for rules).
    pub key: String,
    /// `"top"` / `"sub"` when known.
    pub part: Option<String>,
    pub order: Option<i64>,
}

impl TermPole {
    pub fn pole(&self) -> u64 {
        match self.order {
            Some(v) if v < 0 => v.unsigned_abs(),
            _ => 0,
        }
    }
}

/// Summary of a term list: (pole order all, pole order top, terms with pole, non-zero terms).
pub fn summarise(terms: &[TermPole]) -> (u64, u64, usize, usize) {
    let mut all = 0;
    let mut top = 0;
    let mut with = 0;
    let mut nz = 0;
    for t in terms {
        if t.order.is_none() {
            continue;
        }
        nz += 1;
        let p = t.pole();
        if p > 0 {
            with += 1;
        }
        all = all.max(p);
        if t.part.as_deref() != Some("sub") {
            top = top.max(p);
        }
    }
    (all, top, with, nz)
}

/// sha256 hex of the canonical body of an engine relation record (`rustred.relation-entry.v1` 2.1: volatile members
/// dropped, members sorted bytewise, no whitespace).
pub fn entry_digest(record: &Value) -> String {
    let mut body = record.clone();
    if let Some(m) = body.as_object_mut() {
        for k in VOLATILE_RELATION_MEMBERS {
            m.remove(k);
        }
    }
    let t = crate::schema::canon::canonical_json(&body, true).unwrap_or_else(|_| body.to_string());
    crate::replay_base::sha256::sha256_hex(t.as_bytes())
}

/// The D1 record of one relation (scope key) or one rule (scope rule).
#[derive(Debug, Clone, PartialEq)]
pub struct PoleRelationRecord {
    pub node: String,
    pub d0: String,
    /// `"key"` or `"rule"`.
    pub scope: String,
    pub key: Option<Vec<i64>>,
    /// `{table, index, chart?, record_sha256}` for rules.
    pub rule: Option<Value>,
    /// `{kind: recipe|symmetry|point|table|image, id, recipe_digest, step, relation_rule_taken}` (free members).
    pub source: Value,
    /// The relation's identity: `{relation_digest, entry_digest}` (or `{record_sha256}` for rules).
    pub relation: Value,
    /// `"normalised"` (engine records) or `"unnormalised"` (a source that gives the target coefficient).
    pub representation: String,
    /// Order of the unnormalised target coefficient at d0 (`None` = normalised source; `Some(None)` = zero at the key).
    pub target_order: Option<Option<i64>>,
    pub terms: Vec<TermPole>,
    pub verdict: RegularAt,
    /// Rules: the n-independent order of the denominators / numerators and the locus where the order changes.
    pub factor_order: Option<i64>,
    pub locus: Option<Value>,
    /// Rules: per-key verdicts `[{key, verdict, pole_order}]` (TRICKS 9: never one flag per rule).
    pub per_key: Vec<Value>,
    /// A pole of the proof weights / lift hidden poles at d0 (information only; `None` = not known).
    pub certificate_pole_at_d0: Option<bool>,
    /// Section 2 `alternative` (default `{"status":"not_tried"}`) and the choice (section 8), filled by the caller.
    pub alternative: Value,
    pub chosen: Option<String>,
    pub choice_reason: Option<String>,
    pub policy_digest: Option<String>,
    /// Inherent vs spurious (TRICKS 12(c)) when decided: `{verdict: inherent|spurious|undecided, ...}`.
    pub classification: Option<Value>,
    /// How many terms of the maximal order `worst` lists (spec `relations.worst_max`).
    pub worst_max: usize,
    /// The valuation method line of the algebra crate.
    pub method: String,
}

impl PoleRelationRecord {
    /// The record as JSON, with `record_digest` (sha256 of the canonical record without it).
    pub fn to_json(&self) -> Value {
        let (all, top, with, nz) = summarise(&self.terms);
        let mut worst: Vec<&TermPole> = self
            .terms
            .iter()
            .filter(|t| t.pole() > 0 && t.pole() == all)
            .collect();
        worst.sort_by(|a, b| a.key.cmp(&b.key));
        worst.truncate(self.worst_max);
        let mut m = Map::new();
        m.insert("schema".into(), json!(POLE_RELATION_SCHEMA));
        m.insert("node".into(), json!(self.node));
        m.insert("d0".into(), json!(self.d0));
        m.insert("scope".into(), json!(self.scope));
        m.insert("key".into(), json!(self.key));
        m.insert("rule".into(), self.rule.clone().unwrap_or(Value::Null));
        m.insert("source".into(), self.source.clone());
        m.insert("relation".into(), self.relation.clone());
        m.insert("representation".into(), json!(self.representation));
        m.insert(
            "target_order".into(),
            match self.target_order {
                None => Value::Null,
                Some(None) => json!("zero"),
                Some(Some(v)) => json!(v),
            },
        );
        m.insert("verdict".into(), self.verdict.to_json());
        m.insert("pole_order".into(), json!(all));
        m.insert("pole_order_top".into(), json!(top));
        m.insert("terms_with_pole".into(), json!(with));
        m.insert("terms".into(), json!(nz));
        m.insert("terms_listed".into(), json!(self.terms.len()));
        m.insert(
            "worst".into(),
            Value::Array(
                worst
                    .iter()
                    .map(|t| json!({"key": t.key, "order": t.pole()}))
                    .collect(),
            ),
        );
        m.insert(
            "orders".into(),
            Value::Array(
                self.terms
                    .iter()
                    .map(|t| json!([t.key, t.part, t.order.map_or(json!("zero"), |v| json!(v))]))
                    .collect(),
            ),
        );
        m.insert("factor_order".into(), json!(self.factor_order));
        m.insert("locus".into(), self.locus.clone().unwrap_or(Value::Null));
        m.insert("per_key".into(), Value::Array(self.per_key.clone()));
        m.insert(
            "certificate_pole_at_d0".into(),
            json!(self.certificate_pole_at_d0),
        );
        m.insert("alternative".into(), self.alternative.clone());
        m.insert("chosen".into(), json!(self.chosen));
        m.insert("choice_reason".into(), json!(self.choice_reason));
        m.insert("policy_digest".into(), json!(self.policy_digest));
        m.insert(
            "classification".into(),
            self.classification.clone().unwrap_or(Value::Null),
        );
        m.insert("arith".into(), json!("exact"));
        m.insert("method".into(), json!(self.method));
        let v = Value::Object(m);
        let t =
            crate::schema::canon::canonical_json(&v, false).expect("pole records hold no floats");
        let mut v = v;
        v["record_digest"] = json!(crate::replay_base::sha256::sha256_hex(t.as_bytes()));
        v
    }
}

/// Check a record's `record_digest` (a reader's integrity test).
pub fn check_record_digest(v: &Value) -> Result<(), String> {
    let stated = v["record_digest"]
        .as_str()
        .ok_or("no record_digest")?
        .to_string();
    let mut w = v.clone();
    w.as_object_mut()
        .ok_or("not an object")?
        .remove("record_digest");
    let t = crate::schema::canon::canonical_json(&w, false)?;
    let got = crate::replay_base::sha256::sha256_hex(t.as_bytes());
    if got != stated {
        return Err(format!("record_digest {stated} != computed {got}"));
    }
    if v["schema"] != POLE_RELATION_SCHEMA {
        return Err(format!(
            "schema {} is not {POLE_RELATION_SCHEMA}",
            v["schema"]
        ));
    }
    Ok(())
}
