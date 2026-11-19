//! Lane wireflow (f), interfaces/repairflow.md 21: SEED MINIMISATION of the closing face recipes at P5 (policy
//! `seedmin`; default off). After the closing loop's final result and before the repair flow's report, every
//! EXPLICIT-OFFSET face recipe (the mixed-shell family: centres = the seed offsets, shell S0) that certified at least
//! `min_keys` keys is minimised JOINTLY over exactly those keys (`seedmin::minimise_joint`: row order fixed, drops judged
//! by the policy's objective, pole order first: a drop that worsens a key's pole order is refused; every-key final
//! verification), solved by the supervisor-side child solver (`closeloop::seedmin::ChildTrialSolver`: the solve-recipe
//! contract through the process layer, journaled under `repairflow/seedmin/<recipe>/work`: a restart answers from the
//! journal). One piece holding every key -> the minimised recipe (same domain, centres = the minimised list, id
//! `<id>/min`) is SHIPPED in the export; otherwise the full list stays shipped and the pieces are recorded. The record
//! compares minimised and full per recipe. Never changes a closure, a statement or a verdict.
//!
//! Ledger: input `repairflow:<n>:seedmin` (role repairflow_seedmin) = the record (JSON) naming the minimised recipe
//! files (content-addressed records under `repairflow/seedmin/`). Generic (L8): no node, count or path is known here.

use super::config::NodeCfg;
use super::ids::node_dir;
use crate::adapters::job::{Invocation, Program};
use crate::closeloop::seedmin::{ChildConfig, ChildTrialSolver, FakeAlt, FakeOracle};
use crate::ledger::Event;
use crate::resources::{Sampler, SamplerConfig};
use crate::state::CampaignState;
use crate::supervisor::worksource::{AcceptedRecord, Routing};
use rustred_campaign_core::repairflow::policy::SeedMinStep;
use rustred_campaign_core::seedmin::{
    EngineCaps, LocalValuationJudge, StubSystemPoles, minimise_joint,
};
use rustred_campaign_core::worklist::recipe::{Recipe, RecipeGenerator};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

pub const ROLE_SEEDMIN: &str = "repairflow_seedmin";
pub const SEEDMIN_SCHEMA: &str = "rustred.campaign.repairflow-seedmin.v1";

pub fn input_seedmin(n: &str) -> String {
    format!("repairflow:{n}:seedmin")
}

/// Test stand: the fake oracle of the solve children (policy `fake.seedmin_oracle`: [[key, [alts]], ..]).
pub type FakeSeedmin = Option<(Vec<(Vec<i32>, Vec<FakeAlt>)>, u64)>;

struct MinOut {
    record: Vec<u8>,
    files: Vec<(String, Vec<u8>)>,
}

/// The per-node runtime state (a thread; everything else is in the ledger and the journals).
#[derive(Default)]
pub struct MinStage {
    pass: Option<JoinHandle<Result<MinOut, String>>>,
}

/// What the step needs from the repair flow.
pub struct MinCtx<'a> {
    pub cdir: &'a Path,
    pub n: &'a str,
    pub cfg: &'a NodeCfg,
    pub step: &'a SeedMinStep,
    pub programs: &'a BTreeMap<String, PathBuf>,
    pub licence_env: Option<String>,
    pub fake: FakeSeedmin,
}

fn rec(path: String, bytes: Vec<u8>) -> AcceptedRecord {
    AcceptedRecord { path, bytes }
}

impl MinStage {
    /// Ok(true) = done (or off): the report may be written. Ok(false) = running.
    pub fn poll(
        &mut self,
        st: &CampaignState,
        c: &MinCtx,
        rt: &mut Routing,
    ) -> Result<bool, String> {
        if !c.step.enabled || st.inputs.contains_key(&input_seedmin(c.n)) {
            return Ok(true);
        }
        if self.pass.is_none() {
            let rf = c.cdir.join(format!("{}/repairflow", node_dir(c.n)));
            let stmt = c
                .cdir
                .join(format!("{}/final/statement/statement.jsonl", node_dir(c.n)));
            let prog_name = c
                .fake
                .as_ref()
                .map(|_| "rr_fake_child".to_string())
                .unwrap_or_else(|| c.cfg.solve.program.clone());
            let path = c
                .programs
                .get(&prog_name)
                .ok_or_else(|| format!("seedmin: program {prog_name} not in the spec"))?
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
                rf,
                stmt,
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
                .name(format!("repairflow-seedmin-{}", c.n))
                .spawn(move || run_min(args))
                .map_err(|e| format!("seedmin thread: {e}"))?;
            self.pass = Some(h);
        }
        if !self.pass.as_ref().is_some_and(|h| h.is_finished()) {
            return Ok(false);
        }
        let h = self.pass.take().expect("pass");
        let out = h
            .join()
            .map_err(|_| "seedmin thread panicked".to_string())??;
        for (p, b) in out.files {
            rt.accepts.push(rec(p, b));
        }
        let r = rec(
            format!("{}/repairflow/seedmin/seedmin.json", node_dir(c.n)),
            out.record,
        );
        let f = r.file_ref();
        rt.events.push(Event::InputRegistered {
            input: input_seedmin(c.n),
            role: ROLE_SEEDMIN.into(),
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
    step: SeedMinStep,
    licence_env: Option<String>,
    fake: FakeSeedmin,
    n: String,
}

fn blake3_hex(b: &[u8]) -> String {
    blake3::hash(b).to_hex().to_string()
}

/// The thread: recipes of the composed spec, their certified keys (statement), the joint minimisation of each.
fn run_min(a: Args) -> Result<MinOut, String> {
    // recipe sources of the composed spec: id -> file
    let spec: Value = serde_json::from_slice(
        &std::fs::read(a.rf.join("import-spec.composed.json"))
            .map_err(|e| format!("composed spec: {e}"))?,
    )
    .map_err(|e| format!("composed spec: {e}"))?;
    let mut recipes: BTreeMap<String, PathBuf> = BTreeMap::new();
    for s in spec["sources"].as_array().into_iter().flatten() {
        if s["kind"] == "recipe"
            && let (Some(id), Some(p)) = (s["id"].as_str(), s["path"].as_str())
        {
            recipes.insert(id.to_string(), PathBuf::from(p));
        }
    }
    // certified keys per recipe source (the final statement)
    let mut keys_of: BTreeMap<String, Vec<Vec<i32>>> = BTreeMap::new();
    let text = std::fs::read_to_string(&a.stmt).map_err(|e| format!("statement: {e}"))?;
    for l in text.lines().filter(|l| !l.trim().is_empty()) {
        let Ok(v) = serde_json::from_str::<Value>(l) else {
            continue;
        };
        if v["status"] != "served" {
            continue;
        }
        let Some(src) = v["server"]["source"].as_str() else {
            continue;
        };
        if !recipes.contains_key(src) {
            continue;
        }
        if let Ok(k) = serde_json::from_value::<Vec<i32>>(v["key"].clone()) {
            keys_of.entry(src.to_string()).or_default().push(k);
        }
    }
    let mut order: Vec<(String, Vec<Vec<i32>>)> = keys_of
        .into_iter()
        .filter(|(_, ks)| ks.len() >= a.step.min_keys)
        .collect();
    order.sort_by(|x, y| y.1.len().cmp(&x.1.len()).then(x.0.cmp(&y.0)));
    order.truncate(a.step.max_recipes);
    let sampler = Arc::new(
        Sampler::spawn(SamplerConfig {
            interval: Duration::from_millis(500),
            collect_foreign_pinning: false,
            // procfix7 round 7 field (merge): no effect, pinning is not collected
            foreign_pin_max_cpus: usize::MAX,
        })
        .map_err(|e| format!("sampler: {e}"))?,
    );
    let mut entries: Vec<Value> = vec![];
    let mut files: Vec<(String, Vec<u8>)> = vec![];
    for (src, mut keys) in order {
        keys.sort();
        let path = &recipes[&src];
        let rb = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let recipe: Recipe =
            serde_json::from_slice(&rb).map_err(|e| format!("{}: {e}", path.display()))?;
        let explicit = matches!(&recipe.seeds.generator, RecipeGenerator::Shells(s) if s == &vec!["S0".to_string()])
            && !recipe.seeds.centres.is_empty();
        if !explicit {
            entries.push(json!({"source": src, "keys": keys.len(), "skipped": "not an explicit-offset recipe"}));
            continue;
        }
        let list = recipe.seeds.centres.clone();
        let n = recipe.arity;
        let tag = &blake3_hex(src.as_bytes())[..16];
        let lines: Vec<bool> = recipe.chart.chars().map(|c| c == '1').collect();
        let cfg = ChildConfig {
            program: a.program.clone(),
            family: a.family.clone(),
            order: a.order.clone(),
            lines,
            domain: Some(serde_json::to_value(&recipe.domain).map_err(|e| e.to_string())?),
            relation_rule: a.relation_rule.clone(),
            cpus: (0..a.step.jobs.max(1)).collect(),
            pin_cpus: false,
            threads_per_job: 1,
            cap_bytes: a.cap_bytes,
            key_budget_s: a.key_budget_s,
            max_trace_rows: None,
            wall_cap_s: a.step.wall_cap_s,
            work_root: a.rf.join(format!("seedmin/{tag}/work")),
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
            pole_objective: if a.step.policy.objective.iter().any(|m| m == "pole_order") {
                "on".into()
            } else {
                "off".into()
            },
            verbose: false,
            site: Default::default(),
        };
        let mut solver = ChildTrialSolver::new(
            cfg,
            sampler.clone(),
            Box::new(LocalValuationJudge),
            EngineCaps::default(),
        )?;
        let jm = match minimise_joint(
            &keys,
            &vec![0; n],
            &list,
            &a.step.policy,
            &mut solver,
            &mut StubSystemPoles,
        ) {
            Ok(j) => j,
            Err(e) => {
                entries.push(
                    json!({"source": src, "keys": keys.len(), "full_seeds": list.len(),
                    "error": format!("{e:?}")}),
                );
                continue;
            }
        };
        let one = jm.pieces.len() == 1
            && jm.pieces[0].keys.len() == keys.len()
            && jm.unserved.is_empty()
            && jm.undecided.is_empty();
        let mut entry = json!({"source": src, "recipe": path.display().to_string(), "keys": keys.len(),
            "full_seeds": list.len(), "pieces": jm.pieces.iter().map(|p| json!({"keys": p.keys.len(),
                "seeds": p.list.len(), "how": p.how})).collect::<Vec<_>>(),
            "unserved": jm.unserved.len(), "undecided": jm.undecided.len(), "solves": jm.solves,
            "record": jm.record});
        if one {
            let mut m = recipe.clone();
            m.seeds.centres = jm.pieces[0].list.clone();
            m.id = format!("{}/min", recipe.id);
            if let Some(p) = m.provenance.as_mut() {
                p["seedmin"] = json!({"full_seeds": list.len(), "min_seeds": m.seeds.centres.len(),
                    "verified_at_keys": keys.len()});
            }
            let mut b = serde_json::to_vec(&m).map_err(|e| e.to_string())?;
            b.push(b'\n');
            let rel = format!(
                "{}/repairflow/seedmin/{tag}.min.recipe.json",
                node_dir(&a.n)
            );
            entry["shipped"] = json!("minimised");
            entry["min_seeds"] = json!(m.seeds.centres.len());
            entry["minimised_recipe"] = json!({"path": rel, "blake3": blake3_hex(&b)});
            files.push((rel, b));
        } else {
            entry["shipped"] = json!("full");
        }
        entries.push(entry);
    }
    let record =
        json!({"schema": SEEDMIN_SCHEMA, "node": a.n, "policy": a.step, "recipes": entries});
    let mut b = serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?;
    b.push(b'\n');
    Ok(MinOut { record: b, files })
}
