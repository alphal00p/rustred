//! SPLIT A DOMAIN by the per-key verdicts of ONE recipe / rule (its relation at each key: regular or with a pole of
//! order p; never a statement that the key itself is singular, TRICKS 15 / 16) (TRICKS items 12(b) and 13): a recipe or rule tested at EVERY key of its
//! domain gives regular / singular / undecided parts; this module groups them and derives an exact index condition
//! when one exists among simple forms, else keeps the key lists. It also names the generic CURE CANDIDATES of item 13
//! (a hint for the escalation; the pole lane solves nothing itself).
//!
//! Index condition (derived from the keys alone, exact on the tested keys, generic in the arity): the conjunction of
//! the slot equalities `n_s = v` shared by EVERY singular key; it is reported as `exact_on_tested_keys` when no regular
//! key satisfies it, and reduced to the smallest single equality with that property when one exists. A condition that
//! some regular key also satisfies is not reported (the key lists stand).
//!
//! Cure candidates (item 13): for a singular part whose condition fixes a LINE slot (a slot >= 1 at every key of the
//! domain) at the LOWEST value it takes over the domain, with that value > 1 (a dotted line at its lowest power): the
//! centre with that line one step UP (`{"slot": s, "offset": +1}`), listed first; the regular part keeps its recipe.

use super::record::RegularAt;
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// The split of one domain.
#[derive(Debug, Clone, PartialEq)]
pub struct DomainSplit {
    pub regular: Vec<Vec<i64>>,
    /// (key, pole order)
    pub singular: Vec<(Vec<i64>, u64)>,
    pub undecided: Vec<(Vec<i64>, String)>,
    /// Slot equalities `(slot, value)` shared by every singular key and satisfied together by no regular key.
    pub condition: Option<Vec<(usize, i64)>>,
    /// Generic cure candidates: `{"slot", "offset", "why"}`.
    pub cures: Vec<Value>,
}

/// Group `per_key` verdicts and derive the condition / cures (keys of one arity; empty input = empty split).
pub fn split_domain(per_key: &[(Vec<i64>, RegularAt)]) -> DomainSplit {
    let mut regular = vec![];
    let mut singular = vec![];
    let mut undecided = vec![];
    for (k, v) in per_key {
        match v {
            RegularAt::Regular => regular.push(k.clone()),
            RegularAt::Singular { order } => singular.push((k.clone(), *order)),
            RegularAt::Undecided { reason } => undecided.push((k.clone(), reason.clone())),
        }
    }
    regular.sort();
    singular.sort();
    undecided.sort();
    let mut condition = None;
    let mut cures = vec![];
    if let Some((first, _)) = singular.first() {
        let n = first.len();
        // equalities shared by every singular key
        let shared: Vec<(usize, i64)> = (0..n)
            .filter(|&s| singular.iter().all(|(k, _)| k.get(s) == first.get(s)))
            .map(|s| (s, first[s]))
            .collect();
        let sat = |k: &Vec<i64>, c: &[(usize, i64)]| c.iter().all(|&(s, v)| k.get(s) == Some(&v));
        let singles: Vec<(usize, i64)> = shared
            .iter()
            .copied()
            .filter(|e| !regular.iter().any(|k| sat(k, std::slice::from_ref(e))))
            .collect();
        if let Some(e) = singles.first() {
            condition = Some(vec![*e]);
        } else if !shared.is_empty() && !regular.iter().any(|k| sat(k, &shared)) {
            condition = Some(shared.clone());
        }
        // cure candidates: a line slot of the condition (or of the shared equalities) at its domain minimum > 1
        let all_keys: Vec<&Vec<i64>> = regular
            .iter()
            .chain(singular.iter().map(|(k, _)| k))
            .chain(undecided.iter().map(|(k, _)| k))
            .collect();
        let mut seen = BTreeMap::new();
        for &(s, v) in condition.as_deref().unwrap_or(&shared) {
            let is_line = all_keys.iter().all(|k| k.get(s).is_some_and(|x| *x >= 1));
            let min = all_keys.iter().filter_map(|k| k.get(s)).min().copied();
            if is_line && v > 1 && min == Some(v) && seen.insert(s, ()).is_none() {
                cures.push(json!({"slot": s, "offset": 1,
                    "why": format!("every singular key has line n{s} at its lowest power {v} over the domain: centre with n{s} one step up (TRICKS 13)")}));
            }
        }
    }
    DomainSplit {
        regular,
        singular,
        undecided,
        condition,
        cures,
    }
}

impl DomainSplit {
    pub fn to_json(&self) -> Value {
        json!({
            "regular": {"keys": self.regular.len(), "list": self.regular},
            "pole": {"keys": self.singular.len(),
                "list": self.singular.iter().map(|(k, o)| json!({"key": k, "pole_order": o})).collect::<Vec<_>>()},
            "undecided": {"keys": self.undecided.len(),
                "list": self.undecided.iter().map(|(k, r)| json!({"key": k, "reason": r})).collect::<Vec<_>>()},
            "condition": self.condition.as_ref().map(|c| json!({
                "equalities": c.iter().map(|(s, v)| json!({"slot": s, "value": v})).collect::<Vec<_>>(),
                "text": c.iter().map(|(s, v)| format!("n{s} = {v}")).collect::<Vec<_>>().join(" and "),
                "exact_on_tested_keys": true})),
            "cure_candidates": self.cures,
        })
    }
}
