//! The scheduling interface between the supervisor and the campaign's work (interfaces/supervisor.md section 2), and
//! the deterministic synthetic source used by tests and rehearsals.
//!
//! The supervisor knows nothing about faces, ladders or acceptance policy; a [`WorkSource`] hands it ready jobs and
//! decides what a classified outcome means for its items. Every method is a function of the reduced ledger state, so a
//! restarted supervisor gets the same answers (resume = replay + the same questions).

use super::spec::Synthetic;
use crate::ledger::{AcceptSource, Event, FileRef, ItemEvent, NodeSpecValues};
use crate::state::{
    AttemptState, CampaignState, EvidenceLevel, ItemPhase, NodeEvent, NodePhase, Rung,
};
use rustred_campaign_contract::classify::{Accept, Classification, OutcomeKind};
use rustred_campaign_contract::exit::ExitCode;
use rustred_campaign_contract::fake::{
    FakeOp, FakeRule, FakeScript, Fault, FaultKind, HelperPlan, RssPlan,
};
use rustred_campaign_contract::spec::{Budgets, InputRef, JobKind, WorkCounter};
use rustred_campaign_contract::stop::{CaseResult, StopCode};
use serde::Serialize;
use std::path::PathBuf;

/// What a source may need to know about the run (never part of an id).
#[derive(Clone, Debug)]
pub struct SourceContext {
    /// Input id of each program's binary (`bin:<name>`), for `job_queued.binary` and node inputs (M1).
    pub binaries: std::collections::BTreeMap<String, String>,
    pub campaign_key: String,
}

/// One job the source wants to run now.
#[derive(Clone, Debug)]
pub struct JobRequest {
    /// `<item>-t<k>` (job ids are `[A-Za-z0-9._-]`, contract 3.1.2); an existing job id in state `queued` = the next attempt of a requeued job.
    pub job: String,
    pub item: String,
    pub rung: Rung,
    pub kind: JobKind,
    /// Higher first.
    pub priority: i64,
    pub cap_bytes: u64,
    pub expected_peak_bytes: u64,
    pub cores: usize,
    pub heartbeat_s: u32,
    pub stop_grace_s: u32,
    /// Key of `spec.programs`.
    pub program: String,
    pub args: Vec<String>,
    pub inputs: Vec<InputRef>,
    /// Ledger input ids the job depends on beyond its binary and its item's / node's inputs (tables, seed policy,
    /// ...): stamped into `job_queued.inputs` (input identity).
    pub depends_on: Vec<String>,
    pub budgets: Budgets,
    pub sample_seed: u64,
    pub fake: Option<FakeScript>,
}

impl JobRequest {
    /// Digest of the attempt-independent request (the `job_queued.spec_blake3`).
    pub fn digest(&self) -> String {
        let v = serde_json::json!({
            "job": self.job, "item": self.item, "rung": self.rung.0, "kind": self.kind, "program": self.program,
            "args": self.args, "inputs": self.inputs, "budgets": self.budgets, "seed": self.sample_seed,
            "cap": self.cap_bytes, "fake": self.fake, "depends_on": self.depends_on,
        });
        blake3::hash(&serde_json::to_vec(&v).expect("json"))
            .to_hex()
            .to_string()
    }
}

/// A settled attempt handed to [`WorkSource::route`].
#[derive(Clone, Debug)]
pub struct Settled {
    pub job: String,
    pub attempt: u32,
    pub item: String,
    pub rung: Rung,
    pub classification: Classification,
    /// The sealed table (CDIR-relative path or `large:`), present only for sealed successes.
    pub table: Option<FileRef>,
    /// Canonical lines of the sealed table, in file order.
    pub canonical_lines: Vec<String>,
    /// Canonical digest of the sealed table.
    pub canonical_blake3: Option<String>,
}

/// An accepted record file to write (atomically, content-addressed) BEFORE the ledger line that names it.
#[derive(Clone, Debug)]
pub struct AcceptedRecord {
    /// Path relative to CDIR.
    pub path: String,
    pub bytes: Vec<u8>,
}

impl AcceptedRecord {
    pub fn file_ref(&self) -> FileRef {
        FileRef {
            path: self.path.clone(),
            bytes: self.bytes.len() as u64,
            blake3: blake3::hash(&self.bytes).to_hex().to_string(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Routing {
    pub events: Vec<Event>,
    pub accepts: Vec<AcceptedRecord>,
}

pub trait WorkSource {
    /// Registration events still missing (nodes, first items, reopening of stale rows). Idempotent.
    fn bootstrap(&mut self, st: &CampaignState, ctx: &SourceContext) -> Result<Vec<Event>, String>;
    /// Jobs to run now, highest priority first, at most `limit`, never one that is launching / running.
    fn ready(&mut self, st: &CampaignState, ctx: &SourceContext, limit: usize) -> Vec<JobRequest>;
    /// A settled attempt (never interrupted / lost: the supervisor requeues those itself).
    fn route(
        &mut self,
        st: &CampaignState,
        ctx: &SourceContext,
        done: &Settled,
    ) -> Result<Routing, String>;
    /// Follow-up events after the ledger APPLIED an operator `retry` / `skip` of `target` (the item's phase change is
    /// the ledger's own; most sources return nothing).
    fn control(
        &mut self,
        st: &CampaignState,
        verb: &str,
        target: &str,
    ) -> Result<Vec<Event>, String>;
    /// Lane closeloop (closeloop.md 6.1): ledger events (and the records to write BEFORE them) of the source's own
    /// in-process passes, called every tick before `ready()` while the campaign runs. Default: nothing.
    fn poll(&mut self, _st: &CampaignState, _ctx: &SourceContext) -> Result<Routing, String> {
        Ok(Routing::default())
    }
    /// Decision row 3 (`no_rule`, action `confirm_coverage_else_next_rung`; contract lane contractfix2, audit ac2-m9):
    /// the child only CLAIMS (OUTCOME `per_case`) that no case of the item needs a rule. The source's OWN coverage check
    /// (forward closure / partition of the item's keys) may confirm it: `Ok(Some(events))` closes the item with those
    /// events (committed with `job.finished`); `Ok(None)` (the default: no coverage check) means NOT confirmed, and the
    /// supervisor routes the attempt as row 24 (next rung). The claims never close an item by themselves.
    fn confirm_coverage(
        &mut self,
        _st: &CampaignState,
        _ctx: &SourceContext,
        _done: &Settled,
    ) -> Result<Option<Vec<Event>>, String> {
        Ok(None)
    }
    /// All work final.
    fn finished(&self, st: &CampaignState) -> bool;
    /// lane t266run / closeloop4 (closeloop.md 19): Some(text) while an OPERATOR must decide before the work can go on
    /// (e.g. terminal proposals of a closing loop): the supervisor stops with the attention exit (8) instead of
    /// finishing, both when the source is finished and when it waits with nothing to run. Default: none.
    fn operator_decision(&self, _st: &CampaignState) -> Option<String> {
        None
    }
    /// Number of work units (for logs).
    fn describe(&self) -> String;
    /// procfix5 (gate4 m3): how the work ENDED once `finished` holds: every node closed, some open, or an error. The
    /// exit code of `rr_campaign run` follows it (0 / 10 / 11; interfaces/supervisor.md 19). Default: from the items of
    /// the ledger ([`default_outcome`]); a source with its own verdicts (closeloop: final result.json) overrides it.
    fn outcome(&self, st: &CampaignState) -> CampaignOutcome {
        default_outcome(st)
    }
    /// lane closeloop4 (closeloop.md 19): an operator decision request (`resolve-terminals`): the events (and records)
    /// that record it. Default: refused.
    fn resolve_operator_decision(
        &mut self,
        _st: &CampaignState,
        req: &super::ControlRequest,
    ) -> Result<Routing, String> {
        Err(format!(
            "verb {} not supported by this work source",
            req.verb
        ))
    }
}

/// How a FINISHED campaign's work ended (procfix5, gate4 m3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CampaignOutcome {
    /// Every node closed (every chain accepted or terminal).
    Closed,
    /// The work is done as far as the campaign can go without the operator, but not everything is closed (items in
    /// attention, skipped, rejected, open behind them, stale): the text names them.
    Open(String),
    /// A node ended in error (the source's verdict).
    Error(String),
}

/// The default outcome from the ledger state: CLOSED when every item that is not superseded (escalated) is accepted,
/// terminal or retired; otherwise OPEN, naming up to ten such items with their phase.
pub fn default_outcome(st: &CampaignState) -> CampaignOutcome {
    let mut open: Vec<String> = Vec::new();
    for (id, it) in &st.items {
        let closed = match &it.phase {
            ItemPhase::Accepted(_) | ItemPhase::Terminal | ItemPhase::Retired => true,
            ItemPhase::Escalated(_) => true,
            ItemPhase::Stale => it.stale_from.as_deref() == Some("escalated"),
            _ => false,
        };
        if !closed {
            open.push(format!("{id} ({})", it.phase.tag()));
        }
    }
    if open.is_empty() {
        return CampaignOutcome::Closed;
    }
    let n = open.len();
    open.truncate(10);
    CampaignOutcome::Open(format!(
        "{n} item(s) not closed: {}{}",
        open.join(", "),
        if n > 10 { ", ..." } else { "" }
    ))
}

/// The accepted record written for a rule (deterministic content: no time, no attempt).
#[derive(Serialize)]
struct AcceptedFile<'a> {
    schema: &'static str,
    item: &'a str,
    rule: &'a str,
    table_canonical_blake3: &'a str,
    records: &'a [String],
}

pub const ACCEPTED_SCHEMA: &str = "rustred.campaign.accepted.v1";

/// A file name component from an id (characters outside `[A-Za-z0-9.-]` become `_`).
fn safe_name(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// The two artefacts of a terminal declaration (ledger 5.2, audit A M4): the record that the item's ladder was
/// exhausted within its budgets (cited by `propose_terminal.exhausted`) and the key-set file of the declared
/// terminal (`declare_terminal.keys`). Deterministic content (no time, no attempt): a resumed campaign writes the
/// same bytes.
pub fn terminal_artefacts(
    node: &str,
    item: &str,
    rung: Rung,
    reason: &str,
    keys: &[String],
) -> (AcceptedRecord, AcceptedRecord) {
    let dir = PathBuf::from("terminals").join(node);
    let exhausted = serde_json::to_vec(&serde_json::json!({
        "schema": "rustred.campaign.ladder-exhausted.v1", "item": item, "rung": rung, "reason": reason,
    }))
    .expect("json");
    let keyset = serde_json::to_vec(&serde_json::json!({
        "schema": "rustred.campaign.keyset.v1", "item": item, "keys": keys,
    }))
    .expect("json");
    let safe = safe_name(item);
    (
        AcceptedRecord {
            path: dir
                .join(format!("{safe}.exhausted.json"))
                .to_string_lossy()
                .into_owned(),
            bytes: exhausted,
        },
        AcceptedRecord {
            path: dir
                .join(format!("{safe}.keys.json"))
                .to_string_lossy()
                .into_owned(),
            bytes: keyset,
        },
    )
}

/// Rule id and record file of an accepted sealed table (shared by sources).
pub fn accepted_record(
    node: &str,
    item: &str,
    canon: &str,
    lines: &[String],
) -> (String, AcceptedRecord) {
    let rule = format!("{item}:{}", &canon[..16.min(canon.len())]);
    let bytes = serde_json::to_vec(&AcceptedFile {
        schema: ACCEPTED_SCHEMA,
        item,
        rule: &rule,
        table_canonical_blake3: canon,
        records: lines,
    })
    .expect("json");
    let safe = safe_name(item);
    let path = PathBuf::from("accepted")
        .join(node)
        .join(format!("{safe}.{}.json", &canon[..16.min(canon.len())]));
    (
        rule,
        AcceptedRecord {
            path: path.to_string_lossy().into_owned(),
            bytes,
        },
    )
}

// ------------------------------------------------------------------------------------------------------ synthetic

/// Behaviour of one rung of one synthetic item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Behaviour {
    Rule,
    NoRule,
    MemExcess,
    Crash,
}

/// The deterministic synthetic campaign (interfaces/supervisor.md section 2).
pub struct ScriptedWorkSource {
    pub cfg: Synthetic,
}

fn splitmix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

fn unit(h: u64) -> f64 {
    (h >> 11) as f64 / (1u64 << 53) as f64
}

const RERUN_PRIORITY: i64 = 1 << 40;

impl ScriptedWorkSource {
    pub fn new(cfg: Synthetic) -> Self {
        ScriptedWorkSource { cfg }
    }

    fn h(&self, a: u64, b: u64, salt: u64) -> u64 {
        splitmix(splitmix(splitmix(self.cfg.seed ^ salt) ^ a) ^ b)
    }

    pub fn node_of(&self, i: u32) -> String {
        format!("N{}", i % self.cfg.nodes)
    }

    pub fn item_id(&self, i: u32, r: u8) -> String {
        format!("{}.i{i:04}.L{r}", self.node_of(i))
    }

    pub fn behaviour(&self, i: u32, r: u8) -> Behaviour {
        let u = unit(self.h(i as u64, r as u64, 1));
        let c = &self.cfg;
        if u < c.p_crash {
            Behaviour::Crash
        } else if u < c.p_crash + c.p_mem {
            Behaviour::MemExcess
        } else if u < c.p_crash + c.p_mem + c.p_norule {
            Behaviour::NoRule
        } else {
            Behaviour::Rule
        }
    }

    pub fn deps(&self, i: u32) -> Vec<u32> {
        if i == 0 || unit(self.h(i as u64, 0, 2)) >= self.cfg.deps {
            return Vec::new();
        }
        let a = (self.h(i as u64, 1, 3) % i as u64) as u32;
        let b = (self.h(i as u64, 2, 3) % i as u64) as u32;
        let mut v = vec![a];
        if b != a && self.h(i as u64, 3, 3).is_multiple_of(2) {
            v.push(b);
        }
        v.sort_unstable();
        v
    }

    fn case_ms(&self, i: u32, r: u8) -> u64 {
        let [lo, hi] = self.cfg.case_ms;
        lo + self.h(i as u64, r as u64, 4) % (hi - lo + 1)
    }

    fn cap_bytes(&self) -> u64 {
        self.cfg.mem_cap_mb << 20
    }

    /// Current (highest created) rung item of logical item i.
    fn current<'a>(
        &self,
        st: &'a CampaignState,
        i: u32,
    ) -> Option<(u8, &'a crate::state::ItemRec, String)> {
        (0..self.cfg.rungs).rev().find_map(|r| {
            let id = self.item_id(i, r);
            st.items.get(&id).map(|it| (r, it, id))
        })
    }

    fn chain_final(&self, st: &CampaignState, i: u32) -> bool {
        matches!(
            self.current(st, i).map(|(_, it, _)| &it.phase),
            Some(
                ItemPhase::Accepted(_)
                    | ItemPhase::Terminal
                    | ItemPhase::Retired
                    | ItemPhase::TerminalCandidate
                    | ItemPhase::Skipped
            )
        )
    }

    /// The fake script of (i, r).
    pub fn script(&self, i: u32, r: u8) -> FakeScript {
        let ms = self.case_ms(i, r);
        let id = self.item_id(i, r);
        let rule_case = |ms: u64| FakeOp::Case {
            ordinal: 0,
            ms,
            work: [
                (WorkCounter::Seeds, 100 + i as u64),
                (WorkCounter::Rows, 1000),
            ]
            .into_iter()
            .collect(),
            result: CaseResult::Rule,
            rule: Some(FakeRule {
                record: serde_json::json!({"chart": self.node_of(i), "item": id, "rung": r,
                    "target": format!("I({i},{r})"), "rhs": [["I(0)", format!("{}/{}", i + 1, r + 2)]]}),
                loci: None,
            }),
            children: vec![],
        };
        let end = FakeOp::End {
            stop: Some(StopCode::MaxCases),
            verdict: None,
        };
        let ops = match self.behaviour(i, r) {
            Behaviour::Rule if self.cfg.escape_helpers => vec![
                FakeOp::Prepare { ms: 20 },
                FakeOp::Helper { index: 0 },
                rule_case(ms),
                end,
            ],
            Behaviour::Rule if self.cfg.honest_helpers > 0 => {
                let mut v = vec![FakeOp::Prepare { ms: 20 }];
                v.extend(
                    (0..self.cfg.honest_helpers.min(8) as usize)
                        .map(|index| FakeOp::Helper { index }),
                );
                v.extend([rule_case(ms), end]);
                v
            }
            Behaviour::Rule => vec![FakeOp::Prepare { ms: 20 }, rule_case(ms), end],
            Behaviour::NoRule => vec![
                FakeOp::Prepare { ms: 20 },
                FakeOp::Case {
                    ordinal: 0,
                    ms,
                    work: [(WorkCounter::Seeds, 50)].into_iter().collect(),
                    result: CaseResult::Refused,
                    rule: None,
                    children: vec![],
                },
                end,
            ],
            // Allocate twice the cap and stay there long enough for any sampler to see it.
            Behaviour::MemExcess => vec![
                FakeOp::Prepare { ms: 20 },
                FakeOp::Alloc {
                    mb: 2 * self.cfg.mem_cap_mb,
                },
                rule_case(120_000),
                end,
            ],
            Behaviour::Crash => vec![
                FakeOp::Prepare { ms: 20 },
                FakeOp::Fail {
                    code: ExitCode::Internal,
                    message: format!("synthetic crash of {id}"),
                },
            ],
        };
        let helpers = if self.cfg.escape_helpers {
            vec![HelperPlan {
                sleep_ms: 120_000,
                pdeathsig: false,
                ignore_sigterm: true,
                escape_group: true,
            }]
        } else {
            // procfix7 round 11: honest (compliant) helpers, see `SyntheticCfg::honest_helpers`
            (0..self.cfg.honest_helpers.min(8))
                .map(|k| HelperPlan {
                    sleep_ms: if k % 2 == 0 { 150 } else { 600_000 },
                    pdeathsig: true,
                    ignore_sigterm: false,
                    escape_group: false,
                })
                .collect()
        };
        let faults = if r == 0 && self.cfg.hang_items.contains(&i) {
            vec![Fault {
                at_op: 1,
                kind: FaultKind::StopHeartbeat,
                bytes: 0,
                code: 0,
                path: None,
            }]
        } else {
            vec![]
        };
        FakeScript {
            script: ops,
            on_sigterm: self.cfg.on_sigterm,
            slow_ms: 0,
            start_delay_ms: 0,
            rss: RssPlan::default(),
            helpers,
            faults,
            time_scale: 1.0,
            volatile_jitter: true,
        }
    }

    fn request(&self, i: u32, r: u8, job: String, item: String, priority: i64) -> JobRequest {
        let cap = self.cap_bytes();
        JobRequest {
            job,
            item,
            rung: Rung(r),
            kind: JobKind::Fake,
            priority,
            cap_bytes: cap,
            expected_peak_bytes: cap / 2,
            cores: 1,
            heartbeat_s: self.cfg.heartbeat_s,
            stop_grace_s: self.cfg.stop_grace_s,
            program: "rr_fake_child".into(),
            args: vec![],
            inputs: vec![],
            depends_on: vec![],
            budgets: Budgets::default(),
            sample_seed: self.h(i as u64, r as u64, 5),
            fake: Some(self.script(i, r)),
        }
    }

    fn logical_of(&self, item: &str) -> Option<(u32, u8)> {
        let mut parts = item.split('.');
        let _node = parts.next()?;
        let i = parts.next()?.strip_prefix('i')?.parse().ok()?;
        let r = parts.next()?.strip_prefix('L')?.parse().ok()?;
        Some((i, r))
    }

    fn escalate_or_terminal(&self, item: &str, i: u32, r: u8, reason: &str) -> Routing {
        if r + 1 < self.cfg.rungs {
            let next = self.item_id(i, r + 1);
            Routing {
                events: vec![
                    Event::Item {
                        item: item.to_string(),
                        ev: ItemEvent::Escalate {
                            to: Rung(r + 1),
                            reason: reason.to_string(),
                        },
                    },
                    Event::ItemCreated {
                        item: next,
                        node: self.node_of(i),
                        kind: "synthetic".into(),
                        rung: Rung(r + 1),
                        parent: Some(item.to_string()),
                        inputs: vec![],
                        label: format!("i{i:04}"),
                        weight: 1,
                    },
                ],
                accepts: vec![],
            }
        } else {
            let why = format!("ladder exhausted ({reason})");
            let (exhausted, keys) =
                terminal_artefacts(&self.node_of(i), item, Rung(r), &why, &[format!("I({i})")]);
            Routing {
                events: vec![
                    Event::Item {
                        item: item.to_string(),
                        ev: ItemEvent::ProposeTerminal {
                            reason: why,
                            preferred: false,
                            exhausted: Some(exhausted.file_ref()),
                        },
                    },
                    Event::Item {
                        item: item.to_string(),
                        ev: ItemEvent::DeclareTerminal {
                            count: 1,
                            keys: keys.file_ref(),
                            reason: "ladder exhausted".into(),
                            preferred: false,
                        },
                    },
                ],
                accepts: vec![exhausted, keys],
            }
        }
    }
}

/// The `item.attention` code of a synthetic item whose job crashed twice (decision rows 22 / 23; procfix5 m4).
pub const CRASHED_TWICE: &str = "crashed_twice";

impl WorkSource for ScriptedWorkSource {
    fn bootstrap(&mut self, st: &CampaignState, ctx: &SourceContext) -> Result<Vec<Event>, String> {
        let bin = ctx
            .binaries
            .get("rr_fake_child")
            .ok_or("binary rr_fake_child not registered")?
            .clone();
        let mut ev = Vec::new();
        for n in 0..self.cfg.nodes {
            let node = format!("N{n}");
            match st.nodes.get(&node) {
                None => ev.push(Event::NodeRegistered {
                    node: node.clone(),
                    inputs: vec![bin.clone()],
                    terminal_bound: Some(self.cfg.items),
                    spec_box: None,
                    // The synthetic ladder's top rung is where a non-preferred terminal may be proposed.
                    spec: NodeSpecValues {
                        terminal_rung: Some(Rung(self.cfg.rungs.saturating_sub(1))),
                        ..NodeSpecValues::default()
                    },
                    reference: None,
                }),
                Some(n) if n.state.phase == NodePhase::Stale => ev.push(Event::Node {
                    node: node.clone(),
                    ev: NodeEvent::Reopen {
                        reason: "binary changed: work redone".into(),
                    },
                }),
                _ => {}
            }
        }
        for i in 0..self.cfg.items {
            let id = self.item_id(i, 0);
            if !st.items.contains_key(&id) {
                ev.push(Event::ItemCreated {
                    item: id,
                    node: self.node_of(i),
                    kind: "synthetic".into(),
                    rung: Rung(0),
                    parent: None,
                    inputs: vec![],
                    label: format!("i{i:04}"),
                    weight: 1,
                });
            }
        }
        // Stale items: reopen (after their node, in the same batch).
        for (id, it) in &st.items {
            // An escalated item stays superseded by its next-rung item: only the chain's live item is redone.
            if it.phase == ItemPhase::Stale && it.stale_from.as_deref() != Some("escalated") {
                ev.push(Event::Item {
                    item: id.clone(),
                    ev: ItemEvent::Reopen {
                        reason: "binary changed: work redone".into(),
                    },
                });
            }
        }
        Ok(ev)
    }

    fn ready(&mut self, st: &CampaignState, _ctx: &SourceContext, limit: usize) -> Vec<JobRequest> {
        let mut out = Vec::new();
        for i in 0..self.cfg.items {
            let Some((r, it, id)) = self.current(st, i) else {
                continue;
            };
            if !self.deps(i).iter().all(|&d| self.chain_final(st, d)) {
                continue;
            }
            let base = RERUN_PRIORITY - 1 - i as i64 * 16 - r as i64;
            match &it.phase {
                ItemPhase::Open => {
                    let job = format!("{id}-t{}", it.jobs.len());
                    out.push(self.request(i, r, job, id, base));
                }
                ItemPhase::InProgress => {
                    let Some(last) = it.jobs.last() else {
                        continue;
                    };
                    match st.jobs.get(last).map(|j| j.state) {
                        Some(AttemptState::Queued) => {
                            out.push(self.request(i, r, last.clone(), id, RERUN_PRIORITY + base))
                        }
                        Some(AttemptState::Failed(_)) => {
                            let job = format!("{id}-t{}", it.jobs.len());
                            out.push(self.request(i, r, job, id, base));
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
        let (i, r) = self
            .logical_of(&done.item)
            .ok_or_else(|| format!("unknown synthetic item {}", done.item))?;
        let c = &done.classification;
        let mut out = Routing::default();
        match c.kind {
            OutcomeKind::RuleFound | OutcomeKind::Complete
                if c.accept != Accept::Nothing && done.table.is_some() =>
            {
                let table = done.table.clone().unwrap();
                let canon = done
                    .canonical_blake3
                    .clone()
                    .ok_or("sealed table without canonical digest")?;
                let (rule, rec) =
                    accepted_record(&self.node_of(i), &done.item, &canon, &done.canonical_lines);
                out.events.push(Event::Item {
                    item: done.item.clone(),
                    ev: ItemEvent::Produce {
                        job: done.job.clone(),
                        attempt: done.attempt,
                        table: table.clone(),
                    },
                });
                out.events.push(Event::Item {
                    item: done.item.clone(),
                    ev: ItemEvent::Accept {
                        rule,
                        level: EvidenceLevel::GeometryAccepted,
                        record: rec.file_ref(),
                        source: AcceptSource::Job {
                            job: done.job.clone(),
                            attempt: done.attempt,
                            table_blake3: table.blake3.clone(),
                        },
                        elided: false,
                        evidence: None,
                        round: None,
                    },
                });
                out.accepts.push(rec);
            }
            OutcomeKind::Interrupted | OutcomeKind::Lost => {
                return Err("interrupted / lost attempts are requeued by the supervisor".into());
            }
            OutcomeKind::Crashed => {
                let it = st.items.get(&done.item).ok_or("unknown item")?;
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
                // procfix5 (gate4 m4): decision rows 22 / 23 rerun ONCE, then attention. A crash is never evidence about
                // the work: never a terminal (the former "crashed twice" escalation ended in terminal dispositions).
                if failed_before + 1 >= 2 {
                    out.events.push(Event::Item {
                        item: done.item.clone(),
                        ev: ItemEvent::Attention {
                            code: CRASHED_TWICE.into(),
                            text: format!(
                                "{} crashed twice (row {}{}: {}): never evidence about the work; check, then `rr_campaign control CDIR retry {}`",
                                done.item, c.row, c.sub, c.why, done.item
                            ),
                        },
                    });
                }
            }
            k => {
                out = self.escalate_or_terminal(&done.item, i, r, crate::state::class_word(k));
            }
        }
        Ok(out)
    }

    /// Retry / skip are applied by the ledger itself (`control` records, ledger 5.7: skip -> skipped, retry ->
    /// open with a new generation); the synthetic source needs no follow-up.
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
            "retry" | "skip" => Ok(vec![]),
            v => Err(format!("verb {v} not supported by the synthetic source")),
        }
    }

    /// procfix5 (gate4 m4): finished when every chain is final, or waits for the operator (an item in attention), or
    /// can never start because a chain it depends on waits for the operator.
    fn finished(&self, st: &CampaignState) -> bool {
        let n = self.cfg.items as usize;
        let mut blocked = vec![false; n];
        for i in 0..self.cfg.items {
            let cur = self.current(st, i).map(|(_, it, _)| it.phase.clone());
            let attention = matches!(cur, Some(ItemPhase::Attention(_)));
            let behind = matches!(cur, Some(ItemPhase::Open))
                && self.deps(i).iter().any(|&d| blocked[d as usize]);
            blocked[i as usize] = attention || behind;
            if !(self.chain_final(st, i) || blocked[i as usize]) {
                return false;
            }
        }
        true
    }

    fn describe(&self) -> String {
        format!(
            "synthetic campaign: {} items on {} nodes, {} rungs, seed {}",
            self.cfg.items, self.cfg.nodes, self.cfg.rungs, self.cfg.seed
        )
    }
}

#[allow(clippy::let_underscore_must_use)] // test cleanup (outside the reviewed scope, procfix7)
#[allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code
#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Synthetic {
        serde_json::from_value(serde_json::json!({"items": 60, "seed": 7})).unwrap()
    }

    /// procfix7 round 11 (gate14 item 5b): `honest_helpers` = that many COMPLIANT helper forks in every rule job (valid
    /// scripts; none escapes its group, every one has PDEATHSIG; half short, half killed by the job at its end).
    #[test]
    fn honest_helpers_fork_compliant_helpers_in_rule_jobs() {
        let s = ScriptedWorkSource::new(
            serde_json::from_value(
                serde_json::json!({"items": 40, "seed": 7, "honest_helpers": 4}),
            )
            .unwrap(),
        );
        let mut rule_jobs = 0;
        for i in 0..40 {
            let sc = s.script(i, 0);
            sc.validate().unwrap();
            let forks = sc
                .script
                .iter()
                .filter(|o| matches!(o, FakeOp::Helper { .. }))
                .count();
            if s.behaviour(i, 0) == Behaviour::Rule {
                rule_jobs += 1;
                assert_eq!(forks, 4);
                assert_eq!(sc.helpers.len(), 4);
                assert!(
                    sc.helpers
                        .iter()
                        .all(|h| h.pdeathsig && !h.escape_group && !h.ignore_sigterm)
                );
                assert_eq!(sc.helpers.iter().filter(|h| h.sleep_ms == 150).count(), 2);
            } else {
                assert_eq!(forks, 0);
            }
        }
        assert!(rule_jobs > 0);
        // the default: no helper
        assert!(
            ScriptedWorkSource::new(cfg())
                .script(0, 0)
                .helpers
                .is_empty()
        );
    }

    #[test]
    fn deterministic_and_mixed() {
        let s = ScriptedWorkSource::new(cfg());
        let t = ScriptedWorkSource::new(cfg());
        let mut kinds = std::collections::BTreeMap::new();
        for i in 0..60 {
            for r in 0..3 {
                assert_eq!(s.behaviour(i, r), t.behaviour(i, r));
                *kinds.entry(format!("{:?}", s.behaviour(i, r))).or_insert(0) += 1;
                s.script(i, r).validate().unwrap();
            }
            assert!(s.deps(i).iter().all(|&d| d < i));
        }
        assert_eq!(kinds.len(), 4, "{kinds:?}");
        assert_eq!(s.logical_of("N2.i0017.L1"), Some((17, 1)));
    }
}
