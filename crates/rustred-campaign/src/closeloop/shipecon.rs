//! The SHIP ECONOMY driver (lane shipecon; interfaces/shipecon.md; core `economy::ship`): after a node is closed, per
//! table rule of its material: bytes vs certified keys served, THE GUARD (the bounded recipe solved and certified at
//! every certified key the rule serves: engine `solve-recipe` jobs through the solve-recipe adapter and the process
//! layer's launcher (`adapters::run_attempt`: own session, PDEATHSIG, verified kill, memory cap), each settled attempt
//! through the closeloop's accept chain against the SHIPPED dispatch), the decision per rule, then the CLOSING CHECK of
//! the shipped spec (the import emulates the engine's order: table first match, then recipe records / entries, then
//! the typed refusal), with rules reverted to the table where the check fails behind them. Report mode (default)
//! writes the proposal and the records; apply mode writes the shipped artifact.
//!
//! ITERABLE: the records of an earlier run (`previous`) are read: a rule with the same inputs and decision settings is
//! not measured or decided again; with other settings it is re-decided from its recorded trials and only new trials
//! run. Run it inside a private pid namespace (L10). Generic (L8): node, paths, order, bounds are configuration.

use super::accept::{BATCH_SCHEMA, BatchFile};
use super::termmin::{AcceptBits, accept_point, cost_of};
use rustred_campaign_core::termmin::machine::Cost;
use crate::adapters::job::{InputFile, JobAdapter, Program, RunOptions, run_attempt};
use crate::adapters::solve_recipe::{
    BatchPolicy, RecipeInput, SolveRecipeAdapter, SolveRecipeConfig, SolveRequest as AdReq,
};
use crate::launch::RealLauncher;
use crate::resources::{Sampler, SamplerConfig};
use rustred_campaign_contract::layout::RunLayout;
use rustred_campaign_contract::seal::verify_stream_handle;
use rustred_campaign_core::closure::relations::DispatchSlot;
use rustred_campaign_core::economy::ship::{
    self as ship, Candidacy, Census, CensusBuilder, ClosingCheck, ClosureView, ECONOMY_RECORD_SCHEMA,
    EconomyRecord, KeyTrial, Outcome, RECORD_KIND, REPORT_SCHEMA, Reuse, RuleDecision, RuleRef,
    ShipMode, ShipPolicy, ShippedRecipe, TableTime,
};
use rustred_campaign_core::import::recipe_steps::{RecipeForm, recipes_for_steps};
use rustred_campaign_core::import::spec::FileRef;
use rustred_campaign_core::import::statement::all_rows;
use rustred_campaign_core::import::{
    CheckOptions, Evidence, ImportedSet, RowStatus, Server, TableSetSpec, check, load,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// What the driver needs (all data).
pub struct ShipRunConfig {
    pub node: String,
    pub import_spec: PathBuf,
    pub import_base: PathBuf,
    /// The node's statement rows (`statement.jsonl` of the tool's closing check); None = the check runs here.
    pub statement_rows: Option<PathBuf>,
    pub policy: ShipPolicy,
    pub program: Program,
    pub family: PathBuf,
    pub order: String,
    /// Concurrent solve jobs (unpinned: the caller's unit bounds the CPUs).
    pub parallel: usize,
    pub threads_per_job: u32,
    pub cap_bytes: u64,
    /// Wall cap of one solve child (terminate after it: typed refusal).
    pub wall_cap_s: u64,
    pub work_root: PathBuf,
    pub out_dir: PathBuf,
    pub check_threads: usize,
    pub licence_env: Option<String>,
    /// `records.jsonl` of an earlier run (iteration).
    pub previous: Option<PathBuf>,
    /// Measured table times (`rr_shipecon_table` output, JSONL `{record_sha256, load_s, apply_s, instantiated_bytes,
    /// tool}`); rules without one get the cost-model estimate.
    pub table_times: Option<PathBuf>,
    pub verbose: bool,
}

fn sha256_hex(b: &[u8]) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

fn now() -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("t={t}")
}

fn write(p: &Path, b: &[u8]) -> Result<(), String> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    std::fs::write(p, b).map_err(|e| format!("{}: {e}", p.display()))
}

fn jsonl<T: serde::Serialize>(items: &[T]) -> Vec<u8> {
    let mut out = Vec::new();
    for i in items {
        out.extend_from_slice(
            serde_json::to_string(i)
                .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
                .as_bytes(),
        );
        out.push(b'\n');
    }
    out
}

/// One solve job: one recipe (one explicit seed set, common slots fixed) at the keys of one rule it is assigned.
#[derive(Clone)]
struct Job {
    id: String,
    rule: usize,
    step: String,
    recipe_id: String,
    recipe_path: PathBuf,
    recipe_sha256: String,
    keys: Vec<Vec<i32>>,
}

struct Done {
    job: Job,
    result: Result<(String, Vec<String>, String, String, Cost), String>,
}

/// The closure view of a check: keys, start keys, terminals, open count, blocking words.
fn view_of(set: &ImportedSet, res: &rustred_campaign_core::import::CheckResult) -> Result<ClosureView, String> {
    let rows = all_rows(set, res)?;
    let mut v = ClosureView {
        blocking: res.blocking(),
        ..Default::default()
    };
    for r in &rows {
        v.keys.push(r.key.clone());
        if r.in_start {
            v.start.push(r.key.clone());
        }
        match r.status {
            RowStatus::Terminal => v.terminals.push(r.key.clone()),
            RowStatus::Open => v.open += 1,
            RowStatus::Served => {}
        }
    }
    v.keys.sort();
    v.start.sort();
    v.terminals.sort();
    Ok(v)
}

/// The rules of the dispatch (identity, bytes), in table / record order.
fn rules_of(set: &ImportedSet) -> Vec<(RuleRef, u64)> {
    let mut out = vec![];
    for t in &set.rules {
        for i in t {
            out.push((
                RuleRef {
                    source: i.source.clone(),
                    table: i.table_name.clone(),
                    label: i.label,
                    ordinal: i.ordinal,
                    record_sha256: i.record_sha256.clone(),
                    table_sha256: set.table_digests.get(&i.table_name).cloned(),
                },
                i.bytes,
            ));
        }
    }
    out
}

/// The census and the "before" view: from the statement rows when given, else from a check of the original spec.
fn census_and_before(
    cfg: &ShipRunConfig,
    set: &ImportedSet,
    say: &dyn Fn(String),
) -> Result<(Census, ClosureView, Value), String> {
    let t0 = Instant::now();
    let mut b = CensusBuilder::new(rules_of(set), cfg.policy.candidates.max_keys_per_rule)?;
    let mut v = ClosureView::default();
    let source;
    match &cfg.statement_rows {
        Some(p) => {
            let f = std::fs::File::open(p).map_err(|e| format!("{}: {e}", p.display()))?;
            for l in std::io::BufReader::with_capacity(1 << 20, f).lines() {
                let l = l.map_err(|e| format!("{}: {e}", p.display()))?;
                if l.trim().is_empty() {
                    continue;
                }
                let r: Value = serde_json::from_str(&l).map_err(|e| format!("{}: {e}", p.display()))?;
                let key: Vec<i32> = serde_json::from_value(r["key"].clone())
                    .map_err(|e| format!("{}: key: {e}", p.display()))?;
                let in_start = r["in_start"] == true;
                let sha = (r["status"] == "served" && r["server"]["kind"] == "rule")
                    .then(|| r["server"]["record_sha256"].as_str())
                    .flatten();
                b.add(&key, in_start, sha);
                match r["status"].as_str() {
                    Some("terminal") => v.terminals.push(key.clone()),
                    Some("open") => v.open += 1,
                    _ => {}
                }
                if in_start {
                    v.start.push(key.clone());
                }
                v.keys.push(key);
            }
            source = json!({"statement_rows": p.display().to_string(),
                "bytes": std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)});
        }
        None => {
            let ev = Evidence::from_spec(set, &cfg.import_base).map_err(|e| format!("evidence: {e}"))?;
            let res = check(
                set,
                &ev,
                &CheckOptions {
                    threads: cfg.check_threads.max(1),
                    tier: None,
                    max_rounds: None,
                },
            )
            .map_err(|e| format!("check: {e}"))?;
            for r in all_rows(set, &res)? {
                let sha = match (&r.status, &r.server) {
                    (RowStatus::Served, Some(Server::Rule { table, record })) => {
                        Some(set.rules[*table][*record].record_sha256.as_str())
                    }
                    _ => None,
                };
                b.add(&r.key, r.in_start, sha);
            }
            v = view_of(set, &res)?;
            source = json!({"check": {"spec_digest": res.spec_digest, "blocking": res.blocking(),
                "seconds": res.seconds}});
        }
    }
    v.keys.sort();
    v.start.sort();
    v.terminals.sort();
    say(format!(
        "census: {} rows in {:.1} s; {} keys, {} start, {} terminals, {} open",
        b.rows,
        t0.elapsed().as_secs_f64(),
        v.keys.len(),
        v.start.len(),
        v.terminals.len(),
        v.open
    ));
    Ok((b.finish(), v, source))
}

/// Previous records by record sha256.
fn previous_records(p: Option<&Path>) -> Result<BTreeMap<String, EconomyRecord>, String> {
    let mut out = BTreeMap::new();
    let Some(p) = p else { return Ok(out) };
    let f = std::fs::File::open(p).map_err(|e| format!("{}: {e}", p.display()))?;
    for l in std::io::BufReader::new(f).lines() {
        let l = l.map_err(|e| e.to_string())?;
        if l.trim().is_empty() {
            continue;
        }
        let r: EconomyRecord =
            serde_json::from_str(&l).map_err(|e| format!("{}: {e}", p.display()))?;
        if r.schema == ECONOMY_RECORD_SCHEMA && r.kind == RECORD_KIND {
            out.insert(r.subject.record_sha256.clone(), r);
        }
    }
    Ok(out)
}

/// Measured table times by record sha256.
fn table_times(p: Option<&Path>) -> Result<BTreeMap<String, TableTime>, String> {
    let mut out = BTreeMap::new();
    let Some(p) = p else { return Ok(out) };
    let f = std::fs::File::open(p).map_err(|e| format!("{}: {e}", p.display()))?;
    for l in std::io::BufReader::new(f).lines() {
        let l = l.map_err(|e| e.to_string())?;
        if l.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(&l).map_err(|e| format!("{}: {e}", p.display()))?;
        let Some(sha) = v["record_sha256"].as_str() else {
            continue;
        };
        if v["error"].is_string() {
            continue;
        }
        let m = |k: &str| -> BTreeMap<String, Value> {
            v[k].as_object()
                .map(|o| o.iter().map(|(a, b)| (a.clone(), b.clone())).collect())
                .unwrap_or_default()
        };
        out.insert(
            sha.to_string(),
            TableTime {
                measured: true,
                load_s: v["load_s"].as_f64().unwrap_or(0.0),
                apply_s: m("apply_s")
                    .into_iter()
                    .filter_map(|(k, x)| x.as_f64().map(|x| (k, x)))
                    .collect(),
                instantiated_bytes: m("instantiated_bytes")
                    .into_iter()
                    .filter_map(|(k, x)| x.as_u64().map(|x| (k, x)))
                    .collect(),
                tool: v["tool"].as_str().map(str::to_string),
            },
        );
    }
    Ok(out)
}

/// The header of the frame table (node order: coordinate priority).
fn coordinate_priority(set: &ImportedSet) -> Option<Vec<usize>> {
    Some(set.cmp.equivalent_descriptor().coordinate_priority)
}

/// The recipes of one (rule, step) at the keys: grouped by explicit seed set (the core's step grammar); each group's
/// recipe has the slots common to its keys fixed (the acceptance chain reads `domain.fixed`) and is assigned its keys.
fn recipes_for(
    p: &ShipPolicy,
    set: &ImportedSet,
    rule: &RuleRef,
    step: &str,
    keys: &[Vec<i32>],
) -> Result<Vec<(String, Value, Vec<Vec<i32>>)>, String> {
    let lines: Vec<bool> = set.frame.mask().to_vec();
    let cp = coordinate_priority(set);
    let (lp, np) = rustred_campaign_core::termmin::priorities(&lines, cp.as_deref());
    let lp = p.recipe.line_priority.clone().unwrap_or(lp);
    let np = p.recipe.numerator_priority.clone().unwrap_or(np);
    let assignments: Vec<(String, Vec<i32>)> = keys.iter().map(|k| (step.to_string(), k.clone())).collect();
    let prefix = format!("shipecon/{}/", rule.record_sha256.get(..16).unwrap_or(&rule.record_sha256));
    let g = recipes_for_steps(&assignments, &lines, &lp, &np, RecipeForm::V1_2, &prefix)
        .map_err(|e| format!("recipe step {step}: {e}"))?;
    let mut out = vec![];
    for r in g {
        let mut doc = r.recipe.clone();
        let n = lines.len();
        let fixed: Vec<Value> = (0..n)
            .map(|i| {
                let v0 = r.keys[0][i];
                if r.keys.iter().all(|k| k[i] == v0) {
                    json!(v0)
                } else {
                    Value::Null
                }
            })
            .collect();
        let rule_word = doc["relation_rule"].as_str().unwrap_or("first_admissible").to_string();
        if let Some(o) = doc.as_object_mut() {
            o.remove("relation_rule");
            o.insert(
                "domain".into(),
                json!({"fixed": fixed, "equations": [], "exceptions": []}),
            );
            o["provenance"]["relation_rule"] = Value::String(rule_word.clone());
            o["provenance"]["shipecon_rule"] = json!(rule.record_sha256);
        }
        // the engine's default relation rule is m1_first; a first_admissible step needs the v1.2 field
        if rule_word != "m1_first"
            && let Some(o) = doc.as_object_mut()
        {
            o.insert("relation_rule".into(), Value::String(rule_word));
        }
        out.push((r.id.clone(), doc, r.keys.clone()));
    }
    Ok(out)
}

/// Run one solve job through the adapter and the launcher (worker thread).
fn launch(cfg: &ShipRunConfig, job: Job, sampler: Arc<Sampler>, tx: mpsc::Sender<Done>) -> Result<(), String> {
    let adcfg = SolveRecipeConfig {
        program: cfg.program.clone(),
        family: InputFile::from_file(&cfg.family).map_err(|e| format!("{e:?}"))?,
        order: cfg.order.clone(),
        batch: BatchPolicy {
            max_keys_per_job: job.keys.len().max(1),
            max_threads: cfg.threads_per_job.max(1),
            min_keys_per_thread: 1,
        },
        max_trace_rows: cfg.policy.recipe.max_trace_rows,
        key_budget_s: cfg.policy.recipe.key_budget_s,
        expect: None,
        determinism_id: None,
        heartbeat_s: 10,
        stop_grace_s: 30,
        package: None,
        live_repair: false,
        licence_env: cfg.licence_env.clone(),
        fake: None,
    };
    let reqs: Vec<AdReq> = job
        .keys
        .iter()
        .map(|k| {
            Ok(AdReq {
                recipe: RecipeInput {
                    id: job.recipe_id.clone(),
                    file: InputFile::from_file(&job.recipe_path).map_err(|e| format!("{e:?}"))?,
                    digest: None,
                    sha256: Some(job.recipe_sha256.clone()),
                },
                key: k.iter().map(|&x| x as i64).collect(),
                tag: Some(job.id.clone()),
                context: None,
            })
        })
        .collect::<Result<_, String>>()?;
    let work = cfg.work_root.join(crate::closeloop::ids::safe(&job.id));
    let opts = RunOptions {
        cpus: vec![],
        memory_cap_bytes: Some(cfg.cap_bytes),
        licence_env: cfg.licence_env.clone(),
        wall_cap: Some(Duration::from_secs(cfg.wall_cap_s)),
        grace: Some(Duration::from_secs(30)),
    };
    std::thread::Builder::new()
        .name(format!("shipecon-{}", job.id))
        .spawn(move || {
            let res = (|| {
                std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
                let ad = SolveRecipeAdapter::new(adcfg);
                let plans = ad.plan(&reqs).map_err(|e| format!("{e:?}"))?;
                let plan = plans.first().ok_or("no plan")?;
                if plans.len() != 1 {
                    return Err(format!("{} plans for one job", plans.len()));
                }
                let mut l = RealLauncher::new(sampler);
                let (out, rep) =
                    run_attempt(&mut l, &ad, &reqs, plan, &work, 1, &opts).map_err(|e| format!("{e:?}"))?;
                let run_dir = rep.prepared.spec.run_dir.clone();
                let cost = cost_of(Path::new(&run_dir), &out.job_id);
                let c = &out.classification;
                let decided = matches!((c.row, c.sub.as_str()), (1, "") | (28, "") | (29, ""));
                if !decided {
                    return Ok((
                        format!(
                            "row {}{} ({:?}): {}; refusal {:?}",
                            c.row,
                            c.sub,
                            c.kind,
                            c.why,
                            out.refusal.as_ref().map(|r| &r.why)
                        ),
                        vec![],
                        String::new(),
                        run_dir,
                        cost,
                    ));
                }
                let layout = RunLayout::new(&run_dir);
                let (_, h) = verify_stream_handle(&layout.root, &RunLayout::out_rel("results.jsonl"), &["end"])
                    .map_err(|e| format!("results stream: {e:?}"))?;
                let lines: Vec<String> = h
                    .canonical_lines()
                    .map_err(|e| format!("canonical lines: {e:?}"))?
                    .into_iter()
                    .map(|b| String::from_utf8_lossy(&b).into_owned())
                    .collect();
                let p = layout.root.join(RunLayout::out_rel("results.jsonl"));
                let b3 = crate::atomic::blake3_file(&p).map(|x| x.1).unwrap_or_default();
                Ok((p.display().to_string(), lines, b3, run_dir, cost))
            })();
            let _ = tx.send(Done { job, result: res });
        })
        .map_err(|e| format!("worker thread: {e}"))?;
    Ok(())
}

/// Per key of a results file: the engine's volatile `seconds` and the record (raw line).
fn raw_records(results: &str) -> BTreeMap<Vec<i32>, Value> {
    let mut out = BTreeMap::new();
    if let Ok(f) = std::fs::File::open(results) {
        for l in std::io::BufReader::new(f).lines().map_while(Result::ok) {
            if let Ok(v) = serde_json::from_str::<Value>(&l)
                && v["type"] == "relation"
                && let Ok(k) = serde_json::from_value::<Vec<i32>>(v["key"].clone())
            {
                out.insert(k, v);
            }
        }
    }
    out
}

/// Members of an engine relation record that a `rustred.relation-entry.v1` body drops (volatile / batch-bound).
const ENTRY_DROPPED: [&str; 4] = ["seconds", "vmhwm_kb", "ordinal", "tag"];

/// The entry body of a record (canonical JSON, volatile members dropped).
fn entry_body(rec: &Value) -> Value {
    let mut b = rec.clone();
    if let Some(o) = b.as_object_mut() {
        for k in ENTRY_DROPPED {
            o.remove(k);
        }
    }
    b
}

/// Run the ship economy to its end; returns the report (also written to `out_dir/report.json`).
pub fn run_ship(cfg: &ShipRunConfig) -> Result<Value, String> {
    let t0 = Instant::now();
    let p = &cfg.policy;
    p.validate()?;
    let say = |m: String| {
        if cfg.verbose {
            eprintln!("[shipecon {} {}] {m}", cfg.node, now());
        }
    };
    std::fs::create_dir_all(&cfg.out_dir).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&cfg.work_root).map_err(|e| e.to_string())?;
    let isp = TableSetSpec::read(&cfg.import_spec).map_err(|e| format!("import spec: {e}"))?;
    say(format!("loading {}", cfg.import_spec.display()));
    let mut set0 = load(&isp, &cfg.import_base).map_err(|e| format!("import load: {e}"))?;
    let load_s = t0.elapsed().as_secs_f64();
    let (census, before, census_source) = census_and_before(cfg, &set0, &say)?;
    let cand = ship::candidacy(p, &census);
    let prev = previous_records(cfg.previous.as_deref())?;
    let ttimes = table_times(cfg.table_times.as_deref())?;
    // plan per examined rule
    let mut plans: BTreeMap<usize, Reuse> = BTreeMap::new();
    for (i, c) in cand.iter().enumerate() {
        if *c == Candidacy::Examine {
            plans.insert(i, ship::plan(p, &census.rules[i], prev.get(&census.rules[i].rule.record_sha256)));
        }
    }
    let n_exam = plans.len();
    let n_unchanged = plans.values().filter(|r| **r == Reuse::Unchanged).count();
    say(format!(
        "census: {} rules, {} bytes; examined {n_exam} ({n_unchanged} unchanged since the previous run), kept aside {}",
        census.rules.len(),
        census.total_bytes,
        cand.iter().filter(|c| **c == Candidacy::KeptAside).count()
    ));
    // the tentative shipped dispatch: every examined and kept-aside rule removed (the accept chain's dispatch check)
    let tentative: Vec<RuleDecision> = cand
        .iter()
        .enumerate()
        .filter(|(_, c)| matches!(c, Candidacy::Examine | Candidacy::KeptAside))
        .map(|(i, _)| {
            let mut d = ship::decide_unexamined(&census.rules[i], &Candidacy::KeptAside);
            d.outcome = Outcome::KeptAside;
            d.why = "tentative removal (guard dispatch)".into();
            d
        })
        .collect();
    let tspec = ship::shipped_spec(&isp, &tentative, &[], &[], p.stored.precedence)?;
    let tspec_path = cfg.out_dir.join("work/tentative.spec.json");
    write(
        &tspec_path,
        &serde_json::to_vec_pretty(&ship::spec_json(&tspec)).map_err(|e| e.to_string())?,
    )?;
    let set_t = load(&tspec, &cfg.import_base).map_err(|e| format!("tentative load: {e}"))?;
    // the keys a LATER table rule takes once the examined / kept-aside rules leave the shipped table
    let mut taken_by_table: BTreeSet<Vec<i32>> = BTreeSet::new();
    for (i, c) in cand.iter().enumerate() {
        if matches!(c, Candidacy::Examine) {
            for (k, _) in &census.rules[i].keys {
                if matches!(
                    set_t.disp.first_match(k),
                    Ok(rustred_campaign_core::cover::Dispatch::Covered(_))
                ) {
                    taken_by_table.insert(k.clone());
                }
            }
        }
    }
    say(format!(
        "tentative shipped dispatch: {} examined keys taken by a later table rule",
        taken_by_table.len()
    ));
    drop(set_t);
    // the accept chain certifies each relation as such (no table before it): the slot before the first table
    let set_a = &mut set0;
    let first_table = {
        let tables = set_a.disp.tables();
        set_a
            .disp
            .priority()
            .first()
            .map(|&t| tables[t].name.clone())
            .ok_or("no table in the dispatch")?
    };
    let slot_a = DispatchSlot::BeforeTable(first_table);
    let certified_keys: std::collections::HashSet<&Vec<i32>> = before.keys.iter().collect();
    // trials: reusable ones from the plan, new ones by rounds of steps
    let mut trials: BTreeMap<usize, Vec<KeyTrial>> = BTreeMap::new();
    for (&i, r) in &plans {
        let have = match r {
            Reuse::Continue { trials, .. } => trials.clone(),
            Reuse::Unchanged => prev
                .get(&census.rules[i].rule.record_sha256)
                .map(|x| x.trials.clone())
                .unwrap_or_default(),
            Reuse::Fresh => vec![],
        };
        trials.insert(i, have);
    }
    let prog_bytes = std::fs::read(&cfg.program.path).map_err(|e| format!("{}: {e}", cfg.program.path.display()))?;
    let bits = AcceptBits {
        boxed: true,
        determinism_id: None,
        binary_blake3: blake3::hash(&prog_bytes).to_hex().to_string(),
        binary_sha256: sha256_hex(&prog_bytes),
        binary_path: cfg.program.path.display().to_string(),
        family_path: cfg.family.display().to_string(),
        family_blake3: crate::atomic::blake3_file(&cfg.family).map_err(|e| e.to_string())?.1,
        order: cfg.order.clone(),
        max_trace_rows: p.recipe.max_trace_rows,
        key_budget_s: p.recipe.key_budget_s,
    };
    let sampler = Arc::new(
        Sampler::spawn(SamplerConfig {
            interval: Duration::from_millis(500),
            collect_foreign_pinning: false,
        })
        .map_err(|e| format!("sampler: {e}"))?,
    );
    let mut solved_lines: Vec<String> = vec![];
    let mut records_by_key: BTreeMap<Vec<i32>, Value> = BTreeMap::new();
    let mut recipe_files: BTreeMap<String, (PathBuf, String, u64)> = BTreeMap::new();
    let mut jobs_log: Vec<Value> = vec![];
    let mut guard_cpu_s = 0.0f64;
    let mut guard_wall_s = 0.0f64;
    let bound = p.bound_value();
    for round in 0..p.recipe.steps.len() {
        // the (rule, step, keys) still needed
        let mut need: BTreeMap<(usize, String), Vec<Vec<i32>>> = BTreeMap::new();
        for (&i, r) in &plans {
            if *r == Reuse::Unchanged {
                continue;
            }
            for (s, k) in ship::missing_trials(p, &census.rules[i], &trials[&i]) {
                need.entry((i, s)).or_default().push(k);
            }
        }
        if need.is_empty() {
            break;
        }
        let mut queue: Vec<Job> = vec![];
        for ((i, s), keys) in &need {
            let rule = &census.rules[*i].rule;
            for (rid, doc, rkeys) in recipes_for(p, set_a, rule, s, keys)? {
                let mut text = rustred_campaign_core::schema::canon::canonical_json(&doc, false)?;
                text.push('\n');
                let sha = sha256_hex(text.as_bytes());
                let path = cfg
                    .out_dir
                    .join("recipes")
                    .join(format!("{}.json", crate::closeloop::ids::safe(&rid)));
                write(&path, text.as_bytes())?;
                recipe_files.insert(rid.clone(), (path.clone(), sha.clone(), text.len() as u64));
                for (c, chunk) in rkeys.chunks(p.recipe.max_keys_per_job).enumerate() {
                    queue.push(Job {
                        id: format!("r{round}-{}-c{c}", crate::closeloop::ids::safe(&rid)),
                        rule: *i,
                        step: s.clone(),
                        recipe_id: rid.clone(),
                        recipe_path: path.clone(),
                        recipe_sha256: sha.clone(),
                        keys: chunk.to_vec(),
                    });
                }
            }
        }
        say(format!(
            "round {round}: {} jobs, {} keys",
            queue.len(),
            queue.iter().map(|j| j.keys.len()).sum::<usize>()
        ));
        let (tx, rx) = mpsc::channel::<Done>();
        let mut in_flight = 0usize;
        let mut queue = queue.into_iter();
        loop {
            while in_flight < cfg.parallel.max(1) {
                let Some(j) = queue.next() else { break };
                launch(cfg, j, sampler.clone(), tx.clone())?;
                in_flight += 1;
            }
            if in_flight == 0 {
                break;
            }
            let d = rx.recv().map_err(|e| format!("worker channel: {e}"))?;
            in_flight -= 1;
            let job = d.job;
            let mut new_trials: Vec<KeyTrial> = vec![];
            let mut job_rec = json!({"job": job.id, "rule": census.rules[job.rule].rule.id(), "step": job.step,
                "recipe": job.recipe_id, "keys": job.keys.len()});
            match d.result {
                Err(e) => {
                    job_rec["error"] = json!(e);
                    for k in &job.keys {
                        new_trials.push(failed_trial(&job, k, "REFUSED", &e, &bound));
                    }
                }
                Ok((results, lines, b3, run_dir, cost)) => {
                    job_rec["run_dir"] = json!(run_dir);
                    job_rec["cost"] = json!({"cpu_s": cost.cpu_s, "wall_s": cost.wall_s, "vmhwm_kb": cost.vmhwm_kb});
                    guard_cpu_s += cost.cpu_s.unwrap_or(0.0);
                    guard_wall_s += cost.wall_s.unwrap_or(0.0);
                    if lines.is_empty() {
                        job_rec["error"] = json!(results);
                        for k in &job.keys {
                            new_trials.push(failed_trial(&job, k, "REFUSED", &results, &bound));
                        }
                    } else {
                        let batch = BatchFile {
                            schema: BATCH_SCHEMA.into(),
                            node: cfg.node.clone(),
                            item: job.id.clone(),
                            round: round as u32,
                            source: job.recipe_id.clone(),
                            recipe_path: job.recipe_path.display().to_string(),
                            recipe_sha256: job.recipe_sha256.clone(),
                            recipe_blake3: crate::atomic::blake3_file(&job.recipe_path)
                                .map(|x| x.1)
                                .unwrap_or_default(),
                            engine_digest: None,
                            relation_rule: p.recipe.relation_rule.clone(),
                            keys: job.keys.clone(),
                        };
                        let raw = raw_records(&results);
                        match accept_point(
                            set_a,
                            None,
                            &batch,
                            &slot_a,
                            &lines,
                            &bits,
                            (&results, &b3),
                            &run_dir,
                            None,
                        ) {
                            Err(e) => {
                                job_rec["error"] = json!(format!("accept chain: {e}"));
                                for k in &job.keys {
                                    new_trials.push(failed_trial(&job, k, "REFUSED", &e, &bound));
                                }
                            }
                            Ok(dec) => {
                                solved_lines.extend(dec.lines.iter().cloned());
                                let cert: BTreeSet<&Vec<i32>> = dec.certified.iter().collect();
                                let failed: BTreeMap<&Vec<i32>, &String> =
                                    dec.failed.iter().map(|(k, w)| (k, w)).collect();
                                let retry: BTreeMap<&Vec<i32>, &String> =
                                    dec.retry.iter().map(|(k, w)| (k, w)).collect();
                                for k in &job.keys {
                                    let r = raw.get(k);
                                    let verdict = r
                                        .and_then(|r| r["verdict"].as_str())
                                        .unwrap_or("MISSING")
                                        .to_string();
                                    let body = r.map(entry_body);
                                    let entry_bytes = body.as_ref().map(|b| {
                                        rustred_campaign_core::schema::canon::canonical_json(b, true)
                                            .map(|t| t.len() as u64 + 600)
                                            .unwrap_or(0)
                                    });
                                    if cert.contains(k)
                                        && let Some(rv) = r
                                    {
                                        records_by_key.insert(k.clone(), rv.clone());
                                    }
                                    new_trials.push(KeyTrial {
                                        key: k.clone(),
                                        step: job.step.clone(),
                                        recipe_id: job.recipe_id.clone(),
                                        recipe_sha256: job.recipe_sha256.clone(),
                                        recipe_digest: r.and_then(|r| r["recipe_digest"].as_str()).map(str::to_string),
                                        verdict: verdict.clone(),
                                        certified: cert.contains(k),
                                        relation_digest: r
                                            .and_then(|r| r["relation_digest"].as_str())
                                            .map(str::to_string),
                                        seconds: r.and_then(|r| r["seconds"].as_f64()),
                                        rhs_terms: r.and_then(|r| r["rhs_count"].as_u64()),
                                        entry_bytes,
                                        pole_order_d4: r
                                            .and_then(|r| r["pole_order_d4"].as_u64())
                                            .map(|x| x as u32),
                                        rhs_outside_certified: r.map(|r| {
                                            r["rhs"]
                                                .as_array()
                                                .into_iter()
                                                .flatten()
                                                .filter(|t| t[2] == "top")
                                                .filter_map(|t| {
                                                    t[0].as_str()
                                                        .and_then(rustred_campaign_core::assemble::parse_key_text)
                                                })
                                                .filter(|k| !certified_keys.contains(k))
                                                .count() as u64
                                        }),
                                        why: if cert.contains(k) {
                                            None
                                        } else {
                                            Some(
                                                failed
                                                    .get(k)
                                                    .or(retry.get(k))
                                                    .map(|w| w.to_string())
                                                    .unwrap_or_else(|| verdict.clone()),
                                            )
                                        },
                                        bound: bound.clone(),
                                    });
                                }
                                job_rec["certified"] = json!(dec.certified.len());
                                job_rec["failed"] = json!(dec.failed.len() + dec.retry.len());
                            }
                        }
                    }
                }
            }
            say(format!(
                "settled {}: {}",
                job.id,
                job_rec["error"].as_str().map_or_else(
                    || format!("certified {} of {}", job_rec["certified"], job.keys.len()),
                    |e| format!("refused: {e}")
                )
            ));
            jobs_log.push(job_rec);
            let t = trials.entry(job.rule).or_default();
            for nt in new_trials {
                t.retain(|x| !(x.key == nt.key && x.step == nt.step));
                t.push(nt);
            }
        }
    }
    let guard_s = t0.elapsed().as_secs_f64() - load_s;
    // decisions
    let mut decisions: Vec<RuleDecision> = vec![];
    let mut guards: BTreeMap<usize, ship::GuardRecord> = BTreeMap::new();
    let mut table_time_of: BTreeMap<usize, TableTime> = BTreeMap::new();
    for (i, c) in cand.iter().enumerate() {
        let svc = &census.rules[i];
        match c {
            Candidacy::Examine => {
                if plans.get(&i) == Some(&Reuse::Unchanged)
                    && let Some(pr) = prev.get(&svc.rule.record_sha256)
                {
                    decisions.push(pr.decision.clone());
                    if let Some(g) = &pr.guard {
                        guards.insert(i, g.clone());
                    }
                    if let Some(tt) = &pr.table_time {
                        table_time_of.insert(i, tt.clone());
                    }
                    continue;
                }
                let tt = ttimes
                    .get(&svc.rule.record_sha256)
                    .cloned()
                    .unwrap_or_else(|| TableTime::estimate(&p.cost, svc.bytes, svc.monomials));
                let g = ship::guard(p, svc, trials.get(&i).map(Vec::as_slice).unwrap_or(&[]));
                let rb: BTreeMap<String, u64> = recipe_files.iter().map(|(k, v)| (k.clone(), v.2)).collect();
                decisions.push(ship::decide(p, svc, &g, &tt, &rb, &taken_by_table));
                guards.insert(i, g);
                table_time_of.insert(i, tt);
            }
            other => decisions.push(ship::decide_unexamined(svc, other)),
        }
    }
    // the closing check of the shipped spec, reverting the rules behind a failure
    let solved_path = cfg.out_dir.join("ship/solved.jsonl");
    let mut solved_text = String::new();
    for l in &solved_lines {
        solved_text.push_str(l);
        solved_text.push('\n');
    }
    write(&solved_path, solved_text.as_bytes())?;
    let solved_ref = FileRef {
        path: solved_path.display().to_string(),
        sha256: Some(sha256_hex(solved_text.as_bytes())),
        manifest: None,
        family: None,
    };
    let mut check_log: Vec<Value> = vec![];
    let mut closing: Option<ClosingCheck> = None;
    let mut final_spec: Option<TableSetSpec> = None;
    for it in 0..=p.check.max_iterations {
        let (recipes, assign) = shipped_recipes(cfg, &decisions, &recipe_files)?;
        let mut sspec = ship::shipped_spec(
            &isp,
            &decisions,
            &recipes,
            std::slice::from_ref(&solved_ref),
            p.stored.precedence,
        )?;
        sspec.evidence.expected_recipe_binary = Some(bits.binary_sha256.clone());
        let sp = cfg.out_dir.join(format!("ship/spec.iter{it}.json"));
        write(
            &sp,
            &serde_json::to_vec_pretty(&ship::spec_json(&sspec)).map_err(|e| e.to_string())?,
        )?;
        let tc = Instant::now();
        let set_s = load(&sspec, &cfg.import_base).map_err(|e| format!("shipped load: {e}"))?;
        let mut ev = Evidence::from_spec(&set_s, &cfg.import_base).map_err(|e| format!("shipped evidence: {e}"))?;
        ev.accept_ledger_verified_files();
        let res = check(
            &set_s,
            &ev,
            &CheckOptions {
                threads: cfg.check_threads.max(1),
                tier: None,
                max_rounds: None,
            },
        )
        .map_err(|e| format!("shipped check: {e}"))?;
        let after = view_of(&set_s, &res)?;
        let cc = ClosingCheck::compare(&before, &after);
        say(format!(
            "closing check {it}: closed {} same start {} same terminals {} open {} new keys {} dropped {} ({:.1} s)",
            cc.closed,
            cc.same_start,
            cc.same_terminals,
            cc.open_after,
            cc.new_keys,
            cc.dropped_keys,
            tc.elapsed().as_secs_f64()
        ));
        let mut entry = json!({"iteration": it, "check": cc, "recipes": recipes.len(), "assignments": assign,
            "migrating": decisions.iter().filter(|d| d.outcome.migrates()).count(),
            "seconds": tc.elapsed().as_secs_f64()});
        if cc.holds {
            check_log.push(entry);
            closing = Some(cc);
            final_spec = Some(sspec);
            break;
        }
        // culprits: open keys / new terminals; the rules that served them before, or whose relations reach them
        let rows = all_rows(&set_s, &res)?;
        let bad: BTreeSet<Vec<i32>> = rows
            .iter()
            .filter(|r| {
                r.status == RowStatus::Open
                    || (r.status == RowStatus::Terminal && !before.terminals.contains(&r.key))
            })
            .map(|r| r.key.clone())
            .collect();
        let mut culprits: BTreeSet<usize> = BTreeSet::new();
        for (di, d) in decisions.iter().enumerate() {
            if !d.outcome.migrates() {
                continue;
            }
            let reaches = d.keys.iter().any(|k| {
                bad.contains(&k.key)
                    || records_by_key.get(&k.key).is_some_and(|r| {
                        r["rhs"].as_array().into_iter().flatten().any(|t| {
                            t[0].as_str()
                                .and_then(rustred_campaign_core::assemble::parse_key_text)
                                .is_some_and(|rk| bad.contains(&rk))
                        })
                    })
            });
            if reaches {
                culprits.insert(di);
            }
        }
        if culprits.is_empty() || it == p.check.max_iterations {
            // nothing attributable: revert every migration (the shipped spec = the original material)
            entry["reverted"] = json!("all (no attributable culprit or last iteration)");
            for d in decisions.iter_mut().filter(|d| d.outcome.migrates()) {
                ship::revert(d, &format!("closing check iteration {it} failed: {:?}", cc.blocking));
            }
        } else {
            entry["reverted"] = json!(culprits.iter().map(|&di| decisions[di].rule.id()).collect::<Vec<_>>());
            for &di in &culprits {
                ship::revert(
                    &mut decisions[di],
                    &format!("its keys or relations reach {} open / new-terminal keys", bad.len()),
                );
            }
        }
        entry["bad_keys"] = json!(bad.len());
        check_log.push(entry);
        closing = Some(cc);
    }
    // records, report, artifact
    let mut records: Vec<EconomyRecord> = vec![];
    for (i, c) in cand.iter().enumerate() {
        if !matches!(c, Candidacy::Examine | Candidacy::KeptAside) {
            continue;
        }
        let svc = &census.rules[i];
        let d = decisions[i].clone();
        let g = guards.get(&i).cloned();
        let info = set0
            .find_sha(&svc.rule.record_sha256)
            .map(|(t, r)| set0.rules[t][r].clone());
        records.push(EconomyRecord {
            schema: ECONOMY_RECORD_SCHEMA.into(),
            kind: RECORD_KIND.into(),
            node: cfg.node.clone(),
            subject: svc.rule.clone(),
            inputs_digest: ship::inputs_digest(svc),
            decision_digest: p.decision_digest(),
            policy_digest: p.digest(),
            trials: trials.get(&i).cloned().unwrap_or_default(),
            table_time: table_time_of.get(&i).cloned(),
            not_tried: ship::not_tried(p, &d, g.as_ref()),
            decision: d,
            guard: g,
            revert: json!({"table_path": info.as_ref().map(|x| x.path.clone()), "table_sha256": svc.rule.table_sha256,
                "offset": info.as_ref().map(|x| x.offset), "bytes": svc.bytes,
                "how": "remove the rule's entry from the shipped spec's `removed` list (the original table is kept)"}),
        });
    }
    write(&cfg.out_dir.join("records.jsonl"), &jsonl(&records))?;
    let guard_list: Vec<&ship::GuardRecord> = guards.values().collect();
    write(&cfg.out_dir.join("guards.jsonl"), &jsonl(&guard_list))?;
    write(&cfg.out_dir.join("jobs.jsonl"), &jsonl(&jobs_log))?;
    let totals = ship::totals(&census, &decisions);
    let applied = if p.mode == ShipMode::Apply && closing.as_ref().is_some_and(|c| c.holds) {
        Some(write_artifact(cfg, &census, &decisions, final_spec.as_ref(), &records_by_key, &set0)?)
    } else {
        None
    };
    let per_rule: Vec<Value> = decisions
        .iter()
        .filter(|d| d.outcome != Outcome::NotExamined)
        .map(|d| serde_json::to_value(d).unwrap_or(Value::Null))
        .collect();
    let not_examined: BTreeMap<String, u64> = decisions
        .iter()
        .filter(|d| d.outcome == Outcome::NotExamined)
        .fold(BTreeMap::new(), |mut m, d| {
            let w = d.why.split(" <").next().unwrap_or("").split(" >").next().unwrap_or("").to_string();
            *m.entry(w).or_default() += 1;
            m
        });
    let v = json!({
        "schema": REPORT_SCHEMA,
        "node": cfg.node,
        "mode": p.mode,
        "policy": p.to_value(),
        "policy_digest": p.digest(),
        "decision_digest": p.decision_digest(),
        "import_spec": {"path": cfg.import_spec.display().to_string(), "digest": isp.digest()},
        "census_source": census_source,
        "census": census.totals(),
        "before": {"keys": before.keys.len(), "start": before.start.len(), "terminals": before.terminals, "open": before.open},
        "examined": n_exam,
        "unchanged_since_previous": n_unchanged,
        "previous": cfg.previous.as_ref().map(|x| x.display().to_string()),
        "binary": {"path": bits.binary_path, "sha256": bits.binary_sha256, "blake3": bits.binary_blake3},
        "table_times": cfg.table_times.as_ref().map(|x| x.display().to_string()),
        "totals": totals,
        "closing_check": closing,
        "closing_iterations": check_log,
        "decisions": per_rule,
        "not_examined_by_reason": not_examined,
        "applied": applied,
        "timing": {"load_s": load_s, "guard_s": guard_s, "guard_jobs_cpu_s": guard_cpu_s,
                   "guard_jobs_wall_s": guard_wall_s, "wall_s": t0.elapsed().as_secs_f64(),
                   "note": "wall times on a shared host, unpinned (not a benchmark)"},
    });
    write(
        &cfg.out_dir.join("report.json"),
        &serde_json::to_vec_pretty(&v).map_err(|e| e.to_string())?,
    )?;
    Ok(v)
}

fn failed_trial(job: &Job, k: &[i32], verdict: &str, why: &str, bound: &Value) -> KeyTrial {
    KeyTrial {
        key: k.to_vec(),
        step: job.step.clone(),
        recipe_id: job.recipe_id.clone(),
        recipe_sha256: job.recipe_sha256.clone(),
        recipe_digest: None,
        verdict: verdict.into(),
        certified: false,
        relation_digest: None,
        seconds: None,
        rhs_terms: None,
        entry_bytes: None,
        pole_order_d4: None,
        rhs_outside_certified: None,
        why: Some(why.into()),
        bound: bound.clone(),
    }
}

/// The recipe sources of the shipped spec: one per recipe used by a migrating rule's recipe / stored keys, assigned
/// exactly those keys (assignment side files written next to the recipes).
fn shipped_recipes(
    cfg: &ShipRunConfig,
    decisions: &[RuleDecision],
    recipe_files: &BTreeMap<String, (PathBuf, String, u64)>,
) -> Result<(Vec<ShippedRecipe>, usize), String> {
    let mut by: BTreeMap<(String, String), Vec<Vec<i32>>> = BTreeMap::new();
    for d in decisions.iter().filter(|d| d.outcome.migrates()) {
        for k in &d.keys {
            if !matches!(k.form.as_str(), "recipe" | "stored") {
                continue;
            }
            let Some(rid) = &k.recipe_id else { continue };
            by.entry((rid.clone(), k.form.clone()))
                .or_default()
                .push(k.key.clone());
        }
    }
    let mut out = vec![];
    let mut n = 0usize;
    for ((rid, form), keys) in by {
        let (path, sha, _) = recipe_files
            .get(&rid)
            .ok_or_else(|| format!("recipe {rid}: no file"))?;
        let a = format!("{}\n", json!({"recipe": rid, "keys": keys}));
        let ap = cfg
            .out_dir
            .join("ship/assign")
            .join(format!("{form}.{}.jsonl", crate::closeloop::ids::safe(&rid)));
        write(&ap, a.as_bytes())?;
        n += keys.len();
        out.push(ShippedRecipe {
            id: format!("shipecon:{form}:{rid}"),
            path: path.display().to_string(),
            sha256: sha.clone(),
            form,
            assignment: Some(FileRef {
                path: ap.display().to_string(),
                sha256: Some(sha256_hex(a.as_bytes())),
                manifest: None,
                family: None,
            }),
        });
    }
    Ok((out, n))
}

/// Apply mode: the shipped artifact (`out_dir/ship/`): the shipped spec, the kept-aside list, the engine recipe
/// records (explicit certified keys + certified relation digests + the work bound), the stored relations as
/// `rustred.relation-entry.v1`, the removed list (revert), SHA256SUMS.
fn write_artifact(
    cfg: &ShipRunConfig,
    census: &Census,
    decisions: &[RuleDecision],
    spec: Option<&TableSetSpec>,
    records_by_key: &BTreeMap<Vec<i32>, Value>,
    set: &ImportedSet,
) -> Result<Value, String> {
    let dir = cfg.out_dir.join("ship");
    let mut sums = String::new();
    let mut put = |name: &str, b: &[u8]| -> Result<(), String> {
        write(&dir.join(name), b)?;
        sums.push_str(&format!("{}  {name}\n", sha256_hex(b)));
        Ok(())
    };
    if let Some(s) = spec {
        put(
            "spec.json",
            &serde_json::to_vec_pretty(&ship::spec_json(s)).map_err(|e| e.to_string())?,
        )?;
    }
    let mut kept = vec![];
    let mut removed = vec![];
    let mut recipe_records: BTreeMap<String, Value> = BTreeMap::new();
    let mut entries = vec![];
    let fam_text = family_source_of(set);
    let node_identity = blake3::hash(
        json!({"family_blake3": blake3::hash(fam_text.as_bytes()).to_hex().to_string(),
               "order": serde_json::from_str::<Value>(&cfg.order).unwrap_or(Value::String(cfg.order.clone())),
               "arity": set.frame.arity()})
        .to_string()
        .as_bytes(),
    )
    .to_hex()
    .to_string();
    for (i, d) in decisions.iter().enumerate() {
        if !d.outcome.migrates() {
            continue;
        }
        let svc = &census.rules[i];
        let info = set.find_sha(&svc.rule.record_sha256).map(|(t, r)| &set.rules[t][r]);
        let row = json!({"rule": d.rule, "outcome": d.outcome, "bytes": d.bytes_before,
            "table_path": info.map(|x| x.path.clone()), "offset": info.map(|x| x.offset)});
        if d.outcome == Outcome::KeptAside {
            kept.push(row.clone());
        }
        removed.push(row);
        for k in &d.keys {
            let Some(rec) = records_by_key.get(&k.key) else { continue };
            match k.form.as_str() {
                "recipe" => {
                    let rid = k.recipe_id.clone().unwrap_or_default();
                    let e = recipe_records.entry(rid.clone()).or_insert_with(|| {
                        json!({"id": rid, "recipe_path": null, "recipe_digest": rec["recipe_digest"],
                               "domain": {"keys": []}, "relation_digests": {},
                               "max_trace_rows": cfg.policy.recipe.max_trace_rows})
                    });
                    if let Some(a) = e["domain"]["keys"].as_array_mut() {
                        a.push(json!(k.key));
                    }
                    e["relation_digests"][ship::key_text(&k.key)] = rec["relation_digest"].clone();
                }
                "stored" => {
                    let body = entry_body(rec);
                    let canon = rustred_campaign_core::schema::canon::canonical_json(&body, true)?;
                    entries.push(json!({"schema": "rustred.relation-entry.v1", "node": cfg.node,
                        "node_identity": node_identity, "key_frame": "table", "key": k.key,
                        "entry_digest": sha256_hex(canon.as_bytes()), "body": body,
                        "recipe": {"id": k.recipe_id, "file_sha256": null, "engine_digest": rec["recipe_digest"]},
                        "bound_to": {"determinism_id": rec["determinism_id"], "node_identity": node_identity,
                                     "relation_rule": rec["relation_rule"]},
                        "evidence": {"level": "per-key", "proof": {"tool": "rustred.recipe-solve/exact-weights",
                                     "record_sha256": sha256_hex(canon.as_bytes()), "verdict": "SOLVED"}},
                        "diagnostics": {"pole_order_at": {"4": rec["pole_order_d4"]}, "recipe": k.recipe_id},
                        "written": {"by": "rustred-campaign shipecon"}}));
                }
                _ => {}
            }
        }
    }
    put("kept_aside.jsonl", &jsonl(&kept))?;
    put("removed.jsonl", &jsonl(&removed))?;
    let rr: Vec<&Value> = recipe_records.values().collect();
    put("recipes.records.jsonl", &jsonl(&rr))?;
    put("relations.entries.jsonl", &jsonl(&entries))?;
    std::fs::write(dir.join("SHA256SUMS"), sums.as_bytes()).map_err(|e| e.to_string())?;
    Ok(json!({"dir": dir.display().to_string(), "kept_aside": kept.len(), "removed": removed.len(),
        "recipe_records": rr.len(), "stored_entries": entries.len(), "node_identity": node_identity}))
}

/// The frame table's `original_family_source` (else `family_source`) header text.
fn family_source_of(set: &ImportedSet) -> String {
    let Some(i) = set.rules.iter().flatten().next() else {
        return String::new();
    };
    let Ok(f) = std::fs::File::open(&i.path) else {
        return String::new();
    };
    let mut first = String::new();
    if std::io::BufReader::new(f).read_line(&mut first).is_err() {
        return String::new();
    }
    let h: Value = serde_json::from_str(&first).unwrap_or(Value::Null);
    h["original_family_source"]
        .as_str()
        .or(h["family_source"].as_str())
        .unwrap_or("")
        .to_string()
}
