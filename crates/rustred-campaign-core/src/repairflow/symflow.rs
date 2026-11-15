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

/// A key with its composite outcomes or the reason it could not be judged.
pub type KeyResult<'a, E> = (Vec<i32>, Result<&'a KeyComposites<E>, String>);

/// Per face: keys, served by a plain relation, by a composite (and of those, how many have NO plain alternative =
/// keys the image-only check rejected), not served (with the refusal classes of every element), errors.
pub fn summarise<E>(results: &[KeyResult<'_, E>], lines: &[bool]) -> Value {
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

/// The path of a table file of the spec's `frame_from` source in a loaded set (a `table_set` source has no single
/// `path`; every table of the source carries the same frame header, checked by the import): the first one found.
pub fn frame_table_path(set: &crate::import::load::ImportedSet) -> Option<String> {
    let src = set
        .spec
        .sources
        .iter()
        .find(|s| s.id == set.spec.frame_from)?;
    if let Some(p) = &src.path {
        return Some(p.clone());
    }
    set.rules
        .iter()
        .flatten()
        .find(|r| r.source == set.spec.frame_from)
        .map(|r| r.path.clone())
}

/// Regions per face from the per-key records (lane regions' `regionfit`, the policy's `regions` section): one record per
/// face (`RegionFit::record()`, schema rustred.campaign.regionfit.v1) or the error of that face. Deterministic.
pub fn fit_face_regions(
    records: &[super::composite::KeyChoices],
    policy: &super::policy::RegionsPolicy,
) -> Value {
    use crate::regionfit::keychoices::{faces_from_key_choices, ranking_from_preferred};
    if !policy.enabled {
        return json!({"enabled": false});
    }
    let ok: Vec<super::composite::KeyChoices> = records
        .iter()
        .filter(|r| r.status == "ok")
        .cloned()
        .collect();
    let faces = match faces_from_key_choices(&ok, &policy.adapter) {
        Ok(f) => f,
        Err(e) => return json!({"error": e}),
    };
    let mut fit = policy.fit.clone();
    if fit.ranking.is_empty() {
        fit.ranking = ranking_from_preferred(&ok, &policy.adapter);
    }
    let mut out = serde_json::Map::new();
    for f in &faces {
        let v = match crate::regionfit::fit_regions(f, None, &fit) {
            Ok(r) => r.record(),
            Err(e) => json!({"error": format!("{e:?}")}),
        };
        out.insert(f.signature.clone(), v);
    }
    json!({"enabled": true, "adapter": policy.adapter, "faces": out})
}

/// Lane pieces' SHIP GATE over everything a checked spec ships (`pieces::import_edges::shipped_relations`), with the
/// right-hand-side terms that have a POSITIVE numerator and an UNKNOWN coefficient counted as their own class
/// (`positive_numerator_unknown`: no zero fact loaded yet, lane zeros2) instead of being judged for descent; every other
/// term (zero terms skipped by the gate) is an edge of the gate. The verdict is the gate's over those edges.
pub fn ship_gate_split(
    set: &crate::import::load::ImportedSet,
    ev: &crate::import::Evidence,
    res: &crate::import::check::CheckResult,
    cfg: crate::pieces::gate::GateConfig,
) -> Result<Value, String> {
    use crate::pieces::NonZero;
    use crate::pieces::gate::ShipGate;
    let lines = set.frame.mask().to_vec();
    let posnum = |k: &[i32]| k.iter().zip(&lines).any(|(&x, &l)| !l && x > 0);
    let mut gate = ShipGate::new(&set.cmp, set.arity(), cfg).map_err(|e| e.to_string())?;
    let mut pu: BTreeMap<String, u64> = BTreeMap::new();
    let mut pu_relations = 0u64;
    let stats = {
        let mut sink =
            |key: &[i32], prov: &str, terms: &[(Vec<i32>, NonZero)]| -> Result<(), String> {
                let mut kept: Vec<(&[i32], NonZero)> = Vec::with_capacity(terms.len());
                let mut any = false;
                for (k, n) in terms {
                    if *n == NonZero::Unknown && posnum(k) {
                        *pu.entry(prov.to_string()).or_default() += 1;
                        any = true;
                    } else {
                        kept.push((k.as_slice(), *n));
                    }
                }
                if any {
                    pu_relations += 1;
                }
                gate.push_relation(key, prov, kept)
                    .map_err(|e| e.to_string())
            };
        crate::pieces::import_edges::shipped_relations(set, ev, ev, res, &mut sink)?
    };
    let rep = gate.finish().map_err(|e| e.to_string())?;
    let total: u64 = pu.values().sum();
    Ok(
        json!({"gate": rep, "shipped": {"rule_rows": stats.rule_rows, "credit_rows": stats.credit_rows,
        "relation_rows": stats.relation_rows, "refused_relation_rows": stats.refused_relation_rows,
        "open_rows": stats.open_rows, "terminal_rows": stats.terminal_rows, "no_body": stats.no_body,
        "terms": stats.terms},
        "positive_numerator_unknown": {"terms": total, "relations": pu_relations, "by_provenance": pu,
            "note": "positive-numerator right-hand-side terms without a zero fact: not judged for descent here (a zero-fact source, lane zeros2, decides them)"}}),
    )
}

/// The result of a [`bounded_walk`].
#[derive(Clone, Debug, Default)]
pub struct WalkResult {
    /// Keys served and expanded by this walk (by a table rule or an extra relation).
    pub served: std::collections::HashSet<Vec<i32>>,
    pub served_by_table: u64,
    pub served_by_extra: u64,
    /// Top-sector keys no table rule and no extra relation serves (in walk order).
    pub uncovered: Vec<Vec<i32>>,
    /// Keys whose first selected rule cannot be used there (reason).
    pub blocked: Vec<(Vec<i32>, String)>,
    /// Keys reached but NOT expanded because `expand` refused them (beyond the layer bound).
    pub beyond: u64,
    /// Keys reached that are leaves (closed earlier: not re-walked).
    pub leaf_hits: u64,
    pub terminals: u64,
    /// Right-hand-side terms with a positive numerator (need a zero fact; not followed).
    pub positive_numerator_terms: u64,
    /// Right-hand-side terms in a sub-sector (some line <= 0: other sectors / nodes; not followed).
    pub subsector_terms: u64,
    pub no_body: u64,
}

/// A KEYS-ONLY forward walk of the first-match dispatch from `starts` (TRICKS 10: close layer by layer): a key in
/// `leaves` is not re-walked; a key `expand` refuses is counted as beyond the bound and not expanded; a key no table
/// rule serves is served by `extra` (key -> right-hand-side keys: shipped composites, core relations) when present,
/// else recorded uncovered; blocked keys are recorded with the dispatcher's reason. Every right-hand-side term of a
/// rule is followed (keys-only: a zero coefficient is not known here), except positive-numerator and sub-sector terms
/// (counted). Pure; memory = the keys visited.
pub fn bounded_walk(
    set: &crate::import::load::ImportedSet,
    starts: &[Vec<i32>],
    extra: &HashMap<Vec<i32>, Vec<Vec<i32>>>,
    leaves: &std::collections::HashSet<Vec<i32>>,
    expand: &dyn Fn(&[i32]) -> bool,
) -> WalkResult {
    use crate::cover::Dispatch;
    let lines = set.frame.mask().to_vec();
    let n = lines.len();
    let terminals: std::collections::HashSet<&Vec<i32>> = set
        .terminals_preferred
        .iter()
        .chain(set.terminals_other.iter())
        .collect();
    let mut out = WalkResult::default();
    let mut seen: std::collections::HashSet<Vec<i32>> = std::collections::HashSet::new();
    let mut queue: std::collections::VecDeque<Vec<i32>> = std::collections::VecDeque::new();
    for s in starts {
        if seen.insert(s.clone()) {
            queue.push_back(s.clone());
        }
    }
    let mut buf: Vec<i32> = vec![];
    let mut cm: Vec<u64> = vec![];
    let mut push = |k: Vec<i32>,
                    seen: &mut std::collections::HashSet<Vec<i32>>,
                    queue: &mut std::collections::VecDeque<Vec<i32>>,
                    out: &mut WalkResult| {
        let pos = k.iter().zip(&lines).any(|(&x, &l)| !l && x > 0);
        if pos {
            out.positive_numerator_terms += 1;
            return;
        }
        let sub = k.iter().zip(&lines).any(|(&x, &l)| l && x <= 0);
        if sub {
            out.subsector_terms += 1;
            return;
        }
        if seen.insert(k.clone()) {
            queue.push_back(k);
        }
    };
    while let Some(k) = queue.pop_front() {
        if leaves.contains(&k) {
            out.leaf_hits += 1;
            continue;
        }
        if terminals.contains(&k) {
            out.terminals += 1;
            continue;
        }
        if !expand(&k) {
            out.beyond += 1;
            continue;
        }
        match set.disp.first_match(&k) {
            Ok(Dispatch::Covered(rr)) => {
                buf.clear();
                cm.clear();
                match set.bodies.instantiate(&set.disp, rr, &k, &mut buf, &mut cm) {
                    Ok(Some(c)) => {
                        out.served_by_table += 1;
                        let terms: Vec<Vec<i32>> =
                            (0..c).map(|j| buf[j * n..(j + 1) * n].to_vec()).collect();
                        out.served.insert(k);
                        for t in terms {
                            push(t, &mut seen, &mut queue, &mut out);
                        }
                    }
                    _ => out.no_body += 1,
                }
            }
            Ok(Dispatch::Uncovered) => match extra.get(&k) {
                Some(rhs) => {
                    out.served_by_extra += 1;
                    out.served.insert(k);
                    for t in rhs {
                        push(t.clone(), &mut seen, &mut queue, &mut out);
                    }
                }
                None => out.uncovered.push(k),
            },
            Ok(d) => {
                if let Some(rhs) = extra.get(&k) {
                    out.served_by_extra += 1;
                    out.served.insert(k);
                    for t in rhs {
                        push(t.clone(), &mut seen, &mut queue, &mut out);
                    }
                } else {
                    let why = match d {
                        Dispatch::Blocked { reason, .. } => format!("blocked:{reason:?}"),
                        Dispatch::Undecided { .. } => "undecided".to_string(),
                        Dispatch::OutsideChart { .. } => "outside_chart".to_string(),
                        _ => "other".to_string(),
                    };
                    out.blocked.push((k, why));
                }
            }
            Err(e) => out.blocked.push((k, format!("error:{e:?}"))),
        }
    }
    out
}
