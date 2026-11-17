//! The DIRECT DRIVER of terminal minimisation (interfaces/termmin.md section 8): the core state machine on the
//! terminals and the closed key set of existing material (an import spec: a statement's material), without the
//! supervisor: each recipe solve is ONE child through the solve-recipe adapter and the process layer's launcher
//! (`adapters::run_attempt`: own session, PDEATHSIG, verified kill, memory cap), at most one child per held CPU, each
//! pinned to its CPU; every settled attempt goes through the closeloop's accept chain with the terminal undeclared.
//! Run it inside a private pid namespace (L10). Generic: node, paths, order and CPUs are configuration.

use super::{AcceptBits, accept_point, answer_of, build_machine, cost_of, termmin_group};
use crate::adapters::job::{InputFile, JobAdapter, Program, RunOptions, run_attempt};
use crate::adapters::solve_recipe::{
    BatchPolicy, RecipeInput, SolveRecipeAdapter, SolveRecipeConfig, SolveRequest as AdReq,
};
use crate::closeloop::accept::{BATCH_SCHEMA, BatchFile};
use crate::launch::RealLauncher;
use crate::resources::{Sampler, SamplerConfig};
use rustred_campaign_contract::layout::RunLayout;
use rustred_campaign_contract::seal::verify_stream_handle;
use rustred_campaign_core::closure::relations::DispatchSlot;
use rustred_campaign_core::import::check::CheckOptions;
use rustred_campaign_core::import::{Evidence, TableSetSpec};
use rustred_campaign_core::termmin::machine::{Cost, SolveAnswer, SolvePurpose, SolveRequest};
use rustred_campaign_core::termmin::{ImportExtension, TermminSpec, key_text};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// What the direct driver needs (all data).
pub struct DirectConfig {
    pub node: String,
    pub import_spec: PathBuf,
    pub import_base: PathBuf,
    /// Extra solved-relation files (import 6.2) to add to the evidence (e.g. a closing loop's answers).
    pub extra_solved: Vec<PathBuf>,
    /// Explicit terminal list (default: the terminals reached by the base check).
    pub terminals: Option<Vec<Vec<i32>>>,
    pub spec: TermminSpec,
    pub program: Program,
    pub family: PathBuf,
    pub order: String,
    /// One child per CPU, each pinned to it.
    pub cpus: Vec<usize>,
    pub work_root: PathBuf,
    pub out_dir: PathBuf,
    pub check_threads: usize,
    pub licence_env: Option<String>,
    /// Wall cap of one solve child (terminate after it: typed refusal).
    pub wall_cap_s: u64,
    /// Progress lines on stderr.
    pub verbose: bool,
}

struct Done {
    id: String,
    key: Vec<i32>,
    step: String,
    recipe_id: String,
    recipe_path: String,
    batch: BatchFile,
    closing: bool,
    result: Result<(String, Vec<String>, String, String, Cost), String>,
}

fn sha256_hex(b: &[u8]) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

/// Run the pass to its end; returns the report (also written to `out_dir/report.json`).
pub fn run_direct(cfg: &DirectConfig) -> Result<Value, String> {
    let t0 = Instant::now();
    std::fs::create_dir_all(cfg.out_dir.join("recipes")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&cfg.work_root).map_err(|e| e.to_string())?;
    let say = |m: String| {
        if cfg.verbose {
            eprintln!("[termmin {}] {m}", chrono_now());
        }
    };
    let isp = TableSetSpec::read(&cfg.import_spec).map_err(|e| format!("import spec: {e}"))?;
    say(format!("loading {}", cfg.import_spec.display()));
    let set = rustred_campaign_core::import::load(&isp, &cfg.import_base)
        .map_err(|e| format!("import load: {e}"))?;
    let mut ev =
        Evidence::from_spec(&set, &cfg.import_base).map_err(|e| format!("evidence: {e}"))?;
    // the measurement tool's extra solved files are named by its own (hand-written) config
    ev.accept_ledger_verified_files();
    for f in &cfg.extra_solved {
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
        .map_err(|e| format!("solved {}: {e}", f.display()))?;
    }
    let load_s = t0.elapsed().as_secs_f64();
    let group = termmin_group(&cfg.spec, &isp, &cfg.import_base, &set)?;
    say(format!(
        "loaded in {load_s:.1} s; symmetry group: {}",
        group
            .as_ref()
            .map(|g| format!("{} non-identity elements", g.group().elements.len()))
            .unwrap_or("none".into())
    ));
    let mut ext = ImportExtension::new(
        set,
        ev,
        CheckOptions {
            threads: cfg.check_threads.max(1),
            tier: None,
            max_rounds: None,
        },
        group,
        cfg.spec.caps.symmetry_max_terms,
    )?;
    let base_s = t0.elapsed().as_secs_f64() - load_s;
    say(format!(
        "base check {base_s:.1} s: {} keys, {} terminals {:?}, {} open",
        ext.base().economy.closure_keys,
        ext.base().terminals.len(),
        ext.base()
            .terminals
            .iter()
            .map(|k| key_text(k))
            .collect::<Vec<_>>(),
        ext.base().open.len()
    ));
    let mut m = build_machine(&cfg.spec, &ext, cfg.terminals.as_deref())?;
    say(format!(
        "attempt order: {:?}",
        m.order().iter().map(|k| key_text(k)).collect::<Vec<_>>()
    ));
    let prog_bytes = std::fs::read(&cfg.program.path)
        .map_err(|e| format!("{}: {e}", cfg.program.path.display()))?;
    let bits = AcceptBits {
        boxed: true,
        determinism_id: None,
        binary_blake3: blake3::hash(&prog_bytes).to_hex().to_string(),
        binary_sha256: sha256_hex(&prog_bytes),
        binary_path: cfg.program.path.display().to_string(),
        family_path: cfg.family.display().to_string(),
        family_blake3: crate::atomic::blake3_file(&cfg.family)
            .map_err(|e| e.to_string())?
            .1,
        order: cfg.order.clone(),
        max_trace_rows: cfg.spec.caps.max_trace_rows,
        key_budget_s: cfg.spec.caps.key_budget_s,
    };
    let sampler = Arc::new(
        Sampler::spawn(SamplerConfig {
            interval: Duration::from_millis(500),
            collect_foreign_pinning: false,
            foreign_pin_max_cpus: usize::MAX,
        })
        .map_err(|e| format!("sampler: {e}"))?,
    );
    let (tx, rx) = mpsc::channel::<Done>();
    let mut free: Vec<usize> = cfg.cpus.clone();
    let mut in_flight: BTreeMap<String, usize> = BTreeMap::new();
    let mut answers: BTreeMap<String, SolveAnswer> = BTreeMap::new();
    let mut log: Vec<Value> = vec![];
    loop {
        m.advance(&mut ext)?;
        if m.done() && in_flight.is_empty() {
            break;
        }
        for r in m.pending_solves() {
            if in_flight.contains_key(&r.id) || answers.contains_key(&r.id) {
                continue;
            }
            let Some(cpu) = free.pop() else { break };
            in_flight.insert(r.id.clone(), cpu);
            launch(cfg, &r, cpu, sampler.clone(), tx.clone())?;
            say(format!(
                "launched {} on CPU {cpu}: {} at {}",
                r.id,
                purpose_word(&r),
                key_text(&r.key)
            ));
        }
        if in_flight.is_empty() {
            if m.done() {
                break;
            }
            return Err("termmin: nothing in flight and the machine is not done".into());
        }
        let d = rx.recv().map_err(|e| format!("worker channel: {e}"))?;
        if let Some(c) = in_flight.remove(&d.id) {
            free.push(c);
        }
        let ans = match d.result {
            Err(e) => SolveAnswer::Failed {
                class: "refused".into(),
                verdict: None,
                failure: json!({"class": "refused", "message": e}),
                line: None,
                cost: Cost::default(),
            },
            Ok((results_path, lines, results_b3, run_dir, cost)) => {
                if lines.is_empty() {
                    SolveAnswer::Failed {
                        class: "refused".into(),
                        verdict: None,
                        failure: json!({"class": "refused", "message": results_path, "run_dir": run_dir}),
                        line: None,
                        cost,
                    }
                } else {
                    let slot = if d.closing {
                        ext.set
                            .recipes
                            .iter()
                            .find(|x| x.id == d.batch.source)
                            .map(|x| x.slot.clone())
                            .unwrap_or(DispatchSlot::Last)
                    } else {
                        DispatchSlot::Last
                    };
                    let undeclare = (!d.closing).then_some(d.key.as_slice());
                    match accept_point(
                        &mut ext.set,
                        undeclare,
                        &d.batch,
                        &slot,
                        &lines,
                        &bits,
                        (&results_path, &results_b3),
                        &run_dir,
                        None,
                    ) {
                        Ok(dec) => {
                            answer_of(&dec, &d.key, &d.step, &d.recipe_id, &d.recipe_path, cost)
                        }
                        Err(e) => SolveAnswer::Failed {
                            class: "refused".into(),
                            verdict: None,
                            failure: json!({"class": "refused", "message": e}),
                            line: None,
                            cost,
                        },
                    }
                }
            }
        };
        say(format!("settled {}: {}", d.id, answer_word(&ans)));
        log.push(json!({"request": d.id, "answer": ans}));
        answers.insert(d.id.clone(), ans.clone());
        m.answer(&d.id, ans)?;
    }
    let rep = m.report(&cfg.spec.digest());
    let mut v = serde_json::to_value(&rep).map_err(|e| e.to_string())?;
    v["node"] = json!(cfg.node);
    v["import_spec"] = json!(cfg.import_spec.display().to_string());
    v["binary"] = json!({"path": bits.binary_path, "sha256": bits.binary_sha256, "blake3": bits.binary_blake3});
    v["cpus"] = json!(cfg.cpus);
    v["timing"] = json!({"load_s": load_s, "base_check_s": base_s, "wall_s": t0.elapsed().as_secs_f64(),
                         "import_checks": ext.checks});
    v["base"] = json!({"keys": ext.base().economy.closure_keys, "open": ext.base().open.len(),
                       "terminals": ext.base().terminals.iter().map(|k| key_text(k)).collect::<Vec<_>>()});
    std::fs::write(
        cfg.out_dir.join("report.json"),
        serde_json::to_vec_pretty(&v).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let mut al = String::new();
    for l in &log {
        al.push_str(&l.to_string());
        al.push('\n');
    }
    std::fs::write(cfg.out_dir.join("answers.jsonl"), al).map_err(|e| e.to_string())?;
    Ok(v)
}

fn chrono_now() -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("t={t}")
}

fn purpose_word(r: &SolveRequest) -> String {
    match &r.purpose {
        SolvePurpose::Step { recipe, .. } => {
            format!("step {} ({} seeds)", recipe.step, recipe.seeds)
        }
        SolvePurpose::Closing { solve, .. } => format!("closing try with {}", solve.source),
    }
}

fn answer_word(a: &SolveAnswer) -> String {
    match a {
        SolveAnswer::Relation { candidate, cost } => format!(
            "RELATION ({} rhs keys; cpu {:?} s, vmhwm {:?} kB)",
            candidate.rhs.len(),
            cost.cpu_s,
            cost.vmhwm_kb
        ),
        SolveAnswer::Failed {
            class,
            verdict,
            cost,
            ..
        } => {
            format!(
                "{class} ({verdict:?}; cpu {:?} s, vmhwm {:?} kB)",
                cost.cpu_s, cost.vmhwm_kb
            )
        }
    }
}

/// One solve in its own thread (own launcher on the shared sampler), pinned to `cpu`.
fn launch(
    cfg: &DirectConfig,
    r: &SolveRequest,
    cpu: usize,
    sampler: Arc<Sampler>,
    tx: mpsc::Sender<Done>,
) -> Result<(), String> {
    let (source, rpath, rsha, step, rid, closing) = match &r.purpose {
        SolvePurpose::Step { recipe, .. } => {
            let p = cfg
                .out_dir
                .join("recipes")
                .join(format!("{}.json", crate::closeloop::ids::safe(&r.id)));
            std::fs::write(&p, recipe.text.as_bytes()).map_err(|e| e.to_string())?;
            (
                format!("termmin/{}", recipe.step),
                p,
                recipe.sha256.clone(),
                recipe.step.clone(),
                recipe.id.clone(),
                false,
            )
        }
        SolvePurpose::Closing { solve, .. } => (
            solve.source.clone(),
            PathBuf::from(&solve.recipe_path),
            solve.recipe_sha256.clone(),
            "closing".to_string(),
            solve.source.clone(),
            true,
        ),
    };
    let rb = std::fs::read(&rpath).map_err(|e| format!("{}: {e}", rpath.display()))?;
    let relation_rule = match &r.purpose {
        SolvePurpose::Step { recipe, .. } => recipe.relation_rule.clone(),
        SolvePurpose::Closing { .. } => "m1_first".to_string(),
    };
    let batch = BatchFile {
        schema: BATCH_SCHEMA.into(),
        node: cfg.node.clone(),
        item: r.id.clone(),
        round: 0,
        source: source.clone(),
        recipe_path: rpath.display().to_string(),
        recipe_sha256: rsha.clone(),
        recipe_blake3: blake3::hash(&rb).to_hex().to_string(),
        engine_digest: None,
        relation_rule,
        keys: vec![r.key.clone()],
    };
    let caps = &cfg.spec.caps;
    let adcfg = SolveRecipeConfig {
        program: cfg.program.clone(),
        family: InputFile::from_file(&cfg.family).map_err(|e| format!("{e:?}"))?,
        order: cfg.order.clone(),
        batch: BatchPolicy {
            max_keys_per_job: 1,
            max_threads: caps.threads.max(1),
            min_keys_per_thread: 1,
        },
        max_trace_rows: caps.max_trace_rows,
        key_budget_s: caps.key_budget_s,
        expect: None,
        determinism_id: None,
        heartbeat_s: 10,
        stop_grace_s: 30,
        package: None,
        live_repair: false,
        licence_env: cfg.licence_env.clone(),
        fake: None,
    };
    let req = AdReq {
        recipe: RecipeInput {
            id: source,
            file: InputFile::from_file(&rpath).map_err(|e| format!("{e:?}"))?,
            digest: None,
            sha256: Some(rsha),
        },
        key: r.key.iter().map(|&x| x as i64).collect(),
        tag: Some(r.id.clone()),
        context: None,
    };
    let work = cfg.work_root.join(crate::closeloop::ids::safe(&r.id));
    let opts = RunOptions {
        cpus: vec![cpu],
        memory_cap_bytes: Some(caps.cap_bytes),
        licence_env: cfg.licence_env.clone(),
        wall_cap: Some(Duration::from_secs(cfg.wall_cap_s)),
        grace: Some(Duration::from_secs(30)),
    };
    let id = r.id.clone();
    let key = r.key.clone();
    let rpath_s = rpath.display().to_string();
    std::thread::Builder::new()
        .name(format!("termmin-{id}"))
        .spawn(move || {
            let res = (|| -> Result<(String, Vec<String>, String, String, Cost), String> {
                std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
                let ad = SolveRecipeAdapter::new(adcfg);
                let reqs = vec![req];
                let plans = ad.plan(&reqs).map_err(|e| format!("{e:?}"))?;
                let plan = plans.first().ok_or("no plan")?;
                let mut l = RealLauncher::new(sampler);
                let (out, rep) = run_attempt(&mut l, &ad, &reqs, plan, &work, 1, &opts)
                    .map_err(|e| format!("{e:?}"))?;
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
                        run_dir.clone(),
                        cost,
                    ));
                }
                let layout = RunLayout::new(&run_dir);
                let (_, h) = verify_stream_handle(
                    &layout.root,
                    &RunLayout::out_rel("results.jsonl"),
                    &["end"],
                )
                .map_err(|e| format!("results stream: {e:?}"))?;
                let lines: Vec<String> = h
                    .canonical_lines()
                    .map_err(|e| format!("canonical lines: {e:?}"))?
                    .into_iter()
                    .map(|b| String::from_utf8_lossy(&b).into_owned())
                    .collect();
                let p = layout.root.join(RunLayout::out_rel("results.jsonl"));
                let b3 = crate::atomic::blake3_file(&p)
                    .map(|x| x.1)
                    .unwrap_or_default();
                Ok((p.display().to_string(), lines, b3, run_dir.clone(), cost))
            })();
            let _ = tx.send(Done {
                id,
                key,
                step,
                recipe_id: rid,
                recipe_path: rpath_s,
                batch,
                closing,
                result: res,
            });
        })
        .map_err(|e| format!("worker thread: {e}"))?;
    Ok(())
}

/// One solve of an EXPLICIT recipe file at one key through the solve-recipe adapter and the process layer (a
/// reproduction harness: e.g. a reference seed list written as explicit centres). Returns the adapter's classification
/// row, the accepted relations (the adapter's checks: contract, record bits, request binding), the failures, the raw
/// sealed `relation` record of the key (verified stream) and the measured cost.
pub fn solve_recipe_once(
    cfg: &DirectConfig,
    recipe: &Path,
    key: &[i32],
    id: &str,
    cpu: usize,
    sampler: Arc<Sampler>,
) -> Result<Value, String> {
    let caps = &cfg.spec.caps;
    let adcfg = SolveRecipeConfig {
        program: cfg.program.clone(),
        family: InputFile::from_file(&cfg.family).map_err(|e| format!("{e:?}"))?,
        order: cfg.order.clone(),
        batch: BatchPolicy {
            max_keys_per_job: 1,
            max_threads: caps.threads.max(1),
            min_keys_per_thread: 1,
        },
        max_trace_rows: caps.max_trace_rows,
        key_budget_s: caps.key_budget_s,
        expect: None,
        determinism_id: None,
        heartbeat_s: 10,
        stop_grace_s: 30,
        package: None,
        live_repair: false,
        licence_env: cfg.licence_env.clone(),
        fake: None,
    };
    let rb = std::fs::read(recipe).map_err(|e| format!("{}: {e}", recipe.display()))?;
    let req = AdReq {
        recipe: RecipeInput {
            id: id.to_string(),
            file: InputFile::from_file(recipe).map_err(|e| format!("{e:?}"))?,
            digest: None,
            sha256: Some(sha256_hex(&rb)),
        },
        key: key.iter().map(|&x| x as i64).collect(),
        tag: Some(id.to_string()),
        context: None,
    };
    let work = cfg.work_root.join(crate::closeloop::ids::safe(id));
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let opts = RunOptions {
        cpus: vec![cpu],
        memory_cap_bytes: Some(caps.cap_bytes),
        licence_env: cfg.licence_env.clone(),
        wall_cap: Some(Duration::from_secs(cfg.wall_cap_s)),
        grace: Some(Duration::from_secs(30)),
    };
    let ad = SolveRecipeAdapter::new(adcfg);
    let reqs = vec![req];
    let plans = ad.plan(&reqs).map_err(|e| format!("{e:?}"))?;
    let plan = plans.first().ok_or("no plan")?;
    let mut l = RealLauncher::new(sampler);
    let (out, rep) =
        run_attempt(&mut l, &ad, &reqs, plan, &work, 1, &opts).map_err(|e| format!("{e:?}"))?;
    let run_dir = rep.prepared.spec.run_dir.clone();
    let cost = cost_of(Path::new(&run_dir), &out.job_id);
    let c = &out.classification;
    let mut record = Value::Null;
    let layout = RunLayout::new(&run_dir);
    if let Ok((_, h)) =
        verify_stream_handle(&layout.root, &RunLayout::out_rel("results.jsonl"), &["end"])
        && let Ok(lines) = h.canonical_lines()
    {
        for b in lines {
            if let Ok(v) = serde_json::from_slice::<Value>(&b)
                && v["type"] == "relation"
            {
                record = v;
            }
        }
    }
    Ok(json!({
        "id": id, "key": key, "recipe": recipe.display().to_string(), "recipe_sha256": sha256_hex(&rb),
        "row": c.row, "sub": c.sub, "why": c.why, "run_dir": run_dir,
        "accepted": out.relations.iter().map(|r| json!({"relation_digest": r.relation_digest, "rhs_terms": r.rhs.len(),
            "proof": r.proof, "relation_rule": r.relation_rule, "relation_rule_taken": r.relation_rule_taken})).collect::<Vec<_>>(),
        "failures": out.failures.iter().map(|f| json!({"class": f.class, "verdict": f.verdict, "message": f.message})).collect::<Vec<_>>(),
        "refusal": out.refusal.as_ref().map(|r| r.why.clone()),
        "record": {"verdict": record["verdict"], "relation_rule": record["relation_rule"],
                   "relation_rule_taken": record["relation_rule_taken"], "m1_steps": record["m1_steps"],
                   "m1_fallback_reason": record["m1_fallback_reason"], "rhs_count": record["rhs_count"],
                   "rhs_sub_count": record["rhs_sub_count"], "all_lower": record["all_lower"],
                   "seeds_total": record["seeds_total"], "rows": record["rows"], "trace_rows": record["trace_rows"],
                   "relation_digest": record["relation_digest"], "recipe_digest": record["recipe_digest"],
                   "determinism_id": record["determinism_id"], "proof": record["proof"],
                   "top_rhs": record["top_rhs"].as_array().map(|a| a.len()),
                   "top_all_lower": record["top_rhs"].as_array().map(|a| a.iter().all(|t| t["lower"] == true)),
                   "seconds": record["seconds"], "vmhwm_kb": record["vmhwm_kb"], "failure": record["failure"]},
        "cost": cost,
    }))
}
