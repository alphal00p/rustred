//! Lane t266run (interfaces/closeloop.md 17): the campaign's OWN escalation of uncovered keys and the DERIVED part of
//! the effective import spec.
//!
//! Per check pass, every uncovered key (nothing serves it) goes, once, through:
//! 1. the exact SYMMETRY RELATION AT THE KEY (verified group, expanded multi-term rows; core
//!    `termmin::symmetry_relation_at_key`), computed in-process: a relation enters the next pass as a supplied relation
//!    that the import RE-DERIVES from the verified group (level per-key, basis symmetry_relation);
//! 2. else per-key POINT recipes of the configured step sequence (core `termmin::point_recipe`; steps with an already
//!    used seed set or over the seed cap are skipped and recorded): they enter the next pass as recipe sources in step
//!    order, so the import asks them one after the other (first without an answer), each answer certified or refused
//!    by the acceptance chain;
//! 3. a key whose escalation is exhausted (every candidate answered or in attention, nothing more can be issued) is a
//!    TERMINAL PROPOSAL; when no other work is left and there are at most `terminals.max_terminals` of them they are
//!    accepted as declared terminals of the node (the user's "any finite terminal set" rule as data), else the node
//!    ends open naming the rule.
//! Everything is recorded as ledger inputs (roles below); the derived spec part is a pure function of those inputs, so a
//! resumed campaign rebuilds the same effective spec. Nothing node-specific (L8): steps, priorities, caps and the
//! terminal bound are configuration.

use super::config::EscalationCfg;
use rustred_campaign_core::import::ImportedSet;
use rustred_campaign_core::import::lines::key_text;
use rustred_campaign_core::import::symmetry::{GroupSpec, load_verified_group};
use rustred_campaign_core::independent_algebra::Independent;
use rustred_campaign_core::replay_base::symrows::VerifiedGroup;
use rustred_campaign_core::termmin::{SymRelation, point_recipe, priorities, symmetry_relation_at_key};
use rustred_campaign_core::termmin::symrel::q_text;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub const ROLE_SYMREL: &str = "closeloop_symrel";
pub const ROLE_RECIPES: &str = "closeloop_recipes";
pub const ROLE_TERMINALS: &str = "closeloop_terminals";
/// Source ids of the derived spec part.
pub const SYMREL_SOURCE: &str = "esc-symmetry-at-key";
pub const TERMINALS_SOURCE: &str = "esc-terminal-proposals";
pub const RECIPE_CLASS: &str = "escalation";

/// The escalation recorded in the ledger for one node (read from its registered inputs, in ledger order).
#[derive(Clone, Debug, Default)]
pub struct DerivedState {
    /// Keys whose symmetry relation at the key was attempted (any outcome).
    pub symrel_tried: BTreeSet<Vec<i32>>,
    /// key -> rhs (key, coefficient text) of the relations found.
    pub relations: BTreeMap<Vec<i32>, Vec<(Vec<i32>, String)>>,
    /// Keys whose step recipes were generated.
    pub escalated: BTreeSet<Vec<i32>>,
    /// (source id, recipe path, sha256) in generation order.
    pub recipe_sources: Vec<(String, String, String)>,
    /// Accepted terminal proposals.
    pub terminals: BTreeSet<Vec<i32>>,
}

fn read_lines(p: &Path) -> Result<Vec<Value>, String> {
    let t = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
    t.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).map_err(|e| format!("{}: {e}", p.display())))
        .collect()
}

fn key_of(v: &Value) -> Result<Vec<i32>, String> {
    serde_json::from_value(v.clone()).map_err(|e| format!("key {v}: {e}"))
}

impl DerivedState {
    /// Fold the node's escalation inputs (each list in ledger order).
    pub fn read(symrel: &[PathBuf], recipes: &[PathBuf], terminals: &[PathBuf]) -> Result<Self, String> {
        let mut s = DerivedState::default();
        for p in symrel {
            for v in read_lines(p)? {
                let k = key_of(&v["key"])?;
                s.symrel_tried.insert(k.clone());
                if v["verdict"] == "relation" {
                    let mut rhs = vec![];
                    for t in v["rhs"].as_array().into_iter().flatten() {
                        rhs.push((key_of(&t[0])?, t[1].as_str().unwrap_or("").to_string()));
                    }
                    s.relations.insert(k, rhs);
                }
            }
        }
        for p in recipes {
            for v in read_lines(p)? {
                let k = key_of(&v["key"])?;
                s.escalated.insert(k);
                for r in v["recipes"].as_array().into_iter().flatten() {
                    s.recipe_sources.push((
                        r["id"].as_str().unwrap_or("").to_string(),
                        r["path"].as_str().unwrap_or("").to_string(),
                        r["sha256"].as_str().unwrap_or("").to_string(),
                    ));
                }
            }
        }
        for p in terminals {
            let t = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
            let v: Value = serde_json::from_str(&t).map_err(|e| format!("{}: {e}", p.display()))?;
            for k in v["terminals"].as_array().into_iter().flatten() {
                s.terminals.insert(key_of(k)?);
            }
        }
        Ok(s)
    }

    pub fn is_empty(&self) -> bool {
        self.relations.is_empty() && self.recipe_sources.is_empty() && self.terminals.is_empty()
    }
}

/// The effective import spec: the node's base spec + the derived sources (symmetry relations as ONE supplied relation
/// set re-derived from the verified group, the step recipes in generation order, the accepted terminal proposals).
/// Writes the derived files under `dir` (idempotent: same state, same bytes).
pub fn effective_spec(
    base_text: &str,
    ds: &DerivedState,
    esc: Option<&EscalationCfg>,
    dir: &Path,
    chart: &str,
) -> Result<String, String> {
    if ds.is_empty() {
        return Ok(base_text.to_string());
    }
    let mut v: Value = serde_json::from_str(base_text).map_err(|e| format!("import spec: {e}"))?;
    let srcs = v["sources"]
        .as_array_mut()
        .ok_or("import spec: no sources")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let write = |name: &str, body: &str| -> Result<PathBuf, String> {
        let p = dir.join(name);
        if std::fs::read(&p).ok().as_deref() != Some(body.as_bytes()) {
            crate::atomic::write_atomic(&p, body.as_bytes())
                .map_err(|e| format!("{}: {e}", p.display()))?;
        }
        Ok(p)
    };
    if !ds.relations.is_empty() {
        let mut t = json!({"type": "header", "schema": "table_dump.v1",
            "note": "closeloop escalation: exact symmetry relations at the key (verified group, expanded rows)"})
        .to_string();
        t.push('\n');
        for (i, (k, rhs)) in ds.relations.iter().enumerate() {
            let line = json!({"type": "rule", "chart": chart, "index": i, "kind": "symmetry-at-key",
                "case": "point", "fixed": k, "equations": [], "target": key_text(k),
                "rhs": rhs.iter().map(|(rk, c)| json!([key_text(rk), c])).collect::<Vec<_>>(),
                "exceptions": [], "dispatch": "Point", "witness_loci": []});
            t.push_str(&line.to_string());
            t.push('\n');
        }
        let p = write("symmetry-at-key.table.jsonl", &t)?;
        let group = esc
            .and_then(|e| e.group.clone())
            .unwrap_or_else(|| json!({"group_format": "derive"}));
        srcs.push(json!({"id": SYMREL_SOURCE, "kind": "supplied_relations", "class": "symmetry",
            "path": p.display().to_string(), "verify": group}));
    }
    for (id, path, sha) in &ds.recipe_sources {
        srcs.push(json!({"id": id, "kind": "recipe", "class": RECIPE_CLASS, "path": path, "sha256": sha}));
    }
    if !ds.terminals.is_empty() {
        let list: Vec<String> = ds.terminals.iter().map(|k| key_text(k)).collect();
        let p = write(
            "terminal-proposals.json",
            &(serde_json::to_string_pretty(&list).unwrap_or_default() + "\n"),
        )?;
        srcs.push(json!({"id": TERMINALS_SOURCE, "kind": "terminals", "path": p.display().to_string()}));
    }
    Ok(serde_json::to_string_pretty(&v).map_err(|e| e.to_string())? + "\n")
}

/// The verified group of the symmetry step: the configured GroupSpec, else derived from the frame header's family.
pub fn group_of(
    esc: &EscalationCfg,
    spec_text: &str,
    base: &Path,
    set: &ImportedSet,
) -> Result<Option<VerifiedGroup>, String> {
    let import = rustred_campaign_core::import::TableSetSpec::from_json(spec_text)
        .map_err(|e| format!("import spec: {e}"))?;
    let header_family = super::termmin::frame_header_family(&import, base);
    let gs: GroupSpec = match (&esc.group, &header_family) {
        (Some(g), _) => serde_json::from_value(g.clone()).map_err(|e| format!("escalation group: {e}"))?,
        (None, Some(_)) => serde_json::from_value(json!({})).map_err(|e| e.to_string())?,
        (None, None) => return Ok(None),
    };
    let (g, _) = load_verified_group(
        &gs,
        base,
        set.frame.mask(),
        set.cmp.presentation().new_to_old(),
        header_family.as_deref(),
    )
    .map_err(|e| format!("escalation group: {e}"))?;
    Ok(Some(g))
}

/// The escalation output of one pass.
#[derive(Clone, Debug, Default)]
pub struct EscOut {
    /// New symmetry attempts (one JSON line per key: key, verdict, rhs, rows / rank / columns, why).
    pub symrel_lines: Vec<String>,
    pub symrel_relations: usize,
    /// New recipe files: (file name, canonical text) and the manifest lines (one per key).
    pub recipe_files: Vec<(String, String)>,
    pub recipe_manifest: Vec<String>,
    /// Keys not to batch this round (a symmetry relation was found: served next round).
    pub hold: BTreeSet<Vec<i32>>,
    /// Accepted terminal proposals of this pass (with their attempt records).
    pub terminals: Option<Value>,
    /// Exhausted keys over the terminal rule's bound (the node ends open naming the rule).
    pub over_rule: Option<String>,
}

impl EscOut {
    /// Whether the pass produced escalation work (the node needs another round).
    pub fn progress(&self) -> bool {
        self.symrel_relations > 0 || !self.recipe_files.is_empty() || self.terminals.is_some()
    }
}

fn hex16(s: &str) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(s.as_bytes());
    h.finish_hex()[..16].to_string()
}

/// The recipe source id of step `step` at `key` (charset `[A-Za-z0-9._]`: first 16 hex of sha256 of the key text).
pub fn recipe_source_id(key: &[i32], step: &str) -> String {
    format!("esc.{}.{step}", hex16(&key_text(key)))
}

/// Steps 1-2 for the uncovered keys of a pass (`dir_rel` = the round's recipe directory, absolute).
pub fn escalate(
    set: &ImportedSet,
    esc: &EscalationCfg,
    ds: &DerivedState,
    uncovered: &[Vec<i32>],
    group: Option<&VerifiedGroup>,
    recipe_dir: &Path,
) -> Result<EscOut, String> {
    let mut out = EscOut::default();
    let lines: Vec<bool> = set.frame.mask().to_vec();
    let (lp, np) = priorities(&lines, esc.coordinate_priority.as_deref());
    let cmp = |a: &[i32], b: &[i32]| set.cmp.cmp(a, b);
    for k in uncovered {
        if ds.terminals.contains(k) || ds.relations.contains_key(k) {
            continue;
        }
        let mut try_recipes = !esc.symmetry_at_key || ds.symrel_tried.contains(k);
        if esc.symmetry_at_key && !ds.symrel_tried.contains(k) {
            let line = match group {
                None => json!({"key": k, "verdict": "unavailable", "why": "no verified group"}),
                Some(g) => match symmetry_relation_at_key(&Independent, g, k, &cmp, esc.symmetry_max_terms) {
                    SymRelation::Relation { rhs, rows, columns, elements } => {
                        out.symrel_relations += 1;
                        out.hold.insert(k.clone());
                        json!({"key": k, "verdict": "relation", "rows": rows, "columns": columns,
                            "elements": elements, "group_digest": g.digest(),
                            "rhs": rhs.iter().map(|(rk, q)| json!([rk, q_text(q)])).collect::<Vec<_>>()})
                    }
                    SymRelation::NoRelation { rows, rank, columns } => {
                        json!({"key": k, "verdict": "no_relation", "rows": rows, "rank": rank, "columns": columns})
                    }
                    SymRelation::Unknown(w) => json!({"key": k, "verdict": "unknown", "why": w}),
                },
            };
            let found = line["verdict"] == "relation";
            out.symrel_lines.push(line.to_string());
            // no relation at the key: its recipes are generated in the same pass (no extra round)
            try_recipes = !found;
        }
        if !try_recipes || ds.escalated.contains(k) {
            continue;
        }
        let kt = key_text(k);
        let mut recipes = vec![];
        let mut skipped = vec![];
        let mut seen_sets: BTreeSet<String> = BTreeSet::new();
        for name in &esc.steps {
            let st = rustred_campaign_core::import::recipe_steps::parse_step(name)
                .map_err(|e| format!("escalation step {name}: {e}"))?;
            let pr = point_recipe(&st, k, &lines, &lp, &np)?;
            if esc.max_seeds.is_some_and(|m| pr.seeds > m) {
                skipped.push(json!({"step": name, "why": "over_seed_cap", "seeds": pr.seeds}));
                continue;
            }
            if !seen_sets.insert(format!(
                "{}|{}|{}|{}",
                pr.seed_set_digest,
                pr.shells.join("+"),
                pr.symmetry_rows,
                pr.relation_rule
            )) {
                skipped.push(json!({"step": name, "why": "same_seed_set_as_an_earlier_step", "seeds": pr.seeds}));
                continue;
            }
            let id = recipe_source_id(k, name);
            let mut doc = pr.recipe.clone();
            doc["id"] = json!(id);
            let mut text = rustred_campaign_core::schema::canon::canonical_json(&doc, false)?;
            text.push('\n');
            let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
            h.update(text.as_bytes());
            let sha = h.finish_hex();
            let file = format!("{id}.recipe.json");
            let path = recipe_dir.join(&file);
            recipes.push(json!({"id": id, "step": name, "path": path.display().to_string(), "sha256": sha,
                "seeds": pr.seeds, "relation_rule": pr.relation_rule, "symmetry_rows": pr.symmetry_rows}));
            out.recipe_files.push((file, text));
        }
        out.recipe_manifest
            .push(json!({"key": k, "key_text": kt, "recipes": recipes, "skipped": skipped}).to_string());
    }
    Ok(out)
}
