//! SECOND ROUTES from the engine (lane symcert; comparison only, NEVER evidence):
//! (a) `rr_node symface` (frozen engine) per key against our own symmetry choices on the same keys: does each side
//!     serve the key, with the same element (compared by LINE PERMUTATION: the two group enumerations index elements
//!     differently), and is the engine's relation equal term by term to the one we shipped?
//! (b) the engine's `pole_order_d4` of a relation record against our own valuation of the same record: equal at every
//!     relation? A difference is a FINDING (both values and the record named).
//! Pure comparisons over data the caller reads; generic (no node, no arity).

use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// One key of a symface `check_keys.jsonl` (the engine's answer at a listed key).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineKey {
    pub key: Vec<i64>,
    pub element: Option<usize>,
    pub terms: Vec<(Vec<i64>, String)>,
}

/// Read a symface `check_keys.jsonl` (lines `{"element": e, "key": [..], "terms": [[[..], "c"], ..]}`).
pub fn engine_keys(text: &str) -> Result<Vec<EngineKey>, String> {
    let mut out = vec![];
    for (i, l) in text.lines().enumerate() {
        if l.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(l).map_err(|e| format!("check_keys {}: {e}", i + 1))?;
        let key: Vec<i64> = serde_json::from_value(v["key"].clone())
            .map_err(|e| format!("check_keys {}: {e}", i + 1))?;
        let mut terms = vec![];
        for t in v["terms"].as_array().into_iter().flatten() {
            terms.push((
                serde_json::from_value::<Vec<i64>>(t[0].clone()).map_err(|e| e.to_string())?,
                t[1].as_str().unwrap_or("").to_string(),
            ));
        }
        out.push(EngineKey {
            key,
            element: v["element"].as_u64().map(|e| e as usize),
            terms,
        });
    }
    Ok(out)
}

/// Our side at one key: every acceptable symmetry choice (element, kind, rhs terms) and the relation we shipped.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OurKey {
    pub choices: Vec<(Option<usize>, String, usize)>,
    pub shipped: Option<(usize, super::producers::Rhs)>,
}

/// The comparison at one key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct KeyComparison {
    pub key: Vec<i64>,
    /// `both` | `ours_only` | `engine_only` | `neither` | `ours_composite_only` (face mode has no catalogue composition)
    pub class: String,
    /// Some(true) = the engine's element is one of ours (by line permutation); None = not comparable.
    pub same_element: Option<bool>,
    /// Some(true) = the engine's relation equals the one we shipped term by term (same element only).
    pub same_relation: Option<bool>,
    pub engine_element: Option<usize>,
    pub ours: Vec<(Option<usize>, String, usize)>,
}

fn norm(t: &[(Vec<i64>, String)]) -> Vec<(Vec<i64>, String)> {
    let mut x: Vec<(Vec<i64>, String)> = t
        .iter()
        .map(|(k, c)| (k.clone(), c.trim().trim_start_matches('+').to_string()))
        .collect();
    x.sort();
    x
}

/// Compare the engine's answers with our choices key by key (every key of either side). `engine_perm` / `our_perm`
/// give an element's line permutation in each enumeration.
pub fn compare_symface(
    engine: &[EngineKey],
    ours: &BTreeMap<Vec<i64>, OurKey>,
    engine_perm: &dyn Fn(usize) -> Option<Vec<usize>>,
    our_perm: &dyn Fn(usize) -> Option<Vec<usize>>,
) -> Vec<KeyComparison> {
    let mut keys: BTreeMap<Vec<i64>, Option<&EngineKey>> = BTreeMap::new();
    for e in engine {
        keys.insert(e.key.clone(), Some(e));
    }
    for k in ours.keys() {
        keys.entry(k.clone()).or_insert(None);
    }
    let mut out = vec![];
    for (k, e) in keys {
        let o = ours.get(&k).cloned().unwrap_or_default();
        let served_e = e.is_some_and(|e| e.element.is_some() && !e.terms.is_empty());
        let plain_ours = o.choices.iter().any(|(_, kind, _)| kind == "plain");
        let any_ours = !o.choices.is_empty();
        let class = match (served_e, plain_ours, any_ours) {
            (true, _, true) => "both",
            (true, _, false) => "engine_only",
            (false, true, _) => "ours_only",
            (false, false, true) => "ours_composite_only",
            (false, false, false) => "neither",
        };
        let ee = e.and_then(|e| e.element);
        let same_element = match (served_e, ee) {
            (true, Some(x)) if any_ours => engine_perm(x).map(|p| {
                o.choices
                    .iter()
                    .filter_map(|(oe, _, _)| oe.and_then(our_perm))
                    .any(|q| q == p)
            }),
            _ => None,
        };
        let same_relation = match (&o.shipped, e, ee) {
            (Some((se, rhs)), Some(e), Some(x)) if served_e => {
                match (engine_perm(x), our_perm(*se)) {
                    (Some(p), Some(q)) if p == q => Some(norm(rhs) == norm(&e.terms)),
                    _ => None,
                }
            }
            _ => None,
        };
        out.push(KeyComparison {
            key: k,
            class: class.into(),
            same_element,
            same_relation,
            engine_element: ee,
            ours: o.choices,
        });
    }
    out
}

/// Counts of a comparison by class, plus element / relation agreement among `both`.
pub fn counts(c: &[KeyComparison]) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for x in c {
        *m.entry(x.class.clone()).or_insert(0) += 1;
        let tag = |b: Option<bool>, name: &str| match b {
            Some(true) => format!("{name}:equal"),
            Some(false) => format!("{name}:differs"),
            None => format!("{name}:not_comparable"),
        };
        if x.class == "both" {
            *m.entry(tag(x.same_element, "element")).or_insert(0) += 1;
            *m.entry(tag(x.same_relation, "relation")).or_insert(0) += 1;
        }
    }
    m
}

/// One pole-order comparison: the engine's `pole_order_d4` of a record against our valuation of the same record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PoleComparison {
    pub id: String,
    pub key: Vec<i64>,
    pub relation_digest: Option<String>,
    /// The engine's field (None = absent / null in the record).
    pub engine: Option<u64>,
    /// Our valuation (None = undecided).
    pub ours: Option<u64>,
    /// `equal` | `differs` (a FINDING) | `engine_absent` | `ours_undecided`
    pub class: String,
}

/// Classify one pole-order pair.
pub fn compare_pole(
    id: &str,
    key: &[i64],
    digest: Option<&str>,
    engine: Option<u64>,
    ours: Option<u64>,
) -> PoleComparison {
    let class = match (engine, ours) {
        (None, _) => "engine_absent",
        (_, None) => "ours_undecided",
        (Some(a), Some(b)) if a == b => "equal",
        _ => "differs",
    };
    PoleComparison {
        id: id.to_string(),
        key: key.to_vec(),
        relation_digest: digest.map(str::to_string),
        engine,
        ours,
        class: class.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symface_classes_and_agreement() {
        let eng = engine_keys(
            "{\"element\":9,\"key\":[2,1],\"terms\":[[[1,2],\"1\"]]}\n{\"element\":null,\"key\":[3,1],\"terms\":[]}\n",
        )
        .unwrap();
        let mut ours = BTreeMap::new();
        ours.insert(
            vec![2, 1],
            OurKey {
                choices: vec![(Some(3), "plain".into(), 1)],
                shipped: Some((3, vec![(vec![1, 2], "1".into())])),
            },
        );
        ours.insert(
            vec![4, 1],
            OurKey {
                choices: vec![(Some(1), "composite".into(), 9)],
                shipped: None,
            },
        );
        let ep = |e: usize| (e == 9).then(|| vec![1, 0]);
        let op = |e: usize| match e {
            3 => Some(vec![1, 0]),
            1 => Some(vec![0, 1]),
            _ => None,
        };
        let c = compare_symface(&eng, &ours, &ep, &op);
        let by: BTreeMap<_, _> = c.iter().map(|x| (x.key.clone(), x)).collect();
        assert_eq!(by[&vec![2, 1]].class, "both");
        assert_eq!(by[&vec![2, 1]].same_element, Some(true));
        assert_eq!(by[&vec![2, 1]].same_relation, Some(true));
        assert_eq!(by[&vec![3, 1]].class, "neither");
        assert_eq!(by[&vec![4, 1]].class, "ours_composite_only");
        let n = counts(&c);
        assert_eq!(n["both"], 1);
        assert_eq!(n["relation:equal"], 1);
    }

    #[test]
    fn pole_pairs() {
        assert_eq!(
            compare_pole("a", &[1], None, Some(0), Some(0)).class,
            "equal"
        );
        assert_eq!(
            compare_pole("a", &[1], None, Some(1), Some(0)).class,
            "differs"
        );
        assert_eq!(
            compare_pole("a", &[1], None, None, Some(0)).class,
            "engine_absent"
        );
        assert_eq!(
            compare_pole("a", &[1], None, Some(0), None).class,
            "ours_undecided"
        );
    }
}
