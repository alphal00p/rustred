//! The EXTENSION CHECK on imported material (interfaces/termmin.md section 4): a [`ClosureOracle`] that owns the node's
//! `ImportedSet` and `Evidence` and decides a candidate by EXTENDING the closure and re-running `import::check`: the
//! terminal is removed from the declared terminals, the candidate is added as the LAST relation set (a recipe relation:
//! an in-memory recipe source with `domain.keys = [t]` plus its solved relation in the evidence, exactly what import 5.3
//! certifies; a symmetry relation: a supplied point relation; `dispatch`: nothing), the closing-try answers are added to
//! the evidence of the node's own recipe sources, and the check runs with the base options. Every temporary change is
//! undone after the check; a kept candidate is re-applied for good (`commit`). Library only: no process, no file write.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use serde_json::{Value, json};

use super::machine::{Candidate, ClosingSolve, ClosureOracle, Economy, Extension, Key, SymStep};
use super::symrel::{SymRelation, q_text, symmetry_relation_at_key};
use crate::closure::relations::{
    DispatchSlot, RelationSet, RelationStoreBuilder, VERDICT_CERTIFIED, VERDICT_EXACT,
};
use crate::domain::{CaseDomain, TargetMap};
use crate::import::check::{CheckOptions, CheckResult, RowStatus, Server, check};
use crate::import::evidence::{Evidence, SolvedRelation};
use crate::import::load::{ImportedSet, RecipeSource, RelEntry, SuppliedInfo};
use crate::import::supplied_verify::{
    SuppliedRoute, UNVERIFIED_NO_GROUP, UNVERIFIED_NOT_IN_SPAN,
};
use crate::import::symmetry::verify_relation_with;
use crate::independent_algebra::Independent;
use crate::replay_base::symrows::VerifiedGroup;

/// Name prefix of the relation sets this module adds.
pub const SOURCE_PREFIX: &str = "termmin";

/// Summary of one check (base or extension).
#[derive(Clone, Debug, Default)]
pub struct CheckView {
    pub open: BTreeSet<Key>,
    pub terminals: Vec<Key>,
    pub economy: Economy,
    pub served: BTreeMap<Key, String>,
    pub solve_requests: Vec<ClosingSolve>,
    pub refusals: BTreeMap<Key, String>,
    pub open_status: BTreeMap<Key, String>,
}

/// Economy and terminals of a check result.
pub fn view(set: &ImportedSet, res: &CheckResult) -> CheckView {
    let mut v = CheckView::default();
    let mut recipes: BTreeSet<String> = BTreeSet::new();
    for r in &res.rows {
        match r.status {
            RowStatus::Terminal => v.terminals.push(r.key.clone()),
            RowStatus::Open => {
                v.open.insert(r.key.clone());
                v.open_status.insert(
                    r.key.clone(),
                    format!(
                        "{}{}",
                        r.open_status.clone().unwrap_or_else(|| "open".into()),
                        r.open_reason
                            .as_ref()
                            .map(|x| format!(": {x}"))
                            .unwrap_or_default()
                    ),
                );
            }
            RowStatus::Served => {
                let name = match &r.server {
                    Some(Server::Rule { table, record })
                    | Some(Server::Credit { table, record }) => {
                        format!(
                            "rule:{}#{}",
                            set.rules[*table][*record].source, set.rules[*table][*record].label
                        )
                    }
                    Some(Server::Relation { set: s, recipe }) => {
                        if *recipe {
                            recipes.insert(s.clone());
                        }
                        format!("relation:{s}")
                    }
                    None => "-".into(),
                };
                v.served.insert(r.key.clone(), name);
            }
        }
    }
    v.economy = Economy {
        terminals: v.terminals.len() as u64,
        closure_keys: res.rows.len() as u64,
        used_rules: res.summary.used_rules,
        recipes: recipes.len() as u64,
    };
    for q in &res.solve_requests {
        v.solve_requests.push(ClosingSolve {
            source: q.source.clone(),
            recipe_path: q.recipe_path.clone(),
            recipe_sha256: q.recipe_sha256.clone(),
            engine_digest: q.engine_digest.clone(),
            key: q.key.clone(),
        });
    }
    for r in &res.solve_refusals {
        v.refusals
            .insert(r.key.clone(), format!("{}: {}", r.source, r.reason));
    }
    v
}

fn point_domain(set: &ImportedSet, t: &[i32]) -> CaseDomain {
    CaseDomain {
        arity: t.len(),
        chart: Some(set.frame.mask().to_vec()),
        fixed: t.iter().map(|&x| Some(x)).collect(),
        target: TargetMap::Canonical,
        equations: vec![],
        bounds: vec![],
        exceptions: vec![],
        recenter_bounds: vec![],
        poles: vec![],
        witness_elided: false,
    }
}

fn json_key(v: &Value, n: usize) -> Option<Key> {
    serde_json::from_value::<Key>(v.clone())
        .ok()
        .filter(|k| k.len() == n)
}

/// A solved-relation v1 line (import 6.2) as the import's evidence record, under `source`.
pub fn solved_from_line(
    line: &Value,
    source: &str,
    n: usize,
    file: &str,
) -> Result<SolvedRelation, String> {
    let key = json_key(&line["key"], n).ok_or("solved line: key")?;
    let mut rhs = vec![];
    for t in line["rhs"].as_array().into_iter().flatten() {
        rhs.push(json_key(&t["key"], n).ok_or("solved line: rhs key")?);
    }
    let s = |k: &str| line.get(k).and_then(Value::as_str).map(str::to_string);
    Ok(SolvedRelation {
        source: source.to_string(),
        key,
        verdict: s("verdict").unwrap_or_default(),
        recipe_sha256: s("recipe_sha256"),
        recipe_digest: s("recipe_digest"),
        determinism_id: s("determinism_id"),
        binary_sha256: s("binary_sha256"),
        relation_digest: s("relation_digest"),
        rhs,
        proof_ok: line["proof"]["ok"] == true,
        regular: line["regular"] == true,
        file: file.to_string(),
    })
}

/// The recipe source id a candidate is added under.
pub fn candidate_source(t: &[i32], cand: &Candidate) -> String {
    format!(
        "{SOURCE_PREFIX}:{}:{}",
        super::steps::key_text(t),
        cand.recipe_id.clone().unwrap_or_else(|| cand.step.clone())
    )
}

#[derive(Default)]
struct Undo {
    terminal: Option<(Key, usize)>,
    recipe: bool,
    supplied: Option<String>,
    solved: Vec<(String, Key, Option<SolvedRelation>)>,
}

/// The oracle on an imported set.
pub struct ImportExtension {
    pub set: ImportedSet,
    pub ev: Evidence,
    pub opts: CheckOptions,
    pub group: Option<VerifiedGroup>,
    pub symmetry_max_terms: usize,
    base: CheckView,
    cur: CheckView,
    commits: Vec<String>,
    cache: HashMap<String, (Extension, CheckView)>,
    /// number of import checks run (cost accounting)
    pub checks: usize,
}

impl ImportExtension {
    /// Run the base check. The set must be the node's material as checked at the end of the closing loop.
    pub fn new(
        set: ImportedSet,
        ev: Evidence,
        opts: CheckOptions,
        group: Option<VerifiedGroup>,
        symmetry_max_terms: usize,
    ) -> Result<ImportExtension, String> {
        let res = check(&set, &ev, &opts).map_err(|e| format!("base check: {e}"))?;
        let base = view(&set, &res);
        Ok(ImportExtension {
            set,
            ev,
            opts,
            group,
            symmetry_max_terms,
            cur: base.clone(),
            base,
            commits: vec![],
            cache: HashMap::new(),
            checks: 1,
        })
    }

    pub fn base(&self) -> &CheckView {
        &self.base
    }
    pub fn current(&self) -> &CheckView {
        &self.cur
    }
    /// The candidates committed so far (source ids, in order).
    pub fn commits(&self) -> &[String] {
        &self.commits
    }
    /// Give the set back (after the pass).
    pub fn into_parts(self) -> (ImportedSet, Evidence) {
        (self.set, self.ev)
    }

    fn apply(&mut self, t: &[i32], cand: &Candidate, extras: &[Value]) -> Result<Undo, String> {
        let mut u = Undo::default();
        let n = self.set.arity();
        if let Some(p) = self
            .set
            .terminals_other
            .iter()
            .position(|k| k.as_slice() == t)
        {
            u.terminal = Some((self.set.terminals_other.remove(p), p));
        } else {
            return Err(format!(
                "{} is not a declared (non-preferred) terminal of the set",
                super::steps::key_text(t)
            ));
        }
        let src = candidate_source(t, cand);
        match cand.kind.as_str() {
            "dispatch" => {}
            "recipe" => {
                let line = cand
                    .payload
                    .get("solved")
                    .ok_or("recipe candidate without its solved line")?;
                let mut rel = solved_from_line(line, &src, n, "<termmin>")?;
                rel.source = src.clone();
                if rel.key.as_slice() != t {
                    return Err("the solved line is for another key".into());
                }
                let mut assigned = HashSet::new();
                assigned.insert(t.to_vec());
                self.set.recipes.push(RecipeSource {
                    id: src.clone(),
                    class: SOURCE_PREFIX.into(),
                    path: cand.payload["recipe_path"]
                        .as_str()
                        .unwrap_or("<termmin>")
                        .to_string(),
                    recipe_sha256: cand.recipe_sha256.clone().unwrap_or_default(),
                    engine_digest: None,
                    recipe_id: cand.recipe_id.clone(),
                    domain: point_domain(&self.set, t),
                    slot: DispatchSlot::Last,
                    assigned: Some(assigned),
                });
                self.set
                    .rel_order
                    .push(RelEntry::Recipe(self.set.recipes.len() - 1));
                self.set.classes.insert(src.clone(), SOURCE_PREFIX.into());
                u.recipe = true;
                let old = self
                    .ev
                    .solved
                    .entry(src.clone())
                    .or_default()
                    .insert(t.to_vec(), rel);
                u.solved.push((src.clone(), t.to_vec(), old));
            }
            "symmetry" => {
                let mut b = RelationStoreBuilder::new(self.set.codec);
                let mut d = [0u8; 32];
                if let Some(h) = &cand.relation_digest {
                    for (i, byte) in d.iter_mut().enumerate() {
                        *byte = h
                            .get(2 * i..2 * i + 2)
                            .and_then(|x| u8::from_str_radix(x, 16).ok())
                            .unwrap_or(0);
                    }
                }
                let check_v = self.group.as_ref().map(|g| {
                    let rhs: Vec<(Vec<i32>, String)> = cand.payload["coefficients"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|x| Some((json_key(&x[0], n)?, x[1].as_str()?.to_string())))
                        .collect();
                    verify_relation_with(
                        &Independent,
                        g,
                        t,
                        &rhs,
                        &[t.to_vec()],
                        self.symmetry_max_terms,
                    )
                });
                // lane symfix (import.md 13.3): EXACT (served) only when re-derived from the verified group
                let route = match &check_v {
                    Some(c) if c.verified() => SuppliedRoute::GroupRederived,
                    Some(c) => SuppliedRoute::unverified(UNVERIFIED_NOT_IN_SPAN, format!("{c:?}")),
                    None => SuppliedRoute::unverified(UNVERIFIED_NO_GROUP, "no verified group"),
                };
                let bits = if route.exact() {
                    VERDICT_CERTIFIED
                } else {
                    VERDICT_CERTIFIED & !VERDICT_EXACT
                };
                b.push(t, &cand.rhs, d, bits)
                    .map_err(|e| format!("{e:?}"))?;
                let relations = b.finish().map_err(|e| format!("{e:?}"))?;
                self.set.supplied.push(Arc::new(RelationSet {
                    name: src.clone(),
                    recipe_digest: format!(
                        "symmetry:{}",
                        cand.relation_digest.clone().unwrap_or_default()
                    ),
                    domain: point_domain(&self.set, t),
                    relations,
                    slot: DispatchSlot::Last,
                }));
                self.set
                    .rel_order
                    .push(RelEntry::Supplied(self.set.supplied.len() - 1));
                self.set
                    .classes
                    .insert(src.clone(), format!("{SOURCE_PREFIX}_symmetry"));
                self.set
                    .supplied_info
                    .entry(src.clone())
                    .or_default()
                    .insert(
                        t.to_vec(),
                        SuppliedInfo {
                            source: src.clone(),
                            class: format!("{SOURCE_PREFIX}_symmetry"),
                            path: "<termmin>".into(),
                            label: None,
                            ordinal: 0,
                            record_sha256: cand.relation_digest.clone().unwrap_or_default(),
                            hand_name: None,
                            verification_sha256: None,
                            check: check_v,
                            // a terminal-minimisation relation is a candidate with its own check, not a keys-only source
                            diagnostic: false,
                            route,
                        },
                    );
                u.supplied = Some(src.clone());
            }
            k => return Err(format!("unknown candidate kind {k}")),
        }
        for l in extras {
            let Some(s) = l.get("source").and_then(Value::as_str) else {
                continue;
            };
            if !self.set.recipes.iter().any(|r| r.id == s) {
                continue;
            }
            let rel = solved_from_line(l, s, n, "<termmin closing try>")?;
            let k = rel.key.clone();
            let old = self
                .ev
                .solved
                .entry(s.to_string())
                .or_default()
                .insert(k.clone(), rel);
            u.solved.push((s.to_string(), k, old));
        }
        Ok(u)
    }

    fn undo(&mut self, u: Undo) {
        for (s, k, old) in u.solved.into_iter().rev() {
            let m = self.ev.solved.entry(s.clone()).or_default();
            match old {
                Some(o) => {
                    m.insert(k, o);
                }
                None => {
                    m.remove(&k);
                    if m.is_empty() {
                        self.ev.solved.remove(&s);
                    }
                }
            }
        }
        if let Some(s) = u.supplied {
            self.set.supplied.pop();
            self.set.rel_order.pop();
            self.set.classes.remove(&s);
            self.set.supplied_info.remove(&s);
        }
        if u.recipe {
            let r = self.set.recipes.pop();
            self.set.rel_order.pop();
            if let Some(r) = r {
                self.set.classes.remove(&r.id);
            }
        }
        if let Some((k, p)) = u.terminal {
            self.set
                .terminals_other
                .insert(p.min(self.set.terminals_other.len()), k);
        }
    }

    fn key_of(&self, t: &[i32], cand: &Candidate, extras: &[Value]) -> String {
        let c = serde_json::to_string(&(t, cand, extras)).unwrap_or_default();
        let mut h = crate::closure::sha256::Sha256::new();
        h.update(self.commits.join("\n").as_bytes());
        h.update(b"\x00");
        h.update(c.as_bytes());
        h.finish_hex()
    }

    fn run(&mut self, t: &[i32], cand: &Candidate, extras: &[Value]) -> (Extension, CheckView) {
        let ck = self.key_of(t, cand, extras);
        if let Some(x) = self.cache.get(&ck) {
            return x.clone();
        }
        let out = match self.apply(t, cand, extras) {
            Err(e) => (
                Extension {
                    served: false,
                    reason: Some(e),
                    ..Extension::default()
                },
                CheckView::default(),
            ),
            Ok(u) => {
                let r = check(&self.set, &self.ev, &self.opts);
                self.checks += 1;
                self.undo(u);
                match r {
                    Err(e) => (
                        Extension {
                            served: false,
                            reason: Some(format!("check: {e}")),
                            ..Extension::default()
                        },
                        CheckView::default(),
                    ),
                    Ok(res) => {
                        let v = view(&self.set, &res);
                        let served = v.served.contains_key(t);
                        let reason = if served {
                            None
                        } else {
                            Some(
                                v.refusals
                                    .get(t)
                                    .cloned()
                                    .or_else(|| v.open_status.get(t).cloned())
                                    .unwrap_or_else(|| "not served".into()),
                            )
                        };
                        let new_open: Vec<Key> = v
                            .open
                            .iter()
                            .filter(|k| !self.cur.open.contains(*k) && k.as_slice() != t)
                            .cloned()
                            .collect();
                        (
                            Extension {
                                served,
                                reason,
                                new_open,
                                terminals_after: v.terminals.clone(),
                                economy: v.economy,
                            },
                            v,
                        )
                    }
                }
            }
        };
        self.cache.insert(ck, out.clone());
        out
    }
}

impl ClosureOracle for ImportExtension {
    fn lower(&self, a: &[i32], b: &[i32]) -> bool {
        self.set.cmp.lower(a, b)
    }

    fn lines(&self) -> Vec<bool> {
        self.set.frame.mask().to_vec()
    }

    fn symmetry(&mut self, t: &[i32]) -> SymStep {
        let Some(g) = &self.group else {
            return SymStep::Unavailable("no_verified_group".into());
        };
        if g.group().elements.is_empty() {
            return SymStep::Unavailable("no_verified_group (trivial group)".into());
        }
        let cmp = |a: &[i32], b: &[i32]| -> Ordering { self.set.cmp.cmp(a, b) };
        match symmetry_relation_at_key(&Independent, g, t, &cmp, self.symmetry_max_terms) {
            SymRelation::Relation {
                rhs,
                rows,
                columns,
                elements,
            } => {
                let coefficients: Vec<Value> =
                    rhs.iter().map(|(k, q)| json!([k, q_text(q)])).collect();
                let canon = json!({"target": t, "rhs": coefficients});
                let mut h = crate::closure::sha256::Sha256::new();
                h.update(
                    crate::schema::canon::canonical_json(&canon, false)
                        .unwrap_or_default()
                        .as_bytes(),
                );
                SymStep::Relation(Candidate {
                    kind: "symmetry".into(),
                    step: "symmetry_relation".into(),
                    rhs: rhs.iter().map(|(k, _)| k.clone()).collect(),
                    relation_digest: Some(h.finish_hex()),
                    recipe_id: None,
                    recipe_sha256: None,
                    payload: json!({"coefficients": coefficients, "elements": elements, "rows": rows,
                                    "columns": columns, "group_digest": g.digest()}),
                })
            }
            SymRelation::NoRelation {
                rows,
                rank,
                columns,
            } => SymStep::NoRelation(format!(
                "no relation from the symmetry rows at the key ({rows} rows, rank {rank}, {columns} columns)"
            )),
            SymRelation::Unknown(w) => {
                SymStep::NoRelation(format!("symmetry rows not decided: {w}"))
            }
        }
    }

    fn extend(&mut self, t: &[i32], cand: &Candidate, extras: &[Value]) -> Extension {
        self.run(t, cand, extras).0
    }

    fn closing_requests(
        &mut self,
        t: &[i32],
        cand: &Candidate,
        ext: &Extension,
    ) -> Vec<ClosingSolve> {
        let (_, v) = self.run(t, cand, &[]);
        let want: BTreeSet<&Key> = ext.new_open.iter().collect();
        let mut out: Vec<ClosingSolve> = v
            .solve_requests
            .into_iter()
            .filter(|q| want.contains(&q.key) && !q.source.starts_with(SOURCE_PREFIX))
            .collect();
        out.sort_by(|a, b| (&a.source, &a.key).cmp(&(&b.source, &b.key)));
        out.dedup_by(|a, b| a.key == b.key);
        out
    }

    fn commit(&mut self, t: &[i32], cand: &Candidate, extras: &[Value]) -> Result<(), String> {
        let (ext, v) = self.run(t, cand, extras);
        if !ext.served || !ext.new_open.is_empty() {
            return Err(format!(
                "commit of {} refused: the extension is not closed ({:?})",
                super::steps::key_text(t),
                ext.reason
            ));
        }
        let ck = self.key_of(t, cand, extras);
        self.apply(t, cand, extras)?;
        self.commits.push(ck);
        self.cur = v;
        Ok(())
    }
}
