//! Supervisor side of lane seedmin (interfaces/seedmin.md 8): [`ChildTrialSolver`] answers the minimiser's trials with
//! child jobs of the EXISTING kind `solve-recipe` (no new job kind): one explicit `rustred.recipe.v1` per trial (the
//! seeds `centre + o` as centres, shell S0, no floor, sector filter on, the caller's domain template and relation rule),
//! through the adapter and the process layer (own session, PDEATHSIG, verified kill, memory cap), each child pinned to
//! its CPUs, at most `threads_per_job` (<= 4) threads per child. Every settled trial is appended to `journal.jsonl`
//! (digest -> answers): a restarted run replays it (resume). Run directories are removed after settling unless
//! `keep_run_dirs`. A relation is answered `Relation` only when the adapter ACCEPTED its record (contract checks, SOLVED,
//! exact proof ok, bound to the request); its pole order comes from the configured [`RelationPoles`] judge BEFORE the
//! answer is journaled. Fake mode: a data oracle writes the fake child's script per job. Run inside a private pid
//! namespace (L10). Generic: node, paths, order, chart and CPUs are configuration.

use crate::adapters::job::{InputFile, JobAdapter, Program, RunOptions, run_attempt};
use crate::adapters::poleat::{PoleAdapter, PoleRequest};
use crate::adapters::solve_recipe::{
    BatchPolicy, RecipeInput, SolveRecipeAdapter, SolveRecipeConfig, SolveRequest as AdReq,
};
use crate::launch::RealLauncher;
use crate::resources::Sampler;
use rustred_campaign_contract::classify::recipe::Verdict;
use rustred_campaign_contract::fake::{FakeOp, FakeScript, OnSigterm, RssPlan};
use rustred_campaign_contract::layout::RunLayout;
use rustred_campaign_contract::seal::verify_stream_handle;
use rustred_campaign_core::seedmin::journal::JournalLine;
use rustred_campaign_core::seedmin::{
    EngineCaps, Key, Offset, PoleOrder, RelationPoles, Trial, TrialAnswer, TrialMode, TrialSolver,
    facts_from_record, sha256_hex,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Fake mode: per key the alternatives (the seeds needed, as offsets FROM THE KEY, and the rhs keys); a key solves
/// with a list iff the list contains one alternative's seeds (the first in order wins).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FakeOracle {
    pub alts: BTreeMap<Key, Vec<FakeAlt>>,
    pub ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FakeAlt {
    /// Seeds needed, as offsets FROM THE KEY.
    pub needs: Vec<Offset>,
    pub rhs: Vec<Vec<i64>>,
}

impl FakeOracle {
    fn answer(&self, key_offsets: &[Offset], k: &Key) -> (Verdict, Vec<Vec<i64>>) {
        let have: BTreeSet<&Offset> = key_offsets.iter().collect();
        match self.alts.get(k) {
            None => (Verdict::NotInDomain, vec![]),
            Some(v) => v
                .iter()
                .find(|a| a.needs.iter().all(|o| have.contains(o)))
                .map(|a| (Verdict::Solved, a.rhs.clone()))
                .unwrap_or((Verdict::NoRelationInSeeds, vec![])),
        }
    }

    fn script(&self, t: &Trial) -> FakeScript {
        let offs = t.key_offsets();
        let mut ops = vec![FakeOp::Prepare { ms: 5 }];
        for k in &t.keys {
            let (verdict, rhs) = self.answer(&offs, k);
            ops.push(FakeOp::Relation {
                ms: self.ms,
                verdict,
                rhs,
            });
        }
        ops.push(FakeOp::End {
            stop: None,
            verdict: None,
        });
        FakeScript {
            script: ops,
            on_sigterm: OnSigterm::Graceful,
            slow_ms: 0,
            start_delay_ms: 0,
            rss: RssPlan::default(),
            helpers: vec![],
            faults: vec![],
            time_scale: 1.0,
            volatile_jitter: false,
        }
    }
}

/// What the child solver needs (all data).
pub struct ChildConfig {
    pub program: Program,
    pub family: PathBuf,
    /// The node order exactly as `solve-recipe --order` takes it (name or NodeOrder JSON).
    pub order: String,
    /// The chart mask (true = line), presented frame.
    pub lines: Vec<bool>,
    /// The recipe `domain` template (default: every slot free).
    pub domain: Option<Value>,
    /// `first_admissible` (comparator) | `m1_first`.
    pub relation_rule: String,
    /// CPUs held by the caller (one child per CPU group).
    pub cpus: Vec<usize>,
    /// Pin each child to its CPUs (false: the CPUs are only a pool size; fake tests).
    pub pin_cpus: bool,
    /// Threads (= CPUs) per child, <= 4.
    pub threads_per_job: usize,
    pub cap_bytes: u64,
    pub key_budget_s: Option<u64>,
    pub max_trace_rows: Option<u64>,
    pub wall_cap_s: u64,
    pub work_root: PathBuf,
    pub licence_env: Option<String>,
    pub keep_run_dirs: bool,
    /// d0 of the pole judge.
    pub d0: String,
    /// Append every NEW accepted relation record here (dedup by digest; for cross-checks), up to `records_cap_bytes`.
    pub records_out: Option<PathBuf>,
    pub records_cap_bytes: u64,
    /// Fake children (tests): the oracle writes the script per job.
    pub fake: Option<FakeOracle>,
    /// Pole orders the in-process judge leaves UNDECIDED: ONE child job of the contract kind `pole-regular-at` (lane
    /// coreseed's PolesCfg: program rr_poles, Invocation::Legacy) per batch, through the process layer; None = they stay
    /// undecided (the policy's on_undecided_regularity decides).
    pub poles: Option<crate::closeloop::coreseed::PolesCfg>,
    pub verbose: bool,
}

/// Cost counters (separate from every record).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Stats {
    pub trials: u64,
    pub journal_hits: u64,
    pub jobs: u64,
    pub keys_solved: u64,
    pub cpu_s: f64,
    pub wall_s: f64,
    pub max_vmhwm_kb: u64,
    pub records_written: u64,
    pub records_bytes: u64,
    pub pole_jobs: u64,
}

/// The child solver.
pub struct ChildTrialSolver {
    cfg: ChildConfig,
    sampler: Arc<Sampler>,
    judge: Box<dyn RelationPoles>,
    caps: EngineCaps,
    journal: BTreeMap<String, Vec<TrialAnswer>>,
    journal_path: PathBuf,
    seen_records: BTreeSet<String>,
    pub stats: Stats,
}

enum Raw {
    Record { record: Value, accepted: bool },
    Missing(String),
}

struct Done {
    idx: usize,
    result: Result<Vec<Raw>, String>,
    cpu_s: f64,
    wall_s: f64,
    vmhwm_kb: u64,
}

/// Remove a run directory THIS solver created: only a direct child of `<work_root>/runs/` named by a trial digest prefix
/// (24 hex); anything else is left alone (USER, L18: scoped removals).
fn remove_own_run_dir(work_root: &Path, dir: &Path) {
    let runs = work_root.join("runs");
    let ok = dir.parent() == Some(runs.as_path())
        && dir
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.len() == 24 && n.bytes().all(|b| b.is_ascii_hexdigit()));
    if ok {
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// A decided answer (journaled and replayed); undecided ones (interrupted, refused, budget, error) are re-run.
fn decided(a: &TrialAnswer) -> bool {
    !matches!(a, TrialAnswer::Undecided { .. })
}

/// The engine's capabilities from its MANIFEST: a `version.json` file with the `features` list that `rr_node version
/// --json` prints (every frozen engine directory carries one). No process is started here (every child of the
/// supervisor goes through the ONE launch path); a missing or unreadable manifest gives no capability (typed refusals
/// downstream, never a guess).
pub fn engine_caps_from_manifest(manifest: &Path) -> EngineCaps {
    let features: Vec<String> = std::fs::read(manifest)
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| v["features"].as_array().cloned())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    EngineCaps::from_features(&features)
}

impl ChildTrialSolver {
    /// A solver; loads `work_root/journal.jsonl` when it exists (resume). `caps` = the engine's capabilities (see
    /// [`engine_caps_from_manifest`]; fake mode: none).
    pub fn new(
        cfg: ChildConfig,
        sampler: Arc<Sampler>,
        judge: Box<dyn RelationPoles>,
        caps: EngineCaps,
    ) -> Result<ChildTrialSolver, String> {
        if cfg.threads_per_job == 0 || cfg.threads_per_job > 4 {
            return Err("threads_per_job must be 1..=4 (rule: <= 4 threads per process)".into());
        }
        if cfg.cpus.is_empty() {
            return Err("no CPUs".into());
        }
        std::fs::create_dir_all(cfg.work_root.join("recipes")).map_err(|e| e.to_string())?;
        let journal_path = cfg.work_root.join("journal.jsonl");
        let mut journal = BTreeMap::new();
        if let Ok(t) = std::fs::read_to_string(&journal_path) {
            for (i, l) in t.lines().enumerate() {
                if l.trim().is_empty() {
                    continue;
                }
                let j: JournalLine =
                    serde_json::from_str(l).map_err(|e| format!("journal line {}: {e}", i + 1))?;
                // an undecided answer (interrupted, refused, budget) is never replayed: the trial runs again
                if j.answers.iter().all(decided) {
                    journal.insert(j.digest, j.answers);
                }
            }
        }
        Ok(ChildTrialSolver {
            cfg,
            sampler,
            judge,
            caps,
            journal,
            journal_path,
            seen_records: BTreeSet::new(),
            stats: Stats::default(),
        })
    }

    pub fn journal_len(&self) -> usize {
        self.journal.len()
    }

    fn say(&self, m: String) {
        if self.cfg.verbose {
            let t = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            eprintln!("[seedmin t={t}] {m}");
        }
    }

    /// The explicit recipe of a trial (canonical text + "\n").
    pub fn recipe_text(&self, t: &Trial) -> Result<String, String> {
        let n = self.cfg.lines.len();
        let chart: String = self
            .cfg
            .lines
            .iter()
            .map(|&l| if l { '1' } else { '0' })
            .collect();
        let domain = self.cfg.domain.clone().unwrap_or_else(
            || json!({"fixed": vec![Value::Null; n], "equations": [], "exceptions": []}),
        );
        let mut elim = json!({"convention": "rustred.recipe-elim.v1"});
        if let Some(m) = &t.engine_minimise {
            elim["minimise"] = m.clone();
        }
        let doc = json!({"schema": "rustred.recipe.v1", "id": t.label, "arity": n, "chart": chart,
            "domain": domain,
            "seeds": {"unit": "harder", "centres": t.key_offsets(), "generator": {"shells": ["S0"]},
                      "sector_filter": true},
            "symmetry_rows": false, "relation_rule": self.cfg.relation_rule, "elimination": elim,
            "provenance": {"lane": "seedmin", "trial": t.digest(), "row_order": "list order"}});
        let mut text = rustred_campaign_core::schema::canon::canonical_json(&doc, false)?;
        text.push('\n');
        Ok(text)
    }

    fn append_journal(&self, line: &JournalLine) -> Result<(), String> {
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.journal_path)
            .map_err(|e| format!("{}: {e}", self.journal_path.display()))?;
        let mut s = serde_json::to_string(line).map_err(|e| e.to_string())?;
        s.push('\n');
        f.write_all(s.as_bytes()).map_err(|e| e.to_string())
    }

    fn keep_record(&mut self, rec: &Value) {
        let Some(path) = self.cfg.records_out.clone() else {
            return;
        };
        let Some(d) = rec["relation_digest"].as_str() else {
            return;
        };
        if self.seen_records.contains(d) || self.stats.records_bytes >= self.cfg.records_cap_bytes {
            return;
        }
        let mut line = rec.to_string();
        line.push('\n');
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            && f.write_all(line.as_bytes()).is_ok()
        {
            self.seen_records.insert(d.to_string());
            self.stats.records_written += 1;
            self.stats.records_bytes += line.len() as u64;
        }
    }

    /// One job (in its own thread).
    fn launch(
        &self,
        idx: usize,
        t: &Trial,
        cpus: Vec<usize>,
        tx: mpsc::Sender<Done>,
    ) -> Result<(), String> {
        let digest = t.digest();
        let text = self.recipe_text(t)?;
        let rpath = self
            .cfg
            .work_root
            .join("recipes")
            .join(format!("{}.json", &digest[..24]));
        std::fs::write(&rpath, text.as_bytes()).map_err(|e| e.to_string())?;
        let threads = cpus.len().min(t.keys.len()).max(1) as u32;
        let adcfg = SolveRecipeConfig {
            program: self.cfg.program.clone(),
            family: InputFile::from_file(&self.cfg.family).map_err(|e| format!("{e:?}"))?,
            order: self.cfg.order.clone(),
            batch: BatchPolicy {
                max_keys_per_job: t.keys.len().max(1),
                max_threads: threads,
                min_keys_per_thread: 1,
            },
            max_trace_rows: self.cfg.max_trace_rows,
            key_budget_s: self.cfg.key_budget_s,
            expect: None,
            determinism_id: None,
            heartbeat_s: 10,
            stop_grace_s: 30,
            package: None,
            live_repair: false,
            licence_env: self.cfg.licence_env.clone(),
            fake: self.cfg.fake.as_ref().map(|f| f.script(t)),
        };
        let rinput = RecipeInput {
            id: format!("seedmin/{}", &digest[..16]),
            file: InputFile::from_file(&rpath).map_err(|e| format!("{e:?}"))?,
            digest: None,
            sha256: Some(sha256_hex(text.as_bytes())),
        };
        let reqs: Vec<AdReq> = t
            .keys
            .iter()
            .map(|k| AdReq {
                recipe: rinput.clone(),
                key: k.iter().map(|&x| x as i64).collect(),
                tag: Some(format!(
                    "{}#{}",
                    &digest[..12],
                    crate::closeloop::ids::safe(&t.label)
                )),
                context: None,
            })
            .collect();
        let work = self.cfg.work_root.join("runs").join(&digest[..24]);
        let opts = RunOptions {
            cpus: if self.cfg.pin_cpus { cpus } else { vec![] },
            memory_cap_bytes: Some(self.cfg.cap_bytes),
            licence_env: self.cfg.licence_env.clone(),
            wall_cap: Some(Duration::from_secs(self.cfg.wall_cap_s)),
            grace: Some(Duration::from_secs(30)),
        };
        let keys = t.keys.clone();
        let sampler = self.sampler.clone();
        let keep = self.cfg.keep_run_dirs;
        let work_root = self.cfg.work_root.clone();
        std::thread::Builder::new()
            .name(format!("seedmin-{}", &digest[..8]))
            .spawn(move || {
                let t0 = Instant::now();
                let mut cpu_s = 0.0;
                let mut vm = 0;
                let result = (|| -> Result<Vec<Raw>, String> {
                    remove_own_run_dir(&work_root, &work);
                    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
                    let ad = SolveRecipeAdapter::new(adcfg);
                    let plans = ad.plan(&reqs).map_err(|e| format!("{e:?}"))?;
                    if plans.len() != 1 {
                        return Err(format!("{} plans for one trial", plans.len()));
                    }
                    let mut l = RealLauncher::new(sampler);
                    let (out, rep) = run_attempt(&mut l, &ad, &reqs, &plans[0], &work, 1, &opts)
                        .map_err(|e| format!("{e:?}"))?;
                    let run_dir = rep.prepared.spec.run_dir.clone();
                    let c = crate::closeloop::termmin::cost_of(Path::new(&run_dir), &out.job_id);
                    cpu_s = c.cpu_s.unwrap_or(0.0);
                    vm = c.vmhwm_kb.unwrap_or(0);
                    let cl = &out.classification;
                    let decided =
                        matches!((cl.row, cl.sub.as_str()), (1, "") | (28, "") | (29, ""));
                    if !decided {
                        let why = format!(
                            "row {}{} ({:?}): {}; refusal {:?}",
                            cl.row,
                            cl.sub,
                            cl.kind,
                            cl.why,
                            out.refusal.as_ref().map(|r| &r.why)
                        );
                        return Ok(keys.iter().map(|_| Raw::Missing(why.clone())).collect());
                    }
                    let layout = RunLayout::new(&run_dir);
                    let (_, h) = verify_stream_handle(
                        &layout.root,
                        &RunLayout::out_rel("results.jsonl"),
                        &["end"],
                    )
                    .map_err(|e| format!("results stream: {e:?}"))?;
                    let lines = h
                        .canonical_lines()
                        .map_err(|e| format!("canonical lines: {e:?}"))?;
                    let mut by_ord: BTreeMap<u64, Value> = BTreeMap::new();
                    for b in lines {
                        if let Ok(v) = serde_json::from_slice::<Value>(&b)
                            && v["type"] == "relation"
                            && let Some(o) = v["ordinal"].as_u64()
                        {
                            by_ord.insert(o, v);
                        }
                    }
                    let accepted: BTreeSet<(Vec<i64>, String)> = out
                        .relations
                        .iter()
                        .map(|r| (r.key.clone(), r.relation_digest.clone()))
                        .collect();
                    let raws = keys
                        .iter()
                        .enumerate()
                        .map(|(i, k)| match by_ord.remove(&(i as u64)) {
                            Some(v) => {
                                let kk: Vec<i64> = k.iter().map(|&x| x as i64).collect();
                                let d = v["relation_digest"].as_str().unwrap_or("").to_string();
                                let acc = accepted.contains(&(kk, d));
                                Raw::Record {
                                    record: v,
                                    accepted: acc,
                                }
                            }
                            None => Raw::Missing(format!("no record for ordinal {i}")),
                        })
                        .collect();
                    if !keep {
                        remove_own_run_dir(&work_root, &work);
                    }
                    Ok(raws)
                })();
                let _ = tx.send(Done {
                    idx,
                    result,
                    cpu_s,
                    wall_s: t0.elapsed().as_secs_f64(),
                    vmhwm_kb: vm,
                });
            })
            .map_err(|e| format!("worker thread: {e}"))?;
        Ok(())
    }

    /// The answer, and the record when its pole order is undecided (for the pole job).
    fn answer_and_record(
        &mut self,
        raw: Raw,
        key: &Key,
        centre: &[i32],
    ) -> (TrialAnswer, Option<Value>) {
        let rec = match &raw {
            Raw::Record { record, .. } => Some(record.clone()),
            Raw::Missing(_) => None,
        };
        let a = self.answer_of(raw, key, centre);
        let undecided_pole =
            matches!(&a, TrialAnswer::Relation(f) if f.pole == PoleOrder::Undecided);
        (a, rec.filter(|_| undecided_pole))
    }

    /// ONE pole-regular-at job for `pairs` (key, record); per pair Some(order) or None (undecided / job not read).
    fn pole_job(&mut self, pairs: &[(Key, Value)]) -> Vec<Option<PoleOrder>> {
        let none = || vec![None; pairs.len()];
        let Some(pc) = self.cfg.poles.clone() else {
            return none();
        };
        let reqs: Vec<PoleRequest> = pairs
            .iter()
            .enumerate()
            .map(|(i, (k, rec))| PoleRequest {
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
        let Ok(plans) = ad.plan(&reqs) else {
            return none();
        };
        let Some(plan) = plans.first() else {
            return none();
        };
        self.stats.pole_jobs += 1;
        let work = self
            .cfg
            .work_root
            .join("poles")
            .join(format!("{:06}", self.stats.pole_jobs));
        let opts = RunOptions {
            cpus: vec![],
            memory_cap_bytes: Some(pc.cap_bytes),
            licence_env: self.cfg.licence_env.clone(),
            wall_cap: Some(Duration::from_secs(pc.wall_cap_s)),
            grace: Some(Duration::from_secs(10)),
        };
        for attempt in 1..=2u32 {
            let mut l = RealLauncher::new(self.sampler.clone());
            match run_attempt(&mut l, &ad, &reqs, plan, &work, attempt, &opts) {
                Ok((b, _)) if b.unread.is_none() => {
                    let by_id: BTreeMap<String, (String, Option<u64>)> = b
                        .answers
                        .iter()
                        .filter_map(|a| {
                            a.id.as_str()
                                .map(|s| (s.to_string(), (a.verdict.clone(), a.pole_order)))
                        })
                        .collect();
                    return reqs
                        .iter()
                        .map(|q| match by_id.get(&q.id) {
                            Some((v, _)) if v == "regular" => Some(PoleOrder::regular()),
                            Some((v, Some(p))) if v == "pole" => {
                                Some(PoleOrder::Order { order: *p as u32 })
                            }
                            Some((v, None)) if v == "pole" => Some(PoleOrder::Singular),
                            _ => None,
                        })
                        .collect();
                }
                // a host resource failure (row 15b) is retried once; anything else leaves the poles undecided
                Ok((b, _)) if b.classification.row == 15 && b.classification.sub == "b" => continue,
                _ => return none(),
            }
        }
        none()
    }

    fn answer_of(&mut self, raw: Raw, key: &Key, centre: &[i32]) -> TrialAnswer {
        match raw {
            Raw::Missing(why) => TrialAnswer::Undecided {
                class: "refused".into(),
                message: why,
            },
            Raw::Record { record, accepted } => {
                let verdict = record["verdict"].as_str().unwrap_or("").to_string();
                match verdict.as_str() {
                    "SOLVED" if accepted => {
                        match facts_from_record(
                            &record,
                            key,
                            centre,
                            &self.cfg.d0,
                            self.judge.as_mut(),
                        ) {
                            Ok(f) => {
                                self.keep_record(&record);
                                TrialAnswer::Relation(Box::new(f))
                            }
                            Err(e) => TrialAnswer::Undecided {
                                class: "refused".into(),
                                message: e,
                            },
                        }
                    }
                    "SOLVED" => TrialAnswer::Undecided {
                        class: "refused".into(),
                        message: "SOLVED record not accepted by the adapter".into(),
                    },
                    "NO_RELATION_IN_SEEDS" => TrialAnswer::NoRelation,
                    "NOT_IN_DOMAIN" => TrialAnswer::NotInDomain,
                    v => TrialAnswer::Undecided {
                        class: v.to_lowercase(),
                        message: record["failure"]["message"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                    },
                }
            }
        }
    }
}

impl TrialSolver for ChildTrialSolver {
    fn solve(&mut self, trials: &[Trial]) -> Result<Vec<Vec<TrialAnswer>>, String> {
        let digests: Vec<String> = trials.iter().map(|t| t.digest()).collect();
        // trials to run (unique, not journaled)
        let mut todo: Vec<usize> = vec![];
        for (i, d) in digests.iter().enumerate() {
            if !self.journal.contains_key(d) && !todo.iter().any(|&j| digests[j] == *d) {
                todo.push(i);
            }
        }
        self.stats.trials += trials.len() as u64;
        self.stats.journal_hits += (trials.len() - todo.len()) as u64;
        // probe trials: no engine here has probe mode (the minimiser refuses before; answer undecided if asked)
        let mut runnable = vec![];
        for &i in &todo {
            if let TrialMode::Probe { .. } = trials[i].mode {
                let a: Vec<TrialAnswer> = trials[i]
                    .keys
                    .iter()
                    .map(|_| TrialAnswer::Undecided {
                        class: "refused".into(),
                        message: "engine lacks probe mode".into(),
                    })
                    .collect();
                let line = JournalLine {
                    digest: digests[i].clone(),
                    label: trials[i].label.clone(),
                    answers: a.clone(),
                };
                self.append_journal(&line)?;
                self.journal.insert(digests[i].clone(), a);
            } else {
                runnable.push(i);
            }
        }
        // schedule: each job takes min(threads_per_job, keys) CPUs from the free pool
        let mut transient: BTreeMap<String, Vec<TrialAnswer>> = BTreeMap::new();
        let mut pending: Vec<(usize, usize, Key, Value)> = vec![];
        let (tx, rx) = mpsc::channel::<Done>();
        let mut free: Vec<usize> = self.cfg.cpus.clone();
        let mut held: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        let mut queue: std::collections::VecDeque<usize> = runnable.into_iter().collect();
        while !queue.is_empty() || !held.is_empty() {
            while let Some(&i) = queue.front() {
                let want = self.cfg.threads_per_job.min(trials[i].keys.len()).max(1);
                let take = want.min(free.len());
                if take == 0 {
                    break;
                }
                queue.pop_front();
                let cpus: Vec<usize> = free.drain(..take).collect();
                held.insert(i, cpus.clone());
                self.say(format!(
                    "launch {} ({} seeds, {} keys) on CPUs {:?}",
                    trials[i].label,
                    trials[i].list.len(),
                    trials[i].keys.len(),
                    cpus
                ));
                self.launch(i, &trials[i], cpus, tx.clone())?;
                self.stats.jobs += 1;
            }
            let d = rx.recv().map_err(|e| format!("worker channel: {e}"))?;
            if let Some(c) = held.remove(&d.idx) {
                free.extend(c);
            }
            self.stats.cpu_s += d.cpu_s;
            self.stats.wall_s += d.wall_s;
            self.stats.max_vmhwm_kb = self.stats.max_vmhwm_kb.max(d.vmhwm_kb);
            let t = &trials[d.idx];
            let answers: Vec<TrialAnswer> = match d.result {
                Err(e) => t
                    .keys
                    .iter()
                    .map(|_| TrialAnswer::Undecided {
                        class: "refused".into(),
                        message: e.clone(),
                    })
                    .collect(),
                Ok(raws) => {
                    let mut v = vec![];
                    for (ki, (raw, k)) in raws.into_iter().zip(&t.keys).enumerate() {
                        let (a, rec) = self.answer_and_record(raw, k, &t.centre);
                        if let Some(r) = rec
                            && self.cfg.poles.is_some()
                        {
                            pending.push((d.idx, ki, k.clone(), r));
                        }
                        v.push(a);
                    }
                    v
                }
            };
            self.stats.keys_solved +=
                answers.iter().filter(|a| a.relation().is_some()).count() as u64;
            self.say(format!(
                "settled {}: {:?} (cpu {:.1} s, wall {:.1} s)",
                t.label,
                answers.iter().map(|a| a.word()).collect::<Vec<_>>(),
                d.cpu_s,
                d.wall_s
            ));
            let waits_for_poles = pending.iter().any(|p| p.0 == d.idx);
            if answers.iter().all(decided) && !waits_for_poles {
                let line = JournalLine {
                    digest: digests[d.idx].clone(),
                    label: t.label.clone(),
                    answers: answers.clone(),
                };
                self.append_journal(&line)?;
            }
            transient.insert(digests[d.idx].clone(), answers.clone());
            self.journal.insert(digests[d.idx].clone(), answers);
        }
        // the pole orders the in-process judge left undecided: ONE pole-regular-at job, then the waiting trials are
        // journaled (a job that answered nothing leaves them undecided and unjournaled: a resumed run asks again)
        if !pending.is_empty() {
            let pairs: Vec<(Key, Value)> =
                pending.iter().map(|p| (p.2.clone(), p.3.clone())).collect();
            let got = self.pole_job(&pairs);
            let answered = got.iter().all(Option::is_some);
            for ((ti, ki, _, _), g) in pending.iter().zip(&got) {
                if let (Some(o), Some(ans)) = (g, self.journal.get_mut(&digests[*ti]))
                    && let TrialAnswer::Relation(f) = &mut ans[*ki]
                {
                    f.pole = *o;
                }
            }
            let mut done: BTreeSet<usize> = BTreeSet::new();
            for (ti, _, _, _) in &pending {
                if !answered || !done.insert(*ti) {
                    continue;
                }
                let answers = self.journal[&digests[*ti]].clone();
                if answers.iter().all(decided) {
                    let line = JournalLine {
                        digest: digests[*ti].clone(),
                        label: trials[*ti].label.clone(),
                        answers,
                    };
                    self.append_journal(&line)?;
                }
            }
        }
        let out = digests.iter().map(|d| self.journal[d].clone()).collect();
        // undecided answers serve this call only: a later call (or a resumed run) solves the trial again
        for (d, a) in transient {
            if !a.iter().all(decided) {
                self.journal.remove(&d);
            }
        }
        Ok(out)
    }

    fn capabilities(&self) -> EngineCaps {
        self.caps.clone()
    }
}
