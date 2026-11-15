//! The ROUTE-POLE census of a closure (lane poles, B4; pole-diagnostics.md section 4 read with POLES_OBJECTIVE.md 2(A)):
//! for every served key, the pole order at d0 of the relation that SERVES it (the closure's first-match winner, its
//! coefficients evaluated exactly at the key by the algebra crate), summarised per rule and per key.
//!
//! WHAT IT IS NOT: a statement about need. A route pole cancels in the composed reduction on a regular terminal set
//! (POLES_OBJECTIVE 2(A)); the composed census on a terminal set (eps need of the terminals) belongs to lanes epsdepth /
//! regbasis. This census measures the ROUTE: how many used rules carry a pole somewhere, at how many keys, of which order:
//! the input of the preference and of the cures of TRICKS 12-15.
//!
//! Rows carry `in_box` so the summary is given for the box and for the whole closure. A row whose verdict is undecided is
//! counted apart, never as regular. Pure: no I/O.

use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// Schema of the census summary.
pub const ROUTE_CENSUS_SCHEMA: &str = "rustred.campaign.route-pole-census.v1";

/// The key a right-hand-side pattern names at `key`: `I(a0,..)` with each argument an integer, `n<k>`, `n<k>+c` or
/// `n<k>-c` (the rule-record pattern grammar; literal keys are the all-integer case). `None` = not this grammar.
pub fn eval_key_pattern(text: &str, key: &[i64]) -> Option<Vec<i64>> {
    let body = text.trim().strip_prefix("I(")?.strip_suffix(')')?;
    body.split(',')
        .map(|a| {
            let a = a.trim();
            if let Some(rest) = a.strip_prefix('n') {
                let cut = rest.find(['+', '-']).unwrap_or(rest.len());
                let k: usize = rest[..cut].parse().ok()?;
                let off: i64 = if cut == rest.len() {
                    0
                } else {
                    rest[cut..].parse().ok()?
                };
                Some(key.get(k)? + off)
            } else {
                a.parse().ok()
            }
        })
        .collect()
}

/// Is `rhs` in the top sector of `key` (every slot >= 1 at the key is >= 1 in rhs)?
pub fn in_top_sector(key: &[i64], rhs: &[i64]) -> bool {
    key.len() == rhs.len() && key.iter().zip(rhs).all(|(k, r)| *k < 1 || *r >= 1)
}

/// (dots, rank) of a top-sector key: lines = slots >= 1 (dots = sum of power - 1), numerators = slots <= 0.
pub fn dots_rank(key: &[i64]) -> (i64, i64) {
    key.iter().fold(
        (0, 0),
        |(d, r), &x| if x >= 1 { (d + x - 1, r) } else { (d, r - x) },
    )
}

/// One served key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CensusRow {
    pub key: Vec<i64>,
    /// The serving rule's label (catalogue name); `None` for a terminal (not served).
    pub label: Option<String>,
    /// The serving class (bank, face, point, recipe, symmetry, terminal, ...): data, free text.
    pub class: String,
    pub in_box: bool,
    /// `Some(0)` regular, `Some(p)` pole of order p, `None` undecided (terminals: `Some(0)`, no relation).
    pub order: Option<u64>,
    /// Terms of the serving relation with a pole at the key.
    pub terms_with_pole: usize,
    /// The largest pole order over TOP-sector terms (`None` = unknown / undecided).
    pub order_top: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Part {
    keys: usize,
    keys_with_pole: usize,
    keys_with_pole_top: usize,
    keys_with_pole_sub_only: usize,
    undecided: usize,
    hist: BTreeMap<u64, usize>,
    rules: BTreeSet<String>,
    rules_with_pole: BTreeSet<String>,
    by_class: BTreeMap<String, (usize, usize)>,
}

impl Part {
    fn add(&mut self, r: &CensusRow) {
        self.keys += 1;
        let c = self.by_class.entry(r.class.clone()).or_default();
        c.0 += 1;
        if let Some(l) = &r.label {
            self.rules.insert(l.clone());
        }
        match r.order {
            None => self.undecided += 1,
            Some(p) => {
                *self.hist.entry(p).or_default() += 1;
                if p > 0 {
                    self.keys_with_pole += 1;
                    if r.order_top.is_some_and(|t| t > 0) {
                        self.keys_with_pole_top += 1;
                    } else if r.order_top == Some(0) {
                        self.keys_with_pole_sub_only += 1;
                    }
                    c.1 += 1;
                    if let Some(l) = &r.label {
                        self.rules_with_pole.insert(l.clone());
                    }
                }
            }
        }
    }

    fn to_json(&self) -> Value {
        json!({
            "keys": self.keys,
            "keys_with_pole": self.keys_with_pole,
            "keys_with_pole_on_a_top_term": self.keys_with_pole_top,
            "keys_with_pole_on_subsector_terms_only": self.keys_with_pole_sub_only,
            "undecided": self.undecided,
            "rules_used": self.rules.len(),
            "rules_with_pole": self.rules_with_pole.len(),
            "max_order": self.hist.keys().next_back().copied().unwrap_or(0),
            "keys_by_order": self.hist.iter().map(|(k, v)| (k.to_string(), json!(v))).collect::<serde_json::Map<_, _>>(),
            "by_class": self.by_class.iter().map(|(k, (n, p))| (k.clone(), json!({"keys": n, "keys_with_pole": p})))
                .collect::<serde_json::Map<_, _>>(),
        })
    }
}

/// Per rule: keys served, keys with a pole, max order (box and closure together).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuleCensus {
    pub keys: usize,
    pub keys_in_box: usize,
    pub keys_with_pole: usize,
    pub keys_with_pole_in_box: usize,
    pub max_order: u64,
    pub undecided: usize,
}

/// The summary of a census (box and closure parts, per-rule table).
#[derive(Debug, Clone, PartialEq)]
pub struct CensusSummary {
    box_part: Part,
    closure_part: Part,
    pub per_rule: BTreeMap<String, RuleCensus>,
}

/// Summarise rows (any order; the result does not depend on it).
pub fn summarise(rows: &[CensusRow]) -> CensusSummary {
    let mut b = Part::default();
    let mut c = Part::default();
    let mut per_rule: BTreeMap<String, RuleCensus> = BTreeMap::new();
    for r in rows {
        c.add(r);
        if r.in_box {
            b.add(r);
        }
        if let Some(l) = &r.label {
            let e = per_rule.entry(l.clone()).or_default();
            e.keys += 1;
            e.keys_in_box += usize::from(r.in_box);
            match r.order {
                None => e.undecided += 1,
                Some(p) if p > 0 => {
                    e.keys_with_pole += 1;
                    e.keys_with_pole_in_box += usize::from(r.in_box);
                    e.max_order = e.max_order.max(p);
                }
                Some(_) => {}
            }
        }
    }
    CensusSummary {
        box_part: b,
        closure_part: c,
        per_rule,
    }
}

impl CensusSummary {
    /// `rustred.campaign.route-pole-census.v1` summary (no per-key rows; those are written beside it).
    pub fn to_json(&self, d0: &str, extra: Value) -> Value {
        let mut rules_by_max: BTreeMap<u64, usize> = BTreeMap::new();
        for r in self.per_rule.values().filter(|r| r.keys_with_pole > 0) {
            *rules_by_max.entry(r.max_order).or_default() += 1;
        }
        json!({
            "schema": ROUTE_CENSUS_SCHEMA,
            "d0": d0,
            "meaning": "route poles: the order at d0 of the relation SERVING each key, exact at the key; never a statement about the eps need of the terminals (composition on a regular terminal set decides that)",
            "box": self.box_part.to_json(),
            "closure": self.closure_part.to_json(),
            "rules_with_pole_by_max_order": rules_by_max.iter().map(|(k, v)| (k.to_string(), json!(v))).collect::<serde_json::Map<_, _>>(),
            "inputs": extra,
        })
    }
}
