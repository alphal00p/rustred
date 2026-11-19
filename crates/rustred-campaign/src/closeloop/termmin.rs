//! PHASE E of the closing loop: terminal minimisation by point solves (interfaces/termmin.md section 6), and the shared
//! pieces of the direct driver (`termmin::direct`, section 8).
//!
//! `TermminSource` is a DECORATOR around `CloseFromMaterialSource`: phases A-D are the inner source's, unchanged. For a
//! node whose phase-D verdict is `closed` and whose statement lists terminals, the inner's `DeclareClosed` is WITHHELD
//! (recorded as an input) while phase E runs; phase E never declares the node open. Its decisions come from the core
//! state machine (`rustred_campaign_core::termmin`), fed with the ledger's answers inputs in ledger order (resume =
//! replay: the same decisions, each compared with its recorded decision input). Recipe steps and closing-try solves are
//! items of kind `termmin-solve` (one solve-recipe job each, the closeloop's job form); a settled job goes through the
//! closeloop's accept chain (`closeloop::accept::decide`) with the terminal undeclared. At the end the EXTENDED import
//! spec is written, loaded and checked afresh: the SECOND statement beside the first, the report, then the withheld
//! events (preceded by a fresh `ClosureResult` when phase E accepted an item): the node ends closed in every case.
//! Generic (L8): nothing here knows a node, an arity or a count.

pub mod direct;

use super::accept::{AcceptCtx, BATCH_SCHEMA, BatchFile, Decided, decide};
use super::config::{CONFIG_ROLE, CloseLoopConfig, NodeCfg, Target};
use super::ids::{import_item, input_final, node_dir, safe};
use super::pass::prefix_dispatcher;
use super::source::CloseFromMaterialSource;
use crate::ledger::{AcceptSource, Event, ItemEvent, LevelEvidence};
use crate::state::{AttemptState, CampaignState, EvidenceLevel, ItemPhase, NodeEvent, Rung};
use crate::supervisor::spec::CampaignSpec;
use crate::supervisor::worksource::{
    AcceptedRecord, JobRequest, Routing, Settled, SourceContext, WorkSource,
};
use rustred_campaign_contract::classify::recipe::Verdict;
use rustred_campaign_contract::fake::{FakeOp, FakeScript, OnSigterm, RssPlan};
use rustred_campaign_contract::spec::{Budgets as JobBudgets, InputRef, JobKind};
use rustred_campaign_core::closure::relations::DispatchSlot;
use rustred_campaign_core::import::check::CheckOptions;
use rustred_campaign_core::import::symmetry::load_verified_group;
use rustred_campaign_core::import::{Evidence, ImportedSet, TableSetSpec};
use rustred_campaign_core::replay_base::symrows::VerifiedGroup;
use rustred_campaign_core::termmin::machine::{
    Candidate, Cost, SolveAnswer, SolvePurpose, SolveRequest,
};
use rustred_campaign_core::termmin::{ImportExtension, Termmin, TermminSpec, key_text, priorities};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;

/// Campaign input role of the optional termmin spec file (`rustred.campaign.termmin.v1`).
pub const TERMMIN_ROLE: &str = "termmin";
pub const KIND_TERMMIN: &str = "termmin-solve";
pub const ROLE_WITHHELD: &str = "termmin_withheld";
pub const ROLE_TM_BATCH: &str = "termmin_batch";
pub const ROLE_TM_ANSWERS: &str = "termmin_answers";
pub const ROLE_TM_DECISION: &str = "termmin_decision";
pub const ROLE_TM_RESULT: &str = "termmin_result";
pub const TM_ANSWER_SCHEMA: &str = "rustred.campaign.termmin-answer.v1";

pub fn input_withheld(n: &str) -> String {
    format!("closeloop:{n}:tm:withheld")
}
pub fn input_tm_result(n: &str) -> String {
    format!("closeloop:{n}:termmin")
}
pub fn input_tm_decision(n: &str, seq: usize) -> String {
    format!("closeloop:{n}:tm:decision:{seq:04}")
}
pub fn tm_item(n: &str, req: &str) -> String {
    format!("{n}.tm.{}", safe(req))
}
pub fn input_tm_batch(item: &str) -> String {
    format!("closeloop:{item}:tmbatch")
}
pub fn input_tm_answers(item: &str, job: &str) -> String {
    format!("closeloop:{item}:tmanswers:{job}")
}

fn blake3_hex(b: &[u8]) -> String {
    blake3::hash(b).to_hex().to_string()
}

fn sha256_hex(b: &[u8]) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

fn json_bytes(v: &Value) -> Vec<u8> {
    let mut b = serde_json::to_vec_pretty(v).expect("json");
    b.push(b'\n');
    b
}

fn canon_line(v: &Value) -> String {
    rustred_campaign_core::schema::canon::canonical_json(v, false).unwrap_or_else(|_| v.to_string())
}

// ------------------------------------------------------------------ shared: acceptance and answers ----------------------

/// Identity of the engine and the node for the accept chain.
#[derive(Clone, Debug, Default)]
pub struct AcceptBits {
    pub boxed: bool,
    pub determinism_id: Option<String>,
    pub binary_blake3: String,
    pub binary_sha256: String,
    pub binary_path: String,
    pub family_path: String,
    pub family_blake3: String,
    pub order: String,
    /// lane closeloop3: the per-key caps of the solves (recorded on the answer line of a key that hit one).
    pub max_trace_rows: Option<u64>,
    pub key_budget_s: Option<u64>,
    /// Lane polefix (audit r2-poles F4): the objective of the SOLVED lines' `choice` annotation (`None` = off).
    pub pole_objective: Option<rustred_campaign_core::poles::PoleObjective>,
    /// lane wireflow (h): the engine's pole orders as the choice's profile (AcceptCtx::pole_profiles).
    pub pole_profiles: bool,
}

/// Run the closeloop's accept chain on one settled point solve: `undeclare` = the terminal the solve is FOR (removed
/// from the declared terminals while the chain runs, else the chain would stop at the terminal), `slot` = where the
/// recipe sits in the dispatch (`Last` for a termmin point recipe; the node recipe's own slot for a closing solve).
#[allow(clippy::too_many_arguments)]
pub fn accept_point(
    set: &mut ImportedSet,
    undeclare: Option<&[i32]>,
    batch: &BatchFile,
    slot: &DispatchSlot,
    lines: &[String],
    bits: &AcceptBits,
    results: (&str, &str),
    run_dir: &str,
    pinned: Option<String>,
) -> Result<Decided, String> {
    let removed = undeclare.and_then(|t| {
        let p = set.terminals_other.iter().position(|k| k.as_slice() == t)?;
        Some((p, set.terminals_other.remove(p)))
    });
    let before = prefix_dispatcher(set, slot);
    let out = before.and_then(|before| {
        let ctx = AcceptCtx {
            set,
            before: before.as_ref(),
            boxed: bits.boxed,
            batch,
            pinned_digest: pinned,
            determinism_id: bits.determinism_id.clone(),
            binary_blake3: bits.binary_blake3.clone(),
            binary_sha256: bits.binary_sha256.clone(),
            binary_path: bits.binary_path.clone(),
            family_path: bits.family_path.clone(),
            family_blake3: bits.family_blake3.clone(),
            order: bits.order.clone(),
            results_path: results.0.to_string(),
            results_blake3: results.1.to_string(),
            run_dir: run_dir.to_string(),
            max_trace_rows: bits.max_trace_rows,
            key_budget_s: bits.key_budget_s,
            pole_objective: bits.pole_objective.clone(),
            pole_profiles: bits.pole_profiles,
        };
        decide(&ctx, lines)
    });
    if let Some((p, k)) = removed {
        set.terminals_other
            .insert(p.min(set.terminals_other.len()), k);
    }
    out
}

/// The machine's answer for the decided key `key` of one attempt (`step` / `recipe_id` name the candidate).
pub fn answer_of(
    d: &Decided,
    key: &[i32],
    step: &str,
    recipe_id: &str,
    recipe_path: &str,
    cost: Cost,
) -> SolveAnswer {
    let line: Option<Value> = d
        .lines
        .iter()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find(|v| {
            serde_json::from_value::<Vec<i32>>(v["key"].clone())
                .ok()
                .as_deref()
                == Some(key)
        });
    match line {
        Some(v) if v["verdict"] == "SOLVED" => {
            let rhs: Vec<Vec<i32>> = v["rhs"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|t| serde_json::from_value(t["key"].clone()).ok())
                .collect();
            SolveAnswer::Relation {
                candidate: Candidate {
                    kind: "recipe".into(),
                    step: step.into(),
                    rhs,
                    relation_digest: v["relation_digest"].as_str().map(str::to_string),
                    recipe_id: Some(recipe_id.into()),
                    recipe_sha256: v["recipe_sha256"].as_str().map(str::to_string),
                    payload: json!({"solved": v, "recipe_path": recipe_path}),
                },
                cost,
            }
        }
        Some(v) => SolveAnswer::Failed {
            class: v["failure"]["class"]
                .as_str()
                .unwrap_or("refused")
                .to_string(),
            verdict: v["verdict"].as_str().map(str::to_string),
            failure: v
                .get("failure_record")
                .cloned()
                .unwrap_or(v["failure"].clone()),
            line: Some(v),
            cost,
        },
        None => SolveAnswer::Failed {
            class: "refused".into(),
            verdict: None,
            failure: json!({"class": "refused", "message": d.refused.clone().unwrap_or_else(|| "no decided line for the key".into())}),
            line: None,
            cost,
        },
    }
}

/// Measured cost of a settled run directory (its OUTCOME: timing, memory); never part of a decision.
pub fn cost_of(run_dir: &Path, job: &str) -> Cost {
    // procfix7 round 5: a cost that cannot be read is UNKNOWN (every field None), never zero
    let o: Value = match crate::fsx::read("closeloop.input", &run_dir.join("OUTCOME.json")) {
        crate::fsx::Read::Content(b) => serde_json::from_slice(&b).unwrap_or(Value::Null),
        crate::fsx::Read::Absent | crate::fsx::Read::Failed(_) => Value::Null,
    };
    Cost {
        cpu_s: o["timing"]["cpu_s"]
            .as_f64()
            .or(o["job"]["timing"]["cpu_s"].as_f64()),
        wall_s: o["timing"]["wall_s"]
            .as_f64()
            .or(o["job"]["timing"]["wall_s"].as_f64()),
        vmhwm_kb: o["memory"]["vmhwm_kb"]
            .as_u64()
            .or(o["job"]["memory"]["vmhwm_kb"].as_u64()),
        job: Some(job.to_string()),
    }
}

/// The verified group of the `symmetry_relation` step: the spec's `group` (an import GroupSpec), else derived from
/// the frame table header's `family_source` (engine-table frames). None (with the reason) when neither exists.
pub fn termmin_group(
    spec: &TermminSpec,
    import: &TableSetSpec,
    base: &Path,
    set: &ImportedSet,
) -> Result<Option<VerifiedGroup>, String> {
    let header_family = frame_header_family(import, base)?;
    let gs = match (&spec.group, &header_family) {
        (Some(g), _) => g.clone(),
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
    .map_err(|e| format!("termmin group: {e}"))?;
    Ok(Some(g))
}

/// The `family_source` text of the frame table's header (engine-table frame sources only).
/// procfix7 round 5: Err when a file on the way cannot be READ (the header check is never skipped because of an I/O
/// error); Ok(None) only when the source kind has no header or the file is proven absent or has no such field.
pub fn frame_header_family(import: &TableSetSpec, base: &Path) -> Result<Option<String>, String> {
    let read = |p: &Path| -> Result<Option<Vec<u8>>, String> {
        match crate::fsx::read("closeloop.input", p) {
            crate::fsx::Read::Content(b) => Ok(Some(b)),
            crate::fsx::Read::Absent => Ok(None),
            crate::fsx::Read::Failed(e) => Err(format!("frame header {}: {e}", p.display())),
        }
    };
    let Some(p) = frame_header_path(import, base, &read)? else {
        return Ok(None);
    };
    // the header line only (a table may be large): open and first-line read errors are Err, never "no header"
    let f = match crate::fsx::open("closeloop.input", &p) {
        crate::fsx::Read::Content(f) => f,
        crate::fsx::Read::Absent => return Ok(None),
        crate::fsx::Read::Failed(e) => return Err(format!("frame header {}: {e}", p.display())),
    };
    use std::io::BufRead;
    let mut first = String::new();
    std::io::BufReader::new(f)
        .read_line(&mut first)
        .map_err(|e| format!("frame header {}: {e}", p.display()))?;
    Ok(serde_json::from_str::<Value>(&first)
        .ok()
        .and_then(|h| h["family_source"].as_str().map(str::to_string)))
}

type HeaderRead<'a> = &'a dyn Fn(&Path) -> Result<Option<Vec<u8>>, String>;

/// The frame table whose header names the family (None when the source kind has none or a file is proven absent).
fn frame_header_path(
    import: &TableSetSpec,
    base: &Path,
    read: HeaderRead<'_>,
) -> Result<Option<PathBuf>, String> {
    let Ok(v) = serde_json::to_value(import) else {
        return Ok(None);
    };
    let Some(src) = v["sources"].as_array().and_then(|a| {
        a.iter()
            .find(|s| s["id"].as_str() == Some(import.frame_from.as_str()))
    }) else {
        return Ok(None);
    };
    let rel = |x: &str| -> PathBuf {
        let p = PathBuf::from(x);
        if p.is_absolute() { p } else { base.join(p) }
    };
    Ok(match src["kind"].as_str() {
        Some("engine_table") => src["path"].as_str().map(rel),
        // a table set (coord-bank-tables.md layout): the first table of the variant in the priority file
        Some("table_set") => {
            let Some(pfile) = src["priority_file"].as_str() else {
                return Ok(None);
            };
            let Some(pb) = read(&rel(pfile))? else {
                return Ok(None);
            };
            let first = serde_json::from_slice::<Value>(&pb).ok().and_then(|pf| {
                let v = src["variant"].as_str()?;
                Some(pf[v].as_array()?.first()?["file"].as_str()?.to_string())
            });
            match (first, src["tables_dir"].as_str()) {
                (Some(f), Some(d)) => Some(rel(d).join(f)),
                _ => None,
            }
        }
        _ => None,
    })
}

/// The machine for a loaded node: terminals reached by the base check, preferred = the import's preferred terminals +
/// the spec's, priorities from the node order (or the spec).
pub fn build_machine(
    spec: &TermminSpec,
    ext: &ImportExtension,
    explicit: Option<&[Vec<i32>]>,
) -> Result<Termmin, String> {
    let lines = ext.set.frame.mask().to_vec();
    let mut preferred: BTreeSet<Vec<i32>> = ext.set.terminals_preferred.iter().cloned().collect();
    preferred.extend(spec.preferred.iter().cloned());
    let cp = ext.set.cmp.equivalent_descriptor().coordinate_priority;
    let (lp, np) = priorities(&lines, Some(&cp));
    let lp = spec.line_priority.clone().unwrap_or(lp);
    let np = spec.numerator_priority.clone().unwrap_or(np);
    let terminals: Vec<Vec<i32>> = match explicit {
        Some(t) => t.to_vec(),
        None => ext.base().terminals.clone(),
    };
    let cmp = |a: &[i32], b: &[i32]| ext.set.cmp.cmp(a, b);
    Termmin::new(
        spec,
        &terminals,
        &preferred,
        &lines,
        &lp,
        &np,
        ext.base().economy,
        &cmp,
    )
}

// ------------------------------------------------------------------ the extended import spec (second statement) ----------

/// The node's import spec EXTENDED by the kept eliminations: one recipe source per eliminated terminal (its point
/// recipe file), the symmetry relations as a `supplied_relations` table verified from the group, the closing-try answers
/// and the eliminating relations as `evidence.solved`, the declared terminals reduced to the remaining keys.
#[allow(clippy::too_many_arguments)]
/// lane closeloop4: the family of an import spec's tables: the header of its `frame_from` source (else the first source
/// with a readable table header).
fn spec_family(v: &Value, base: &Path) -> Result<Option<String>, String> {
    use std::io::BufRead;
    let Some(srcs) = v["sources"].as_array() else {
        return Ok(None);
    };
    let frame = v["frame_from"].as_str();
    let pick = srcs
        .iter()
        .filter(|s| frame.is_none_or(|f| s["id"] == f))
        .chain(srcs.iter());
    for s in pick {
        let Some(p) = s["path"].as_str() else {
            continue;
        };
        let pb = PathBuf::from(p);
        let pb = if pb.is_absolute() { pb } else { base.join(pb) };
        // procfix7 round 5: only a PROVEN absent table is skipped; a table that cannot be opened or whose header line
        // cannot be read fails (never another source's family)
        let f = match crate::fsx::open("closeloop.input", &pb) {
            crate::fsx::Read::Content(f) => f,
            crate::fsx::Read::Absent => continue,
            crate::fsx::Read::Failed(e) => return Err(format!("table {}: {e}", pb.display())),
        };
        let mut l = String::new();
        std::io::BufReader::new(f)
            .read_line(&mut l)
            .map_err(|e| format!("table {}: {e}", pb.display()))?;
        if let Ok(h) = serde_json::from_str::<Value>(l.trim())
            && let Some(fam) = h["family"].as_str()
        {
            return Ok(Some(fam.to_string()));
        }
    }
    Ok(None)
}

#[allow(clippy::too_many_arguments)]
pub fn extended_spec(
    import_text: &str,
    base: &Path,
    rep: &rustred_campaign_core::termmin::Report,
    answers: &BTreeMap<String, SolveAnswer>,
    dir: &Path,
    group_spec: Option<&Value>,
    order: &Value,
    inner_solved: &[PathBuf],
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir.join("recipes")).map_err(|e| e.to_string())?;
    let mut v: Value =
        serde_json::from_str(import_text).map_err(|e| format!("import spec: {e}"))?;
    // relative paths of the original spec stay valid: resolve them against the original base
    let abs = |p: &str| -> String {
        let pb = PathBuf::from(p);
        if pb.is_absolute() {
            p.to_string()
        } else {
            base.join(pb).display().to_string()
        }
    };
    fn fix_paths(v: &mut Value, abs: &dyn Fn(&str) -> String) {
        match v {
            Value::Object(m) => {
                for (k, x) in m.iter_mut() {
                    if matches!(k.as_str(), "path" | "priority_file" | "tables_dir") {
                        if let Some(s) = x.as_str() {
                            *x = Value::String(abs(s));
                        }
                    } else {
                        fix_paths(x, abs);
                    }
                }
            }
            Value::Array(a) => a.iter_mut().for_each(|x| fix_paths(x, abs)),
            _ => {}
        }
    }
    fix_paths(&mut v, &abs);
    let eliminated: BTreeSet<Vec<i32>> = rep
        .terminals
        .iter()
        .filter(|t| t.outcome == "eliminated")
        .map(|t| t.key.clone())
        .collect();
    let remaining: Vec<Vec<i32>> = rep.final_terminals.clone();
    // declared terminals: non-preferred terminal sources keep only the remaining keys
    let sources = v["sources"]
        .as_array_mut()
        .ok_or("import spec without sources")?;
    let mut new_sources: Vec<Value> = vec![];
    for s in sources.iter() {
        if s["kind"] == "terminals" && s["preferred"] != true {
            let mut s2 = s.clone();
            if let Some(o) = s2.as_object_mut() {
                o.remove("path");
                o.insert(
                    "keys".into(),
                    json!(
                        remaining
                            .iter()
                            .filter(|k| !eliminated.contains(*k))
                            .collect::<Vec<_>>()
                    ),
                );
            }
            new_sources.push(s2);
        } else {
            new_sources.push(s.clone());
        }
    }
    let mut solved_lines: Vec<String> = vec![];
    let mut sym_lines: Vec<String> = vec![];
    for t in rep.terminals.iter().filter(|t| t.outcome == "eliminated") {
        let by = t.eliminated_by.clone().unwrap_or(Value::Null);
        let step_idx = t
            .steps
            .iter()
            .find(|s| s.outcome == "eliminated")
            .map(|s| s.index);
        let Some(si) = step_idx else { continue };
        let req = format!("t{}.s{}", t.order, si);
        match by["kind"].as_str() {
            Some("recipe") => {
                let Some(SolveAnswer::Relation { candidate, .. }) = answers.get(&req) else {
                    return Err(format!(
                        "no answer {req} for the eliminated {}",
                        key_text(&t.key)
                    ));
                };
                let src =
                    rustred_campaign_core::termmin::extend::candidate_source(&t.key, candidate);
                let rp = candidate.payload["recipe_path"]
                    .as_str()
                    .ok_or("recipe path")?;
                let text = std::fs::read(rp).map_err(|e| format!("{rp}: {e}"))?;
                let local = dir.join("recipes").join(format!("{}.json", safe(&src)));
                std::fs::write(&local, &text).map_err(|e| e.to_string())?;
                new_sources.push(json!({"id": src, "kind": "recipe", "class": "termmin",
                    "path": local.display().to_string(), "sha256": sha256_hex(&text)}));
                let mut l = candidate.payload["solved"].clone();
                l["source"] = json!(src);
                solved_lines.push(canon_line(&l));
            }
            Some("symmetry") => {
                let Some(c) = rep
                    .terminals
                    .iter()
                    .find(|x| x.key == t.key)
                    .and_then(|x| x.eliminated_by.clone())
                else {
                    continue;
                };
                let coefs = c["coefficients"].clone();
                let tt = key_text(&t.key);
                let rhs: Vec<Value> = coefs
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|x| {
                        let k: Vec<i32> = serde_json::from_value(x[0].clone()).ok()?;
                        Some(json!([key_text(&k), x[1].clone()]))
                    })
                    .collect();
                sym_lines.push(
                    json!({"type": "rule", "chart": "", "index": sym_lines.len(), "kind": "symmetry-relation",
                       "case": "point", "fixed": t.key, "equations": [], "target": tt, "rhs": rhs,
                       "exceptions": [], "dispatch": "Point", "witness_loci": [],
                       "symmetry_elements": c["elements"].clone()})
                    .to_string(),
                );
            }
            _ => {}
        }
    }
    // closing-try answers (the node's own recipe sources) of the eliminated terminals
    for (id, a) in answers {
        if !id.contains(".c") {
            continue;
        }
        let ti: Option<usize> = id
            .strip_prefix('t')
            .and_then(|x| x.split('.').next())
            .and_then(|x| x.parse().ok());
        let Some(ti) = ti else { continue };
        let Some(t) = rep.terminals.get(ti) else {
            continue;
        };
        if t.outcome != "eliminated" {
            continue;
        }
        let l = match a {
            SolveAnswer::Relation { candidate, .. } => candidate.payload.get("solved").cloned(),
            SolveAnswer::Failed { line, .. } => line.clone(),
        };
        if let Some(l) = l {
            solved_lines.push(canon_line(&l));
        }
    }
    if !sym_lines.is_empty() {
        let p = dir.join("symmetry-relations.table.jsonl");
        let mut txt = json!({"type": "header", "schema": "table_dump.v1", "family": "termmin", "order": order}).to_string();
        txt.push('\n');
        for l in &sym_lines {
            txt.push_str(l);
            txt.push('\n');
        }
        std::fs::write(&p, &txt).map_err(|e| e.to_string())?;
        let mut s = json!({"id": "termmin-symmetry", "kind": "supplied_relations", "class": "termmin_symmetry",
            "path": p.display().to_string(), "sha256": sha256_hex(txt.as_bytes())});
        if let Some(g) = group_spec {
            s["verify"] = g.clone();
        }
        new_sources.push(s);
    }
    v["sources"] = Value::Array(new_sources);
    // lane closeloop4 (audit-closeloop m4): evidence names the family of the set's tables (the frame source's header)
    let fam = spec_family(&v, base)?;
    // the closing loop's own recipe answers (ledger inputs of the first statement's passes)
    if !inner_solved.is_empty() {
        if !v["evidence"].is_object() {
            v["evidence"] = json!({});
        }
        let mut arr = v["evidence"]["solved"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        for f in inner_solved {
            let b = std::fs::read(f).map_err(|e| format!("{}: {e}", f.display()))?;
            arr.push(
                json!({"path": f.display().to_string(), "sha256": sha256_hex(&b), "family": fam}),
            );
        }
        v["evidence"]["solved"] = Value::Array(arr);
    }
    if !solved_lines.is_empty() {
        let p = dir.join("termmin-answers.jsonl");
        let mut txt = solved_lines.join("\n");
        txt.push('\n');
        std::fs::write(&p, &txt).map_err(|e| e.to_string())?;
        let ev = v["evidence"].as_object_mut().map(|_| ()).is_some();
        if !ev {
            v["evidence"] = json!({});
        }
        let arr = v["evidence"]["solved"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let mut arr = arr;
        arr.push(json!({"path": p.display().to_string(), "sha256": sha256_hex(txt.as_bytes()), "family": fam}));
        v["evidence"]["solved"] = Value::Array(arr);
    }
    let out = dir.join("import-spec.json");
    std::fs::write(&out, json_bytes(&v)).map_err(|e| e.to_string())?;
    Ok(out)
}

// ------------------------------------------------------------------ phase E: the decorator ------------------------------

enum Loaded {
    Ext(Box<ImportExtension>, Box<Termmin>, Option<Value>),
    Second(Box<Value>),
}

struct PhaseE {
    cfg: NodeCfg,
    n: String,
    spec: TermminSpec,
    spec_text: String,
    loading: Option<JoinHandle<Result<Loaded, String>>>,
    ext: Option<ImportExtension>,
    group_spec: Option<Value>,
    machine: Option<Termmin>,
    fed: BTreeSet<String>,
    answers: BTreeMap<String, SolveAnswer>,
    batches: BTreeMap<String, BatchFile>,
    failed: Option<String>,
    second: Option<Value>,
}

/// The decorator.
pub struct TermminSource {
    inner: CloseFromMaterialSource,
    cdir: PathBuf,
    nodes: Vec<PhaseE>,
    programs: BTreeMap<String, PathBuf>,
}

/// Wrap the closing loop's source (the ONE construction hook, closeloop.md 6.2 / termmin.md 6-7).
pub fn wrap(
    inner: CloseFromMaterialSource,
    spec: &CampaignSpec,
    cdir: &Path,
) -> Result<TermminSource, String> {
    let tspec = match spec.inputs.iter().find(|i| i.role == TERMMIN_ROLE) {
        Some(i) => TermminSpec::parse(
            &std::fs::read_to_string(&i.path)
                .map_err(|e| format!("termmin spec {}: {e}", i.path.display()))?,
        )?,
        None => TermminSpec::default(),
    };
    let cin = spec
        .inputs
        .iter()
        .find(|i| i.role == CONFIG_ROLE)
        .ok_or("termmin: no closeloop config input")?;
    let cfg = CloseLoopConfig::read_with_resources(&cin.path, &spec.resources)?;
    let mut nodes = vec![];
    for nc in cfg.nodes {
        let s = tspec.for_node(&nc.node)?;
        // the EFFECTIVE import spec (the inner's ledger input) is read when phase E starts
        let spec_text = String::new();
        nodes.push(PhaseE {
            n: safe(&nc.node),
            cfg: nc,
            spec: s,
            spec_text,
            loading: None,
            ext: None,
            group_spec: None,
            machine: None,
            fed: BTreeSet::new(),
            answers: BTreeMap::new(),
            batches: BTreeMap::new(),
            failed: None,
            second: None,
        });
    }
    let programs = spec
        .programs
        .iter()
        .map(|(k, p)| (k.clone(), p.path.clone()))
        .collect();
    Ok(TermminSource {
        inner,
        cdir: cdir.to_path_buf(),
        nodes,
        programs,
    })
}

/// Fake-child answers per (source, key) (tests only).
type FakeOracle = BTreeMap<(String, Vec<i32>), (Verdict, Vec<Vec<i64>>)>;

fn record(path: String, bytes: Vec<u8>) -> AcceptedRecord {
    AcceptedRecord { path, bytes }
}

impl TermminSource {
    fn abs(&self, rel: &str) -> PathBuf {
        self.cdir.join(rel)
    }

    fn node_of_item(&self, st: &CampaignState, item: &str) -> Option<usize> {
        let it = st.items.get(item)?;
        if it.kind != KIND_TERMMIN {
            return None;
        }
        self.nodes.iter().position(|p| p.cfg.node == it.node)
    }

    fn read_input(st: &CampaignState, id: &str) -> Result<Vec<u8>, String> {
        let r = st
            .inputs
            .get(id)
            .ok_or_else(|| format!("input {id} not registered"))?;
        let b = std::fs::read(&r.path).map_err(|e| format!("input {id} ({}): {e}", r.path))?;
        if blake3_hex(&b) != r.blake3 {
            return Err(format!(
                "input {id} ({}) changed since it was recorded: refused",
                r.path
            ));
        }
        Ok(b)
    }

    /// Phase E state of node `i`: None = not (yet) active; Some(false) = active; Some(true) = finished.
    fn phase(&self, st: &CampaignState, i: usize) -> Option<bool> {
        let p = &self.nodes[i];
        if st.inputs.contains_key(&input_tm_result(&p.n)) {
            return Some(true);
        }
        st.inputs
            .contains_key(&input_withheld(&p.n))
            .then_some(false)
    }

    /// procfix7 round 5: the accept bits are never computed from bytes that were not read (an error fails the route).
    fn bits(&self, p: &PhaseE) -> Result<AcceptBits, String> {
        let prog = p.cfg.solve.program.clone();
        let path = self.programs.get(&prog).cloned().unwrap_or_default();
        let bytes = crate::fsx::read("closeloop.input", &path)
            .required(&path)
            .map_err(|e| format!("solve program {}: {e}", path.display()))?;
        let family_blake3 = crate::atomic::blake3_file(&p.cfg.solve.family)
            .map(|x| x.1)
            .map_err(|e| format!("family {}: {e}", p.cfg.solve.family.display()))?;
        Ok(AcceptBits {
            boxed: p.cfg.target == Target::Boxed,
            determinism_id: p.cfg.solve.determinism_id.clone(),
            binary_blake3: blake3_hex(&bytes),
            binary_sha256: sha256_hex(&bytes),
            binary_path: path.display().to_string(),
            family_path: p.cfg.solve.family.display().to_string(),
            family_blake3,
            order: p.cfg.solve.order_arg(),
            max_trace_rows: p.spec.caps.max_trace_rows.or(p.cfg.solve.max_trace_rows),
            key_budget_s: p.spec.caps.key_budget_s.or(p.cfg.solve.key_budget_s),
            // the node's setting (validated with the config; an unreadable one cannot reach here)
            pole_objective: p.cfg.pole_objective_setting().unwrap_or_default(),
            // lane wireflow (h): the node's switch or the termmin spec's
            pole_profiles: p.cfg.pole_profiles || p.spec.pole_profiles,
        })
    }

    /// The inner's routing: withhold `DeclareClosed` of a node that enters phase E (same routing as its final input).
    fn filter_inner(&mut self, mut rt: Routing) -> Result<Routing, String> {
        for i in 0..self.nodes.len() {
            let (n, node) = (self.nodes[i].n.clone(), self.nodes[i].cfg.node.clone());
            if !self.nodes[i].spec.enabled {
                continue;
            }
            let fin = input_final(&n);
            let Some(fr) = rt.events.iter().find_map(|e| match e {
                Event::InputRegistered { input, path, .. } if *input == fin => Some(path.clone()),
                _ => None,
            }) else {
                continue;
            };
            // the result record is among the routing's accepts (written before the events)
            let res: Value = rt
                .accepts
                .iter()
                .find(|a| self.abs(&a.path).display().to_string() == fr || a.path == fr)
                .and_then(|a| serde_json::from_slice(&a.bytes).ok())
                .unwrap_or(Value::Null);
            if res["verdict"] != "closed" {
                continue;
            }
            let sdir = PathBuf::from(res["statement"]["dir"].as_str().unwrap_or(""));
            // procfix7 round 5: an unreadable terminals file fails the routing (never "no terminals", which would let
            // DeclareClosed through)
            let terms: Vec<Value> =
                match crate::fsx::read("closeloop.input", &sdir.join("terminals.json")) {
                    crate::fsx::Read::Content(b) => serde_json::from_slice::<Value>(&b)
                        .ok()
                        .map(|v| terminals_of(&v))
                        .unwrap_or_default(),
                    crate::fsx::Read::Absent => Vec::new(),
                    crate::fsx::Read::Failed(e) => {
                        return Err(format!("{}/terminals.json: {e}", sdir.display()));
                    }
                };
            if terms.is_empty() {
                continue;
            }
            let pos = rt.events.iter().position(|e| {
                matches!(e, Event::Node { node: x, ev: NodeEvent::DeclareClosed } if *x == node)
            });
            let withheld: Vec<Event> = pos.map(|p| vec![rt.events.remove(p)]).unwrap_or_default();
            let w = record(
                format!("{}/termmin/withheld.json", node_dir(&n)),
                json_bytes(
                    &json!({"schema": "rustred.campaign.termmin-withheld.v1", "node": node,
                    "events": withheld, "terminals": terms}),
                ),
            );
            let r = w.file_ref();
            rt.events.push(Event::InputRegistered {
                input: input_withheld(&n),
                role: ROLE_WITHHELD.into(),
                path: self.abs(&w.path).display().to_string(),
                bytes: r.bytes,
                blake3: r.blake3,
            });
            rt.events.push(Event::Note {
                text: format!(
                    "closeloop {node}: phase E (terminal minimisation) starts; the node's closed verdict is declared when it ends"
                ),
            });
            rt.accepts.push(w);
        }
        Ok(rt)
    }

    fn start_load(&mut self, st: &CampaignState, i: usize) -> Result<(), String> {
        let n = self.nodes[i].n.clone();
        let node = self.nodes[i].cfg.node.clone();
        // lane repairflow (repairflow.md 6): the composed spec when the repair flow wrote one
        let text = super::repair::effective_spec_text(st, &n)?;
        self.nodes[i].spec_text = text.clone();
        // the inner's recipe answers (the closing loop's evidence), ledger order, digests checked in the thread
        let mut files: Vec<(u64, PathBuf, String)> = st
            .inputs
            .iter()
            .filter(|(id, r)| {
                r.role == super::ids::ROLE_ANSWERS
                    && id
                        .strip_prefix("closeloop:")
                        .and_then(|x| x.split_once(":answers:"))
                        .and_then(|(item, _)| st.items.get(item))
                        .is_some_and(|it| it.node == node)
            })
            .map(|(_, r)| (r.registered_seq, PathBuf::from(&r.path), r.blake3.clone()))
            .collect();
        files.sort();
        let p = &self.nodes[i];
        let base = p.cfg.base();
        let spec = p.spec.clone();
        let threads = p.cfg.check_threads;
        let h = std::thread::Builder::new()
            .name(format!("termmin-load-{n}"))
            .spawn(move || -> Result<Loaded, String> {
                let isp =
                    TableSetSpec::from_json(&text).map_err(|e| format!("import spec: {e}"))?;
                let set = rustred_campaign_core::import::load(&isp, &base)
                    .map_err(|e| format!("import load: {e}"))?;
                let mut ev =
                    Evidence::from_spec(&set, &base).map_err(|e| format!("evidence: {e}"))?;
                // lane closeloop4: answers files are verified against their ledger BLAKE3 just below
                ev.accept_ledger_verified_files();
                for (_, f, b3) in files {
                    let (_, d) = crate::atomic::blake3_file(&f)
                        .map_err(|e| format!("{}: {e}", f.display()))?;
                    if d != b3 {
                        return Err(format!(
                            "answers file {} changed since it was recorded",
                            f.display()
                        ));
                    }
                    ev.load_solved(
                        &set,
                        Path::new("/"),
                        &rustred_campaign_core::import::spec::FileRef {
                            path: f.display().to_string(),
                            sha256: None,
                            manifest: None,
                            family: None,
                        },
                    )
                    .map_err(|e| format!("answers {}: {e}", f.display()))?;
                }
                let group = termmin_group(&spec, &isp, &base, &set)?;
                let gs = spec
                    .group
                    .as_ref()
                    .map(|g| serde_json::to_value(g).unwrap_or(Value::Null));
                let ext = ImportExtension::new(
                    set,
                    ev,
                    CheckOptions {
                        threads: threads.max(1),
                        tier: None,
                        max_rounds: None,
                    },
                    group,
                    spec.caps.symmetry_max_terms,
                )?;
                let m = build_machine(&spec, &ext, None)?;
                Ok(Loaded::Ext(Box::new(ext), Box::new(m), gs))
            })
            .map_err(|e| format!("load thread: {e}"))?;
        self.nodes[i].loading = Some(h);
        Ok(())
    }

    /// Replay the recorded phase-E answers (ledger order) into the machine, advance, and emit what follows.
    fn step_node(&mut self, st: &CampaignState, i: usize, rt: &mut Routing) -> Result<(), String> {
        let n = self.nodes[i].n.clone();
        let node = self.nodes[i].cfg.node.clone();
        // answers inputs of this node's termmin items, in ledger order
        let mut ans: Vec<(u64, String)> = st
            .inputs
            .iter()
            .filter(|(id, r)| {
                r.role == ROLE_TM_ANSWERS
                    && id
                        .strip_prefix("closeloop:")
                        .and_then(|x| x.split_once(":tmanswers:"))
                        .and_then(|(item, _)| st.items.get(item))
                        .is_some_and(|it| it.node == node)
            })
            .map(|(id, r)| (r.registered_seq, id.clone()))
            .collect();
        ans.sort();
        for (_, id) in ans {
            if self.nodes[i].fed.contains(&id) {
                continue;
            }
            let v: Value = serde_json::from_slice(&Self::read_input(st, &id)?)
                .map_err(|e| format!("{id}: {e}"))?;
            let req = v["request"]
                .as_str()
                .ok_or("answer without request")?
                .to_string();
            let a: SolveAnswer =
                serde_json::from_value(v["answer"].clone()).map_err(|e| format!("{id}: {e}"))?;
            let p = &mut self.nodes[i];
            p.machine
                .as_mut()
                .ok_or("no machine")?
                .answer(&req, a.clone())?;
            p.answers.insert(req, a);
            p.fed.insert(id);
        }
        {
            let p = &mut self.nodes[i];
            let (m, ext) = (
                p.machine.as_mut().ok_or("no machine")?,
                p.ext.as_mut().ok_or("no ext")?,
            );
            m.advance(ext)?;
        }
        // decisions: recorded ones must equal the recomputed ones; new ones are recorded
        let decisions = self.nodes[i]
            .machine
            .as_ref()
            .map(|m| m.decisions().to_vec())
            .unwrap_or_default();
        for d in &decisions {
            let id = input_tm_decision(&n, d.seq);
            let v = serde_json::to_value(d).map_err(|e| e.to_string())?;
            match st.inputs.get(&id) {
                Some(_) => {
                    let old: Value = serde_json::from_slice(&Self::read_input(st, &id)?)
                        .map_err(|e| e.to_string())?;
                    if old != v {
                        return Err(format!(
                            "termmin decision {} recomputed differently on resume (recorded {old}, now {v})",
                            d.seq
                        ));
                    }
                }
                None => {
                    let r = record(
                        format!("{}/termmin/decisions/{:04}.json", node_dir(&n), d.seq),
                        json_bytes(&v),
                    );
                    let f = r.file_ref();
                    rt.events.push(Event::InputRegistered {
                        input: id,
                        role: ROLE_TM_DECISION.into(),
                        path: self.abs(&r.path).display().to_string(),
                        bytes: f.bytes,
                        blake3: f.blake3,
                    });
                    rt.accepts.push(r);
                }
            }
        }
        // new solve items
        let pending = self.nodes[i]
            .machine
            .as_ref()
            .map(|m| m.pending_solves())
            .unwrap_or_default();
        for r in pending {
            let item = tm_item(&n, &r.id);
            if st.items.contains_key(&item) || self.nodes[i].batches.contains_key(&item) {
                continue;
            }
            self.new_item(i, &item, &r, rt)?;
        }
        Ok(())
    }

    fn new_item(
        &mut self,
        i: usize,
        item: &str,
        r: &SolveRequest,
        rt: &mut Routing,
    ) -> Result<(), String> {
        let n = self.nodes[i].n.clone();
        let node = self.nodes[i].cfg.node.clone();
        let dir = format!("{}/termmin", node_dir(&n));
        let (source, path, sha, step, rid) = match &r.purpose {
            SolvePurpose::Step { recipe, .. } => {
                let rel = format!("{dir}/recipes/{}.json", safe(&r.id));
                let p = self.abs(&rel);
                if let Some(d) = p.parent() {
                    std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
                }
                // procfix7 round 5: rewritten only when PROVEN absent or different
                let same = match crate::fsx::read("closeloop.output", &p) {
                    crate::fsx::Read::Content(b) => b == recipe.text.as_bytes(),
                    crate::fsx::Read::Absent => false,
                    crate::fsx::Read::Failed(e) => return Err(format!("{}: {e}", p.display())),
                };
                if !same {
                    crate::atomic::write_atomic(&p, recipe.text.as_bytes())
                        .map_err(|e| e.to_string())?;
                }
                (
                    format!("termmin/{}", recipe.step),
                    p.display().to_string(),
                    recipe.sha256.clone(),
                    recipe.step.clone(),
                    recipe.id.clone(),
                )
            }
            SolvePurpose::Closing { solve, .. } => (
                solve.source.clone(),
                solve.recipe_path.clone(),
                solve.recipe_sha256.clone(),
                "closing".to_string(),
                solve.source.clone(),
            ),
        };
        let rb = std::fs::read(&path).map_err(|e| format!("{path}: {e}"))?;
        let bf = BatchFile {
            schema: BATCH_SCHEMA.into(),
            node: node.clone(),
            item: item.to_string(),
            round: 0,
            source,
            recipe_path: path,
            recipe_sha256: sha,
            recipe_blake3: blake3_hex(&rb),
            engine_digest: None,
            relation_rule: match &r.purpose {
                SolvePurpose::Step { recipe, .. } => recipe.relation_rule.clone(),
                SolvePurpose::Closing { .. } => self.nodes[i].cfg.solve.relation_rule.clone(),
            },
            keys: vec![r.key.clone()],
        };
        let mut kb = serde_json::to_vec(&json!({"key": r.key})).expect("json");
        kb.push(b'\n');
        let keys = record(format!("{dir}/batches/{item}.keys.jsonl"), kb);
        let meta = json!({"batch": bf, "request": r.id, "terminal": r.terminal, "step": step, "recipe_id": rid});
        let bfr = record(format!("{dir}/batches/{item}.json"), json_bytes(&meta));
        let f = bfr.file_ref();
        rt.events.push(Event::InputRegistered {
            input: input_tm_batch(item),
            role: ROLE_TM_BATCH.into(),
            path: self.abs(&bfr.path).display().to_string(),
            bytes: f.bytes,
            blake3: f.blake3,
        });
        rt.events.push(Event::ItemCreated {
            item: item.to_string(),
            node,
            kind: KIND_TERMMIN.into(),
            rung: Rung(0),
            parent: Some(import_item(&n)),
            inputs: vec![input_tm_batch(item)],
            label: format!("termmin {} {}", key_text(&r.terminal), step),
            weight: 1,
        });
        rt.accepts.push(keys);
        rt.accepts.push(bfr);
        self.nodes[i].batches.insert(item.to_string(), bf);
        Ok(())
    }

    fn meta(&self, st: &CampaignState, item: &str) -> Result<Value, String> {
        serde_json::from_slice(&Self::read_input(st, &input_tm_batch(item))?)
            .map_err(|e| e.to_string())
    }

    fn fake_script(&self, i: usize, b: &BatchFile) -> Option<FakeScript> {
        let f = self.nodes[i].cfg.solve.fake.as_ref()?;
        let mut oracle: FakeOracle = BTreeMap::new();
        // the fake oracle (test double; set only by test specs): unreadable = no fake script
        let crate::fsx::Read::Content(text) =
            crate::fsx::read_to_string("closeloop.input", &f.oracle)
        else {
            return None;
        };
        for l in text.lines().filter(|l| !l.trim().is_empty()) {
            let x: Value = serde_json::from_str(l).ok()?;
            let key: Vec<i32> = serde_json::from_value(x["key"].clone()).ok()?;
            let verdict: Verdict = serde_json::from_value(x["verdict"].clone()).ok()?;
            let rhs: Vec<Vec<i64>> =
                serde_json::from_value(x.get("rhs").cloned().unwrap_or(json!([]))).ok()?;
            oracle.insert(
                (x["source"].as_str().unwrap_or("").to_string(), key),
                (verdict, rhs),
            );
        }
        let mut ops: Vec<FakeOp> = f
            .quirks
            .iter()
            .map(|q| FakeOp::RecipeQuirk { quirk: *q })
            .collect();
        for q in &self.nodes[i].spec.fake_quirks {
            if let Ok(quirk) = serde_json::from_value(Value::String(q.clone())) {
                ops.push(FakeOp::RecipeQuirk { quirk });
            }
        }
        ops.push(FakeOp::Prepare { ms: 10 });
        for k in &b.keys {
            let (verdict, rhs) = oracle
                .get(&(b.source.clone(), k.clone()))
                .cloned()
                .unwrap_or((Verdict::NoRelationInSeeds, vec![]));
            ops.push(FakeOp::Relation {
                ms: f.ms,
                verdict,
                rhs,
            });
        }
        ops.push(FakeOp::End {
            stop: None,
            verdict: None,
        });
        Some(FakeScript {
            script: ops,
            on_sigterm: OnSigterm::Graceful,
            slow_ms: 0,
            start_delay_ms: 0,
            rss: RssPlan::default(),
            helpers: vec![],
            faults: vec![],
            time_scale: 1.0,
            volatile_jitter: false,
        })
    }

    fn request(
        &self,
        st: &CampaignState,
        i: usize,
        item: &str,
        job: String,
    ) -> Result<JobRequest, String> {
        let p = &self.nodes[i];
        let b = p.batches.get(item).cloned().map(Ok).unwrap_or_else(|| {
            serde_json::from_value::<BatchFile>(self.meta(st, item)?["batch"].clone())
                .map_err(|e| e.to_string())
        })?;
        let dir = format!("{}/termmin", node_dir(&p.n));
        let keys_abs = self.abs(&format!("{dir}/batches/{item}.keys.jsonl"));
        let keys_b3 = crate::atomic::blake3_file(&keys_abs)
            .map_err(|e| e.to_string())?
            .1;
        let sc = &p.cfg.solve;
        let caps = &p.spec.caps;
        let mut args = vec![
            "--order".to_string(),
            sc.order_arg(),
            "--threads".into(),
            caps.threads.to_string(),
        ];
        if let Some(x) = caps.max_trace_rows.or(sc.max_trace_rows) {
            args.extend(["--max-trace-rows".into(), x.to_string()]);
        }
        if let Some(x) = caps.key_budget_s.or(sc.key_budget_s) {
            args.extend(["--key-budget-s".into(), x.to_string()]);
        }
        Ok(JobRequest {
            job,
            item: item.to_string(),
            rung: Rung(0),
            kind: JobKind::SolveRecipe,
            priority: -1_000_000_000,
            cap_bytes: caps.cap_bytes,
            expected_peak_bytes: caps.expected_peak_bytes,
            cores: caps.threads as usize,
            heartbeat_s: sc.heartbeat_s,
            stop_grace_s: sc.stop_grace_s,
            program: sc.program.clone(),
            args,
            inputs: vec![
                InputRef {
                    role: "family".into(),
                    path: sc.family.display().to_string(),
                    blake3: crate::atomic::blake3_file(&sc.family)
                        .map_err(|e| e.to_string())?
                        .1,
                    upto: None,
                },
                InputRef {
                    role: "recipe".into(),
                    path: b.recipe_path.clone(),
                    blake3: b.recipe_blake3.clone(),
                    upto: None,
                },
                InputRef {
                    role: "keys".into(),
                    path: keys_abs.display().to_string(),
                    blake3: keys_b3,
                    upto: None,
                },
            ],
            depends_on: vec![input_tm_batch(item)],
            budgets: JobBudgets::default(),
            sample_seed: 0,
            fake: self.fake_script(i, &b),
        })
    }

    /// The end of phase E: the extended spec, the second statement (in a thread), then the report and the release.
    fn finish_node(
        &mut self,
        st: &CampaignState,
        i: usize,
        rt: &mut Routing,
    ) -> Result<(), String> {
        let n = self.nodes[i].n.clone();
        let node = self.nodes[i].cfg.node.clone();
        if self.nodes[i].second.is_none() {
            if self.nodes[i].loading.is_some() {
                return Ok(());
            }
            let rep = self.nodes[i]
                .machine
                .as_ref()
                .map(|m| m.report(&self.nodes[i].spec.digest()));
            let Some(rep) = rep else { return Ok(()) };
            let any_elim = rep.terminals.iter().any(|t| t.outcome == "eliminated");
            if !any_elim {
                self.nodes[i].second = Some(
                    json!({"written": false, "why": "no terminal eliminated: the first statement stands alone"}),
                );
            } else {
                let dir = self.abs(&format!("{}/termmin", node_dir(&n)));
                let order =
                    serde_json::to_value(&self.nodes[i].cfg.solve.order).unwrap_or(Value::Null);
                let mut inner: Vec<(u64, PathBuf)> = st
                    .inputs
                    .iter()
                    .filter(|(id, r)| {
                        r.role == super::ids::ROLE_ANSWERS
                            && id
                                .strip_prefix("closeloop:")
                                .and_then(|x| x.split_once(":answers:"))
                                .and_then(|(item, _)| st.items.get(item))
                                .is_some_and(|it| it.node == node)
                    })
                    .map(|(_, r)| (r.registered_seq, PathBuf::from(&r.path)))
                    .collect();
                inner.sort();
                let inner: Vec<PathBuf> = inner.into_iter().map(|x| x.1).collect();
                let sp = extended_spec(
                    &self.nodes[i].spec_text,
                    &self.nodes[i].cfg.base(),
                    &rep,
                    &self.nodes[i].answers,
                    &dir,
                    self.nodes[i].group_spec.as_ref(),
                    &order,
                    &inner,
                )?;
                let threads = self.nodes[i].cfg.check_threads;
                // lane closeloop3: recipe-served keys of the second statement are per-key only with their bound proof
                let proved = self.inner.proved_relations(st, &node);
                let h = std::thread::Builder::new()
                    .name(format!("termmin-second-{n}"))
                    .spawn(move || -> Result<Loaded, String> {
                        let isp = TableSetSpec::read(&sp).map_err(|e| format!("extended spec: {e}"))?;
                        let base = sp.parent().map(Path::to_path_buf).unwrap_or_default();
                        let set = rustred_campaign_core::import::load(&isp, &base).map_err(|e| format!("extended load: {e}"))?;
                        let mut ev = Evidence::from_spec(&set, &base).map_err(|e| format!("extended evidence: {e}"))?;
                        ev.recipe_proofs = Some(proved);
                        let res = rustred_campaign_core::import::check(
                            &set,
                            &ev,
                            &CheckOptions { threads: threads.max(1), tier: None, max_rounds: None },
                        )
                        .map_err(|e| format!("extended check: {e}"))?;
                        let sdir = base.join("statement");
                        let dig = rustred_campaign_core::import::write_statement(&set, &ev, &res, &sdir, None)
                            .map_err(|e| format!("second statement: {e}"))?;
                        let v = rustred_campaign_core::termmin::extend::view(&set, &res);
                        // lane closeloop4 (audit-closeloop m6): `closed` only under the closing loop's own criteria
                        // (Tier C, strict dispatch, not diagnostic, every row certified at the required level with its
                        // facts decided), never from the blocking list alone
                        let (strict, _) = super::pass::strictness(&set);
                        let diagnostic = res.summary.diagnostic
                            || res.summary.verdict == rustred_campaign_core::closure::Verdict::Diagnostic;
                        let basis_ok = res.tier == rustred_campaign_core::closure::Tier::C && strict && !diagnostic;
                        let all_cert = res.streamed.is_none()
                            && res.rows.iter().all(|r| super::pass::row_certified(r, res.required, basis_ok));
                        let closed2 = res.blocking().is_empty() && v.open.is_empty() && basis_ok && all_cert;
                        Ok(Loaded::Second(Box::new(json!({"written": true, "spec": sp.display().to_string(),
                            "dir": sdir.display().to_string(), "files": dig.files,
                            "blocking": res.blocking(), "closed": closed2, "tier": format!("{:?}", res.tier),
                            "strict": strict,
                            "open": v.open.len(), "economy": v.economy,
                            "terminals": v.terminals.iter().map(|k| key_text(k)).collect::<Vec<_>>()}))))
                    })
                    .map_err(|e| e.to_string())?;
                self.nodes[i].loading = Some(h);
                return Ok(());
            }
        }
        let second = self.nodes[i].second.clone().unwrap_or(Value::Null);
        let mut rep = self.nodes[i]
            .machine
            .as_ref()
            .map(|m| {
                serde_json::to_value(m.report(&self.nodes[i].spec.digest())).unwrap_or(Value::Null)
            })
            .unwrap_or(json!({"schema": rustred_campaign_core::termmin::spec::REPORT_SCHEMA}));
        rep["node"] = json!(node);
        rep["second_statement"] = second;
        rep["failed"] = json!(self.nodes[i].failed);
        rep["proof_requests"] = json!(
            "recipe relations of phase E: per-key level from the verb's exact-weight proof (import 5.3); rr_proof per-key proofs through lane closeloop2's proof jobs once merged"
        );
        let r = record(
            format!("{}/termmin/report.json", node_dir(&n)),
            json_bytes(&rep),
        );
        let f = r.file_ref();
        // the release: a fresh closure result binding the node's current rule set when phase E accepted an item
        let accepted = st.items.values().any(|it| {
            it.node == node && it.kind == KIND_TERMMIN && matches!(it.phase, ItemPhase::Accepted(_))
        });
        let withheld: Vec<Event> = Self::read_input(st, &input_withheld(&n))
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .and_then(|v| serde_json::from_value(v["events"].clone()).ok())
            .unwrap_or_default();
        rt.events.push(Event::InputRegistered {
            input: input_tm_result(&n),
            role: ROLE_TM_RESULT.into(),
            path: self.abs(&r.path).display().to_string(),
            bytes: f.bytes,
            blake3: f.blake3,
        });
        rt.accepts.push(r);
        if accepted && !withheld.is_empty() {
            let nr = st.nodes.get(&node).ok_or("node not registered")?;
            if let (Some(c), Some(d)) = (&nr.state.closure, &nr.state.dispatch_order) {
                rt.events.push(Event::Node {
                    node: node.clone(),
                    ev: NodeEvent::ClosureResult {
                        round: nr.state.closure_rounds + 1,
                        defect_keys: 0,
                        closure_keys: c.closure_keys,
                        rules_used: None,
                        rules_total: None,
                        keyset: None,
                        rule_set_digest: nr.rule_set.digest(),
                        dispatch_order_digest: d.clone(),
                        envelope: crate::ledger::EnvelopeKind::Measured,
                        envelope_tables: vec![],
                    },
                });
            }
        }
        rt.events.push(Event::Note {
            text: format!(
                "closeloop {node}: phase E ended: terminals {} -> {}; report {}",
                rep["economy_before"]["terminals"], rep["economy_after"]["terminals"], f.path
            ),
        });
        rt.events.extend(withheld);
        Ok(())
    }
}

/// The terminal keys of a statement's `terminals.json` (a list of `I(..)` texts / key arrays, or an object with a list).
fn terminals_of(v: &Value) -> Vec<Value> {
    match v {
        Value::Array(a) => a.clone(),
        Value::Object(o) => o
            .get("terminals")
            .or_else(|| o.get("declared"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        _ => vec![],
    }
}

impl WorkSource for TermminSource {
    fn bootstrap(&mut self, st: &CampaignState, ctx: &SourceContext) -> Result<Vec<Event>, String> {
        self.inner.bootstrap(st, ctx)
    }

    fn ready(&mut self, st: &CampaignState, ctx: &SourceContext, limit: usize) -> Vec<JobRequest> {
        let mut out = self.inner.ready(st, ctx, limit);
        for i in 0..self.nodes.len() {
            if self.phase(st, i) != Some(false) || self.nodes[i].machine.is_none() {
                continue;
            }
            let node = self.nodes[i].cfg.node.clone();
            let max_att = self.nodes[i].cfg.budgets.max_job_attempts;
            let items: Vec<(String, ItemPhase, Vec<String>)> = st
                .items
                .iter()
                .filter(|(_, it)| it.node == node && it.kind == KIND_TERMMIN)
                .map(|(k, it)| (k.clone(), it.phase.clone(), it.jobs.clone()))
                .collect();
            for (item, ph, jobs) in items {
                let failed = jobs
                    .iter()
                    .filter(|x| {
                        matches!(
                            st.jobs.get(*x).map(|r| r.state),
                            Some(AttemptState::Failed(_) | AttemptState::BudgetStopped(_))
                        )
                    })
                    .count() as u32;
                let job = match ph {
                    ItemPhase::Open => Some(format!("{item}-t{}", jobs.len())),
                    ItemPhase::InProgress => {
                        match jobs.last().and_then(|l| st.jobs.get(l)).map(|r| r.state) {
                            Some(AttemptState::Queued) => jobs.last().cloned(),
                            Some(AttemptState::Failed(_) | AttemptState::BudgetStopped(_))
                                if failed < max_att =>
                            {
                                Some(format!("{item}-t{}", jobs.len()))
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                };
                if let Some(job) = job {
                    match self.request(st, i, &item, job) {
                        Ok(r) => out.push(r),
                        Err(e) => eprintln!("termmin: no job for {item}: {e}"),
                    }
                }
            }
        }
        out.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.job.cmp(&b.job)));
        out.truncate(limit);
        out
    }

    fn route(
        &mut self,
        st: &CampaignState,
        ctx: &SourceContext,
        done: &Settled,
    ) -> Result<Routing, String> {
        let Some(i) = self.node_of_item(st, &done.item) else {
            return self.inner.route(st, ctx, done);
        };
        let it = st.items.get(&done.item).ok_or("unknown item")?;
        let meta = self.meta(st, &done.item)?;
        let b: BatchFile =
            serde_json::from_value(meta["batch"].clone()).map_err(|e| e.to_string())?;
        let req = meta["request"]
            .as_str()
            .ok_or("batch without request")?
            .to_string();
        let terminal: Vec<i32> =
            serde_json::from_value(meta["terminal"].clone()).map_err(|e| e.to_string())?;
        let step = meta["step"].as_str().unwrap_or("").to_string();
        let rid = meta["recipe_id"].as_str().unwrap_or("").to_string();
        let c = &done.classification;
        let decided_row = matches!((c.row, c.sub.as_str()), (1, "") | (28, "") | (29, ""));
        let mut rt = Routing::default();
        let run_dir = st
            .jobs
            .get(&done.job)
            .and_then(|j| j.run_dir.clone())
            .unwrap_or_default();
        let cost = cost_of(Path::new(&run_dir), &done.job);
        let answer: SolveAnswer = if !(decided_row
            && !done.canonical_lines.is_empty()
            && done.table.is_some())
        {
            let failed_before = it
                .jobs
                .iter()
                .filter(|j| *j != &done.job)
                .filter(|j| {
                    matches!(
                        st.jobs.get(*j).map(|r| r.state),
                        Some(AttemptState::Failed(_) | AttemptState::BudgetStopped(_))
                    )
                })
                .count() as u32;
            if failed_before + 1 < self.nodes[i].cfg.budgets.max_job_attempts {
                return Ok(rt); // rerun (ready)
            }
            SolveAnswer::Failed {
                class: "refused".into(),
                verdict: None,
                failure: json!({"class": "refused", "row": c.row, "sub": c.sub, "why": c.why}),
                line: None,
                cost,
            }
        } else {
            let table = done.table.clone().expect("checked");
            let bits = self.bits(&self.nodes[i])?;
            let slot = if step == "closing" {
                self.nodes[i]
                    .ext
                    .as_ref()
                    .and_then(|e| {
                        e.set
                            .recipes
                            .iter()
                            .find(|r| r.id == b.source)
                            .map(|r| r.slot.clone())
                    })
                    .unwrap_or(DispatchSlot::Last)
            } else {
                DispatchSlot::Last
            };
            let ext = self.nodes[i]
                .ext
                .as_mut()
                .ok_or("termmin: settled before the set was loaded")?;
            let undeclare = (step != "closing").then_some(terminal.as_slice());
            let d = match accept_point(
                &mut ext.set,
                undeclare,
                &b,
                &slot,
                &done.canonical_lines,
                &bits,
                (&table.path, &table.blake3),
                &run_dir,
                None,
            ) {
                Ok(d) => d,
                // a chain that cannot judge the attempt is a REFUSAL recorded as the answer (never a hang)
                Err(e) => Decided {
                    refused: Some(e),
                    ..Decided::default()
                },
            };
            if !d.certified.is_empty() {
                let canon = done.canonical_blake3.clone().unwrap_or_default();
                let keys = record(
                    format!(
                        "{}/termmin/answers/{}.{}.keys.json",
                        node_dir(&self.nodes[i].n),
                        done.item,
                        done.job
                    ),
                    json_bytes(
                        &json!({"schema": "rustred.campaign.closeloop-keys.v1", "keys": d.certified}),
                    ),
                );
                let kref = keys.file_ref();
                let lines_rec = record(
                    format!(
                        "{}/termmin/answers/{}.{}.jsonl",
                        node_dir(&self.nodes[i].n),
                        done.item,
                        done.job
                    ),
                    {
                        let mut x = d.lines.join("\n").into_bytes();
                        x.push(b'\n');
                        x
                    },
                );
                let lref = lines_rec.file_ref();
                rt.events.push(Event::Item {
                    item: done.item.clone(),
                    ev: ItemEvent::Produce {
                        job: done.job.clone(),
                        attempt: done.attempt,
                        table: table.clone(),
                    },
                });
                rt.events.push(Event::Item {
                    item: done.item.clone(),
                    ev: ItemEvent::Accept {
                        rule: format!("{}:{}", done.item, &canon[..16.min(canon.len())]),
                        level: EvidenceLevel::IdentityProved,
                        record: lref.clone(),
                        source: AcceptSource::Job {
                            job: done.job.clone(),
                            attempt: done.attempt,
                            table_blake3: table.blake3.clone(),
                        },
                        elided: false,
                        evidence: Some(LevelEvidence::IdentityProved {
                            proofs: lref,
                            keys: kref,
                        }),
                        round: Some(0),
                    },
                });
                rt.accepts.push(keys);
                rt.accepts.push(lines_rec);
            }
            let ans = answer_of(&d, &b.keys[0], &step, &rid, &b.recipe_path, cost);
            // lane closeloop3 (closeloop.md 16.7): the certified relation as point records for the proof pass (its
            // recipe id = the one the extended spec gives it: the termmin point recipe, or the node's own recipe for a
            // closing solve)
            if let SolveAnswer::Relation { candidate, .. } = &ans
                && !d.records.is_empty()
            {
                let src = if step == "closing" {
                    b.source.clone()
                } else {
                    rustred_campaign_core::termmin::extend::candidate_source(&terminal, candidate)
                };
                let node = self.nodes[i].cfg.node.clone();
                if let Some(rel) = self
                    .inner
                    .phase_e_relation_table(&node, &b, &src, &done.item, &done.job, &d.records)?
                {
                    let f = rel.file_ref();
                    rt.events.push(Event::InputRegistered {
                        input: super::ids::input_relations(&done.item, &done.job),
                        role: super::ids::ROLE_RELATIONS.into(),
                        path: self.abs(&rel.path).display().to_string(),
                        bytes: f.bytes,
                        blake3: f.blake3,
                    });
                    rt.accepts.push(rel);
                }
            }
            ans
        };
        if !matches!(answer, SolveAnswer::Relation { .. }) {
            rt.events.push(Event::Item {
                item: done.item.clone(),
                ev: ItemEvent::Escalate {
                    to: Rung(1),
                    reason: "termmin: no certified relation at the key".into(),
                },
            });
        }
        let a = record(
            format!(
                "{}/termmin/answers/{}.{}.answer.json",
                node_dir(&self.nodes[i].n),
                done.item,
                done.job
            ),
            json_bytes(&json!({"schema": TM_ANSWER_SCHEMA, "request": req, "answer": answer})),
        );
        let f = a.file_ref();
        rt.events.push(Event::InputRegistered {
            input: input_tm_answers(&done.item, &done.job),
            role: ROLE_TM_ANSWERS.into(),
            path: self.abs(&a.path).display().to_string(),
            bytes: f.bytes,
            blake3: f.blake3,
        });
        rt.accepts.push(a);
        Ok(rt)
    }

    fn control(
        &mut self,
        st: &CampaignState,
        verb: &str,
        target: &str,
    ) -> Result<Vec<Event>, String> {
        if self.node_of_item(st, target).is_some() {
            return match verb {
                "retry" | "skip" => Ok(vec![]),
                v => Err(format!("verb {v} not supported by the termmin phase")),
            };
        }
        self.inner.control(st, verb, target)
    }

    fn poll(&mut self, st: &CampaignState, ctx: &SourceContext) -> Result<Routing, String> {
        let inner = self.inner.poll(st, ctx)?;
        let mut rt = self.filter_inner(inner)?;
        // phase E steps run on the state BEFORE the inner's new events (next tick sees them)
        for i in 0..self.nodes.len() {
            if self.phase(st, i) != Some(false) {
                continue;
            }
            // a finished thread
            if self.nodes[i]
                .loading
                .as_ref()
                .is_some_and(|h| h.is_finished())
            {
                let h = self.nodes[i].loading.take().expect("loading");
                match h
                    .join()
                    .unwrap_or_else(|_| Err("termmin thread panicked".into()))
                {
                    Ok(Loaded::Ext(ext, m, gs)) => {
                        self.nodes[i].ext = Some(*ext);
                        self.nodes[i].machine = Some(*m);
                        self.nodes[i].group_spec = gs;
                    }
                    Ok(Loaded::Second(v)) => self.nodes[i].second = Some(*v),
                    Err(e) => self.nodes[i].failed = Some(e),
                }
            }
            if self.nodes[i].loading.is_some() {
                continue;
            }
            if self.nodes[i].failed.is_none() && self.nodes[i].ext.is_none() {
                if let Err(e) = self.start_load(st, i) {
                    self.nodes[i].failed = Some(e);
                }
                continue;
            }
            if self.nodes[i].failed.is_none()
                && let Err(e) = self.step_node(st, i, &mut rt)
            {
                self.nodes[i].failed = Some(e);
            }
            // finished when the machine is done (or failed) and no termmin item is in flight
            let node = self.nodes[i].cfg.node.clone();
            // lane closeloop3 (closeloop.md 16.7): phase-E relations get span proof items of the inner's proof pass;
            // the second statement waits until every proof item of the node is final
            let pr = self.inner.phase_e_proof_items(st, &node, KIND_TERMMIN)?;
            rt.events.extend(pr.events);
            rt.accepts.extend(pr.accepts);
            let busy = self.inner.proofs_pending(st, &node)
                || st.items.values().any(|it| {
                    it.node == node
                        && it.kind == KIND_TERMMIN
                        && matches!(
                            it.phase,
                            ItemPhase::Open | ItemPhase::InProgress | ItemPhase::Produced
                        )
                });
            let new_items = rt
                .events
                .iter()
                .any(|e| matches!(e, Event::ItemCreated { node: x, .. } if *x == node));
            let done = self.nodes[i].failed.is_some()
                || self.nodes[i].machine.as_ref().is_some_and(|m| m.done());
            if done && !busy && !new_items {
                if let Some(f) = &self.nodes[i].failed {
                    rt.events.push(Event::Note {
                        text: format!("closeloop {node}: phase E stopped: {f}"),
                    });
                    self.nodes[i].second = Some(json!({"written": false, "why": f}));
                }
                self.finish_node(st, i, &mut rt)?;
            }
        }
        Ok(rt)
    }

    /// lane closeloop3 (procfix5): the closing loop's outcome (phase E never changes a node's verdict).
    fn outcome(&self, st: &CampaignState) -> crate::supervisor::worksource::CampaignOutcome {
        self.inner.outcome(st)
    }

    /// lane t266run / closeloop4 (closeloop.md 17.3, 19): the closing loop's operator decisions pass through phase E.
    fn operator_decision(&self, st: &CampaignState) -> Option<String> {
        self.inner.operator_decision(st)
    }

    fn resolve_operator_decision(
        &mut self,
        st: &CampaignState,
        req: &crate::supervisor::ControlRequest,
    ) -> Result<crate::supervisor::worksource::Routing, String> {
        self.inner.resolve_operator_decision(st, req)
    }

    fn blocked_on_operator(&self, st: &CampaignState) -> Option<String> {
        // lane t266run (L17): forwarded
        self.inner.blocked_on_operator(st)
    }

    fn finished(&self, st: &CampaignState) -> bool {
        self.inner.finished(st)
            && (0..self.nodes.len())
                .all(|i| self.phase(st, i) != Some(false) && self.nodes[i].loading.is_none())
            && self.nodes.iter().all(|p| {
                // a node entering phase E in this very tick has its withheld input registered by now
                !st.inputs.contains_key(&input_withheld(&p.n))
                    || st.inputs.contains_key(&input_tm_result(&p.n))
            })
    }

    fn describe(&self) -> String {
        format!(
            "{} + terminal minimisation (phase E)",
            self.inner.describe()
        )
    }
}
