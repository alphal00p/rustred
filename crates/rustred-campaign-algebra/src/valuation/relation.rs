//! D1 per relation (lane poles, B1; pole-diagnostics.md section 2; TRICKS items 9, 12, 13): the exact pole order at d0
//! of a relation at a key, from the relation AS ITS SOURCE GIVES IT, and the D1 record.
//!
//! ## API FOR OTHER LANES (stable)
//! [`is_regular_at`]`(record, key, d0) -> RegularAt` (`Regular` | `Singular { order }` | `Undecided { reason }`) and
//! [`pole_order_of`]`(record, key, d0) -> Result<u64, String>` (the ORDER; `Err` = undecided). Both judge THIS
//! RELATION at this key (TRICKS 15 / 16: never the key, the system of rows, or "inherent"):
//! - `record` = one JSON record: an engine solve-recipe `"type":"relation"` record (results.jsonl; coefficients are
//!   rational functions of d alone, the key already in; NORMALISED to target coefficient 1), a table `"type":"rule"`
//!   record (parametric coefficients in `n0 .. n{N-1}`, d: the key's indices are PUT IN before the valuation, TRICKS 9),
//!   or a generic relation `{"key"?, "target_coefficient": "<text>", "rhs": [[key, coefficient, part?], ..]}` meaning
//!   `c_0 I(key) = sum_j c_j I(k_j)` (UNNORMALISED; TRICKS 12(a): the target coefficient is tested too);
//! - `key` = the key the relation is applied at (must equal the record's `key` when it has one);
//! - exact (Symbolica, [`super::Coefficient`]); cost = parsing the coefficient texts once.
//! The verdict is made on the NORMALISED orders `v(c_j) - v(c_0)` (`v(c_0) = 0` for normalised records), so a zero of
//! the target coefficient at d0 and the equivalent rhs poles of the normalised form give the SAME verdict.
//! A rule record is assumed to APPLY at the key (domain and exceptions are the caller's: this call judges coefficients).
//!
//! [`relation_poles`] gives the per-term orders; [`relation_record`] / [`rule_record`] build the
//! `rustred.campaign.pole-relation.v1` record of the core crate.

use super::{Coefficient, D0, GenericOrder, Prepared, VALUATION_METHOD, Valuation};
use rustred_campaign_core::poles::record::{
    PoleRelationRecord, RegularAt, TermPole, relation_identity, summarise,
};
use serde_json::{Value, json};

/// The source form of a relation record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    /// Engine solve-recipe relation: coefficients in d alone, normalised.
    EngineRelation,
    /// Table rule: coefficients in the indices and d, normalised.
    TableRule,
    /// A relation with an explicit (unnormalised) target coefficient.
    Unnormalised,
}

/// One relation's terms as text.
#[derive(Debug, Clone)]
pub struct RelationText {
    pub form: Form,
    pub key: Option<Vec<i64>>,
    pub target_coefficient: Option<String>,
    /// (key text, coefficient text, part)
    pub terms: Vec<(String, String, Option<String>)>,
}

impl RelationText {
    /// Read a record (see the module header). `Err` = not a relation this module can judge.
    pub fn of_record(record: &Value) -> Result<RelationText, String> {
        let key = match &record["key"] {
            Value::Null => None,
            k => Some(
                k.as_array()
                    .ok_or("key is not a list")?
                    .iter()
                    .map(|x| x.as_i64().ok_or("key entry is not an integer"))
                    .collect::<Result<Vec<i64>, _>>()?,
            ),
        };
        let target_coefficient = match &record["target_coefficient"] {
            Value::Null => None,
            Value::String(s) => Some(s.clone()),
            _ => return Err("target_coefficient is not a text".into()),
        };
        let form = match (record["type"].as_str(), &target_coefficient) {
            (_, Some(_)) => Form::Unnormalised,
            (Some("relation"), None) => {
                let v = record["verdict"].as_str().unwrap_or("");
                if !v.starts_with("SOLVED") {
                    return Err(format!("relation record with verdict {v:?} (no relation)"));
                }
                Form::EngineRelation
            }
            (Some("rule"), None) => Form::TableRule,
            (t, None) => {
                return Err(format!(
                    "record type {t:?} is neither a relation nor a rule"
                ));
            }
        };
        let rhs = record["rhs"].as_array().ok_or("rhs is not a list")?;
        let mut terms = Vec::with_capacity(rhs.len());
        for (i, x) in rhs.iter().enumerate() {
            let a = x
                .as_array()
                .ok_or_else(|| format!("rhs[{i}] is not a list"))?;
            let k = a
                .first()
                .and_then(Value::as_str)
                .ok_or_else(|| format!("rhs[{i}] key"))?;
            let c = a
                .get(1)
                .and_then(Value::as_str)
                .ok_or_else(|| format!("rhs[{i}] coefficient"))?;
            let part = a.get(2).and_then(Value::as_str).map(str::to_string);
            terms.push((k.to_string(), c.to_string(), part));
        }
        Ok(RelationText {
            form,
            key,
            target_coefficient,
            terms,
        })
    }
}

/// The per-term NORMALISED orders of a relation at a key, and the target coefficient's own order (unnormalised form).
#[derive(Debug, Clone, PartialEq)]
pub struct RelationPoles {
    /// `None` = normalised source; `Some(v)` = the order of the target coefficient (`v > 0`: it vanishes at d0).
    pub target_order: Option<i64>,
    pub terms: Vec<TermPole>,
}

impl RelationPoles {
    pub fn verdict(&self) -> RegularAt {
        let (all, ..) = summarise(&self.terms);
        if all == 0 {
            RegularAt::Regular
        } else {
            RegularAt::Singular { order: all }
        }
    }
}

/// Orders of every term at `key` (`key` used to substitute indices of a rule; checked against the record's key).
pub fn relation_poles(rel: &RelationText, key: &[i64], d0: &D0) -> Result<RelationPoles, String> {
    if let Some(k) = &rel.key
        && k.as_slice() != key
    {
        return Err(format!("the relation is for the key {k:?}, not {key:?}"));
    }
    // engine relations: coefficients in d only (arity 0); rules and generic relations may carry indices
    let arity = match rel.form {
        Form::EngineRelation => 0,
        Form::TableRule | Form::Unnormalised => key.len(),
    };
    let order = |text: &str| -> Result<Valuation, String> {
        let c = Coefficient::parse(text, arity)
            .map_err(|e| format!("coefficient {}: {e}", short(text)))?;
        if arity == 0 {
            c.order(d0)
        } else {
            c.at_key(key, d0)
        }
        .map_err(|e| format!("coefficient {}: {e}", short(text)))
    };
    let v0 = match &rel.target_coefficient {
        None => None,
        Some(t) => match order(t)? {
            Valuation::Zero => {
                return Err(
                    "the target coefficient vanishes at the key: no relation for the target".into(),
                );
            }
            Valuation::Order(v) => Some(v),
        },
    };
    let mut terms = Vec::with_capacity(rel.terms.len());
    for (k, c, part) in &rel.terms {
        let o = match order(c)? {
            Valuation::Zero => None,
            Valuation::Order(v) => Some(v - v0.unwrap_or(0)),
        };
        terms.push(TermPole {
            key: k.clone(),
            part: part.clone(),
            order: o,
        });
    }
    Ok(RelationPoles {
        target_order: v0,
        terms,
    })
}

fn short(t: &str) -> String {
    if t.len() <= 80 {
        t.to_string()
    } else {
        format!("{}.. ({} bytes)", &t[..t.floor_char_boundary(80)], t.len())
    }
}

/// THE CALL for other lanes (module header): regular / singular(order) / undecided(reason) of `record` at `key`.
pub fn is_regular_at(record: &Value, key: &[i64], d0: &D0) -> RegularAt {
    let rel = match RelationText::of_record(record) {
        Ok(r) => r,
        Err(e) => return RegularAt::Undecided { reason: e },
    };
    match relation_poles(&rel, key, d0) {
        Ok(p) => p.verdict(),
        Err(e) => RegularAt::Undecided { reason: e },
    }
}

/// The pole order at d0 of THIS relation at `key` (0 = regular): the order of the record, not of the key or of the
/// system of rows (TRICKS 15 / 16). `Err(reason)` = undecided.
pub fn pole_order_of(record: &Value, key: &[i64], d0: &D0) -> Result<u64, String> {
    match is_regular_at(record, key, d0) {
        RegularAt::Undecided { reason } => Err(reason),
        v => Ok(v.pole_order().unwrap_or(0)),
    }
}

/// The D1 record (scope `key`) of one relation at `key`. `source` = `{kind, id, recipe_digest, step, ..}` (caller's).
pub fn relation_record(
    record: &Value,
    key: &[i64],
    d0: &D0,
    node: &str,
    source: Value,
    worst_max: usize,
) -> PoleRelationRecord {
    let rel = RelationText::of_record(record);
    let (verdict, target_order, terms) = match rel
        .as_ref()
        .map_err(Clone::clone)
        .and_then(|r| relation_poles(r, key, d0))
    {
        Ok(p) => (p.verdict(), p.target_order.map(Some), p.terms),
        Err(e) => (RegularAt::Undecided { reason: e }, None, vec![]),
    };
    let representation = match rel.as_ref().map(|r| r.form) {
        Ok(Form::Unnormalised) => "unnormalised",
        _ => "normalised",
    };
    let relation = relation_identity(record);
    // d_poles mixes the lift's hidden poles and the coefficient denominators (C3): the certificate side is known only
    // when the engine reports `hidden_d_poles` (line 7); their (d - d0) factors are checked as coefficients of d
    let certificate_pole_at_d0 = record["hidden_d_poles"].as_array().map(|hs| {
        hs.iter().filter_map(Value::as_str).any(|h| {
            Coefficient::parse(&format!("1/({h})"), 0)
                .ok()
                .and_then(|c| c.order(d0).ok())
                .is_some_and(|v| v.pole_order() > 0)
        })
    });
    PoleRelationRecord {
        node: node.to_string(),
        d0: d0.text().to_string(),
        scope: "key".into(),
        key: Some(key.to_vec()),
        rule: None,
        source,
        relation,
        representation: representation.into(),
        target_order,
        terms,
        verdict,
        factor_order: None,
        locus: None,
        per_key: vec![],
        certificate_pole_at_d0,
        alternative: json!({"status": "not_tried"}),
        chosen: None,
        choice_reason: None,
        policy_digest: None,
        classification: None,
        worst_max,
        method: VALUATION_METHOD.into(),
    }
}

/// The per-rule analysis of a parametric rule: every coefficient's GENERIC order and loci (Symbolica), and the
/// per-key verdicts at the given keys (each exact, indices put in).
#[derive(Debug, Clone)]
pub struct RuleAnalysis {
    /// The rule's n-independent pole order: max over coefficients of the generic pole.
    pub factor_order: u64,
    /// Per coefficient with a non-empty locus: (term key, generic order, den locus factors, num locus factors).
    pub loci: Vec<(String, GenericOrder)>,
    /// Per key: verdict.
    pub per_key: Vec<(Vec<i64>, RegularAt)>,
}

/// Analyse a rule record at `keys` (each key: arity = key length; the rule must apply at each).
pub fn analyse_rule(
    record: &Value,
    keys: &[Vec<i64>],
    arity: usize,
    d0: &D0,
) -> Result<RuleAnalysis, String> {
    let rel = RelationText::of_record(record)?;
    let mut parsed = Vec::with_capacity(rel.terms.len());
    for (k, c, _) in &rel.terms {
        let co = Coefficient::parse(c, arity).map_err(|e| format!("coefficient of {k}: {e}"))?;
        parsed.push((k.clone(), co));
    }
    let target = match &rel.target_coefficient {
        Some(t) => {
            Some(Coefficient::parse(t, arity).map_err(|e| format!("target coefficient: {e}"))?)
        }
        None => None,
    };
    let mut factor_order = 0u64;
    let mut loci = vec![];
    for (k, co) in &parsed {
        let g = co.generic(d0);
        factor_order = factor_order.max(g.order.pole_order());
        if !g.den_locus.is_empty() || !g.num_locus.is_empty() {
            loci.push((k.clone(), g));
        }
    }
    let mut per_key = Vec::with_capacity(keys.len());
    for key in keys {
        let v = (|| -> Result<RegularAt, String> {
            let v0 = match &target {
                None => 0,
                Some(t) => match t.at_key(key, d0).map_err(|e| e.to_string())? {
                    Valuation::Zero => {
                        return Err("the target coefficient vanishes at the key".into());
                    }
                    Valuation::Order(v) => v,
                },
            };
            let mut worst = 0u64;
            for (k, co) in &parsed {
                if let Valuation::Order(v) = co
                    .at_key(key, d0)
                    .map_err(|e| format!("coefficient of {k}: {e}"))?
                {
                    let n = v - v0;
                    if n < 0 {
                        worst = worst.max(n.unsigned_abs());
                    }
                }
            }
            Ok(if worst == 0 {
                RegularAt::Regular
            } else {
                RegularAt::Singular { order: worst }
            })
        })()
        .unwrap_or_else(|reason| RegularAt::Undecided { reason });
        per_key.push((key.clone(), v));
    }
    Ok(RuleAnalysis {
        factor_order,
        loci,
        per_key,
    })
}

/// The D1 record (scope `rule`) of a parametric rule with its per-key verdicts and loci (TRICKS 9: never one flag per
/// rule). `rule_id` = `{table, index, chart?, record_sha256}`.
pub fn rule_record(
    record: &Value,
    keys: &[Vec<i64>],
    arity: usize,
    d0: &D0,
    node: &str,
    rule_id: Value,
    worst_max: usize,
) -> PoleRelationRecord {
    let (verdict, factor_order, locus, per_key) = match analyse_rule(record, keys, arity, d0) {
        Ok(a) => {
            let worst = a
                .per_key
                .iter()
                .filter_map(|(_, v)| v.pole_order())
                .max()
                .unwrap_or(0);
            let undecided = a
                .per_key
                .iter()
                .any(|(_, v)| matches!(v, RegularAt::Undecided { .. }));
            let verdict = if undecided {
                RegularAt::Undecided {
                    reason: "undecided at some key (per_key)".into(),
                }
            } else if worst == 0 {
                RegularAt::Regular
            } else {
                RegularAt::Singular { order: worst }
            };
            let locus = json!({
                "terms": a.loci.iter().map(|(k, g)| json!({"key": k, "generic_order": g.order,
                    "den_multiplicity": g.den_multiplicity, "num_multiplicity": g.num_multiplicity,
                    "pole_rises_on": g.den_locus.factors, "pole_falls_on": g.num_locus.factors})).collect::<Vec<_>>(),
                "meaning": "the order at a key differs from the generic order only on the union of these zero sets (index polynomials at d = d0); every per-key verdict is computed at the key",
            });
            let per_key: Vec<Value> = a
                .per_key
                .iter()
                .map(|(k, v)| {
                    let mut j = v.to_json();
                    j["key"] = json!(k);
                    j
                })
                .collect();
            (verdict, Some(a.factor_order as i64), Some(locus), per_key)
        }
        Err(e) => (RegularAt::Undecided { reason: e }, None, None, vec![]),
    };
    PoleRelationRecord {
        node: node.to_string(),
        d0: d0.text().to_string(),
        scope: "rule".into(),
        key: None,
        rule: Some(rule_id.clone()),
        source: json!({"kind": "table"}),
        relation: json!({"record_sha256": rule_id["record_sha256"]}),
        representation: if record["target_coefficient"].is_null() {
            "normalised"
        } else {
            "unnormalised"
        }
        .into(),
        target_order: None,
        terms: vec![],
        verdict,
        factor_order,
        locus,
        per_key,
        certificate_pole_at_d0: None,
        alternative: json!({"status": "not_tried"}),
        chosen: None,
        choice_reason: None,
        policy_digest: None,
        classification: None,
        worst_max,
        method: VALUATION_METHOD.into(),
    }
}

/// A rule / relation record parsed ONCE and prepared at one d0, evaluated at many keys (the census path): per key the
/// pole order of the record's NORMALISED coefficients (target coefficient tested too when given), exact
/// ([`Prepared::at_key`]).
#[derive(Debug, Clone)]
pub struct PreparedRule {
    terms: Vec<(String, Prepared)>,
    target: Option<Prepared>,
}

impl PreparedRule {
    /// `arity` = the key length (engine relation records: their coefficients hold no index; any arity works).
    pub fn of_record(record: &Value, arity: usize, d0: &D0) -> Result<PreparedRule, String> {
        let rel = RelationText::of_record(record)?;
        let mut terms = Vec::with_capacity(rel.terms.len());
        for (k, c, _) in &rel.terms {
            let co =
                Coefficient::parse(c, arity).map_err(|e| format!("coefficient of {k}: {e}"))?;
            terms.push((k.clone(), Prepared::new(co, d0)));
        }
        let target = match &rel.target_coefficient {
            Some(t) => Some(Prepared::new(
                Coefficient::parse(t, arity).map_err(|e| format!("target coefficient: {e}"))?,
                d0,
            )),
            None => None,
        };
        Ok(PreparedRule { terms, target })
    }

    pub fn terms(&self) -> usize {
        self.terms.len()
    }

    /// The verdict of THIS record at `key` and the number of terms with a pole there.
    pub fn at_key(&self, key: &[i64]) -> (RegularAt, usize) {
        let v0 = match &self.target {
            None => 0,
            Some(t) => match t.at_key(key) {
                Ok(Valuation::Order(v)) => v,
                Ok(Valuation::Zero) => {
                    return (
                        RegularAt::Undecided {
                            reason: "the target coefficient vanishes at the key".into(),
                        },
                        0,
                    );
                }
                Err(e) => {
                    return (
                        RegularAt::Undecided {
                            reason: format!("target coefficient: {e}"),
                        },
                        0,
                    );
                }
            },
        };
        let mut worst = 0u64;
        let mut with = 0usize;
        for (k, p) in &self.terms {
            match p.at_key(key) {
                Ok(Valuation::Order(v)) if v - v0 < 0 => {
                    with += 1;
                    worst = worst.max((v - v0).unsigned_abs());
                }
                Ok(_) => {}
                Err(e) => {
                    return (
                        RegularAt::Undecided {
                            reason: format!("coefficient of {k}: {e}"),
                        },
                        0,
                    );
                }
            }
        }
        (
            if worst == 0 {
                RegularAt::Regular
            } else {
                RegularAt::Singular { order: worst }
            },
            with,
        )
    }
}
