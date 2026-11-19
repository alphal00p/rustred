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
//!
//! Everything is recorded as ledger inputs (roles below); the derived spec part is a pure function of those inputs, so a
//! resumed campaign rebuilds the same effective spec. Nothing node-specific (L8): steps, priorities, caps and the
//! terminal bound are configuration.

use super::config::EscalationCfg;
use rustred_campaign_core::import::ImportedSet;
use rustred_campaign_core::import::lines::key_text;
use rustred_campaign_core::import::symmetry::{GroupSpec, load_verified_group};
use rustred_campaign_core::independent_algebra::Independent;
use rustred_campaign_core::replay_base::symrows::VerifiedGroup;
use rustred_campaign_core::termmin::symrel::q_text;
use rustred_campaign_core::termmin::{
    SymRelation, point_recipe, priorities, symmetry_relation_at_key,
};
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
    /// Keys whose step recipes were generated (the first keys of each face) and the faces escalated.
    pub escalated: BTreeSet<Vec<i32>>,
    pub faces: BTreeSet<String>,
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
    pub fn read(
        symrel: &[PathBuf],
        recipes: &[PathBuf],
        terminals: &[PathBuf],
    ) -> Result<Self, String> {
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
                if let Some(f) = v["face"].as_str() {
                    s.faces.insert(f.to_string());
                }
                for k in v["keys"].as_array().into_iter().flatten() {
                    s.escalated.insert(key_of(k)?);
                }
                if v.get("key").is_some() {
                    s.escalated.insert(key_of(&v["key"])?);
                }
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

/// How [`effective_spec`] treats the derived files the spec names BY PATH (the symmetry-relation table, the accepted
/// terminal proposals; the recipes carry their sha256): `Write` (a pass: written when absent or different) or
/// `Verify` (a restarted session, lane t266run: each file must exist with exactly the bytes re-derived from the
/// BLAKE3-verified ledger inputs; any difference or absence REFUSES, typed `effective_spec_mismatch`; nothing is
/// rewritten).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DerivedFiles {
    Write,
    Verify,
}

/// The effective import spec: the node's base spec + the derived sources (symmetry relations as ONE supplied relation
/// set re-derived from the verified group, the step recipes in generation order, the accepted terminal proposals).
/// Writes (or, with [`DerivedFiles::Verify`], compares byte for byte) the derived files under `dir` (idempotent: same
/// state, same bytes).
pub fn effective_spec(
    base_text: &str,
    ds: &DerivedState,
    esc: Option<&EscalationCfg>,
    dir: &Path,
    chart: &str,
    mode: DerivedFiles,
    emission: Option<&rustred_campaign_core::emission::Settings>,
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
        let disk = std::fs::read(&p).ok();
        let b3 = |b: &[u8]| blake3::hash(b).to_hex().to_string();
        if disk.as_deref() == Some(body.as_bytes()) {
            if mode == DerivedFiles::Verify {
                eprintln!(
                    "closeloop: restart: derived file {} verified byte for byte against its re-derivation (blake3 {}, {} bytes)",
                    p.display(),
                    b3(body.as_bytes()),
                    body.len()
                );
            }
            return Ok(p);
        }
        if mode == DerivedFiles::Verify {
            return Err(format!(
                "{}: derived file {}: on disk {}, re-derived from the ledger blake3 {} ({} bytes): refused (never rewritten)",
                super::source::EFFECTIVE_SPEC_MISMATCH,
                p.display(),
                match &disk {
                    Some(d) => format!("blake3 {} ({} bytes)", b3(d), d.len()),
                    None => "absent".to_string(),
                },
                b3(body.as_bytes()),
                body.len()
            ));
        }
        crate::atomic::write_atomic(&p, body.as_bytes())
            .map_err(|e| format!("{}: {e}", p.display()))?;
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
        // EMISSION (lane gencheck): the emitted copy beside (written once; a verifying restart only re-checks it)
        if let Some(es) = emission {
            crate::emission::gate::gate_beside(
                es,
                &t,
                &dir.join("symmetry-at-key.emitted.table.jsonl"),
                crate::emission::gate::GateKind::Symmetry,
                &[],
                mode == DerivedFiles::Write,
            )?;
        }
        let group = esc
            .and_then(|e| e.group.clone())
            .unwrap_or_else(|| json!({"group_format": "derive"}));
        srcs.push(
            json!({"id": SYMREL_SOURCE, "kind": "supplied_relations", "class": "symmetry",
            "path": p.display().to_string(), "verify": group}),
        );
    }
    for (id, path, sha) in &ds.recipe_sources {
        srcs.push(
            json!({"id": id, "kind": "recipe", "class": RECIPE_CLASS, "path": path, "sha256": sha}),
        );
    }
    if !ds.terminals.is_empty() {
        let list: Vec<String> = ds.terminals.iter().map(|k| key_text(k)).collect();
        let p = write(
            "terminal-proposals.json",
            &(serde_json::to_string_pretty(&list).unwrap_or_default() + "\n"),
        )?;
        // lane closeloop4 (audit-closeloop M1): a terminals FILE is pinned by its sha256 (derived from the ledger inputs)
        let d = rustred_campaign_core::import::lines::file_sha256(&p).map_err(|e| e.to_string())?;
        srcs.push(
            json!({"id": TERMINALS_SOURCE, "kind": "terminals", "path": p.display().to_string(),
                         "sha256": d}),
        );
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
        (Some(g), _) => {
            serde_json::from_value(g.clone()).map_err(|e| format!("escalation group: {e}"))?
        }
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
    /// Exhausted keys the run does not accept (awaiting the operator / over the cap): the node ends open naming it.
    pub over_rule: Option<String>,
    /// The proposals document when they are NOT accepted (written to final/terminal_proposals.json).
    pub proposals: Option<Value>,
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

/// The FACE of a key (coordinator 2026-11-14: the uncovered keys are a box slice through faces): its support signature =
/// the dotted line slots (power > 1) and the non-zero numerator slots, e.g. `L0.10|N14` (the cutting rule / exception
/// branch of a table rule is NOT part of this id: not computed here).
pub fn face_id(key: &[i32], lines: &[bool]) -> String {
    let l: Vec<String> = (0..key.len())
        .filter(|&i| lines[i] && key[i] > 1)
        .map(|i| i.to_string())
        .collect();
    let n: Vec<String> = (0..key.len())
        .filter(|&i| !lines[i] && key[i] != 0)
        .map(|i| i.to_string())
        .collect();
    format!("L{}|N{}", l.join("."), n.join("."))
}

/// The step of an escalation recipe source id (`esc.<hex>.<step>`).
pub fn step_of_source(id: &str) -> Option<&str> {
    let rest = id.strip_prefix("esc.")?;
    rest.split_once('.').map(|(_, st)| st)
}

/// Per face: step -> keys it SOLVED (from the answers so far), the order in which later keys of the face try the steps.
pub fn face_successes(
    solved: &BTreeMap<
        String,
        BTreeMap<Vec<i32>, rustred_campaign_core::import::evidence::SolvedRelation>,
    >,
    lines: &[bool],
) -> BTreeMap<String, BTreeMap<String, u64>> {
    let mut m: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
    for (src, ans) in solved {
        let Some(step) = step_of_source(src) else {
            continue;
        };
        for (k, r) in ans {
            if r.verdict == "SOLVED" {
                *m.entry(face_id(k, lines))
                    .or_default()
                    .entry(step.to_string())
                    .or_default() += 1;
            }
        }
    }
    m
}

/// The recipe source id of step `step` on the face of `key` (charset `[A-Za-z0-9._]`).
pub fn recipe_source_id(key: &[i32], step: &str, lines: &[bool]) -> String {
    face_source_id(&face_id(key, lines), step)
}

/// The recipe source id of step `step` on face `face`: first 16 hex of sha256 of the face id.
pub fn face_source_id(face: &str, step: &str) -> String {
    format!("esc.{}.{step}", hex16(face))
}

/// Steps 1-2 for the uncovered keys of a pass (`dir_rel` = the round's recipe directory, absolute).
pub fn escalate(
    set: &ImportedSet,
    esc: &EscalationCfg,
    ds: &DerivedState,
    uncovered: &[Vec<i32>],
    group: Option<&VerifiedGroup>,
    recipe_dir: &Path,
    face_success: &BTreeMap<String, BTreeMap<String, u64>>,
) -> Result<EscOut, String> {
    let mut out = EscOut::default();
    let lines: Vec<bool> = set.frame.mask().to_vec();
    let (lp, np) = priorities(&lines, esc.coordinate_priority.as_deref());
    let cmp = |a: &[i32], b: &[i32]| set.cmp.cmp(a, b);
    let mut new_faces: BTreeMap<String, Vec<Vec<i32>>> = BTreeMap::new();
    for k in uncovered {
        if ds.terminals.contains(k) || ds.relations.contains_key(k) {
            continue;
        }
        let mut try_recipes = !esc.symmetry_at_key || ds.symrel_tried.contains(k);
        let face = face_id(k, &lines);
        if esc.symmetry_at_key && !ds.symrel_tried.contains(k) {
            let mut line = match group {
                None => json!({"key": k, "verdict": "unavailable", "why": "no verified group"}),
                Some(g) => {
                    match symmetry_relation_at_key(&Independent, g, k, &cmp, esc.symmetry_max_terms)
                    {
                        SymRelation::Relation {
                            rhs,
                            rows,
                            columns,
                            elements,
                        } => {
                            out.symrel_relations += 1;
                            out.hold.insert(k.clone());
                            json!({"key": k, "verdict": "relation", "rows": rows, "columns": columns,
                            "elements": elements, "group_digest": g.digest(),
                            "rhs": rhs.iter().map(|(rk, q)| json!([rk, q_text(q)])).collect::<Vec<_>>()})
                        }
                        SymRelation::NoRelation {
                            rows,
                            rank,
                            columns,
                        } => {
                            json!({"key": k, "verdict": "no_relation", "rows": rows, "rank": rank, "columns": columns})
                        }
                        SymRelation::Unknown(w) => {
                            json!({"key": k, "verdict": "unknown", "why": w})
                        }
                    }
                }
            };
            line["face"] = json!(face);
            let found = line["verdict"] == "relation";
            out.symrel_lines.push(line.to_string());
            // no relation at the key: its recipes are generated in the same pass (no extra round)
            try_recipes = !found;
        }
        if !try_recipes || ds.faces.contains(&face) || ds.escalated.contains(k) {
            continue;
        }
        // one recipe per (FACE, step): the explicit seed set of a step depends on the key only through its support
        // signature, so one recipe file with the face's domain (floor slots fixed, dotted lines >= 2, non-zero numerators
        // <= -1) serves every key of the face, in one job (coordinator 2026-11-14: faces, not keys)
        new_faces.entry(face).or_default().push(k.clone());
    }
    for (face, keys) in &new_faces {
        let k = &keys[0];
        let kt = key_text(k);
        let mut recipes = vec![];
        let mut skipped = vec![];
        let mut seen_sets: BTreeSet<String> = BTreeSet::new();
        // a step that already solved keys of this face goes first (most successes first, then the configured order)
        let mut order: Vec<&String> = esc.steps.iter().collect();
        if let Some(fs) = face_success.get(face) {
            order.sort_by_key(|st| std::cmp::Reverse(fs.get(st.as_str()).copied().unwrap_or(0)));
        }
        let mut fixed: Vec<Value> = vec![];
        let mut bmin: Vec<Value> = vec![];
        let mut bmax: Vec<Value> = vec![];
        for (i, &x) in k.iter().enumerate() {
            let floor = if lines[i] { x == 1 } else { x == 0 };
            fixed.push(if floor { json!(x) } else { Value::Null });
            bmin.push(if lines[i] && !floor {
                json!(2)
            } else {
                Value::Null
            });
            bmax.push(if !lines[i] && !floor {
                json!(-1)
            } else {
                Value::Null
            });
        }
        let domain = json!({"fixed": fixed, "equations": [], "exceptions": [],
            "bounds": {"min": bmin, "max": bmax}});
        for name in order {
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
            let id = face_source_id(face, name);
            let mut doc = pr.recipe.clone();
            doc["id"] = json!(id);
            doc["domain"] = domain.clone();
            if let Some(p) = doc.get_mut("provenance").and_then(Value::as_object_mut) {
                p.remove("termmin_key");
                p.insert("face".into(), json!(face));
                p.insert("first_key".into(), json!(kt));
            }
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
        out.recipe_manifest.push(
            json!({"face": face, "keys": keys, "first_key_text": kt, "recipes": recipes, "skipped": skipped})
                .to_string(),
        );
    }
    Ok(out)
}

/// The face table of the escalated keys (final pass): per face the keys, how they were served (symmetry at the key,
/// which recipe steps), the keys still open and the terminals.
pub fn face_table(
    set: &ImportedSet,
    ds: &DerivedState,
    solved: &BTreeMap<
        String,
        BTreeMap<Vec<i32>, rustred_campaign_core::import::evidence::SolvedRelation>,
    >,
    open: &BTreeSet<Vec<i32>>,
) -> Value {
    let lines: Vec<bool> = set.frame.mask().to_vec();
    let mut keys: BTreeSet<Vec<i32>> = ds.symrel_tried.clone();
    keys.extend(ds.escalated.iter().cloned());
    let mut by_face: BTreeMap<String, Vec<Vec<i32>>> = BTreeMap::new();
    for k in keys {
        by_face.entry(face_id(&k, &lines)).or_default().push(k);
    }
    let mut solved_by: BTreeMap<Vec<i32>, String> = BTreeMap::new();
    let mut tried: BTreeMap<Vec<i32>, u64> = BTreeMap::new();
    for (src, ans) in solved {
        let Some(step) = step_of_source(src) else {
            continue;
        };
        for (k, r) in ans {
            *tried.entry(k.clone()).or_default() += 1;
            if r.verdict == "SOLVED" {
                solved_by
                    .entry(k.clone())
                    .or_insert_with(|| step.to_string());
            }
        }
    }
    let mut rows = vec![];
    let (mut one_recipe, mut several, mut faces_open) = (0u64, 0u64, 0u64);
    for (f, ks) in &by_face {
        let mut steps: BTreeMap<String, u64> = BTreeMap::new();
        let (mut sym, mut term, mut op) = (0u64, 0u64, 0u64);
        let mut answers = 0u64;
        for k in ks {
            answers += tried.get(k).copied().unwrap_or(0);
            if ds.relations.contains_key(k) {
                sym += 1;
            } else if let Some(st) = solved_by.get(k) {
                *steps.entry(st.clone()).or_default() += 1;
            } else if ds.terminals.contains(k) {
                term += 1;
            } else if open.contains(k) {
                op += 1;
            }
        }
        if term + op > 0 {
            faces_open += 1;
        } else if steps.len() <= 1 {
            one_recipe += 1;
        } else {
            several += 1;
        }
        rows.push(
            json!({"face": f, "keys": ks.len(), "symmetry_at_key": sym, "served_by_step": steps,
            "terminal": term, "open": op, "recipe_answers": answers}),
        );
    }
    json!({"faces": by_face.len(), "closed_by_one_recipe_or_symmetry": one_recipe, "needed_several_recipes": several,
        "with_open_or_terminal_keys": faces_open, "face_id": "support signature L<dotted line slots>|N<non-zero numerator slots>",
        "table": rows})
}

#[cfg(test)]
mod tests {
    use super::*;

    /// lane t266run: a restarted session verifies the derived files byte for byte (the same bytes pass; another
    /// re-derivation, a tampered or a missing file refuse, typed; nothing is rewritten).
    #[test]
    fn verify_mode_accepts_the_same_bytes_and_refuses_any_difference() {
        let base = std::env::var_os("TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let d = base.join(format!("derived-verify-{}", std::process::id()));
        let spec = json!({"sources": [{"id": "A"}]}).to_string();
        let mut ds = DerivedState::default();
        ds.relations
            .insert(vec![3, 1, 0], vec![(vec![2, 1, 0], "1".to_string())]);
        ds.terminals.insert(vec![1, 1, 0]);
        let w = effective_spec(&spec, &ds, None, &d, "110", DerivedFiles::Write, None).unwrap();
        let v = effective_spec(&spec, &ds, None, &d, "110", DerivedFiles::Verify, None).unwrap();
        assert_eq!(w, v);
        let table = d.join("symmetry-at-key.table.jsonl");
        let before = std::fs::read(&table).unwrap();
        // another re-derivation (another chart): refused, the file untouched
        let e =
            effective_spec(&spec, &ds, None, &d, "111", DerivedFiles::Verify, None).unwrap_err();
        assert!(
            e.starts_with(super::super::source::EFFECTIVE_SPEC_MISMATCH),
            "{e}"
        );
        assert_eq!(std::fs::read(&table).unwrap(), before);
        // a tampered file: refused, not rewritten
        let tp = d.join("terminal-proposals.json");
        std::fs::write(&tp, "[]\n").unwrap();
        let e =
            effective_spec(&spec, &ds, None, &d, "110", DerivedFiles::Verify, None).unwrap_err();
        assert!(
            e.contains("terminal-proposals.json") && e.contains("refused"),
            "{e}"
        );
        assert_eq!(std::fs::read_to_string(&tp).unwrap(), "[]\n");
        // a missing file: refused
        std::fs::remove_file(&table).unwrap();
        let e =
            effective_spec(&spec, &ds, None, &d, "110", DerivedFiles::Verify, None).unwrap_err();
        assert!(e.contains("absent"), "{e}");
        // a pass (write mode) restores both
        effective_spec(&spec, &ds, None, &d, "110", DerivedFiles::Write, None).unwrap();
        assert_eq!(std::fs::read(&table).unwrap(), before);
        assert!(effective_spec(&spec, &ds, None, &d, "110", DerivedFiles::Verify, None).is_ok());
    }
}
