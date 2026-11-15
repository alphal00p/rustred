//! The SYMMETRY PIECES of a node's open keys (interfaces/repairflow.md 17-19): summaries per face of the composite
//! outcomes ([`super::composite`]), the services a node ships, and the GATE over them: a relation graph over KEYS (never
//! faces), 0 non-descending edges and an explicit acyclicity check (TRICKS 1, 4). Pure: no file, no process.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde_json::{Value, json};

use super::composite::{Composite, ElementOutcome, KeyComposites};
use crate::worklist::items::Face;

/// One relation the flow ships for a key (keys only: the gate needs no coefficient).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyService {
    pub key: Vec<i32>,
    pub face: String,
    /// `plain` (route 0b image alone), `composite` (image + catalogue rules), `two_row`, `core`, ...
    pub kind: String,
    pub element: Option<usize>,
    pub rhs: Vec<Vec<i32>>,
}

impl KeyService {
    pub fn from_composite<E>(key: &[i32], c: &Composite<E>, face: &str) -> KeyService {
        KeyService {
            key: key.to_vec(),
            face: face.to_string(),
            kind: if c.is_plain() { "plain" } else { "composite" }.into(),
            element: Some(c.element),
            rhs: c.rhs.iter().map(|(k, _)| k.clone()).collect(),
        }
    }
}

/// Per face: keys, served by a plain relation, by a composite (and of those, how many have NO plain alternative =
/// keys the image-only check rejected), not served (with the refusal classes of every element), errors.
pub fn summarise<E>(
    results: &[(Vec<i32>, Result<&KeyComposites<E>, String>)],
    lines: &[bool],
) -> Value {
    #[derive(Default)]
    struct F {
        keys: u64,
        plain: u64,
        composite: u64,
        composite_no_plain: u64,
        none: u64,
        error: u64,
        over_cap: u64,
        with_key_coefficient: u64,
        dropped_zero_terms: u64,
        terms: Vec<u64>,
        refusals: BTreeMap<String, u64>,
        best_elements: BTreeMap<usize, u64>,
    }
    let mut by: BTreeMap<String, F> = BTreeMap::new();
    for (k, r) in results {
        let face = Face::of_key(k, lines)
            .map(|f| f.label())
            .unwrap_or_default();
        let f = by.entry(face).or_default();
        f.keys += 1;
        let kc = match r {
            Ok(kc) => kc,
            Err(e) => {
                if super::composite::error_class(e).starts_with("over_cap") {
                    f.over_cap += 1;
                } else {
                    f.error += 1;
                }
                continue;
            }
        };
        match kc.best() {
            Some(b) => {
                *f.best_elements.entry(b.element).or_default() += 1;
                f.terms.push(b.rhs.len() as u64);
                f.dropped_zero_terms += b.dropped_zero.len() as u64;
                if b.key_coefficient.is_some() {
                    f.with_key_coefficient += 1;
                }
                if b.is_plain() {
                    f.plain += 1;
                } else {
                    f.composite += 1;
                    if !kc.accepted().any(|c| c.is_plain()) {
                        f.composite_no_plain += 1;
                    }
                }
            }
            None => {
                f.none += 1;
                for o in &kc.outcomes {
                    if let ElementOutcome::Refused { why, .. } = o {
                        *f.refusals.entry(why.class().to_string()).or_default() += 1;
                    }
                }
            }
        }
    }
    let mut totals = json!({"keys": 0, "plain": 0, "composite": 0, "composite_no_plain": 0, "none": 0,
        "over_cap": 0, "error": 0});
    let mut faces = serde_json::Map::new();
    for (face, mut f) in by {
        for (n, v) in [
            ("keys", f.keys),
            ("plain", f.plain),
            ("composite", f.composite),
            ("composite_no_plain", f.composite_no_plain),
            ("none", f.none),
            ("over_cap", f.over_cap),
            ("error", f.error),
        ] {
            totals[n] = json!(totals[n].as_u64().unwrap_or(0) + v);
        }
        f.terms.sort_unstable();
        let med = f.terms.get(f.terms.len() / 2).copied();
        faces.insert(
            face,
            json!({"keys": f.keys, "plain": f.plain, "composite": f.composite,
                "composite_no_plain": f.composite_no_plain, "none": f.none, "over_cap": f.over_cap, "error": f.error,
                "with_key_coefficient": f.with_key_coefficient, "dropped_zero_terms": f.dropped_zero_terms,
                "terms": {"min": f.terms.first(), "median": med, "max": f.terms.last()},
                "refusals_of_unserved": f.refusals, "best_elements": f.best_elements}),
        );
    }
    json!({"totals": totals, "faces": faces})
}

/// The GATE over everything the flow ships for a node: the relation graph over keys, every edge checked with the
/// import's descent test (`lower(rhs, key)`), and an explicit acyclicity check (Kahn's algorithm over the keys that
/// have a shipped relation; a right-hand-side key without one is a leaf). Never per face.
pub fn gate(services: &[KeyService], lower: &dyn Fn(&[i32], &[i32]) -> bool) -> Value {
    let mut non_desc: Vec<Value> = vec![];
    let mut edges = 0u64;
    let mut idx: HashMap<&Vec<i32>, usize> = HashMap::new();
    let mut dup = 0u64;
    for (i, s) in services.iter().enumerate() {
        if idx.insert(&s.key, i).is_some() {
            dup += 1;
        }
    }
    let n = services.len();
    let mut indeg = vec![0usize; n];
    let mut out: Vec<Vec<usize>> = vec![vec![]; n];
    for (i, s) in services.iter().enumerate() {
        for r in &s.rhs {
            edges += 1;
            if !lower(r, &s.key) && non_desc.len() < 20 {
                non_desc.push(json!({"key": s.key, "rhs": r, "kind": s.kind}));
            }
            if let Some(&j) = idx.get(r) {
                out[i].push(j);
                indeg[j] += 1;
            }
        }
    }
    let non_descending = services
        .iter()
        .map(|s| s.rhs.iter().filter(|r| !lower(r, &s.key)).count() as u64)
        .sum::<u64>();
    let mut stack: Vec<usize> = (0..n).filter(|&i| indeg[i] == 0).collect();
    let mut seen = 0usize;
    while let Some(i) = stack.pop() {
        seen += 1;
        for &j in &out[i] {
            indeg[j] -= 1;
            if indeg[j] == 0 {
                stack.push(j);
            }
        }
    }
    let cyclic = n - seen;
    let faces: BTreeSet<&str> = services.iter().map(|s| s.face.as_str()).collect();
    let pass = non_descending == 0 && cyclic == 0 && dup == 0;
    json!({"verdict": if pass { "pass" } else { "fail" }, "relations": n, "edges": edges,
        "non_descending_edges": non_descending, "first_non_descending": non_desc, "cyclic_keys": cyclic,
        "duplicate_keys": dup, "faces": faces.len(),
        "method": "every edge: import descent test; acyclicity: Kahn over the keys with a shipped relation"})
}
