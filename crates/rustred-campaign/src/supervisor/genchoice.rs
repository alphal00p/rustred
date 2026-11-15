//! `GenChoiceSource`: the generation-choice PORTFOLIO of whole nodes (lane sizegen; interfaces/genchoice.md; request
//! reports/COORDINATOR-REQUEST-20261115-SIZE.md; R37, U-13).
//!
//! Per node, from the node DEFINITION alone (its family file) and the policy: probe jobs over the candidate set
//! (orders x poses x ladders) under a deterministic work cap, a deterministic ranking from the sealed probe tables,
//! full runs in ranking order until the first acceptable table, the economy alternatives (extra CPU <= factor x the CPU
//! of the first acceptable table; optimise mode: no cap), the choice, the per-rule comparison with the losers and the
//! import spec of the winner for the closing loop. The decisions are the pure functions of
//! `rustred_campaign_core::economy::genchoice`; this file only turns them into jobs (the supervisor's one launch path,
//! kind `generate`, allow-listed flags), ledger events and registered files. Every decision is a function of the
//! reduced ledger state and of the files it registered (resume = same answers).

use crate::ledger::{Event, FileRef, ItemEvent, NodeSpecValues};
use crate::state::{AttemptState, CampaignState, ItemPhase, NodeEvent, NodePhase, Rung};
use crate::supervisor::spec::CampaignSpec;
use crate::supervisor::worksource::{
    AcceptedRecord, CampaignOutcome, JobRequest, Routing, Settled, SourceContext, WorkSource,
};
use rustred_campaign_contract::classify::OutcomeKind;
use rustred_campaign_contract::fake::{FakeOp, FakeRule, FakeScript, OnSigterm, RssPlan};
use rustred_campaign_contract::spec::{
    ArgShape, Budgets, InputRef, JobKind, WallBudgets, WorkBudgets,
};
use rustred_campaign_contract::stop::{CaseResult, StopCode};
use rustred_campaign_core::economy::genchoice as gc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Campaign input role of the config (`rustred.campaign.genchoice.v1`).
pub const CONFIG_ROLE: &str = "genchoice";
pub const ROLE_FAMILY: &str = "genchoice_family";
pub const ROLE_ANSWER: &str = "genchoice_answer";
pub const ROLE_CANDIDATES: &str = "genchoice_candidates";
pub const ROLE_CHOICE: &str = "genchoice_choice";
pub const KIND_PROBE: &str = "genchoice-probe";
pub const KIND_FULL: &str = "genchoice-full";
pub const KIND_ALT: &str = "genchoice-alt";

fn item_probe(node: &str, cand: &str) -> String {
    format!("{node}.gc.p.{cand}")
}
fn item_full(node: &str, cand: &str) -> String {
    format!("{node}.gc.f.{cand}")
}
fn item_alt(node: &str, cand: &str) -> String {
    format!("{node}.gc.a.{cand}")
}
fn input_family(node: &str) -> String {
    format!("genchoice:{node}:family")
}
fn input_candidates(node: &str) -> String {
    format!("genchoice:{node}:candidates")
}
fn input_choice(node: &str) -> String {
    format!("genchoice:{node}:choice")
}
fn input_answer(item: &str, job: &str, attempt: u32) -> String {
    format!("genchoice:{item}:answer:{job}:a{attempt}")
}
fn node_dir(node: &str) -> String {
    format!("genchoice/{node}")
}

fn json_bytes(v: &Value) -> Vec<u8> {
    let mut b = serde_json::to_vec_pretty(v).expect("json");
    b.push(b'\n');
    b
}

fn blake3_hex(b: &[u8]) -> String {
    blake3::hash(b).to_hex().to_string()
}

/// (bytes, BLAKE3, SHA-256) of a file, streamed.
fn file_digests(p: &Path) -> Result<(u64, String, String), String> {
    let mut f = std::fs::File::open(p).map_err(|e| format!("{}: {e}", p.display()))?;
    let mut b3 = blake3::Hasher::new();
    let mut s2 = rustred_campaign_core::closure::sha256::Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut n = 0u64;
    loop {
        let k = f
            .read(&mut buf)
            .map_err(|e| format!("{}: {e}", p.display()))?;
        if k == 0 {
            break;
        }
        b3.update(&buf[..k]);
        s2.update(&buf[..k]);
        n += k as u64;
    }
    Ok((n, b3.finalize().to_hex().to_string(), s2.finish_hex()))
}

// ------------------------------------------------------------------------------------------------ fake children

/// TESTS ONLY (config key `fake`, removed before the config is parsed): the fake child's table per candidate and stage.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FakeGen {
    pub program: String,
    #[serde(default)]
    pub ms: u64,
    /// candidate id -> stage (`probe` / `full`) -> the run.
    pub runs: BTreeMap<String, BTreeMap<String, FakeRun>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FakeRun {
    /// Rule records written verbatim (the fake child sets `type` and `index`).
    #[serde(default)]
    pub rules: Vec<Value>,
    /// The stop (absent = complete).
    #[serde(default)]
    pub stop: Option<StopCode>,
    /// Milliseconds per case (overrides `ms`).
    #[serde(default)]
    pub ms: Option<u64>,
}

fn fake_script(f: &FakeGen, cand: &str, stage: &str) -> FakeScript {
    let mut ops = vec![FakeOp::Prepare { ms: 5 }];
    let run = f
        .runs
        .get(cand)
        .and_then(|m| m.get(stage).or_else(|| m.get("full")));
    match run {
        Some(r) => {
            for (k, rec) in r.rules.iter().enumerate() {
                ops.push(FakeOp::Case {
                    ordinal: k as u64,
                    ms: r.ms.unwrap_or(f.ms),
                    work: BTreeMap::new(),
                    result: CaseResult::Rule,
                    rule: Some(FakeRule {
                        record: rec.clone(),
                        loci: None,
                    }),
                    children: vec![],
                });
            }
            if r.stop == Some(StopCode::BudgetWall) || r.stop == Some(StopCode::LiftRefused) {
                ops.push(FakeOp::Case {
                    ordinal: r.rules.len() as u64,
                    ms: r.ms.unwrap_or(f.ms),
                    work: BTreeMap::new(),
                    result: if r.stop == Some(StopCode::BudgetWall) {
                        CaseResult::Budget
                    } else {
                        CaseResult::Refused
                    },
                    rule: None,
                    children: vec![],
                });
            }
            ops.push(FakeOp::End {
                stop: r.stop,
                verdict: None,
            });
        }
        None => ops.push(FakeOp::Fail {
            code: rustred_campaign_contract::exit::ExitCode::Input,
            message: format!("fake: no run for {cand} {stage}"),
        }),
    }
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

// ------------------------------------------------------------------------------------------------ answers

/// The answer file of one settled attempt (role `genchoice_answer`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Answer {
    pub schema: String,
    pub node: String,
    pub item: String,
    pub cand: String,
    /// `probe`, `full` or `alt`.
    pub stage: String,
    pub job: String,
    pub attempt: u32,
    /// Decision-table row and outcome class of the attempt.
    pub row: String,
    pub class: String,
    pub status: gc::RunStatus,
    pub order: String,
    pub args: Vec<String>,
    pub wall_job_s: Option<u64>,
    pub table: Option<FileRef>,
    pub table_path: Option<String>,
    pub measure: Option<gc::TableMeasure>,
    pub rows: Vec<gc::RuleRow>,
    /// Heavy relations refused by the probe's pre-lift screen (sealed `F.subcases.jsonl`, kind `refused`) and their
    /// fixed values (PRESENTED = family slots: the orders are comparator-only).
    #[serde(default)]
    pub refused: u64,
    #[serde(default)]
    pub refused_cases: Vec<Vec<Option<i64>>>,
    /// How the subcases list was read (verified against the attempt's OUTCOME, or why not).
    #[serde(default)]
    pub subcases: String,
}

/// The refused cases of an attempt's sealed open list, verified against the OUTCOME's sealed-output entry (role
/// `subcases`: bytes + BLAKE3). Ok((count, fixed values, how)); a missing list = none refused.
fn refused_of(run_dir: &Path) -> (u64, Vec<Vec<Option<i64>>>, String) {
    let oc = run_dir.join(rustred_campaign_contract::layout::OUTCOME);
    let Ok(ob) = std::fs::read(&oc) else {
        return (0, vec![], "no OUTCOME".into());
    };
    let Ok(o) = serde_json::from_slice::<Value>(&ob) else {
        return (0, vec![], "OUTCOME unreadable".into());
    };
    let Some(e) = o["job"]["outputs"]["extra"]
        .as_array()
        .and_then(|a| a.iter().find(|x| x["role"] == "subcases"))
    else {
        return (0, vec![], "no subcases output (no open case)".into());
    };
    let (Some(rel), Some(b3)) = (e["path"].as_str(), e["blake3"].as_str()) else {
        return (0, vec![], "subcases entry without path / blake3".into());
    };
    let p = run_dir.join(rel);
    let Ok(b) = std::fs::read(&p) else {
        return (0, vec![], format!("{}: unreadable", p.display()));
    };
    if blake3_hex(&b) != b3 {
        return (
            0,
            vec![],
            format!("{}: differs from its OUTCOME seal: not used", p.display()),
        );
    }
    let mut cases = Vec::new();
    for line in String::from_utf8_lossy(&b).lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if v["type"] == "subcase" && v["kind"] == "refused" {
            let f: Vec<Option<i64>> = v["case"]["fixed"]
                .as_array()
                .map(|a| a.iter().map(|x| x.as_i64()).collect())
                .unwrap_or_default();
            cases.push(f);
        }
    }
    (cases.len() as u64, cases, "verified against OUTCOME".into())
}

struct NodeRt {
    e: gc::NodeEntry,
    shape: gc::FamilyShape,
    family_bytes: u64,
    family_blake3: String,
}

/// The generation-choice work source.
pub struct GenChoiceSource {
    cfg_input: String,
    cfg: gc::Config,
    cdir: PathBuf,
    nodes: Vec<NodeRt>,
    fake: Option<FakeGen>,
    /// item -> its latest answer (cache of the registered answer files).
    answers: BTreeMap<String, Answer>,
    /// node -> BFI priority (cache of the registered candidates file).
    bfi: BTreeMap<String, Vec<usize>>,
}

/// The allow-list check of the policy's extra flags against the contract's `generate` list (shape of each value).
fn check_flags(args: &[String]) -> Result<(), String> {
    let rules = JobKind::Generate.allowed_args();
    let mut k = 0;
    while k < args.len() {
        let tok = args[k].as_str();
        if tok == "--order" {
            return Err("genchoice policy: --order is the candidate's, not a policy flag".into());
        }
        let r = rules
            .iter()
            .find(|r| r.flag == tok)
            .ok_or_else(|| format!("genchoice policy: {tok} is not on the generate allow-list"))?;
        if r.shape == ArgShape::Flag {
            k += 1;
            continue;
        }
        let v = args
            .get(k + 1)
            .ok_or_else(|| format!("genchoice policy: {tok} needs a value"))?;
        r.shape
            .check(v)
            .map_err(|e| format!("genchoice policy: {tok} {v}: {e}"))?;
        k += 2;
    }
    Ok(())
}

impl GenChoiceSource {
    pub fn from_spec(spec: &CampaignSpec, cdir: &Path) -> Result<Self, String> {
        let ins: Vec<_> = spec
            .inputs
            .iter()
            .filter(|i| i.role == CONFIG_ROLE)
            .collect();
        let [inp] = ins.as_slice() else {
            return Err(format!(
                "genchoice: exactly one input of role {CONFIG_ROLE} expected, {} found",
                ins.len()
            ));
        };
        let text = std::fs::read_to_string(&inp.path)
            .map_err(|e| format!("genchoice config {}: {e}", inp.path.display()))?;
        let mut v: Value =
            serde_json::from_str(&text).map_err(|e| format!("genchoice config: {e}"))?;
        let fake = match v.as_object_mut().and_then(|o| o.remove("fake")) {
            Some(f) => Some(
                serde_json::from_value::<FakeGen>(f)
                    .map_err(|e| format!("genchoice config: fake: {e}"))?,
            ),
            None => None,
        };
        let cfg = gc::Config::parse(&v.to_string())?;
        let prog = fake.as_ref().map_or(&cfg.program, |f| &f.program);
        if !spec.programs.contains_key(prog) {
            return Err(format!("genchoice: program {prog} not in the spec"));
        }
        for p in cfg.policy.poses.iter().chain(&cfg.policy.ladders) {
            check_flags(&p.args)?;
        }
        check_flags(&cfg.policy.common_args)?;
        check_flags(&cfg.policy.probe.args)?;
        let mut nodes = Vec::new();
        for e in &cfg.nodes {
            let p = Path::new(&e.family);
            let (bytes, b3, s2) = file_digests(p)?;
            if s2 != e.family_sha256 {
                return Err(format!(
                    "genchoice: {}: family {} sha256 {s2} differs from the config's {}",
                    e.node, e.family, e.family_sha256
                ));
            }
            if let Some(b) = &e.r#box {
                let (_, _, bs) = file_digests(Path::new(&b.path))?;
                if bs != b.sha256 {
                    return Err(format!(
                        "genchoice: {}: box {} sha256 {bs} differs from the config's {}",
                        e.node, b.path, b.sha256
                    ));
                }
            }
            let text = std::fs::read_to_string(p).map_err(|er| format!("{}: {er}", e.family))?;
            let shape = gc::family_shape(&text)?;
            nodes.push(NodeRt {
                e: e.clone(),
                shape,
                family_bytes: bytes,
                family_blake3: b3,
            });
        }
        Ok(GenChoiceSource {
            cfg_input: inp.id.clone(),
            cfg,
            cdir: cdir.to_path_buf(),
            nodes,
            fake,
            answers: BTreeMap::new(),
            bfi: BTreeMap::new(),
        })
    }

    fn abs(&self, rel: &str) -> PathBuf {
        self.cdir.join(rel)
    }

    fn reg(&self, rt: &mut Routing, id: String, role: &str, rec: AcceptedRecord) {
        let f = rec.file_ref();
        rt.events.push(Event::InputRegistered {
            input: id,
            role: role.into(),
            path: self.abs(&rec.path).display().to_string(),
            bytes: f.bytes,
            blake3: f.blake3,
        });
        rt.accepts.push(rec);
    }

    fn node_index(&self, node: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.e.node == node)
    }

    fn is_ours(it: &crate::state::ItemRec) -> bool {
        it.kind == KIND_PROBE || it.kind == KIND_FULL || it.kind == KIND_ALT
    }

    /// Read a registered input, verified by its recorded BLAKE3.
    fn read_input(st: &CampaignState, id: &str) -> Result<Vec<u8>, String> {
        let r = st
            .inputs
            .get(id)
            .ok_or_else(|| format!("input {id} not registered"))?;
        let b = std::fs::read(&r.path).map_err(|e| format!("{}: {e}", r.path))?;
        if blake3_hex(&b) != r.blake3 {
            return Err(format!("{}: differs from its registered digest", r.path));
        }
        Ok(b)
    }

    /// The latest answer of an item: the cache, else the registered answer file of its last job (resume).
    fn answer(&mut self, st: &CampaignState, item: &str) -> Option<Answer> {
        if let Some(a) = self.answers.get(item) {
            return Some(a.clone());
        }
        let it = st.items.get(item)?;
        for job in it.jobs.iter().rev() {
            let Some(j) = st.jobs.get(job) else {
                continue;
            };
            for att in (0..=j.attempt).rev() {
                let id = input_answer(item, job, att);
                if st.inputs.contains_key(&id) {
                    let b = Self::read_input(st, &id).ok()?;
                    let a: Answer = serde_json::from_slice(&b).ok()?;
                    self.answers.insert(item.to_string(), a.clone());
                    return Some(a);
                }
            }
        }
        None
    }

    /// The BFI priority of a node: the cache, else its registered candidates file.
    fn bfi_of(&mut self, st: &CampaignState, node: &str) -> Option<Vec<usize>> {
        if let Some(b) = self.bfi.get(node) {
            return Some(b.clone());
        }
        let b = Self::read_input(st, &input_candidates(node)).ok()?;
        let v: Value = serde_json::from_slice(&b).ok()?;
        let p: Vec<usize> = serde_json::from_value(v.get("bfi_priority")?.clone()).ok()?;
        self.bfi.insert(node.to_string(), p.clone());
        Some(p)
    }

    fn order_of(&mut self, st: &CampaignState, i: usize, c: &gc::Candidate) -> Option<String> {
        let node = self.nodes[i].e.node.clone();
        let bfi = if c.order == gc::OrderKind::Bfi {
            Some(self.bfi_of(st, &node)?)
        } else {
            None
        };
        let pr = gc::coordinate_priority(c.order, &self.nodes[i].shape, bfi.as_deref())?;
        Some(gc::order_json(c.order.word(), pr.as_deref()))
    }

    fn candidate(&self, id: &str) -> Option<gc::Candidate> {
        self.cfg
            .policy
            .candidates()
            .into_iter()
            .find(|c| c.id == id)
    }

    /// (candidate id, wall cap) of an item from its label (`<cand>` or `<cand> wall_job_s=<n>`).
    fn parse_label(label: &str) -> (String, Option<u64>) {
        let mut parts = label.split_whitespace();
        let cand = parts.next().unwrap_or_default().to_string();
        let cap = parts
            .find_map(|p| p.strip_prefix("wall_job_s="))
            .and_then(|v| v.parse().ok());
        (cand, cap)
    }

    fn request(
        &mut self,
        st: &CampaignState,
        i: usize,
        item: &str,
        job: String,
        priority: i64,
    ) -> Option<JobRequest> {
        let it = st.items.get(item)?;
        let (cand_id, cap) = Self::parse_label(&it.label);
        let c = self.candidate(&cand_id)?;
        let order = self.order_of(st, i, &c)?;
        let p = &self.cfg.policy;
        let mut args = p.args(&c, &order);
        if it.kind == KIND_PROBE {
            args.extend(p.probe.args.iter().cloned());
        }
        let n = &self.nodes[i];
        let (work, wall) = if it.kind == KIND_PROBE {
            (
                WorkBudgets {
                    cases: Some(p.probe.cases),
                    ..WorkBudgets::default()
                },
                WallBudgets {
                    case_s: p.probe.wall_case_s,
                    job_s: p.probe.wall_job_s,
                },
            )
        } else {
            let job_s = match (cap, p.full.wall_job_s) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            };
            (
                WorkBudgets::default(),
                WallBudgets {
                    case_s: p.full.wall_case_s,
                    job_s,
                },
            )
        };
        let stage = if it.kind == KIND_PROBE {
            "probe"
        } else {
            "full"
        };
        let (program, fake) = match &self.fake {
            Some(f) => (f.program.clone(), Some(fake_script(f, &cand_id, stage))),
            None => (self.cfg.program.clone(), None),
        };
        Some(JobRequest {
            job,
            item: item.to_string(),
            rung: it.rung,
            kind: JobKind::Generate,
            priority,
            cap_bytes: if stage == "probe" {
                p.job.cap_bytes
            } else {
                p.job.full_cap_bytes.unwrap_or(p.job.cap_bytes)
            },
            expected_peak_bytes: if stage == "probe" {
                p.job.expected_peak_bytes
            } else {
                p.job
                    .full_expected_peak_bytes
                    .unwrap_or(p.job.expected_peak_bytes)
            },
            cores: 1,
            heartbeat_s: p.job.heartbeat_s,
            stop_grace_s: p.job.stop_grace_s,
            program,
            args,
            inputs: vec![InputRef {
                role: "family".into(),
                path: n.e.family.clone(),
                blake3: n.family_blake3.clone(),
                upto: None,
            }],
            depends_on: vec![input_family(&n.e.node)],
            budgets: Budgets {
                work,
                wall,
                mem_soft_bytes: None,
            },
            sample_seed: 0,
            fake,
        })
    }

    fn item_final(st: &CampaignState, item: &str) -> bool {
        matches!(
            st.items.get(item).map(|it| &it.phase),
            Some(
                ItemPhase::Produced
                    | ItemPhase::Escalated(_)
                    | ItemPhase::Skipped
                    | ItemPhase::Rejected(_)
                    | ItemPhase::Accepted(_)
            )
        )
    }

    fn cpu_of(st: &CampaignState, a: &Answer) -> u64 {
        st.jobs.get(&a.job).map_or(0, |j| j.cpu_ms)
    }

    fn acceptable(a: &Answer) -> bool {
        a.status == gc::RunStatus::Complete
            && a.measure
                .as_ref()
                .is_some_and(|m| m.end_record && m.rules > 0)
    }

    /// Probe answers of a node (None while any probe is not final).
    fn probes_final(
        &mut self,
        st: &CampaignState,
        i: usize,
    ) -> Option<Vec<(gc::Candidate, Option<Answer>)>> {
        let node = self.nodes[i].e.node.clone();
        let mut out = Vec::new();
        for c in self.cfg.policy.candidates() {
            let item = item_probe(&node, &c.id);
            if !st.items.contains_key(&item) || !Self::item_final(st, &item) {
                return None;
            }
            let a = self.answer(st, &item);
            out.push((c, a));
        }
        Some(out)
    }

    /// The final full runs of a node (completed probes included, as non-alternative runs).
    fn full_runs(
        &mut self,
        st: &CampaignState,
        i: usize,
        probes: &[(gc::Candidate, Option<Answer>)],
    ) -> Vec<(gc::FullRun, Answer)> {
        let node = self.nodes[i].e.node.clone();
        let mut out = Vec::new();
        for (c, pa) in probes {
            let fi = item_full(&node, &c.id);
            let ai = item_alt(&node, &c.id);
            if let Some(a) = pa
                && a.status == gc::RunStatus::Complete
            {
                out.push((
                    gc::FullRun {
                        cand: c.index,
                        acceptable: Self::acceptable(a),
                        cpu_ms: Self::cpu_of(st, a),
                        measure: a.measure.clone(),
                        alternative: false,
                    },
                    a.clone(),
                ));
                continue;
            }
            for (item, alt) in [(fi, false), (ai, true)] {
                if st.items.contains_key(&item) && Self::item_final(st, &item) {
                    let a = self.answer(st, &item);
                    if let Some(a) = a {
                        out.push((
                            gc::FullRun {
                                cand: c.index,
                                acceptable: Self::acceptable(&a),
                                cpu_ms: Self::cpu_of(st, &a),
                                measure: a.measure.clone(),
                                alternative: alt,
                            },
                            a,
                        ));
                    } else {
                        out.push((
                            gc::FullRun {
                                cand: c.index,
                                acceptable: false,
                                cpu_ms: 0,
                                measure: None,
                                alternative: alt,
                            },
                            Answer {
                                schema: gc::ANSWER_SCHEMA.into(),
                                node: node.clone(),
                                item: item.clone(),
                                cand: c.id.clone(),
                                stage: if alt { "alt" } else { "full" }.into(),
                                job: String::new(),
                                attempt: 0,
                                row: String::new(),
                                class: "no answer (item final without a settled attempt)".into(),
                                status: gc::RunStatus::Failed,
                                order: String::new(),
                                args: vec![],
                                wall_job_s: None,
                                table: None,
                                table_path: None,
                                measure: None,
                                rows: vec![],
                                refused: 0,
                                refused_cases: vec![],
                                subcases: String::new(),
                            },
                        ));
                    }
                }
            }
        }
        out
    }

    fn create_item(node: &str, item: String, kind: &str, label: String) -> Event {
        Event::ItemCreated {
            item,
            node: node.to_string(),
            kind: kind.into(),
            rung: Rung(0),
            parent: None,
            inputs: vec![input_family(node)],
            label,
            weight: 1,
        }
    }

    /// The BFI vote of a node from its base probe (candidates file).
    fn write_candidates(
        &mut self,
        st: &CampaignState,
        i: usize,
        rt: &mut Routing,
    ) -> Result<(), String> {
        let node = self.nodes[i].e.node.clone();
        let Some(base) = self.cfg.policy.bfi_base() else {
            return Ok(());
        };
        let bitem = item_probe(&node, &base.id);
        if !Self::item_final(st, &bitem) {
            return Ok(());
        }
        let a = self.answer(st, &bitem);
        let rows = a.as_ref().map(|a| a.rows.clone()).unwrap_or_default();
        let refused = a
            .as_ref()
            .map(|a| a.refused_cases.clone())
            .unwrap_or_default();
        let p = gc::bfi_priority(
            &self.nodes[i].shape,
            &rows,
            &refused,
            self.cfg.policy.bfi.weight,
            self.cfg.policy.bfi.refused_weight,
        );
        let mut orders = serde_json::Map::new();
        self.bfi.insert(node.clone(), p.clone());
        for c in self.cfg.policy.candidates() {
            if let Some(o) = self.order_of(st, i, &c) {
                orders.insert(c.id.clone(), json!(o));
            }
        }
        let v = json!({"schema": gc::CANDIDATES_SCHEMA, "node": node, "shape": self.nodes[i].shape,
            "family_sha256": self.nodes[i].e.family_sha256,
            "bfi_base": base.id, "bfi_weight": self.cfg.policy.bfi.weight,
            "bfi_cases_voted": rows.len(), "bfi_refused_voted": refused.len(), "base_status": a.as_ref().map(|a| a.status),
            "bfi_priority": p, "orders": orders});
        let rec = AcceptedRecord {
            path: format!("{}/candidates.json", node_dir(&node)),
            bytes: json_bytes(&v),
        };
        self.reg(rt, input_candidates(&node), ROLE_CANDIDATES, rec);
        Ok(())
    }

    /// The choice of a node: choice.json, per_rule.jsonl, import.json (when the node names a box).
    #[allow(clippy::too_many_arguments)]
    fn decide(
        &mut self,
        st: &CampaignState,
        i: usize,
        probes: &[(gc::Candidate, Option<Answer>)],
        runs: &[(gc::FullRun, Answer)],
        ranking: &[usize],
        no_candidate: Option<String>,
        rt: &mut Routing,
    ) -> Result<(), String> {
        let node = self.nodes[i].e.node.clone();
        let p = self.cfg.policy.clone();
        let cands = p.candidates();
        let fr: Vec<gc::FullRun> = runs.iter().map(|(r, _)| r.clone()).collect();
        let winner = if no_candidate.is_some() {
            None
        } else {
            gc::choose(&fr, p.select)
        };
        let job_info = |a: &Answer| {
            let j = st.jobs.get(&a.job);
            json!({"job": a.job, "attempt": a.attempt, "row": a.row, "class": a.class, "status": a.status,
                "cpu_ms": j.map(|j| j.cpu_ms), "peak_rss_bytes": j.map(|j| j.peak_rss_bytes),
                "wall_job_s_cap": a.wall_job_s, "table": a.table, "table_path": a.table_path,
                "measure": a.measure})
        };
        let mut list = Vec::new();
        for (c, pa) in probes {
            let fulls: Vec<Value> = runs
                .iter()
                .filter(|(r, a)| r.cand == c.index && a.stage != "probe")
                .map(|(r, a)| {
                    let mut v = job_info(a);
                    v["alternative"] = json!(r.alternative);
                    v["acceptable"] = json!(r.acceptable);
                    v
                })
                .collect();
            let verdict = if Some(c.index) == winner {
                "winner".to_string()
            } else if runs.iter().any(|(r, _)| r.cand == c.index && r.acceptable) {
                "acceptable, lost the choice".to_string()
            } else if runs.iter().any(|(r, _)| r.cand == c.index) {
                "full run not acceptable".to_string()
            } else if ranking.contains(&c.index) {
                "ranked, not run (economy / first acceptable found earlier)".to_string()
            } else {
                "probe without a usable table".to_string()
            };
            list.push(
                json!({"id": c.id, "index": c.index, "order_kind": c.order.word(),
                "pose": p.poses[c.pose].name, "ladder": p.ladders[c.ladder].name,
                "order": pa.as_ref().map(|a| a.order.clone()),
                "args": pa.as_ref().map(|a| a.args.clone()),
                "probe": pa.as_ref().map(job_info),
                "probe_key": gc::probe_key(pa.as_ref().map_or(gc::RunStatus::Failed, |a| a.status),
                    pa.as_ref().and_then(|a| a.measure.as_ref()), pa.as_ref().map_or(0, |a| a.refused), c.index),
                "full": fulls, "verdict": verdict}),
            );
        }
        let wa = winner.and_then(|w| {
            runs.iter()
                .filter(|(r, _)| r.cand == w && r.acceptable)
                .map(|(_, a)| a.clone())
                .next()
        });
        let mut extra_files = serde_json::Map::new();
        if let Some(wa) = &wa {
            // per-rule comparison: every other table (full, else probe)
            let mut others: Vec<(String, Vec<gc::RuleRow>)> = Vec::new();
            for (c, pa) in probes {
                if Some(c.index) == winner {
                    continue;
                }
                let full = runs
                    .iter()
                    .filter(|(r, a)| r.cand == c.index && !a.rows.is_empty())
                    .map(|(_, a)| a)
                    .next();
                match (full, pa) {
                    (Some(a), _) => others.push((format!("{}:{}", c.id, a.stage), a.rows.clone())),
                    (None, Some(a)) if !a.rows.is_empty() => {
                        others.push((format!("{}:probe", c.id), a.rows.clone()))
                    }
                    _ => {}
                }
            }
            let (lines, summary) = gc::per_rule_comparison(&wa.rows, &others);
            let mut b = Vec::new();
            for l in &lines {
                b.extend_from_slice(l.to_string().as_bytes());
                b.push(b'\n');
            }
            let rec = AcceptedRecord {
                path: format!("{}/per_rule.jsonl", node_dir(&node)),
                bytes: b,
            };
            extra_files.insert("per_rule".into(), json!(rec.file_ref()));
            extra_files.insert("per_rule_summary".into(), summary);
            rt.accepts.push(rec);
            if let (Some(bx), Some(tp)) = (&self.nodes[i].e.r#box, &wa.table_path) {
                let (_, b3, s2) = file_digests(Path::new(tp))?;
                if wa.table.as_ref().is_some_and(|t| t.blake3 != b3) {
                    return Err(format!(
                        "genchoice: {node}: winner table {tp} differs from the verified table"
                    ));
                }
                let spec = gc::import_spec(&node, &bx.path, &bx.sha256, tp, &s2);
                let rec = AcceptedRecord {
                    path: format!("{}/import.json", node_dir(&node)),
                    bytes: json_bytes(&spec),
                };
                extra_files.insert("import_spec".into(), json!(rec.file_ref()));
                extra_files.insert("winner_table_sha256".into(), json!(s2));
                rt.accepts.push(rec);
            }
        }
        let v = json!({"schema": gc::CHOICE_SCHEMA, "node": node,
            "family": self.nodes[i].e.family, "family_sha256": self.nodes[i].e.family_sha256,
            "family_bytes": self.nodes[i].family_bytes, "shape": self.nodes[i].shape,
            "policy": p, "candidates": list,
            "ranking": ranking.iter().map(|&k| cands[k].id.clone()).collect::<Vec<_>>(),
            "economy": gc::economy_account(ranking, &fr, &p),
            "winner": winner.map(|w| cands[w].id.clone()),
            "winner_measure": wa.as_ref().and_then(|a| a.measure.clone()),
            "winner_table": wa.as_ref().and_then(|a| a.table_path.clone()),
            "no_candidate": no_candidate, "files": extra_files,
            "select": p.select});
        let rec = AcceptedRecord {
            path: format!("{}/choice.json", node_dir(&node)),
            bytes: json_bytes(&v),
        };
        self.reg(rt, input_choice(&node), ROLE_CHOICE, rec);
        Ok(())
    }

    /// One node's step (poll): BFI candidates, then the full-run phase, then the choice.
    fn step_node(&mut self, st: &CampaignState, i: usize, rt: &mut Routing) -> Result<(), String> {
        let node = self.nodes[i].e.node.clone();
        if st.inputs.contains_key(&input_choice(&node)) {
            return Ok(());
        }
        if !st.nodes.contains_key(&node) {
            return Ok(());
        }
        let p = self.cfg.policy.clone();
        // the bfi probes start when the base probe is final
        if p.orders.contains(&gc::OrderKind::Bfi) {
            if !st.inputs.contains_key(&input_candidates(&node)) {
                return self.write_candidates(st, i, rt);
            }
            let before = rt.events.len();
            for c in p.candidates() {
                let item = item_probe(&node, &c.id);
                if c.order == gc::OrderKind::Bfi && !st.items.contains_key(&item) {
                    rt.events
                        .push(Self::create_item(&node, item, KIND_PROBE, c.id.clone()));
                }
            }
            if rt.events.len() > before {
                return Ok(());
            }
        }
        let Some(probes) = self.probes_final(st, i) else {
            return Ok(());
        };
        let rk: Vec<(usize, gc::RunStatus, Option<gc::TableMeasure>, u64)> = probes
            .iter()
            .map(|(c, a)| {
                (
                    c.index,
                    a.as_ref().map_or(gc::RunStatus::Failed, |a| a.status),
                    a.as_ref().and_then(|a| a.measure.clone()),
                    a.as_ref().map_or(0, |a| a.refused),
                )
            })
            .collect();
        let ranking = gc::ranking(&rk);
        let runs = self.full_runs(st, i, &probes);
        let fr: Vec<gc::FullRun> = runs.iter().map(|(r, _)| r.clone()).collect();
        let cands = p.candidates();
        match gc::next_step(&ranking, &fr, &p) {
            gc::NextStep::First(c) => {
                let item = item_full(&node, &cands[c].id);
                if !st.items.contains_key(&item) {
                    rt.events.push(Self::create_item(
                        &node,
                        item,
                        KIND_FULL,
                        cands[c].id.clone(),
                    ));
                }
            }
            gc::NextStep::Alternatives(list) => {
                for (c, cap) in list {
                    let item = item_alt(&node, &cands[c].id);
                    if !st.items.contains_key(&item) {
                        let label = match cap {
                            Some(s) => format!("{} wall_job_s={s}", cands[c].id),
                            None => cands[c].id.clone(),
                        };
                        rt.events
                            .push(Self::create_item(&node, item, KIND_ALT, label));
                    }
                }
            }
            gc::NextStep::Decide => self.decide(st, i, &probes, &runs, &ranking, None, rt)?,
            gc::NextStep::NoCandidate(why) => {
                self.decide(st, i, &probes, &runs, &ranking, Some(why), rt)?
            }
        }
        Ok(())
    }
}

impl WorkSource for GenChoiceSource {
    fn bootstrap(
        &mut self,
        st: &CampaignState,
        _ctx: &SourceContext,
    ) -> Result<Vec<Event>, String> {
        let mut ev = Vec::new();
        for n in &self.nodes {
            let node = &n.e.node;
            let fid = input_family(node);
            match st.inputs.get(&fid) {
                Some(x) if x.blake3 != n.family_blake3 => {
                    return Err(format!(
                        "genchoice: {node}: the family {} changed since the campaign started: refused",
                        n.e.family
                    ));
                }
                Some(_) => {}
                None => ev.push(Event::InputRegistered {
                    input: fid.clone(),
                    role: ROLE_FAMILY.into(),
                    path: n.e.family.clone(),
                    bytes: n.family_bytes,
                    blake3: n.family_blake3.clone(),
                }),
            }
            match st.nodes.get(node) {
                None => {
                    ev.push(Event::NodeRegistered {
                        node: node.clone(),
                        inputs: vec![self.cfg_input.clone(), fid.clone()],
                        terminal_bound: None,
                        spec_box: None,
                        spec: NodeSpecValues::default(),
                        reference: None,
                    });
                    ev.push(Event::Node {
                        node: node.clone(),
                        ev: NodeEvent::Start,
                    });
                }
                Some(x) if x.state.phase == NodePhase::Stale => {
                    return Err(format!(
                        "genchoice: node {node} is stale ({}): start a new campaign directory",
                        x.stale_reason.clone().unwrap_or_default()
                    ));
                }
                Some(x) if x.state.phase == NodePhase::Registered => ev.push(Event::Node {
                    node: node.clone(),
                    ev: NodeEvent::Start,
                }),
                Some(_) => {}
            }
            for c in self.cfg.policy.candidates() {
                if c.order == gc::OrderKind::Bfi {
                    continue;
                }
                let item = item_probe(node, &c.id);
                if !st.items.contains_key(&item) {
                    ev.push(Self::create_item(node, item, KIND_PROBE, c.id.clone()));
                }
            }
        }
        Ok(ev)
    }

    fn ready(&mut self, st: &CampaignState, _ctx: &SourceContext, limit: usize) -> Vec<JobRequest> {
        const RERUN: i64 = 1 << 40;
        let mut out = Vec::new();
        let items: Vec<(String, String)> = st
            .items
            .iter()
            .filter(|(_, it)| Self::is_ours(it))
            .map(|(k, it)| (k.clone(), it.node.clone()))
            .collect();
        for (id, node) in items {
            let Some(i) = self.node_index(&node) else {
                continue;
            };
            let it = &st.items[&id];
            // probes first (the base probe first of all), then first full runs, then alternatives
            let stage_prio: i64 = match it.kind.as_str() {
                KIND_PROBE => 3_000,
                KIND_FULL => 2_000,
                _ => 1_000,
            };
            let (cand, _) = Self::parse_label(&it.label);
            let base = self
                .cfg
                .policy
                .bfi_base()
                .is_some_and(|b| b.id == cand && it.kind == KIND_PROBE);
            let prio = stage_prio + if base { 500 } else { 0 }
                - self.candidate(&cand).map_or(0, |c| c.index as i64)
                - i as i64 * 10_000;
            match &it.phase {
                ItemPhase::Open => {
                    let job = format!("{id}-t{}", it.jobs.len());
                    if let Some(r) = self.request(st, i, &id, job, prio) {
                        out.push(r);
                    }
                }
                ItemPhase::InProgress => {
                    let Some(last) = it.jobs.last().cloned() else {
                        continue;
                    };
                    match st.jobs.get(&last).map(|j| j.state) {
                        Some(AttemptState::Queued) => {
                            if let Some(r) = self.request(st, i, &id, last, RERUN + prio) {
                                out.push(r);
                            }
                        }
                        Some(AttemptState::Failed(_)) => {
                            let job = format!("{id}-t{}", it.jobs.len());
                            if let Some(r) = self.request(st, i, &id, job, prio) {
                                out.push(r);
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        out.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.job.cmp(&b.job)));
        out.truncate(limit);
        out
    }

    fn route(
        &mut self,
        st: &CampaignState,
        _ctx: &SourceContext,
        done: &Settled,
    ) -> Result<Routing, String> {
        let it = st
            .items
            .get(&done.item)
            .ok_or_else(|| format!("genchoice: unknown item {}", done.item))?;
        if !Self::is_ours(it) {
            return Err(format!(
                "genchoice: item {} is not a genchoice item",
                done.item
            ));
        }
        let node = it.node.clone();
        let i = self
            .node_index(&node)
            .ok_or_else(|| format!("genchoice: node {node} not in the config"))?;
        let (cand_id, cap) = Self::parse_label(&it.label);
        let c = &done.classification;
        let mut rt = Routing::default();
        // a crash is retried once (a new job of the item), then the run counts as failed
        if c.kind == OutcomeKind::Crashed {
            let failed_before = it
                .jobs
                .iter()
                .filter(|j| *j != &done.job)
                .filter(|j| {
                    matches!(
                        st.jobs.get(*j).map(|x| x.state),
                        Some(AttemptState::Failed(_))
                    )
                })
                .count();
            if failed_before == 0 {
                return Ok(rt);
            }
        }
        if matches!(c.kind, OutcomeKind::Interrupted | OutcomeKind::Lost) {
            return Err("interrupted / lost attempts are requeued by the supervisor".into());
        }
        let status = match c.kind {
            OutcomeKind::Complete => gc::RunStatus::Complete,
            OutcomeKind::RuleFound if c.row == 5 => gc::RunStatus::OpenRegions,
            OutcomeKind::RuleFound => gc::RunStatus::MaxCases,
            OutcomeKind::Spill | OutcomeKind::KilledTime => gc::RunStatus::Heavy,
            // the RAM policy's per-job cap (launch-time `--mem-cap`): lost at cap, recorded with the measured peak
            OutcomeKind::KilledMemory => gc::RunStatus::LostAtCap,
            OutcomeKind::NoRule if c.row == 4 => gc::RunStatus::Heavy,
            _ => gc::RunStatus::Failed,
        };
        let run_dir = st
            .jobs
            .get(&done.job)
            .and_then(|j| j.run_dir.clone())
            .unwrap_or_default();
        let (measure, rows, table_path) = match (&done.table, done.canonical_lines.is_empty()) {
            (Some(t), false) => {
                let (m, r) = gc::measure_table(
                    done.canonical_lines.iter().map(String::as_str),
                    t.bytes,
                    &self.cfg.policy.cost,
                )?;
                let tp = Path::new(&run_dir).join("out").join("F.table.jsonl");
                (Some(m), r, Some(tp.display().to_string()))
            }
            _ => (None, vec![], None),
        };
        let (refused, refused_cases, subcases) = if it.kind == KIND_PROBE && !run_dir.is_empty() {
            refused_of(Path::new(&run_dir))
        } else {
            (0, vec![], String::new())
        };
        let cand = self.candidate(&cand_id);
        let order = match &cand {
            Some(cd) => self.order_of(st, i, cd).unwrap_or_default(),
            None => String::new(),
        };
        let mut args = cand
            .as_ref()
            .map(|cd| self.cfg.policy.args(cd, &order))
            .unwrap_or_default();
        if it.kind == KIND_PROBE {
            args.extend(self.cfg.policy.probe.args.iter().cloned());
        }
        let stage = match it.kind.as_str() {
            KIND_PROBE => "probe",
            KIND_FULL => "full",
            _ => "alt",
        };
        let a = Answer {
            schema: gc::ANSWER_SCHEMA.into(),
            node: node.clone(),
            item: done.item.clone(),
            cand: cand_id,
            stage: stage.into(),
            job: done.job.clone(),
            attempt: done.attempt,
            row: format!("{}{}", c.row, c.sub),
            class: format!("{:?}: {}", c.kind, c.why),
            status,
            order,
            args,
            wall_job_s: cap,
            table: done.table.clone(),
            table_path,
            measure,
            rows,
            refused,
            refused_cases,
            subcases,
        };
        let rec = AcceptedRecord {
            path: format!(
                "{}/answers/{}.{}.a{}.json",
                node_dir(&node),
                done.item,
                done.job,
                done.attempt
            ),
            bytes: json_bytes(&serde_json::to_value(&a).map_err(|e| e.to_string())?),
        };
        self.reg(
            &mut rt,
            input_answer(&done.item, &done.job, done.attempt),
            ROLE_ANSWER,
            rec,
        );
        let produced = matches!(
            status,
            gc::RunStatus::Complete | gc::RunStatus::MaxCases | gc::RunStatus::OpenRegions
        ) && done.table.is_some();
        if produced {
            rt.events.push(Event::Item {
                item: done.item.clone(),
                ev: ItemEvent::Produce {
                    job: done.job.clone(),
                    attempt: done.attempt,
                    table: done.table.clone().expect("table"),
                },
            });
        } else {
            rt.events.push(Event::Item {
                item: done.item.clone(),
                ev: ItemEvent::Escalate {
                    to: Rung(1),
                    reason: format!(
                        "genchoice: {stage} {:?} (row {}{}): no usable table for the choice",
                        status, c.row, c.sub
                    ),
                },
            });
        }
        self.answers.insert(done.item.clone(), a);
        Ok(rt)
    }

    fn control(
        &mut self,
        st: &CampaignState,
        verb: &str,
        target: &str,
    ) -> Result<Vec<Event>, String> {
        if !st.items.contains_key(target) {
            return Err(format!("unknown item {target}"));
        }
        match verb {
            "retry" | "skip" => {
                self.answers.remove(target);
                Ok(vec![])
            }
            v => Err(format!("verb {v} not supported by the genchoice source")),
        }
    }

    fn poll(&mut self, st: &CampaignState, _ctx: &SourceContext) -> Result<Routing, String> {
        let mut rt = Routing::default();
        for i in 0..self.nodes.len() {
            self.step_node(st, i, &mut rt)?;
        }
        Ok(rt)
    }

    fn finished(&self, st: &CampaignState) -> bool {
        self.nodes
            .iter()
            .all(|n| st.inputs.contains_key(&input_choice(&n.e.node)))
    }

    fn blocked_on_operator(&self, st: &CampaignState) -> Option<String> {
        let att: Vec<&String> = st
            .items
            .iter()
            .filter(|(_, it)| Self::is_ours(it) && matches!(it.phase, ItemPhase::Attention(_)))
            .map(|(k, _)| k)
            .collect();
        (!att.is_empty()).then(|| format!("genchoice items in attention: {att:?}"))
    }

    fn outcome(&self, st: &CampaignState) -> CampaignOutcome {
        let mut open = Vec::new();
        for n in &self.nodes {
            let id = input_choice(&n.e.node);
            let ok = Self::read_input(st, &id)
                .ok()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                .is_some_and(|v| v["winner"].is_string());
            if !ok {
                open.push(n.e.node.clone());
            }
        }
        if open.is_empty() {
            CampaignOutcome::Closed
        } else {
            CampaignOutcome::Open(format!(
                "no acceptable generated table for: {}",
                open.join(", ")
            ))
        }
    }

    fn describe(&self) -> String {
        format!(
            "genchoice: {} node(s), {} candidate(s) each (orders x poses x ladders), probe cap {} cases",
            self.nodes.len(),
            self.cfg.policy.candidates().len(),
            self.cfg.policy.probe.cases
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_and_flags() {
        assert_eq!(
            GenChoiceSource::parse_label("bfi-rung-std wall_job_s=120"),
            ("bfi-rung-std".to_string(), Some(120))
        );
        assert_eq!(
            GenChoiceSource::parse_label("id-default-std"),
            ("id-default-std".to_string(), None)
        );
        check_flags(&[
            "--seed-pose-rung".into(),
            "--numeric-depth-max".into(),
            "3".into(),
        ])
        .unwrap();
        assert!(check_flags(&["--seed-pose-trigger".into(), "5".into()]).is_err());
        assert!(check_flags(&["--numeric-depth-max".into(), "x".into()]).is_err());
        assert!(check_flags(&["--order".into(), "id".into()]).is_err());
    }
}
