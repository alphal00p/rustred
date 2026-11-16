//! Lane coreseed (interfaces/coreseed.md 4): the SUPERVISOR side of the IBP core of a face. [`ChildSolver`] implements
//! the core's [`CoreSolver`]: every request (an explicit recipe at keys) runs as solve-recipe children through the
//! adapter and the process layer (`adapters::run_attempt`: own session, PDEATHSIG, verified kill, memory cap), one job
//! per CPU, each pinned; every settled job goes through the CLOSING LOOP'S ACCEPT CHAIN (`termmin::accept_point` ->
//! `accept::decide` -> core `accept_recipe`: dispatch to the recipe, exact proof, regular lift, descent under the full
//! node comparator, bound to binary / recipe digest / key): no second acceptance. Answers are journaled per request
//! digest (resume = same requests, same answers); costs go to a separate file. Run inside a private pid namespace
//! (L10). Generic: node, paths, order, CPUs and caps are configuration.

use crate::adapters::job::{InputFile, JobAdapter, Program, RunOptions, run_attempt};
use crate::adapters::poleat::{PoleAdapter, PoleRequest};
use crate::adapters::solve_recipe::{
    BatchPolicy, RecipeInput, SolveRecipeAdapter, SolveRecipeConfig, SolveRequest as AdReq,
};
use crate::closeloop::accept::{BATCH_SCHEMA, BatchFile, Decided};
use crate::closeloop::termmin::{AcceptBits, accept_point, cost_of};
use crate::launch::RealLauncher;
use crate::resources::Sampler;
use rustred_campaign_contract::classify::recipe::Verdict;
use rustred_campaign_contract::fake::{FakeOp, FakeScript, OnSigterm, RssPlan};
use rustred_campaign_contract::layout::RunLayout;
use rustred_campaign_contract::seal::verify_stream_handle;
use rustred_campaign_core::closure::relations::DispatchSlot;
use rustred_campaign_core::coreseed::{
    CoreSolver, D0Verdict, DenominatorJudge, Key, KeyAnswer, Offset, RegularityJudge, RelationInfo,
    SolveReq,
};
use rustred_campaign_core::import::ImportedSet;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

/// A fake oracle option (test stand): the key has a relation with `rhs` iff the solved list holds every `needs` seed
/// valid at the key.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FakeOpt {
    pub needs: Vec<Offset>,
    pub rhs: Vec<Key>,
}

/// The fake child's answers as data (the script of each job is written from it).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FakeCore {
    pub opts: BTreeMap<String, Vec<FakeOpt>>,
    /// Keys answered BUDGET_WALL.
    #[serde(default)]
    pub budget: BTreeSet<String>,
    #[serde(default)]
    pub ms: u64,
}

fn key_name(k: &[i32]) -> String {
    format!("{k:?}")
}

impl FakeCore {
    fn answer(&self, list: &[Offset], key: &[i32], lines: &[bool]) -> (Verdict, Vec<Vec<i64>>) {
        if self.budget.contains(&key_name(key)) {
            return (Verdict::BudgetWall, vec![]);
        }
        let valid: BTreeSet<&Offset> = list
            .iter()
            .filter(|o| rustred_campaign_core::coreseed::seeds::valid_at(key, o, lines))
            .collect();
        for op in self.opts.get(&key_name(key)).into_iter().flatten() {
            if op.needs.iter().all(|n| valid.contains(n)) {
                let rhs = op
                    .rhs
                    .iter()
                    .map(|k| k.iter().map(|&x| x as i64).collect())
                    .collect();
                return (Verdict::Solved, rhs);
            }
        }
        (Verdict::NoRelationInSeeds, vec![])
    }
}

/// planfix (audit r2-planners ITEM 6): SITE POLICY of the solve / pole children of coreseed and seedmin (spec data;
/// documented defaults). None of these decides a result; they decide how the host's processes are run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ChildSitePolicy {
    /// Threads per solve child at most (refused above). Default 4 = the host rule "at most 4 CPU-bound threads per
    /// process" (implementer SUBAGENT_RULES 6); a site rule, not an experiment.
    pub max_threads_per_job: usize,
    /// Heartbeat interval of a solve child, seconds. Default 10. Default not yet backed by an experiment.
    pub heartbeat_s: u32,
    /// Grace between the stop request and the kill of a solve child, seconds. Default 30. Default not yet backed by an
    /// experiment.
    pub stop_grace_s: u32,
    /// Grace of a pole-regular-at child at its wall cap, seconds. Default 10. Default not yet backed by an experiment.
    pub pole_grace_s: u64,
}

impl Default for ChildSitePolicy {
    fn default() -> Self {
        ChildSitePolicy {
            max_threads_per_job: 4,
            heartbeat_s: 10,
            stop_grace_s: 30,
            pole_grace_s: 10,
        }
    }
}

/// Configuration of the child solver (all data).
pub struct CoreDriverConfig {
    pub node: String,
    pub program: Program,
    pub family: PathBuf,
    pub order: String,
    /// One job per CPU, each pinned to it (`pin`); unpinned (`pin: false`, DEV-22): the list only sets how many jobs run
    /// at once (the unit's CPU quota carries them).
    pub cpus: Vec<usize>,
    pub pin: bool,
    pub work_root: PathBuf,
    /// The answers journal (resume) and the cost / progress files go next to it.
    pub journal: PathBuf,
    /// Keys per job at most (a request is split over the CPUs).
    pub keys_per_job: usize,
    pub cap_bytes: u64,
    pub key_budget_s: Option<u64>,
    pub max_trace_rows: Option<u64>,
    /// Wall cap of one job (terminate after it: the job's keys are BUDGET, time-decided).
    pub wall_cap_s: u64,
    pub licence_env: Option<String>,
    /// d0 of the regularity verdicts.
    pub d0: String,
    /// Keep the run directories of minimisation trials (default: removed after the answer is journaled).
    pub keep_trial_outputs: bool,
    pub verbose: bool,
    /// Test stand: the fake child's answers.
    pub fake: Option<FakeCore>,
    /// CPU budget of this solver (seconds of child CPU, e.g. one face's variant pass): once spent, every new request
    /// is answered BUDGET ("cpu budget", never "no relation") without solving and is NOT journaled (a later run with a
    /// larger budget solves it). None = unbounded (the closing pass).
    pub cpu_budget_s: Option<f64>,
    /// The pole orders of the certified relations the in-process test leaves undecided: ONE `pole-regular-at` child
    /// job per solve batch (lane poles' `rr_poles regular-at`, many pairs per invocation; interfaces/coreseed.md 10).
    /// None = the in-process sound test only (`regular` or `undecided`).
    pub poles: Option<PolesCfg>,
    /// planfix: the children's site policy (heartbeat, stop grace, pole-child grace).
    pub site: ChildSitePolicy,
}

/// The diagnostic child of the pole orders (program `rr_poles`, `Invocation::Legacy`; or the fake child with a script).
#[derive(Debug, Clone)]
pub struct PolesCfg {
    pub program: Program,
    /// Threads of the child (rr_poles: 1..4).
    pub threads: u32,
    pub cap_bytes: u64,
    pub wall_cap_s: u64,
    /// Test stand: the fake child's script for kind pole-regular-at.
    pub fake: Option<FakeScript>,
}

/// The chart mask of the node (from the import set's frame).
fn lines_of(set: &ImportedSet) -> Vec<bool> {
    set.frame.mask().to_vec()
}

struct Job {
    req: usize,
    keys: Vec<Key>,
    recipe_path: PathBuf,
    recipe_sha256: String,
    label: String,
    work: PathBuf,
}

/// What a worker thread needs to run one job: adapter config, recipe path, keys, recipe sha256, label, work dir.
type JobInput = (
    SolveRecipeConfig,
    PathBuf,
    Vec<Key>,
    String,
    String,
    PathBuf,
);

struct JobDone {
    job: usize,
    result: Result<JobOut, String>,
}

struct JobOut {
    results_path: String,
    results_b3: String,
    lines: Vec<String>,
    run_dir: String,
    cpu_s: Option<f64>,
    vmhwm_kb: Option<u64>,
    /// Not decided by the contract: (time decided, why).
    undecided: Option<(bool, String)>,
}

fn sha256_hex(b: &[u8]) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

/// The regularity judge of the driver: the core's exact in-process denominator test (a `regular` verdict is sound; else
/// `undecided`). The supervisor starts NO process of its own here: every child goes through the process layer as a
/// job. Exact pole orders (lane poles' `rr_poles regular-at`) are a diagnostic job outside this solver (REQUEST: a
/// contract job kind `pole-regular-at`, interfaces/coreseed.md 10); an undecided verdict never blocks a closing relation.
pub struct DriverJudge;

impl RegularityJudge for DriverJudge {
    fn is_regular_at(&self, record: &Value, key: &[i32], d0: &str) -> D0Verdict {
        DenominatorJudge.is_regular_at(record, key, d0)
    }
}

/// The child solver.
pub struct ChildSolver<'a> {
    pub cfg: CoreDriverConfig,
    set: &'a mut ImportedSet,
    bits: AcceptBits,
    judge: Box<dyn RegularityJudge + 'a>,
    journal: BTreeMap<String, Vec<KeyAnswer>>,
    journal_out: std::fs::File,
    sampler: Arc<Sampler>,
    session: String,
    seq: u64,
    lines: Vec<bool>,
    pub requests_solved: u64,
    pub requests_journaled: u64,
    /// Child CPU seconds spent by this solver, and the requests answered by the CPU budget (not tried).
    pub cpu_used_s: f64,
    pub not_tried: Vec<String>,
}

impl<'a> ChildSolver<'a> {
    pub fn new(
        cfg: CoreDriverConfig,
        set: &'a mut ImportedSet,
        judge: Box<dyn RegularityJudge + 'a>,
        sampler: Arc<Sampler>,
    ) -> Result<ChildSolver<'a>, String> {
        std::fs::create_dir_all(&cfg.work_root).map_err(|e| e.to_string())?;
        if let Some(d) = cfg.journal.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        let mut journal = BTreeMap::new();
        if let Ok(f) = std::fs::File::open(&cfg.journal) {
            for l in std::io::BufReader::new(f).lines().map_while(Result::ok) {
                // a torn last line (a crash while writing) is ignored: its request is solved again
                if let Ok(v) = serde_json::from_str::<Value>(&l)
                    && let (Some(d), Ok(a)) = (
                        v["digest"].as_str(),
                        serde_json::from_value::<Vec<KeyAnswer>>(v["answers"].clone()),
                    )
                {
                    // an entry with a transient answer (e.g. an interrupted job of a stopped run, journaled before this
                    // rule existed) is solved again
                    if !a.iter().any(is_transient) {
                        journal.insert(d.to_string(), a);
                    }
                }
            }
        }
        let journal_out = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&cfg.journal)
            .map_err(|e| format!("{}: {e}", cfg.journal.display()))?;
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
            max_trace_rows: cfg.max_trace_rows,
            key_budget_s: cfg.key_budget_s,
            pole_objective: Some(rustred_campaign_core::poles::PoleObjective::default()),
            pole_profiles: false,
        };
        let session = format!(
            "s{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        );
        let lines = lines_of(set);
        Ok(ChildSolver {
            cfg,
            set,
            bits,
            judge,
            requests_journaled: journal.len() as u64,
            journal,
            journal_out,
            sampler,
            session,
            seq: 0,
            lines,
            requests_solved: 0,
            cpu_used_s: 0.0,
            not_tried: vec![],
        })
    }

    fn side_file(&self, name: &str) -> PathBuf {
        self.cfg
            .journal
            .parent()
            .unwrap_or(Path::new("."))
            .join(name)
    }

    fn append(&self, name: &str, v: &Value) {
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.side_file(name))
        {
            let _ = writeln!(f, "{v}");
        }
    }

    fn say(&self, m: &str) {
        if self.cfg.verbose {
            eprintln!("[coreseed {}] {m}", utc_now());
        }
    }

    fn fake_script(&self, req: &SolveReq, keys: &[Key]) -> Option<FakeScript> {
        let f = self.cfg.fake.as_ref()?;
        let list: Vec<Offset> =
            serde_json::from_value(req.recipe["seeds"]["centres"].clone()).unwrap_or_default();
        let mut ops = vec![FakeOp::Prepare { ms: 5 }];
        for k in keys {
            let (verdict, rhs) = f.answer(&list, k, &self.lines);
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

    fn adapter_config(
        &self,
        fake: Option<FakeScript>,
        keys: usize,
    ) -> Result<SolveRecipeConfig, String> {
        Ok(SolveRecipeConfig {
            program: self.cfg.program.clone(),
            family: InputFile::from_file(&self.cfg.family).map_err(|e| format!("{e:?}"))?,
            order: self.cfg.order.clone(),
            batch: BatchPolicy {
                max_keys_per_job: keys.max(1),
                max_threads: 1,
                min_keys_per_thread: 1,
            },
            max_trace_rows: self.cfg.max_trace_rows,
            key_budget_s: self.cfg.key_budget_s,
            expect: None,
            determinism_id: None,
            heartbeat_s: self.cfg.site.heartbeat_s,
            stop_grace_s: self.cfg.site.stop_grace_s,
            package: None,
            live_repair: false,
            licence_env: self.cfg.licence_env.clone(),
            fake,
        })
    }

    /// The per-key answers of one settled job from the accept chain's decisions.
    fn answers_of(&self, d: &Decided, keys: &[Key]) -> Vec<KeyAnswer> {
        let recs: BTreeMap<&Vec<i32>, &Value> = d.records.iter().map(|(k, v)| (k, v)).collect();
        let failed: BTreeMap<&Vec<i32>, &String> = d.failed.iter().map(|(k, c)| (k, c)).collect();
        keys.iter()
            .map(|k| {
                if let Some(rec) = recs.get(k) {
                    return KeyAnswer::Relation(Box::new(self.relation_info(rec, k)));
                }
                match failed.get(k).map(|c| c.as_str()) {
                    Some("no_relation_in_seeds") => KeyAnswer::NoRelation,
                    Some("not_in_domain") => KeyAnswer::NotInDomain,
                    Some(c @ ("budget_work" | "budget_wall")) => {
                        KeyAnswer::Budget { what: c.into() }
                    }
                    Some(c) => KeyAnswer::Failed {
                        class: c.into(),
                        message: d.refused.clone().unwrap_or_default(),
                    },
                    None => KeyAnswer::Failed {
                        class: "missing".into(),
                        message: d.refused.clone().unwrap_or_else(|| "no decision".into()),
                    },
                }
            })
            .collect()
    }

    /// ONE `pole-regular-at` child job for the (record, key) pairs, through the process layer and the contract
    /// (rows 37-39); returns per pair the verdict of the job (None = not read: the in-process verdict stays). A
    /// host-resource end (row 15b) is retried once. Never an error of the solve: a failed diagnostic is recorded.
    fn pole_job(&mut self, pairs: &[(usize, Key, Value)]) -> Vec<Option<D0Verdict>> {
        let none = || vec![None; pairs.len()];
        let Some(pc) = self.cfg.poles.clone() else {
            return none();
        };
        let reqs: Vec<PoleRequest> = pairs
            .iter()
            .enumerate()
            .map(|(i, (_, k, rec))| PoleRequest {
                id: format!("p{i}"),
                key: k.clone(),
                record: rec.clone(),
            })
            .collect();
        let ad = PoleAdapter {
            program: pc.program.clone(),
            d0: self.cfg.d0.clone(),
            threads: pc.threads,
            licence_env: self.cfg.licence_env.clone(),
            fake: pc.fake.clone(),
        };
        self.seq += 1;
        let work = self
            .cfg
            .work_root
            .join(&self.session)
            .join(format!("{:06}-poles", self.seq));
        let opts = RunOptions {
            cpus: if self.cfg.pin {
                self.cfg.cpus.first().copied().into_iter().collect()
            } else {
                vec![]
            },
            memory_cap_bytes: Some(pc.cap_bytes),
            licence_env: self.cfg.licence_env.clone(),
            wall_cap: Some(Duration::from_secs(pc.wall_cap_s)),
            grace: Some(Duration::from_secs(self.cfg.site.pole_grace_s)),
        };
        let note = |m: String| {
            self.append(
                "poles.jsonl",
                &json!({"pairs": pairs.len(), "note": m, "utc": utc_now()}),
            );
        };
        let plans = match ad.plan(&reqs) {
            Ok(p) => p,
            Err(e) => {
                note(format!("plan: {e:?}"));
                return none();
            }
        };
        let Some(plan) = plans.first() else {
            return none();
        };
        for attempt in 1..=2u32 {
            let mut l = RealLauncher::new(self.sampler.clone());
            match run_attempt(&mut l, &ad, &reqs, plan, &work, attempt, &opts) {
                Ok((b, _)) if b.unread.is_none() => {
                    let by_id: BTreeMap<
                        String,
                        &rustred_campaign_contract::classify::poleat::PoleAnswer,
                    > = b
                        .answers
                        .iter()
                        .filter_map(|a| a.id.as_str().map(|s| (s.to_string(), a)))
                        .collect();
                    note(format!(
                        "row {} job {}: {} answers",
                        b.classification.row,
                        b.job_id,
                        b.answers.len()
                    ));
                    return reqs
                        .iter()
                        .map(|q| {
                            by_id
                                .get(&q.id)
                                .map(|a| match (a.verdict.as_str(), a.pole_order) {
                                    ("regular", _) => D0Verdict::Regular,
                                    ("pole", p) => D0Verdict::Singular {
                                        order: p.map(|x| x as u32),
                                    },
                                    _ => D0Verdict::Undecided,
                                })
                        })
                        .collect();
                }
                Ok((b, _)) => {
                    let retry = b.classification.row == 15 && b.classification.sub == "b";
                    note(format!(
                        "attempt {attempt}: nothing read ({}){}",
                        b.unread.unwrap_or_default(),
                        if retry {
                            ", host resource: retried"
                        } else {
                            ""
                        }
                    ));
                    if !retry {
                        return none();
                    }
                }
                Err(e) => {
                    note(format!("attempt {attempt}: {e:?}"));
                    return none();
                }
            }
        }
        none()
    }

    fn relation_info(&self, rec: &Value, key: &[i32]) -> RelationInfo {
        let rhs: Vec<Key> = rec["rhs"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|t| {
                t[0].as_str()
                    .and_then(rustred_campaign_core::assemble::parse_key_text)
            })
            .collect();
        let line =
            rustred_campaign_core::schema::canon::canonical_json(rec, false).unwrap_or_default();
        RelationInfo {
            relation_digest: rec["relation_digest"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            rhs_count: rec["rhs_count"].as_u64().unwrap_or(rhs.len() as u64),
            rhs,
            seeds_total: rec["seeds_total"].as_u64(),
            seeds_used: serde_json::from_value(rec["seeds_used"].clone()).ok(),
            d0: self.judge.is_regular_at(rec, key, &self.cfg.d0),
            recipe_digest: rec["recipe_digest"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            determinism_id: rec["determinism_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            relation_rule_taken: rec["relation_rule_taken"].as_str().map(str::to_string),
            minimise: (!rec["minimise"].is_null()).then(|| rec["minimise"].clone()),
            record_sha256: sha256_hex(line.as_bytes()),
        }
    }
}

fn utc_now() -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("t={t}")
}

/// A failed answer that a later run must solve again (never journaled, never replayed): no answer, a job that did not
/// run, an interrupted job (also in the old form `job_refused` with an `(Interrupted)` classification).
fn is_transient(a: &KeyAnswer) -> bool {
    matches!(a, KeyAnswer::Failed { class, message }
        if class == "missing" || class == "job_error" || class == "interrupted"
            || (class == "job_refused" && message.contains("(Interrupted)")))
}

/// A minimisation trial request (its outputs are not kept): labels of the minimiser's trials.
fn is_trial(label: &str) -> bool {
    label.contains("/min") && !label.ends_with("/domain")
}

fn run_job(
    cfg_ad: SolveRecipeConfig,
    job: &Job,
    cpu: Option<usize>,
    cap_bytes: u64,
    wall_cap_s: u64,
    licence_env: Option<String>,
    sampler: Arc<Sampler>,
) -> Result<JobOut, String> {
    std::fs::create_dir_all(&job.work).map_err(|e| e.to_string())?;
    let reqs: Vec<AdReq> = job
        .keys
        .iter()
        .map(|k| {
            Ok(AdReq {
                recipe: RecipeInput {
                    id: job.label.clone(),
                    file: InputFile::from_file(&job.recipe_path).map_err(|e| format!("{e:?}"))?,
                    digest: None,
                    sha256: Some(job.recipe_sha256.clone()),
                },
                key: k.iter().map(|&x| x as i64).collect(),
                tag: None,
                context: None,
            })
        })
        .collect::<Result<_, String>>()?;
    let ad = SolveRecipeAdapter::new(cfg_ad);
    let plans = ad.plan(&reqs).map_err(|e| format!("{e:?}"))?;
    let [plan] = plans.as_slice() else {
        return Err(format!("{} job plans for one chunk", plans.len()));
    };
    let opts = RunOptions {
        cpus: cpu.into_iter().collect(),
        memory_cap_bytes: Some(cap_bytes),
        licence_env,
        wall_cap: Some(Duration::from_secs(wall_cap_s)),
        grace: Some(Duration::from_secs(30)),
    };
    let mut l = RealLauncher::new(sampler);
    let (out, rep) =
        run_attempt(&mut l, &ad, &reqs, plan, &job.work, 1, &opts).map_err(|e| format!("{e:?}"))?;
    let run_dir = rep.prepared.spec.run_dir.clone();
    let cost = cost_of(Path::new(&run_dir), &out.job_id);
    let c = &out.classification;
    let decided = matches!((c.row, c.sub.as_str()), (1, "") | (28, "") | (29, ""));
    if !decided {
        return Ok(JobOut {
            results_path: String::new(),
            results_b3: String::new(),
            lines: vec![],
            run_dir,
            cpu_s: cost.cpu_s,
            vmhwm_kb: cost.vmhwm_kb,
            undecided: Some((
                c.time_decided,
                format!(
                    "row {}{} ({:?}): {}; refusal {:?}",
                    c.row,
                    c.sub,
                    c.kind,
                    c.why,
                    out.refusal.as_ref().map(|r| &r.why)
                ),
            )),
        });
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
    let b3 = crate::atomic::blake3_file(&p)
        .map(|x| x.1)
        .unwrap_or_default();
    Ok(JobOut {
        results_path: p.display().to_string(),
        results_b3: b3,
        lines,
        run_dir,
        cpu_s: cost.cpu_s,
        vmhwm_kb: cost.vmhwm_kb,
        undecided: None,
    })
}

impl CoreSolver for ChildSolver<'_> {
    fn solve(&mut self, batch: &[SolveReq]) -> Result<Vec<Vec<KeyAnswer>>, String> {
        let mut out: Vec<Option<Vec<KeyAnswer>>> = vec![None; batch.len()];
        let mut jobs: Vec<Job> = vec![];
        let ncpu = self.cfg.cpus.len().max(1);
        for (ri, r) in batch.iter().enumerate() {
            let d = r.digest();
            if let Some(a) = self.journal.get(&d) {
                out[ri] = Some(a.clone());
                continue;
            }
            if self.cfg.cpu_budget_s.is_some_and(|b| self.cpu_used_s >= b) {
                self.not_tried.push(r.label.clone());
                out[ri] = Some(
                    r.keys
                        .iter()
                        .map(|_| KeyAnswer::Budget {
                            what: "cpu budget of the pass (not tried)".into(),
                        })
                        .collect(),
                );
                continue;
            }
            self.seq += 1;
            let rdir = self.cfg.work_root.join(&self.session).join(format!(
                "{:06}-{}",
                self.seq,
                crate::closeloop::ids::safe(&r.label)
            ));
            std::fs::create_dir_all(&rdir).map_err(|e| e.to_string())?;
            let rpath = rdir.join("recipe.json");
            std::fs::write(&rpath, r.text.as_bytes()).map_err(|e| e.to_string())?;
            let per = r
                .keys
                .len()
                .div_ceil(ncpu)
                .clamp(1, self.cfg.keys_per_job.max(1));
            for (ci, chunk) in r.keys.chunks(per).enumerate() {
                jobs.push(Job {
                    req: ri,
                    keys: chunk.to_vec(),
                    recipe_path: rpath.clone(),
                    recipe_sha256: sha256_hex(r.text.as_bytes()),
                    label: r.label.clone(),
                    work: rdir.join(format!("c{ci}")),
                });
            }
        }
        // run the jobs on the CPU pool (one worker thread per CPU, each pinned; jobs in order)
        let queue: Arc<Mutex<VecDeque<usize>>> = Arc::new(Mutex::new((0..jobs.len()).collect()));
        let (tx, rx) = mpsc::channel::<JobDone>();
        let t0 = Instant::now();
        let shared: Arc<Vec<JobInput>> = Arc::new(
            jobs.iter()
                .map(|j| {
                    let fake = self.fake_script(&batch[j.req], &j.keys);
                    Ok((
                        self.adapter_config(fake, j.keys.len())?,
                        j.recipe_path.clone(),
                        j.keys.clone(),
                        j.recipe_sha256.clone(),
                        j.label.clone(),
                        j.work.clone(),
                    ))
                })
                .collect::<Result<_, String>>()?,
        );
        let mut handles = vec![];
        for &cpu in self.cfg.cpus.iter().take(jobs.len().max(1)) {
            let (q, tx, sh, sampler) = (
                queue.clone(),
                tx.clone(),
                shared.clone(),
                self.sampler.clone(),
            );
            let pin = self.cfg.pin.then_some(cpu);
            let (cap, wall, lic) = (
                self.cfg.cap_bytes,
                self.cfg.wall_cap_s,
                self.cfg.licence_env.clone(),
            );
            handles.push(
                std::thread::Builder::new()
                    .name(format!("coreseed-cpu{cpu}"))
                    .spawn(move || {
                        loop {
                            let next = q.lock().ok().and_then(|mut g| g.pop_front());
                            let Some(i) = next else { break };
                            let (c, rp, keys, sha, label, work) = sh[i].clone();
                            let job = Job {
                                req: 0,
                                keys,
                                recipe_path: rp,
                                recipe_sha256: sha,
                                label,
                                work,
                            };
                            let result =
                                run_job(c, &job, pin, cap, wall, lic.clone(), sampler.clone());
                            if tx.send(JobDone { job: i, result }).is_err() {
                                break;
                            }
                        }
                    })
                    .map_err(|e| format!("worker thread: {e}"))?,
            );
        }
        drop(tx);
        let mut done: Vec<Option<Result<JobOut, String>>> = (0..jobs.len()).map(|_| None).collect();
        for d in rx.iter() {
            done[d.job] = Some(d.result);
        }
        for h in handles {
            let _ = h.join();
        }
        // acceptance per job (the accept chain needs the import set: sequential), answers per request
        let mut per_req: BTreeMap<usize, BTreeMap<Key, KeyAnswer>> = BTreeMap::new();
        let mut pole_needed: Vec<(usize, Key, Value)> = vec![];
        let mut cost: BTreeMap<usize, (f64, u64, usize)> = BTreeMap::new();
        for (ji, j) in jobs.iter().enumerate() {
            let r = &batch[j.req];
            let res = done[ji].take().unwrap_or_else(|| Err("job not run".into()));
            let answers: Vec<KeyAnswer> = match res {
                Err(e) => j
                    .keys
                    .iter()
                    .map(|_| KeyAnswer::Failed {
                        class: "job_error".into(),
                        message: e.clone(),
                    })
                    .collect(),
                Ok(o) => {
                    let e = cost.entry(j.req).or_default();
                    e.0 += o.cpu_s.unwrap_or(0.0);
                    e.1 = e.1.max(o.vmhwm_kb.unwrap_or(0));
                    e.2 += 1;
                    if let Some((timed, why)) = &o.undecided {
                        j.keys
                            .iter()
                            .map(|_| {
                                if *timed {
                                    KeyAnswer::Budget {
                                        what: format!("job wall cap: {why}"),
                                    }
                                } else {
                                    KeyAnswer::Failed {
                                        // an interrupted job (the run was stopped) is transient: solved again
                                        class: if why.contains("(Interrupted)") {
                                            "interrupted".into()
                                        } else {
                                            "job_refused".into()
                                        },
                                        message: why.clone(),
                                    }
                                }
                            })
                            .collect()
                    } else {
                        let rb = std::fs::read(&j.recipe_path).map_err(|e| e.to_string())?;
                        let b = BatchFile {
                            schema: BATCH_SCHEMA.into(),
                            node: self.cfg.node.clone(),
                            item: r.label.clone(),
                            round: 0,
                            source: r.label.clone(),
                            recipe_path: j.recipe_path.display().to_string(),
                            recipe_sha256: j.recipe_sha256.clone(),
                            recipe_blake3: blake3::hash(&rb).to_hex().to_string(),
                            engine_digest: None,
                            relation_rule: r.relation_rule.clone(),
                            keys: j.keys.clone(),
                        };
                        let d = accept_point(
                            self.set,
                            None,
                            &b,
                            &DispatchSlot::Last,
                            &o.lines,
                            &self.bits,
                            (&o.results_path, &o.results_b3),
                            &o.run_dir,
                            None,
                        );
                        match d {
                            Ok(d) => {
                                // the accept chain's solved-relation lines (import 6.2): the proof references
                                // of every decided key, next to the sealed results
                                if !is_trial(&r.label) {
                                    let mut t = d.lines.join("\n");
                                    t.push('\n');
                                    let _ = std::fs::write(j.work.join("solved.jsonl"), t);
                                }
                                // relations the in-process test leaves undecided: to the pole job of the batch
                                for (k, rec) in &d.records {
                                    if !self.judge.is_regular_at(rec, k, &self.cfg.d0).is_regular()
                                    {
                                        pole_needed.push((j.req, k.clone(), rec.clone()));
                                    }
                                }
                                self.answers_of(&d, &j.keys)
                            }
                            Err(e) => j
                                .keys
                                .iter()
                                .map(|_| KeyAnswer::Failed {
                                    class: "refused".into(),
                                    message: e.clone(),
                                })
                                .collect(),
                        }
                    }
                }
            };
            let m = per_req.entry(j.req).or_default();
            for (k, a) in j.keys.iter().zip(answers) {
                m.insert(k.clone(), a);
            }
            if is_trial(&r.label) && !self.cfg.keep_trial_outputs {
                let _ = std::fs::remove_dir_all(&j.work);
            }
        }
        // the pole orders of the batch's undecided relations (one diagnostic child job; never blocks a relation)
        if !pole_needed.is_empty() {
            let verdicts = self.pole_job(&pole_needed);
            for ((ri, k, _), v) in pole_needed.iter().zip(verdicts) {
                if let Some(KeyAnswer::Relation(info)) =
                    per_req.get_mut(ri).and_then(|m| m.get_mut(k))
                    && let Some(v) = v
                {
                    info.d0 = v;
                }
            }
        }
        for (ri, m) in per_req {
            let r = &batch[ri];
            let a: Vec<KeyAnswer> = r
                .keys
                .iter()
                .map(|k| {
                    m.get(k).cloned().unwrap_or(KeyAnswer::Failed {
                        class: "missing".into(),
                        message: "no answer".into(),
                    })
                })
                .collect();
            // transient failures are not journaled (a resumed run solves the request again)
            let transient = a.iter().any(is_transient);
            let d = r.digest();
            if !transient {
                let line = json!({"digest": d, "label": r.label, "keys": r.keys, "answers": a});
                writeln!(self.journal_out, "{line}").map_err(|e| e.to_string())?;
                self.journal_out.flush().map_err(|e| e.to_string())?;
                self.journal.insert(d.clone(), a.clone());
            }
            let (cpu_s, kb, njobs) = cost.get(&ri).copied().unwrap_or_default();
            let counts: BTreeMap<&str, usize> = a.iter().fold(BTreeMap::new(), |mut c, x| {
                *c.entry(x.word()).or_default() += 1;
                c
            });
            self.append(
                "costs.jsonl",
                &json!({"label": r.label, "digest": d, "keys": r.keys.len(), "jobs": njobs, "cpu_s": cpu_s,
                        "vmhwm_kb_max": kb, "counts": counts, "batch_wall_s": t0.elapsed().as_secs_f64(),
                        "utc": utc_now()}),
            );
            self.say(&format!(
                "{} keys {} {:?} cpu {:.1} s",
                r.label,
                r.keys.len(),
                counts,
                cpu_s
            ));
            self.requests_solved += 1;
            self.cpu_used_s += cpu_s;
            out[ri] = Some(a);
        }
        out.into_iter()
            .map(|x| x.ok_or_else(|| "request without answers".to_string()))
            .collect()
    }

    fn progress(&mut self, phase: &str, partial: &Value) {
        let v = json!({"phase": phase, "partial": partial, "utc": utc_now(),
                       "requests_solved": self.requests_solved});
        self.append("progress.jsonl", &v);
        self.say(&format!("phase {phase}: {partial}"));
    }

    /// Lane polefix (audit r2-poles F2): the close-first report is written atomically next to the journal
    /// (`core.close_first.json`) the moment the core hands it over, before any variant or list request: an interrupted
    /// or budgeted later phase loses nothing of it (a restart replays the journal and writes the same bytes).
    fn publish(&mut self, report: &rustred_campaign_core::coreseed::CoreReport) {
        let path = self.side_file("core.close_first.json");
        match crate::atomic::write_json_atomic(&path, report) {
            Ok(()) => self.say(&format!("close-first report published: {}", path.display())),
            Err(e) => {
                let v = json!({"publish_error": e.to_string(), "utc": utc_now()});
                self.append("progress.jsonl", &v);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interrupted_and_missing_answers_are_transient_decided_ones_are_not() {
        let f = |class: &str, message: &str| KeyAnswer::Failed {
            class: class.into(),
            message: message.into(),
        };
        assert!(is_transient(&f("interrupted", "")));
        assert!(is_transient(&f("missing", "no answer")));
        assert!(is_transient(&f("job_error", "spawn")));
        assert!(is_transient(&f(
            "job_refused",
            "row 11 (Interrupted): interrupted; refusal None"
        )));
        assert!(!is_transient(&f("job_refused", "row 40 (Refused): usage")));
        assert!(!is_transient(&f("refused", "not lower")));
        assert!(!is_transient(&KeyAnswer::NoRelation));
        assert!(!is_transient(&KeyAnswer::Budget {
            what: "wall".into()
        }));
    }
}
