//! The scheduling interface between the supervisor and the campaign's work (interfaces/supervisor.md section 2), and
//! the deterministic synthetic source used by tests and rehearsals.
//!
//! The supervisor knows nothing about faces, ladders or acceptance policy; a [`WorkSource`] hands it ready jobs and
//! decides what a classified outcome means for its items. Every method is a function of the reduced ledger state, so a
//! restarted supervisor gets the same answers (resume = replay + the same questions).

use super::spec::Synthetic;
use crate::ledger::{AcceptSource, Event, FileRef, ItemEvent};
use crate::state::{AttemptState, CampaignState, EvidenceLevel, ItemPhase, NodeEvent, NodePhase, Rung};
use rustred_campaign_contract::classify::{Accept, Classification, OutcomeKind};
use rustred_campaign_contract::exit::ExitCode;
use rustred_campaign_contract::fake::{FakeOp, FakeRule, FakeScript, OnSigterm, RssPlan};
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
            "cap": self.cap_bytes, "fake": self.fake,
        });
        blake3::hash(&serde_json::to_vec(&v).expect("json")).to_hex().to_string()
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
    /// Operator verbs `retry` / `skip`.
    fn control(&mut self, st: &CampaignState, verb: &str, target: &str) -> Result<Vec<Event>, String>;
    /// All work final.
    fn finished(&self, st: &CampaignState) -> bool;
    /// Number of work units (for logs).
    fn describe(&self) -> String;
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

/// Rule id and record file of an accepted sealed table (shared by sources).
pub fn accepted_record(node: &str, item: &str, canon: &str, lines: &[String]) -> (String, AcceptedRecord) {
    let rule = format!("{item}:{}", &canon[..16.min(canon.len())]);
    let bytes = serde_json::to_vec(&AcceptedFile {
        schema: ACCEPTED_SCHEMA,
        item,
        rule: &rule,
        table_canonical_blake3: canon,
        records: lines,
    })
    .expect("json");
    let safe: String = item
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' })
        .collect();
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
        if b != a && self.h(i as u64, 3, 3) % 2 == 0 {
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
    fn current<'a>(&self, st: &'a CampaignState, i: u32) -> Option<(u8, &'a crate::state::ItemRec, String)> {
        (0..self.cfg.rungs).rev().find_map(|r| {
            let id = self.item_id(i, r);
            st.items.get(&id).map(|it| (r, it, id))
        })
    }

    fn chain_final(&self, st: &CampaignState, i: u32) -> bool {
        matches!(
            self.current(st, i).map(|(_, it, _)| &it.phase),
            Some(ItemPhase::Accepted(_) | ItemPhase::Terminal | ItemPhase::Retired | ItemPhase::TerminalCandidate)
        )
    }

    /// The fake script of (i, r).
    pub fn script(&self, i: u32, r: u8) -> FakeScript {
        let ms = self.case_ms(i, r);
        let id = self.item_id(i, r);
        let rule_case = |ms: u64| FakeOp::Case {
            ordinal: 0,
            ms,
            work: [(WorkCounter::Seeds, 100 + i as u64), (WorkCounter::Rows, 1000)]
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
        FakeScript {
            script: ops,
            on_sigterm: OnSigterm::Graceful,
            slow_ms: 0,
            start_delay_ms: 0,
            rss: RssPlan::default(),
            helpers: vec![],
            faults: vec![],
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

    fn escalate_or_terminal(&self, item: &str, i: u32, r: u8, reason: &str) -> Vec<Event> {
        if r + 1 < self.cfg.rungs {
            let next = self.item_id(i, r + 1);
            vec![
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
            ]
        } else {
            vec![
                Event::Item {
                    item: item.to_string(),
                    ev: ItemEvent::ProposeTerminal {
                        reason: format!("ladder exhausted ({reason})"),
                    },
                },
                Event::Item {
                    item: item.to_string(),
                    ev: ItemEvent::DeclareTerminal {
                        count: 1,
                        keys: None,
                        reason: "ladder exhausted".into(),
                    },
                },
            ]
        }
    }
}

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
            if it.phase == ItemPhase::Stale {
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
                let (rule, rec) = accepted_record(&self.node_of(i), &done.item, &canon, &done.canonical_lines);
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
                    .filter(|j| matches!(st.jobs.get(*j).map(|x| x.state), Some(AttemptState::Failed(_))))
                    .count();
                if failed_before + 1 >= 2 {
                    out.events = self.escalate_or_terminal(&done.item, i, r, "crashed twice");
                }
            }
            k => {
                out.events =
                    self.escalate_or_terminal(&done.item, i, r, crate::state::class_word(k));
            }
        }
        Ok(out)
    }

    fn control(&mut self, st: &CampaignState, verb: &str, target: &str) -> Result<Vec<Event>, String> {
        let it = st
            .items
            .get(target)
            .ok_or_else(|| format!("unknown item {target}"))?;
        match (verb, &it.phase) {
            ("skip", ItemPhase::Open) => Ok(vec![Event::Item {
                item: target.into(),
                ev: ItemEvent::ProposeTerminal {
                    reason: "skipped by operator".into(),
                },
            }]),
            ("skip", ItemPhase::TerminalCandidate) => Ok(vec![]),
            ("skip", p) => Err(format!("skip needs an open item; {target} is {}", p.tag())),
            ("retry", ItemPhase::TerminalCandidate) => Ok(vec![Event::Item {
                item: target.into(),
                ev: ItemEvent::Reopen {
                    reason: "retry by operator".into(),
                },
            }]),
            ("retry", ItemPhase::Open) => Ok(vec![]),
            ("retry", p) => Err(format!(
                "retry needs a skipped (terminal candidate) item; {target} is {}",
                p.tag()
            )),
            (v, _) => Err(format!("verb {v} not supported by the synthetic source")),
        }
    }

    fn finished(&self, st: &CampaignState) -> bool {
        (0..self.cfg.items).all(|i| self.chain_final(st, i))
    }

    fn describe(&self) -> String {
        format!(
            "synthetic campaign: {} items on {} nodes, {} rungs, seed {}",
            self.cfg.items, self.cfg.nodes, self.cfg.rungs, self.cfg.seed
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Synthetic {
        serde_json::from_value(serde_json::json!({"items": 60, "seed": 7})).unwrap()
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
