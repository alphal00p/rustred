//! Lane wireflow (e), interfaces/repairflow.md 24, POLES_OBJECTIVE.md 1 / polefix's split: the BUDGETED VARIANT PHASE
//! of the face cores after close-first, at P5 (policy `variants`; default off). Close-first is the closing loop itself:
//! every face key was closed by the first certifying face-recipe step. Here, for each closing EXPLICIT-OFFSET face
//! recipe (`rf:<face>:<step>`) with >= `min_keys` certified keys, the face's other explicit-offset steps of the composed
//! spec (the mixed-shell variants) are solved at exactly those keys by the supervisor-side child solver
//! (`closeloop::seedmin::ChildTrialSolver`: the solve-recipe contract through the process layer, journaled under
//! `repairflow/variants/<face>/work`), the closing step too (same judge), within `max_requests` trials per node. Per key
//! the alternatives are ranked by the pole objective (regular at d0 first, then lower pole order, undecided last, then
//! fewer terms; the closing step wins ties). One variant preferred at EVERY key and solving them all -> SHIPPED as the
//! face's recipe (export `variants/`); otherwise the alternatives are recorded. Never changes a closure or a verdict.
//!
//! Ledger: input `repairflow:<n>:variants` (role repairflow_variants) = the record. Generic (L8).

use super::config::NodeCfg;
use super::ids::node_dir;
use crate::adapters::job::{Invocation, Program};
use crate::closeloop::seedmin::{ChildConfig, ChildTrialSolver, FakeAlt, FakeOracle};
use crate::ledger::Event;
use crate::resources::{Sampler, SamplerConfig};
use crate::state::CampaignState;
use crate::supervisor::worksource::{AcceptedRecord, Routing};
use rustred_campaign_core::repairflow::policy::VariantsStep;
use rustred_campaign_core::seedmin::{
    EngineCaps, LocalValuationJudge, PoleOrder, Trial, TrialAnswer, TrialMode, TrialSolver,
};
use rustred_campaign_core::worklist::recipe::{Recipe, RecipeGenerator};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

pub const ROLE_VARIANTS: &str = "repairflow_variants";
pub const VARIANTS_SCHEMA: &str = "rustred.campaign.repairflow-variants.v1";

pub fn input_variants(n: &str) -> String {
    format!("repairflow:{n}:variants")
}

struct VarOut {
    record: Vec<u8>,
    files: Vec<(String, Vec<u8>)>,
}

/// The per-node runtime state (a thread; everything else is in the ledger and the journals).
#[derive(Default)]
pub struct VarStage {
    pass: Option<JoinHandle<Result<VarOut, String>>>,
}

/// What the step needs from the repair flow.
pub struct VarCtx<'a> {
    pub cdir: &'a Path,
    pub n: &'a str,
    pub cfg: &'a NodeCfg,
    pub step: &'a VariantsStep,
    pub programs: &'a BTreeMap<String, PathBuf>,
    pub licence_env: Option<String>,
    /// Test stand: the seed-minimisation fake oracle (the same per-key alternatives).
    pub fake: Option<(Vec<(Vec<i32>, Vec<FakeAlt>)>, u64)>,
}

fn rec(path: String, bytes: Vec<u8>) -> AcceptedRecord {
    AcceptedRecord { path, bytes }
}

fn blake3_hex(b: &[u8]) -> String {
    blake3::hash(b).to_hex().to_string()
}

/// The ranking of an alternative (lower = better): pole (regular 0, order p, singular 1000, undecided 2000), terms.
pub fn rank(pole: &PoleOrder, terms: u64) -> (u32, u64) {
    let p = match pole {
        PoleOrder::Order { order } => *order,
        PoleOrder::Singular => 1000,
        PoleOrder::Undecided => 2000,
    };
    (p, terms)
}

impl VarStage {
    /// Ok(true) = done (or off). Ok(false) = running / just registered.
    pub fn poll(
        &mut self,
        st: &CampaignState,
        c: &VarCtx,
        rt: &mut Routing,
    ) -> Result<bool, String> {
        if !c.step.enabled || st.inputs.contains_key(&input_variants(c.n)) {
            return Ok(true);
        }
        if self.pass.is_none() {
            let prog_name = c
                .fake
                .as_ref()
                .map(|_| "rr_fake_child".to_string())
                .unwrap_or_else(|| c.cfg.solve.program.clone());
            let path = c
                .programs
                .get(&prog_name)
                .ok_or_else(|| format!("variants: program {prog_name} not in the spec"))?
                .clone();
            let inv = if c.fake.is_some() {
                Invocation::Contract
            } else {
                Invocation::Legacy
            };
            let mut program =
                Program::from_file(&prog_name, &path, inv).map_err(|e| e.to_string())?;
            if c.fake.is_some() {
                program.features_required = vec!["fake".into()];
            }
            let args = Args {
                rf: c.cdir.join(format!("{}/repairflow", node_dir(c.n))),
                stmt: c
                    .cdir
                    .join(format!("{}/final/statement/statement.jsonl", node_dir(c.n))),
                program,
                family: c.cfg.solve.family.clone(),
                order: c.cfg.solve.order_arg(),
                relation_rule: c.cfg.solve.relation_rule.clone(),
                cap_bytes: c.cfg.solve.cap_bytes,
                key_budget_s: c.cfg.solve.key_budget_s,
                step: c.step.clone(),
                licence_env: c.licence_env.clone(),
                fake: c.fake.clone(),
                n: c.n.to_string(),
            };
            let h = std::thread::Builder::new()
                .name(format!("repairflow-variants-{}", c.n))
                .spawn(move || run_var(args))
                .map_err(|e| format!("variants thread: {e}"))?;
            self.pass = Some(h);
        }
        if !self.pass.as_ref().is_some_and(|h| h.is_finished()) {
            return Ok(false);
        }
        let h = self.pass.take().expect("pass");
        let out = h
            .join()
            .map_err(|_| "variants thread panicked".to_string())??;
        for (p, b) in out.files {
            rt.accepts.push(rec(p, b));
        }
        let r = rec(
            format!("{}/repairflow/variants/variants.json", node_dir(c.n)),
            out.record,
        );
        let f = r.file_ref();
        rt.events.push(Event::InputRegistered {
            input: input_variants(c.n),
            role: ROLE_VARIANTS.into(),
            path: c.cdir.join(&r.path).display().to_string(),
            bytes: f.bytes,
            blake3: f.blake3,
        });
        rt.accepts.push(r);
        Ok(false)
    }
}

struct Args {
    rf: PathBuf,
    stmt: PathBuf,
    program: Program,
    family: PathBuf,
    order: String,
    relation_rule: String,
    cap_bytes: u64,
    key_budget_s: Option<u64>,
    step: VariantsStep,
    licence_env: Option<String>,
    fake: Option<(Vec<(Vec<i32>, Vec<FakeAlt>)>, u64)>,
    n: String,
}

fn explicit(r: &Recipe) -> bool {
    matches!(&r.seeds.generator, RecipeGenerator::Shells(s) if s == &vec!["S0".to_string()])
        && !r.seeds.centres.is_empty()
}

/// The thread: closing face recipes (statement), their face's other explicit-offset steps, the trials, the ranking.
fn run_var(a: Args) -> Result<VarOut, String> {
    let spec: Value = serde_json::from_slice(
        &std::fs::read(a.rf.join("import-spec.composed.json"))
            .map_err(|e| format!("composed spec: {e}"))?,
    )
    .map_err(|e| format!("composed spec: {e}"))?;
    // face recipe sources `rf:<face>:<step>` in spec order
    let mut by_face: BTreeMap<String, Vec<(String, String, PathBuf)>> = BTreeMap::new();
    for s in spec["sources"].as_array().into_iter().flatten() {
        if s["kind"] != "recipe" {
            continue;
        }
        let (Some(id), Some(p)) = (s["id"].as_str(), s["path"].as_str()) else {
            continue;
        };
        let parts: Vec<&str> = id.splitn(3, ':').collect();
        if parts.len() == 3 && parts[0] == "rf" {
            by_face.entry(parts[1].to_string()).or_default().push((
                id.to_string(),
                parts[2].to_string(),
                PathBuf::from(p),
            ));
        }
    }
    // certified keys per serving source (the final statement)
    let mut keys_of: BTreeMap<String, Vec<Vec<i32>>> = BTreeMap::new();
    let text = std::fs::read_to_string(&a.stmt).map_err(|e| format!("statement: {e}"))?;
    for l in text.lines().filter(|l| !l.trim().is_empty()) {
        let Ok(v) = serde_json::from_str::<Value>(l) else {
            continue;
        };
        if v["status"] != "served" {
            continue;
        }
        if let (Some(src), Ok(k)) = (
            v["server"]["source"].as_str(),
            serde_json::from_value::<Vec<i32>>(v["key"].clone()),
        ) {
            keys_of.entry(src.to_string()).or_default().push(k);
        }
    }
    let sampler = Arc::new(
        Sampler::spawn(SamplerConfig {
            interval: Duration::from_millis(500),
            collect_foreign_pinning: false,
        })
        .map_err(|e| format!("sampler: {e}"))?,
    );
    let mut left = a.step.max_requests;
    let mut faces_v: Vec<Value> = vec![];
    let mut files: Vec<(String, Vec<u8>)> = vec![];
    for (face, steps) in &by_face {
        for (src, step_name, path) in steps {
            let Some(keys) = keys_of.get(src).filter(|k| k.len() >= a.step.min_keys) else {
                continue;
            };
            let mut keys = keys.clone();
            keys.sort();
            let load = |p: &Path| -> Result<Recipe, String> {
                serde_json::from_slice(
                    &std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?,
                )
                .map_err(|e| format!("{}: {e}", p.display()))
            };
            let closing = load(path)?;
            if !explicit(&closing) {
                faces_v.push(json!({"face": face, "closing": src, "keys": keys.len(),
                    "skipped": "the closing recipe is not explicit-offset"}));
                continue;
            }
            let mut cands: Vec<(String, Recipe)> = vec![(step_name.clone(), closing.clone())];
            for (_, v, p) in steps {
                if v == step_name || (!a.step.steps.is_empty() && !a.step.steps.contains(v)) {
                    continue;
                }
                if let Ok(r) = load(p)
                    && explicit(&r)
                {
                    cands.push((v.clone(), r));
                }
            }
            if cands.len() < 2 {
                continue;
            }
            let fits = (left as usize).min(cands.len());
            let budget_cut = fits < cands.len();
            cands.truncate(fits);
            left = left.saturating_sub(fits as u64);
            if cands.len() < 2 {
                faces_v.push(json!({"face": face, "closing": src, "keys": keys.len(),
                    "skipped": "variant budget (max_requests) spent"}));
                continue;
            }
            let tag = &blake3_hex(src.as_bytes())[..16];
            let n = closing.arity;
            let cfg = ChildConfig {
                program: a.program.clone(),
                family: a.family.clone(),
                order: a.order.clone(),
                lines: closing.chart.chars().map(|c| c == '1').collect(),
                domain: Some(serde_json::to_value(&closing.domain).map_err(|e| e.to_string())?),
                relation_rule: a.relation_rule.clone(),
                cpus: (0..a.step.jobs.max(1)).collect(),
                pin_cpus: false,
                threads_per_job: 1,
                cap_bytes: a.cap_bytes,
                key_budget_s: a.key_budget_s,
                max_trace_rows: None,
                wall_cap_s: a.step.wall_cap_s,
                work_root: a.rf.join(format!("variants/{tag}/work")),
                licence_env: a.licence_env.clone(),
                keep_run_dirs: false,
                d0: "4".into(),
                records_out: None,
                records_cap_bytes: 0,
                fake: a.fake.as_ref().map(|(alts, ms)| FakeOracle {
                    alts: alts.iter().cloned().collect(),
                    ms: *ms,
                }),
                poles: None,
                pole_objective: "on".into(),
                verbose: false,
                site: Default::default(),
            };
            let mut solver = ChildTrialSolver::new(
                cfg,
                sampler.clone(),
                Box::new(LocalValuationJudge),
                EngineCaps::default(),
            )?;
            let trials: Vec<Trial> = cands
                .iter()
                .map(|(v, r)| Trial {
                    label: v.clone(),
                    centre: vec![0; n],
                    list: r.seeds.centres.clone(),
                    keys: keys.clone(),
                    mode: TrialMode::Solve,
                    engine_minimise: None,
                })
                .collect();
            let answers = match solver.solve(&trials) {
                Ok(x) => x,
                Err(e) => {
                    faces_v.push(
                        json!({"face": face, "closing": src, "keys": keys.len(), "error": e}),
                    );
                    continue;
                }
            };
            // per key: the best alternative (closing wins ties: it is first and the comparison is strict)
            let mut per_key: Vec<Value> = vec![];
            let mut chosen_of: Vec<usize> = vec![];
            let mut solved_all = vec![true; cands.len()];
            for (ki, k) in keys.iter().enumerate() {
                let mut best: Option<(usize, (u32, u64))> = None;
                let mut alts = vec![];
                for (ti, ans) in answers.iter().enumerate() {
                    match ans.get(ki) {
                        Some(TrialAnswer::Relation(f)) => {
                            let r = rank(&f.pole, f.rhs_count);
                            alts.push(json!({"step": cands[ti].0, "terms": f.rhs_count,
                                "pole": serde_json::to_value(&f.pole).unwrap_or(Value::Null)}));
                            if best.is_none_or(|(_, b)| r < b) {
                                best = Some((ti, r));
                            }
                        }
                        other => {
                            solved_all[ti] = false;
                            alts.push(json!({"step": cands[ti].0,
                                "answer": other.map(TrialAnswer::word)}));
                        }
                    }
                }
                let c = best.map(|b| b.0).unwrap_or(0);
                chosen_of.push(c);
                per_key.push(json!({"key": k, "chosen": cands[c].0, "alternatives": alts}));
            }
            let first = chosen_of.first().copied().unwrap_or(0);
            let uniform = chosen_of.iter().all(|&c| c == first);
            let mut entry = json!({"face": face, "closing": src, "keys": keys.len(),
                "tried": cands.iter().map(|c| c.0.clone()).collect::<Vec<_>>(), "budget_cut": budget_cut,
                "improved_keys": chosen_of.iter().filter(|&&c| c != 0).count(), "per_key": per_key});
            if uniform && first != 0 && solved_all[first] {
                let (v, r) = &cands[first];
                let mut b = serde_json::to_vec(r).map_err(|e| e.to_string())?;
                b.push(b'\n');
                let rel = format!(
                    "{}/repairflow/variants/{tag}.variant.recipe.json",
                    node_dir(&a.n)
                );
                entry["shipped"] =
                    json!({"step": v, "recipe_id": r.id, "path": rel, "blake3": blake3_hex(&b)});
                files.push((rel, b));
            } else {
                entry["shipped"] = Value::Null;
            }
            faces_v.push(entry);
        }
    }
    let record = json!({"schema": VARIANTS_SCHEMA, "node": a.n, "policy": a.step,
        "requests_used": a.step.max_requests - left, "faces": faces_v});
    let mut b = serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?;
    b.push(b'\n');
    Ok(VarOut { record: b, files })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ranking_puts_regular_first_then_lower_poles_then_fewer_terms() {
        let reg = rank(&PoleOrder::Order { order: 0 }, 9);
        let p1 = rank(&PoleOrder::Order { order: 1 }, 2);
        let sing = rank(&PoleOrder::Singular, 1);
        let und = rank(&PoleOrder::Undecided, 1);
        assert!(reg < p1 && p1 < sing && sing < und);
        assert!(rank(&PoleOrder::Undecided, 1) < rank(&PoleOrder::Undecided, 2));
    }
}
